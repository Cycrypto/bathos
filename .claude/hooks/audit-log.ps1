# =============================================================================
# BATHOS M6 — audit-log.ps1   (Windows PowerShell 포트)  [Dynamis A3 — 서킷브레이커 카운터 갱신 포함]
# 원본: .claude/hooks/audit-log.sh
# PostToolUse: 모든 도구 사용을 _state/audit-log.jsonl에 append(엔진 CLI 경유) +
#              session-flags.json의 counters.<actor> 갱신(next-action.ps1 서킷브레이커 입력)
# =============================================================================
# 계약:  api-contracts.md §D (PostToolUse audit), §A-7 (감사로그 스키마)
# 예외:  exceptions.md §6 E-AUDIT-TAMPER (hash_prev 체인 무결성)
# DoD:   도구 사용마다 JSONL 1행 append(엔진 경유); hash chain은 엔진이 관리;
#        비강제(오류 시에도 항상 exit 0)
# 이식 메모:
#   - bash판의 "jq 없으면 스킵" 분기는 불필요(ConvertFrom-Json 항상 사용 가능) —
#     대신 stdin 파싱 실패/도구명 미확인 시 동일하게 조용히 통과(fail-open).
#   - Add-BathosAudit(_common.ps1)이 200자 절단 + 제어문자(" \ CR LF Tab) 공백
#     치환을 이미 수행하므로 여기서는 원본과 동일하게 Bash 커맨드만 80자로 먼저
#     자른 뒤 위임한다(원본 이중 절단 구조를 그대로 보존).
#   - counters 갱신은 세션 로컬 파일(session-flags.json)의 advisory 필드이며
#     감사 SSOT가 아니다(원본 주석과 동일 전제) — 실패해도 차단하지 않는다.
# =============================================================================

. (Join-Path $PSScriptRoot '_common.ps1')

$ctx = Get-BathosContext

# _state 디렉터리가 없으면 감사 스킵 (비강제)
if (-not (Test-Path -LiteralPath $ctx.StateDir -PathType Container)) { exit 0 }

$sessionFlagsPath = Join-Path $ctx.StateDir 'session-flags.json'

function script:Set-NoteProp {
    param($Obj, [string]$Name, $Value)
    $Obj | Add-Member -NotePropertyName $Name -NotePropertyValue $Value -Force | Out-Null
}

