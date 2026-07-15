# =============================================================================
# BATHOS M6 — gate-enforce.ps1   (Windows PowerShell 포트)
# 원본: .claude/hooks/gate-enforce.sh
# TaskCompleted: W3 게이트 미통과(FAIL) 시 구현 웨이브 진입 물리 차단
# =============================================================================
# 계약:  api-contracts.md §D (TaskCompleted gate-enforce), §C-2 (게이트 응답)
# 예외:  exceptions.md §1 E-GATE-FAIL, E-GATE-LOOP
#
# ⚙️  GATE-CRITICAL — 원본과 완전히 동일한 exit 코드 계약을 보존한다:
#   verdict==FAIL 인 경우에만 **exit 2**(하드 차단). 그 외 모든 경로(PASS/
#   CONCERNS/verdict 없음/판정 불가/이 훅이 다루지 않는 태스크)는 **exit 0**.
#   이 조건분기는 story-a1-guard-hardening-kr.md AC1의 회귀 금지 대상이므로
#   로직을 원본과 동일하게 유지한다(fingerprint 재승인 방지는 freeze-guard 담당,
#   이 훅과 결선 불필요 — hook-and-gate-design-kr.md §1a).
#
# 우선순위(verdict 판정): bathos gate show > manifest.json gates[] > readiness-report-kr.md
#
# ⚠️  fail-safe 원칙:
#   - 바이너리/서브커맨드 없음      → 경고 후 통과 (비차단)
#   - manifest.json 읽기 실패       → 경고 후 통과 (비차단)
#   - 구현 진입 태스크 아닌 경우    → 조용히 통과
#   - W3 verdict == FAIL 확인 시   → exit 2 차단 (유일 차단 조건)
# 이식 메모: jq 유무 분기 제거 — ConvertFrom-Json 네이티브 사용.
# =============================================================================

. (Join-Path $PSScriptRoot '_common.ps1')

$ctx = Get-BathosContext
$artDir = Join-Path $ctx.ProjectDir '.agent-team'

# --------------------------------------------------------------------------
# 2. stdin JSON 파싱: 태스크 제목 추출
# --------------------------------------------------------------------------
$parsed = Read-HookInput
$taskTitle = ''
$taskDesc = ''
if ($null -ne $parsed) {
    $tt = Get-JsonField $parsed @('task','title'); if ($null -eq $tt) { $tt = Get-JsonField $parsed @('title') }
    if ($null -ne $tt) { $taskTitle = [string]$tt }
    $td = Get-JsonField $parsed @('task','description'); if ($null -eq $td) { $td = Get-JsonField $parsed @('description') }
    if ($null -ne $td) { $taskDesc = [string]$td }
}

# --------------------------------------------------------------------------
# 3. 구현 웨이브 진입 태스크 감지 (본 프로젝트에서 "구현" = charter W4, 표준명칭 W5)
# --------------------------------------------------------------------------
$combined = $taskTitle + ' ' + $taskDesc
$ic = [System.Text.RegularExpressions.RegexOptions]::IgnoreCase
$isW5Entry = [regex]::IsMatch($combined, 'W5|wave.5|wave5|구현.*시작|implementation.*start|implement.*wave|구현.*웨이브|W5.*진입|enter.*W5', $ic)

if (-not $isW5Entry) {
    # 구현 진입 태스크가 아닌 경우: 조용히 통과
    exit 0
}

# --------------------------------------------------------------------------
# 4. W3 게이트 verdict 판정 (우선순위: bathos gate show > manifest > report)
# --------------------------------------------------------------------------
$w3Verdict = ''
$verdictSource = ''

# 4-a. [B3 활성] bathos gate show 서브커맨드 — 1순위 verdict 출처(바이너리 SSOT)
#      출력: {"gate_type":"Implementation","verdict":"PASS|CONCERNS|FAIL",...} / 게이트 없음=빈 출력 exit0.
if (Test-Path -LiteralPath $ctx.Bin) {
    $gateOutput = ''
    try {
        $lines = & $ctx.Bin --state-dir $ctx.StateDir gate show 2>$null
        if ($null -ne $lines) { $gateOutput = ($lines -join "`n") }
    } catch { $gateOutput = '' }

    if (-not [string]::IsNullOrWhiteSpace($gateOutput)) {
        try {
            $gj = $gateOutput | ConvertFrom-Json
            $vp = $gj.PSObject.Properties['verdict']
            if ($null -ne $vp -and $null -ne $vp.Value -and ([string]$vp.Value) -ne '') {
                $w3Verdict = [string]$vp.Value
                $verdictSource = 'bathos gate show'
            }
        } catch { }
    }
}

