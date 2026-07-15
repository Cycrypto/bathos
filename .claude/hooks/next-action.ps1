# =============================================================================
# BATHOS M6 — next-action.ps1   (Windows PowerShell 포트)  [Dynamis A3 — 서킷브레이커 3종 포함]
# 원본: .claude/hooks/next-action.sh
# TeammateIdle: 남은 태스크 / 다음 웨이브 안내 + 침묵·무한루프 능동 안전망
# =============================================================================
# 계약:  api-contracts.md §D (TeammateIdle next-action), §B-3 (웨이브 전이 메시지)
# DoD:   idle 에이전트에게 다음 unblocked 태스크 또는 웨이브 안내 출력(기존, 회귀 금지)
# 참고:  기존 안내 로직은 차단하지 않음(exit 0 고정) — 서킷브레이커도 동일(유도만,
#        실제 프로세스 종료는 상위 오케스트레이터/사용자 판단, 이 훅은 신호만 냄).
#
# [Dynamis A3 — SS2 · CF-A3, open-swe #7 사상]
#   `_state/session-flags.json`의 `counters.<role>` 서브객체를 판정한다:
#     - empty_msg_streak: 이 훅(TeammateIdle) 호출마다 +1. audit-log.ps1(PostToolUse)이
#       도구 호출 시 0으로 리셋 — "도구 호출 없이 idle만 반복"을 정확히 포착한다.
#     - model_calls: audit-log.ps1이 도구 호출마다 +1(모델 호출수 정밀 계측 훅이
#       Claude Code에 없어 "도구 호출 수"로 근사 — 정직 고지).
#     - fail_streak: audit-log.ps1이 tool_response 실패 시 +1, 성공 시 0.
#   임계 초과 시 재주입/중단 유도 메시지를 출력하고 `bathos audit append`로 기록한다.
#   thresholds 기본값: empty_msg_max=2, model_call_max=200, fail_streak_max=3
#   (session-flags.json에 없으면 이 기본값 사용).
# 이식 메모: jq 유무 분기 제거 — ConvertFrom-Json 네이티브 사용.
# =============================================================================

. (Join-Path $PSScriptRoot '_common.ps1')

$ctx = Get-BathosContext
$manifestPath = Join-Path $ctx.StateDir 'manifest.json'
$sessionFlagsPath = Join-Path $ctx.StateDir 'session-flags.json'

function script:Set-NoteProp {
    param($Obj, [string]$Name, $Value)
    $Obj | Add-Member -NotePropertyName $Name -NotePropertyValue $Value -Force | Out-Null
}

# --------------------------------------------------------------------------
# 2. stdin JSON 파싱: 역할·남은 태스크 추출
# --------------------------------------------------------------------------
$parsed = Read-HookInput
$role = ''
$remainingTasks = ''

if ($null -ne $parsed) {
    $r = Get-JsonField $parsed @('role'); if ($null -ne $r) { $role = [string]$r }

    $rt = Get-JsonField $parsed @('remaining_tasks')
    if ($null -ne $rt) {
        $items = @()
        foreach ($it in $rt) { if ($null -ne $it) { $items += ('  - ' + [string]$it) } }
        if ($items.Count -gt 0) { $remainingTasks = ($items -join "`n") }
    }
}
if ([string]::IsNullOrEmpty($role)) {
    $role = $env:BATHOS_ROLE
    if ([string]::IsNullOrEmpty($role)) { $role = '팀원' }
}

# --------------------------------------------------------------------------
# 3. manifest.json 기반 웨이브 상태 파악 (기존 로직 — 회귀 금지)
# --------------------------------------------------------------------------
$currentWave = ''
$gateVerdict = ''
$waveStatusSummary = ''

if (Test-Path -LiteralPath $manifestPath) {
    try {
        $mf = (Get-Content -LiteralPath $manifestPath -Raw) | ConvertFrom-Json

        $waves = $null
        $wp = $mf.PSObject.Properties['waves']; if ($null -ne $wp) { $waves = $wp.Value }
        if ($null -ne $waves) {
            $active = $waves | Where-Object {
                $sp = $_.PSObject.Properties['status']
                ($null -ne $sp) -and ($sp.Value -eq 'active')
            } | Select-Object -First 1
            if ($null -ne $active) {
                $widp = $active.PSObject.Properties['wave_id']
                if ($null -ne $widp -and $null -ne $widp.Value) { $currentWave = [string]$widp.Value }
            }

            $summaryLines = @()
            foreach ($wv in $waves) {
                $widp2 = $wv.PSObject.Properties['wave_id']
                $wid = ''; if ($null -ne $widp2 -and $null -ne $widp2.Value) { $wid = [string]$widp2.Value }
                $stp = $wv.PSObject.Properties['status']
                $st = 'unknown'; if ($null -ne $stp -and $null -ne $stp.Value -and ([string]$stp.Value) -ne '') { $st = [string]$stp.Value }
                $summaryLines += ('  ' + $wid + ': ' + $st)
            }
            $waveStatusSummary = ($summaryLines -join "`n")
        }

        $gates = $null
        $gp = $mf.PSObject.Properties['gates']; if ($null -ne $gp) { $gates = $gp.Value }
        if ($null -ne $gates) {
            $matching = @($gates | Where-Object {
                $gtp = $_.PSObject.Properties['gate_type']; $gt = $null; if ($null -ne $gtp) { $gt = $gtp.Value }
                $widp3 = $_.PSObject.Properties['wave_id']; $wid3 = $null; if ($null -ne $widp3) { $wid3 = $widp3.Value }
                ($gt -eq 'Implementation') -or ($wid3 -eq 'W3')
            })
            if ($matching.Count -gt 0) {
                $lastGate = $matching[$matching.Count - 1]
                $vp = $lastGate.PSObject.Properties['verdict']
                if ($null -ne $vp -and $null -ne $vp.Value -and ([string]$vp.Value) -ne '') { $gateVerdict = [string]$vp.Value }
            }
        }
        if ([string]::IsNullOrEmpty($gateVerdict)) { $gateVerdict = '없음' }
    } catch {
        $gateVerdict = '없음'
    }
}