try {
    $parsed = Read-HookInput

    $toolName = ''
    $target = ''
    $resultSummary = 'unknown'

    if ($null -ne $parsed) {
        $tn = Get-JsonField $parsed @('tool_name'); if ($null -ne $tn) { $toolName = [string]$tn }

        # ----------------------------------------------------------------
        # 2. 도구별 target 추출 (파일 경로 또는 명령 요약) — bash case문 대응
        # ----------------------------------------------------------------
        switch ($toolName) {
            'Write'     { $v = Get-JsonField $parsed @('tool_input','file_path'); if ($null -ne $v) { $target = [string]$v } }
            'Edit'      { $v = Get-JsonField $parsed @('tool_input','file_path'); if ($null -ne $v) { $target = [string]$v } }
            'MultiEdit' { $v = Get-JsonField $parsed @('tool_input','file_path'); if ($null -ne $v) { $target = [string]$v } }
            'Read' {
                $v = Get-JsonField $parsed @('tool_input','file_path')
                if ($null -eq $v) { $v = Get-JsonField $parsed @('tool_input','pattern') }
                if ($null -eq $v) { $v = Get-JsonField $parsed @('tool_input','path') }
                if ($null -ne $v) { $target = [string]$v }
            }
            'Grep' {
                $v = Get-JsonField $parsed @('tool_input','file_path')
                if ($null -eq $v) { $v = Get-JsonField $parsed @('tool_input','pattern') }
                if ($null -eq $v) { $v = Get-JsonField $parsed @('tool_input','path') }
                if ($null -ne $v) { $target = [string]$v }
            }
            'Glob' {
                $v = Get-JsonField $parsed @('tool_input','file_path')
                if ($null -eq $v) { $v = Get-JsonField $parsed @('tool_input','pattern') }
                if ($null -eq $v) { $v = Get-JsonField $parsed @('tool_input','path') }
                if ($null -ne $v) { $target = [string]$v }
            }
            'Bash' {
                # 명령 앞 80자만 기록 (민감정보 최소화) — 원본 head -c 80 + tr 대응
                $v = Get-JsonField $parsed @('tool_input','command')
                if ($null -ne $v) {
                    $s = [string]$v
                    if ($s.Length -gt 80) { $s = $s.Substring(0, 80) }
                    $target = ($s -replace '["\\\r\n\t]', ' ')
                }
            }
            'WebFetch' {
                $v = Get-JsonField $parsed @('tool_input','url')
                if ($null -eq $v) { $v = Get-JsonField $parsed @('tool_input','query') }
                if ($null -ne $v) { $target = [string]$v }
            }
            'WebSearch' {
                $v = Get-JsonField $parsed @('tool_input','url')
                if ($null -eq $v) { $v = Get-JsonField $parsed @('tool_input','query') }
                if ($null -ne $v) { $target = [string]$v }
            }
            default {
                $ti = Get-JsonField $parsed @('tool_input')
                if ($null -ne $ti) {
                    $firstProp = $ti.PSObject.Properties | Select-Object -First 1
                    if ($null -ne $firstProp) { $target = $firstProp.Name }
                }
            }
        }

        # 결과 요약 (성공/실패 여부만)
        $rt = Get-JsonField $parsed @('tool_response','type')
        if ($rt -eq 'result') { $resultSummary = 'ok' }
        elseif ($rt -eq 'error') { $resultSummary = 'error' }
        else { $resultSummary = 'unknown' }
    }

    # 도구명 미확인 시 스킵 (비강제)
    if ([string]::IsNullOrEmpty($toolName)) { exit 0 }

    $actor = $env:BATHOS_ROLE
    if ([string]::IsNullOrEmpty($actor)) { $actor = 'agent' }

    # --------------------------------------------------------------------
    # 3. CLI 경유 감사 append (단일 writer 보장 — Add-BathosAudit이 절단·치환도 수행)
    # --------------------------------------------------------------------
    Add-BathosAudit -Context $ctx -Action ('tool:' + $toolName) -Target $target -Actor $actor

    # ======================================================================
    # 4. [Dynamis A3] 서킷브레이커 카운터 갱신 — jq 없이 네이티브 JSON 처리
    # ======================================================================
    $now = Get-UtcIsoNow

    $flagsObj = $null
    if (Test-Path -LiteralPath $sessionFlagsPath) {
        try { $flagsObj = (Get-Content -LiteralPath $sessionFlagsPath -Raw) | ConvertFrom-Json } catch { $flagsObj = $null }
    }
    if ($null -eq $flagsObj) {
        New-Item -ItemType Directory -Path $ctx.StateDir -Force -ErrorAction SilentlyContinue | Out-Null
        $flagsObj = [PSCustomObject]@{
            '$schema'  = 'bathos:session-flags'
            plan_mode  = 'off'
            intensity  = 'full'
        }
    }

    # counters.<actor> 서브객체 확보 (없으면 생성)
    $counters = $null
    $cProp = $flagsObj.PSObject.Properties['counters']
    if ($null -ne $cProp) { $counters = $cProp.Value }
    if ($null -eq $counters) { $counters = [PSCustomObject]@{} }
    Set-NoteProp $flagsObj 'counters' $counters

    $actorCounter = $null
    $aProp = $counters.PSObject.Properties[$actor]
    if ($null -ne $aProp) { $actorCounter = $aProp.Value }
    if ($null -eq $actorCounter) { $actorCounter = [PSCustomObject]@{ empty_msg_streak = 0; model_calls = 0; fail_streak = 0 } }
    Set-NoteProp $counters $actor $actorCounter

    $prevCalls = 0
    $mcProp = $actorCounter.PSObject.Properties['model_calls']
    if ($null -ne $mcProp -and $null -ne $mcProp.Value) { $prevCalls = [int]$mcProp.Value }

    $prevFail = 0
    $fsProp = $actorCounter.PSObject.Properties['fail_streak']
    if ($null -ne $fsProp -and $null -ne $fsProp.Value) { $prevFail = [int]$fsProp.Value }

    $newFail = 0
    if ($resultSummary -eq 'error') { $newFail = $prevFail + 1 } else { $newFail = 0 }

    Set-NoteProp $actorCounter 'empty_msg_streak' 0
    Set-NoteProp $actorCounter 'model_calls' ($prevCalls + 1)
    Set-NoteProp $actorCounter 'fail_streak' $newFail
    Set-NoteProp $flagsObj 'updated' $now

    try {
        $json = $flagsObj | ConvertTo-Json -Depth 20
        $tmp = [System.IO.Path]::GetTempFileName()
        $enc = New-Object System.Text.UTF8Encoding($false)
        [System.IO.File]::WriteAllText($tmp, $json, $enc)
        Move-Item -LiteralPath $tmp -Destination $sessionFlagsPath -Force
    } catch { }
} catch {
    # 비강제: 어떤 예외가 나도 훅은 차단/지연하지 않는다
}

# 비강제: 항상 통과
exit 0
