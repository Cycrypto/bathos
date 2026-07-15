# =============================================================================
# BATHOS Dynamis A4 — plan-toggle.ps1   (Windows PowerShell 포트, SS5 · CF-A4, LD-4 승격)
# 원본: .claude/hooks/plan-toggle.sh
# UserPromptSubmit: "/bathos plan <on|off>" 파싱 → session-flags.json 갱신
# =============================================================================
# 계약:  story-a4-plan-gate-kr.md §4 "토글 수단 1개(구현 방식은 Phillip 판단)"
#        → intensity-tracker.ps1(A5)와 대칭적인 UserPromptSubmit 파싱 방식으로 구현.
# DoD:   AC4 — 토글 수단 존재, off 후 변경 도구 즉시 허용.
#        AC5 — intensity 필드는 절대 읽거나 쓰지 않는다(같은 파일, 별도 필드).
# 안전:  이 훅은 advisory(차단 없음) — 항상 exit 0.
# 이식 메모:
#   - jq 유무 분기 제거: 네이티브 JSON은 항상 사용 가능하므로 bash의 "jq 없으면
#     최소 스키마로 덮어써 다른 필드 유실" 폴백은 이식하지 않는다 — 대신 항상
#     기존 파일을 읽어 병합(plan_mode/updated만 갱신, 나머지 필드 보존)한다.
#     이는 원본보다 더 안전한 상위호환이며 계약(AC4/AC5)을 위반하지 않는다.
# =============================================================================

. (Join-Path $PSScriptRoot '_common.ps1')

$ctx = Get-BathosContext
$sessionFlagsPath = Join-Path $ctx.StateDir 'session-flags.json'

# --------------------------------------------------------------------------
# 1. stdin JSON 파싱: 사용자 프롬프트 원문 추출
# --------------------------------------------------------------------------
$parsed = Read-HookInput
$prompt = ''
if ($null -ne $parsed) {
    $p = Get-JsonField $parsed @('prompt'); if ($null -ne $p) { $prompt = [string]$p }
}
if ([string]::IsNullOrEmpty($prompt)) { exit 0 }

# --------------------------------------------------------------------------
# 2. "/bathos plan <on|off>" 패턴 감지 (대소문자 무관, 마지막 일치 사용)
# --------------------------------------------------------------------------
$ic = [System.Text.RegularExpressions.RegexOptions]::IgnoreCase
$ms = [regex]::Matches($prompt, '/bathos\s+plan\s+([a-zA-Z]+)', $ic)

# 명령 자체가 없으면(다른 프롬프트) 조용히 통과 — advisory hook
if ($ms.Count -eq 0) { exit 0 }
$level = $ms[$ms.Count - 1].Groups[1].Value.ToLowerInvariant()

# --------------------------------------------------------------------------
# 3. session-flags.json plan_mode 필드 갱신 헬퍼 (네이티브 JSON, 병합 기록)
# --------------------------------------------------------------------------
function script:Set-NoteProp {
    param($Obj, [string]$Name, $Value)
    $Obj | Add-Member -NotePropertyName $Name -NotePropertyValue $Value -Force | Out-Null
}

function script:Write-PlanMode {
    param([string]$Value)

    New-Item -ItemType Directory -Path $ctx.StateDir -Force -ErrorAction SilentlyContinue | Out-Null
    $now = Get-UtcIsoNow

    $obj = $null
    if (Test-Path -LiteralPath $sessionFlagsPath) {
        try { $obj = (Get-Content -LiteralPath $sessionFlagsPath -Raw) | ConvertFrom-Json } catch { $obj = $null }
    }
    if ($null -eq $obj) {
        $obj = [PSCustomObject]@{
            '$schema'  = 'bathos:session-flags'
            plan_mode  = $Value
            intensity  = 'full'
            updated    = $now
        }
    } else {
        Set-NoteProp $obj 'plan_mode' $Value
        Set-NoteProp $obj 'updated' $now
    }

    try {
        $json = $obj | ConvertTo-Json -Depth 20
        $tmp = [System.IO.Path]::GetTempFileName()
        $enc = New-Object System.Text.UTF8Encoding($false)
        [System.IO.File]::WriteAllText($tmp, $json, $enc)
        Move-Item -LiteralPath $tmp -Destination $sessionFlagsPath -Force
        return $true
    } catch {
        return $false
    }
}

switch ($level) {
    'on' {
        if (Write-PlanMode -Value 'on') {
            Write-Output '[bathos plan-toggle] . plan_mode=on 적용 — 계획 확정 전까지 Write/Edit/MultiEdit/변경성 Bash가 차단됩니다.'
            $actor = $env:BATHOS_ROLE; if ([string]::IsNullOrEmpty($actor)) { $actor = 'hook:plan-toggle' }
            Add-BathosAudit -Context $ctx -Action 'plan.toggle' -Target 'on' -Actor $actor
        } else {
            Write-HookError '[bathos plan-toggle] ! plan_mode=on 기록 실패(파일 쓰기 오류) — 다음 행동: STATE_DIR 쓰기 권한을 확인하세요.'
        }
    }
    'off' {
        if (Write-PlanMode -Value 'off') {
            Write-Output '[bathos plan-toggle] . plan_mode=off 적용 — 변경 도구가 즉시 허용됩니다.'
            $actor = $env:BATHOS_ROLE; if ([string]::IsNullOrEmpty($actor)) { $actor = 'hook:plan-toggle' }
            Add-BathosAudit -Context $ctx -Action 'plan.toggle' -Target 'off' -Actor $actor
        } else {
            Write-HookError '[bathos plan-toggle] ! plan_mode=off 기록 실패(파일 쓰기 오류) — 다음 행동: STATE_DIR 쓰기 권한을 확인하세요.'
        }
    }
    default {
        Write-HookError ('[bathos plan-toggle] ! 알 수 없는 값 "' + $level + '" — on|off 중 하나를 사용하세요. (이전 값 유지)')
    }
}

exit 0