# --------------------------------------------------------------------------
# 4. 다음 권장 행동 결정 (기존 로직 — 회귀 금지, api-contracts B-3 on_complete 대응)
# --------------------------------------------------------------------------
function script:Get-NextActionText {
    param([string]$CurrentWave, [string]$GateVerdict)
    $w = $CurrentWave.ToLowerInvariant()

    if ($w -match 'w0|wave0|analysis') { return 'W0 Analysis 완료 → W1 Discovery 시작 (/wave1-discovery)' }
    if ($w -match 'w1|wave1|discovery') { return 'W1 Discovery 완료 → W2 기획·아키텍처·디자인 시작 (/wave2-design)' }
    if ($w -match 'w2|wave2|design') { return 'W2 완료 → W3 Story Engineering & Readiness Gate 시작 (/wave3-story-gate)' }
    if ($w -match 'w3|wave3|story') {
        switch ($GateVerdict) {
            'PASS'     { return ('W3 게이트 ' + $GateVerdict + ' → 구현 웨이브 시작 (/wave4-implement)') }
            'CONCERNS' { return ('W3 게이트 ' + $GateVerdict + ' → 구현 웨이브 시작 (/wave4-implement)') }
            'FAIL'     { return 'W3 게이트 FAIL → W2로 반려 후 보완·재게이트 필요' }
            default    { return 'W3 진행 중 → 게이트 판정 완료 후 구현 웨이브 또는 W2 반려' }
        }
    }
    if ($w -match 'w4|wave4|ip|research') { return 'W4 IP/Research 완료 (플러그) → W6 검증·문서화 또는 Martin 취합 대기' }
    if ($w -match 'w5|wave5|implement') { return '구현 웨이브 완료 → W6 검증·문서화·리포트 시작 (/wave5-verify-report)' }
    if ($w -match 'w6|wave6|verify|report') { return 'W6 진행 중 → Thomas·Timothy·Matthias 완료 후 Martin 취합 → 최종 confirm' }
    return '현재 웨이브 상태를 확인하고 다음 unblocked 태스크를 self-claim 하세요.'
}

$nextAction = Get-NextActionText -CurrentWave $currentWave -GateVerdict $gateVerdict

# ============================================================================
# 5. [Dynamis A3] 서킷브레이커 3종 판정
# ============================================================================
$circuitMessage = ''
$hookActor = $env:BATHOS_ROLE; if ([string]::IsNullOrEmpty($hookActor)) { $hookActor = 'hook:next-action' }