# 4-b. manifest.json에서 직접 읽기 (B2 fail-safe 경로)
$manifestPath = Join-Path $ctx.StateDir 'manifest.json'
if ([string]::IsNullOrEmpty($w3Verdict) -and (Test-Path -LiteralPath $manifestPath)) {
    try {
        $mf = (Get-Content -LiteralPath $manifestPath -Raw) | ConvertFrom-Json
        $gates = $null
        $gp = $mf.PSObject.Properties['gates']; if ($null -ne $gp) { $gates = $gp.Value }
        if ($null -ne $gates) {
            $matching = @($gates | Where-Object {
                $gtProp = $_.PSObject.Properties['gate_type']
                $widProp = $_.PSObject.Properties['wave_id']
                $gtVal = $null; if ($null -ne $gtProp) { $gtVal = $gtProp.Value }
                $widVal = $null; if ($null -ne $widProp) { $widVal = $widProp.Value }
                ($gtVal -eq 'Implementation') -or ($widVal -eq 'W3')
            })
            if ($matching.Count -gt 0) {
                $last = $matching[$matching.Count - 1]
                $vp = $last.PSObject.Properties['verdict']
                if ($null -ne $vp -and $null -ne $vp.Value -and ([string]$vp.Value) -ne '') {
                    $w3Verdict = [string]$vp.Value
                    $verdictSource = 'manifest.json'
                }
            }
        }
    } catch { }
}

# 4-c. readiness-report-kr.md 프론트매터에서 직접 읽기 (최후 수단)
$readinessReport = Join-Path (Join-Path $artDir '03-story-engineering') 'readiness-report-kr.md'
if ([string]::IsNullOrEmpty($w3Verdict) -and (Test-Path -LiteralPath $readinessReport)) {
    try {
        $rlines = Get-Content -LiteralPath $readinessReport -ErrorAction Stop
        $vline = $rlines | Where-Object { $_ -match 'verdict:\s*' } | Select-Object -First 1
        if ($null -ne $vline) {
            if ($vline -match 'verdict:\s*["'']*([A-Za-z]+)["'']*') {
                $candidate = $matches[1].ToUpperInvariant()
                if ($candidate -in @('PASS','CONCERNS','FAIL')) {
                    $w3Verdict = $candidate
                    $verdictSource = 'readiness-report-kr.md'
                }
            }
        }
    } catch { }
}

# --------------------------------------------------------------------------
# 5. verdict 부재 시 fail-safe
# --------------------------------------------------------------------------
if ([string]::IsNullOrEmpty($w3Verdict)) {
    Write-HookError ''
    Write-HookError '[bathos gate-enforce] ! W3 게이트 verdict를 확인할 수 없습니다.'
    Write-HookError '[bathos gate-enforce] (bathos gate show: B3 미구현 / manifest.json: 미존재 또는 gates[] 없음)'
    Write-HookError '[bathos gate-enforce] 구현 웨이브 진입을 허용하되, W3 게이트를 별도로 확인하십시오.'
    Write-HookError '[bathos gate-enforce] 참고: w3-story-engine-design.md §5, exceptions.md §1 E-GATE-FAIL'
    # fail-safe: 차단하지 않고 경고만 (verdict 없는 상황은 초기 설정 시 흔함)
    exit 0
}

# --------------------------------------------------------------------------
# 6. verdict 기반 판정
# --------------------------------------------------------------------------
switch ($w3Verdict) {
    'PASS' {
        Write-HookError ('[bathos gate-enforce] . W3 게이트 PASS (' + $verdictSource + ') — 구현 웨이브 진입 허용')
        exit 0
    }
    'CONCERNS' {
        Write-HookError ('[bathos gate-enforce] ! W3 게이트 CONCERNS (' + $verdictSource + ') — 리스크 로그 후 진행')
        Write-HookError '[bathos gate-enforce] 비차단 리스크가 있습니다. _state/에 리스크 로그를 확인하세요.'
        exit 0
    }
    'FAIL' {
        # ← 유일한 차단 조건 —————————————————————————
        Write-HookError ''
        Write-HookError '[bathos gate-enforce] x W3 게이트 FAIL — 구현 웨이브 진입이 차단되었습니다.'
        Write-HookError ('[bathos gate-enforce] verdict 출처: ' + $verdictSource)
        Write-HookError '[bathos gate-enforce] 이유: critical issue >= 1 (api-contracts §C-2)'
        Write-HookError ''
        Write-HookError '[bathos gate-enforce] 다음 단계:'
        Write-HookError '[bathos gate-enforce]   1. readiness-report-kr.md의 critical issues를 확인하세요.'
        Write-HookError '[bathos gate-enforce]   2. W2(기획·아키텍처·디자인)로 반려하여 산출물을 보완하세요.'
        Write-HookError '[bathos gate-enforce]   3. 재게이트를 통과한 후 구현 웨이브에 진입하세요.'
        Write-HookError '[bathos gate-enforce]   4. regate_count >= 3이면 Paul에게 escalation 하세요.'
        Write-HookError '[bathos gate-enforce] 참고: w3-story-engine-design.md §5, exceptions.md §1 E-GATE-FAIL'
        exit 2  # ← 하드 차단
    }
    default {
        Write-HookError ('[bathos gate-enforce] ! 알 수 없는 verdict 값: "' + $w3Verdict + '" — 통과 처리(보수적)')
        exit 0
    }
}