if (Test-Path -LiteralPath $sessionFlagsPath) {
    $flags = $null
    try { $flags = (Get-Content -LiteralPath $sessionFlagsPath -Raw) | ConvertFrom-Json } catch { $flags = $null }

    if ($null -ne $flags) {
        $countersObj = $null
        $cp = $flags.PSObject.Properties['counters']; if ($null -ne $cp) { $countersObj = $cp.Value }
        $actorCounter = $null
        if ($null -ne $countersObj) {
            $acp = $countersObj.PSObject.Properties[$role]
            if ($null -ne $acp) { $actorCounter = $acp.Value }
        }

        $emptyStreak = 0; $modelCalls = 0; $failStreak = 0
        if ($null -ne $actorCounter) {
            $p1 = $actorCounter.PSObject.Properties['empty_msg_streak']; if ($null -ne $p1 -and $null -ne $p1.Value) { $emptyStreak = [int]$p1.Value }
            $p2 = $actorCounter.PSObject.Properties['model_calls'];      if ($null -ne $p2 -and $null -ne $p2.Value) { $modelCalls = [int]$p2.Value }
            $p3 = $actorCounter.PSObject.Properties['fail_streak'];      if ($null -ne $p3 -and $null -ne $p3.Value) { $failStreak = [int]$p3.Value }
        }

        $thresholdsObj = $null
        $tp = $flags.PSObject.Properties['thresholds']; if ($null -ne $tp) { $thresholdsObj = $tp.Value }
        $emptyMax = 2; $callMax = 200; $failMax = 3
        if ($null -ne $thresholdsObj) {
            $e1 = $thresholdsObj.PSObject.Properties['empty_msg_max'];  if ($null -ne $e1 -and $null -ne $e1.Value) { $emptyMax = [int]$e1.Value }
            $e2 = $thresholdsObj.PSObject.Properties['model_call_max']; if ($null -ne $e2 -and $null -ne $e2.Value) { $callMax  = [int]$e2.Value }
            $e3 = $thresholdsObj.PSObject.Properties['fail_streak_max'];if ($null -ne $e3 -and $null -ne $e3.Value) { $failMax  = [int]$e3.Value }
        }

        # 5-a. 이번 idle 자체를 "도구 호출 없는 턴"으로 집계 — +1
        $newEmptyStreak = $emptyStreak + 1
        $resetEmpty = $false
        $resetCircuit = $false

        if ($newEmptyStreak -ge $emptyMax) {
            $circuitMessage += ('[bathos next-action] ! 연속 무도구-응답 ' + $newEmptyStreak + '회(임계 ' + $emptyMax + ') 감지 — 재주입: 다음 unblocked 태스크를 self-claim 하거나 진행 상황을 보고하세요.' + "`n")
            $resetEmpty = $true
            Add-BathosAudit -Context $ctx -Action 'circuit.reinject' -Target $role -Actor $hookActor
        }

        if (($modelCalls -gt $callMax) -or ($failStreak -ge $failMax)) {
            $circuitMessage += ('[bathos next-action] x 서킷브레이커 발동(model_calls=' + $modelCalls + '/' + $callMax + ', fail_streak=' + $failStreak + '/' + $failMax + ') — 작업을 중단하고 현재까지 결과 1단락 + 리스크를 Paul에게 보고하세요.' + "`n")
            $resetCircuit = $true
            Add-BathosAudit -Context $ctx -Action 'circuit.open' -Target $role -Actor $hookActor
        }

        # 카운터 갱신(신호를 낸 항목은 리셋 — 반복 스팸 방지, 나머지는 갱신값 유지)
        $finalEmpty = $newEmptyStreak
        if ($resetEmpty) { $finalEmpty = 0 }
        $finalFail = $failStreak
        if ($resetCircuit) { $finalFail = 0 }

        if ($null -eq $countersObj) { $countersObj = [PSCustomObject]@{} }
        if ($null -eq $actorCounter) { $actorCounter = [PSCustomObject]@{} }
        Set-NoteProp $actorCounter 'empty_msg_streak' $finalEmpty
        Set-NoteProp $actorCounter 'fail_streak' $finalFail
        Set-NoteProp $countersObj $role $actorCounter
        Set-NoteProp $flags 'counters' $countersObj

        try {
            $json = $flags | ConvertTo-Json -Depth 20
            $tmp = [System.IO.Path]::GetTempFileName()
            $enc = New-Object System.Text.UTF8Encoding($false)
            [System.IO.File]::WriteAllText($tmp, $json, $enc)
            Move-Item -LiteralPath $tmp -Destination $sessionFlagsPath -Force
        } catch { }
    }
}

# --------------------------------------------------------------------------
# 6. 안내 메시지 출력 (기존 + 서킷브레이커 메시지)
# --------------------------------------------------------------------------
Write-HookError ''
Write-HookError ('[bathos next-action] . ' + $role + ' — idle 상태 안내')

if (-not [string]::IsNullOrEmpty($circuitMessage)) {
    Write-HookError ($circuitMessage.TrimEnd("`n"))
}

if (-not [string]::IsNullOrEmpty($remainingTasks)) {
    Write-HookError ('[bathos next-action] 남은 태스크:')
    Write-HookError $remainingTasks
    Write-HookError '[bathos next-action] → 다음 unblocked 태스크를 self-claim 하여 계속 진행하세요.'
} else {
    Write-HookError '[bathos next-action] 할당된 남은 태스크가 없습니다.'
}

if (-not [string]::IsNullOrEmpty($nextAction)) {
    Write-HookError ('[bathos next-action] 권장 다음 행동: ' + $nextAction)
}

if (-not [string]::IsNullOrEmpty($currentWave)) {
    Write-HookError ('[bathos next-action] 현재 웨이브: ' + $currentWave + ' | W3 게이트: ' + $gateVerdict)
}

if (-not [string]::IsNullOrEmpty($waveStatusSummary)) {
    Write-HookError '[bathos next-action] 웨이브 상태:'
    Write-HookError $waveStatusSummary
}

Write-HookError '[bathos next-action] 종료 보고 형식: 핵심 결과 1단락 + 미해결 리스크를 Paul에게 보내세요.'

# 비강제: 항상 통과(서킷브레이커도 "유도"이지 강제 종료가 아님)
exit 0
