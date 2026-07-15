# =============================================================================
# BATHOS Dynamis A4 — plan-gate.ps1   (Windows PowerShell 포트, SS5 · CF-A4, LD-4 승격)
# 원본: .claude/hooks/plan-gate.sh
# PreToolUse(Write/Edit/MultiEdit/Bash): plan_mode=on일 때 변경 도구 deny 게이팅
# =============================================================================
# 계약:  project-context-kr.md §4(훅 계약 5칙), story-a4-plan-gate-kr.md
# 예외:  exceptions.md E-PLAN-GATED
#
# ⚠️  GATE-CRITICAL — exit 코드가 아니라 stdout JSON 이 차단 메커니즘이다.
#   원본 bash와 동일하게 이 훅은 "deny" 시에도 **exit 0**을 반환한다. 차단은
#   `hookSpecificOutput.permissionDecision:"deny"` stdout JSON으로 이루어진다
#   (PreToolUse의 permissionDecision:"deny"는 권한모드 검사보다 먼저 발화하므로
#   `--dangerously-skip-permissions`로도 우회 불가 — deny > defer > ask > allow).
#   이 우선순위·발화 시점은 exit 코드와 무관하다. **exit 코드를 2로 바꾸지 말 것.**
#
# DoD: plan_mode=="on" + Write/Edit/MultiEdit → deny.
#      plan_mode=="on" + 변경성 Bash → deny. 읽기 전용 Bash → allow.
#      plan_mode=="off"/필드 미설정/파일 부재 → 전부 allow(fail-open).
#      intensity 필드는 읽지도 쓰지도 않는다(LD-4 혼선 방지 불변식).
# =============================================================================

. (Join-Path $PSScriptRoot '_common.ps1')

$ctx = Get-BathosContext
$sessionFlagsPath = Join-Path $ctx.StateDir 'session-flags.json'

# --------------------------------------------------------------------------
# 2. session-flags.json 필드 읽기 헬퍼 (네이티브 JSON, 파일 없으면 기본값)
# --------------------------------------------------------------------------
function script:Read-SessionFlag {
    param([string]$Key, [string]$Default)
    if (-not (Test-Path -LiteralPath $sessionFlagsPath)) { return $Default }
    try {
        $obj = (Get-Content -LiteralPath $sessionFlagsPath -Raw) | ConvertFrom-Json
    } catch { return $Default }
    if ($null -eq $obj) { return $Default }
    $prop = $obj.PSObject.Properties[$Key]
    if ($null -eq $prop -or $null -eq $prop.Value -or ([string]$prop.Value) -eq '') { return $Default }
    return [string]$prop.Value
}

$planMode = Read-SessionFlag -Key 'plan_mode' -Default 'off'

# fail-open: off/미설정/파일부재 → 즉시 통과 (AC3)
if ($planMode -ne 'on') { exit 0 }

# --------------------------------------------------------------------------
# 3. stdin JSON 파싱
# --------------------------------------------------------------------------
$parsed = Read-HookInput
$tool = ''
$cmd = ''
$filePath = ''
if ($null -ne $parsed) {
    $t = Get-JsonField $parsed @('tool_name');            if ($null -ne $t) { $tool = [string]$t }
    $c = Get-JsonField $parsed @('tool_input','command');  if ($null -ne $c) { $cmd  = [string]$c }
    $f = Get-JsonField $parsed @('tool_input','file_path'); if ($null -ne $f) { $filePath = [string]$f }
}

# 이 훅이 다루지 않는 도구는 통과 (matcher가 이미 좁히지만 방어적으로 재확인)
switch ($tool) {
    'Write'     { }
    'Edit'      { }
    'MultiEdit' { }
    'Bash'      { }
    ''          { }   # 도구명 미확인 → 보수적으로 계속 진행(read-only 판별에서 걸러짐)
    default     { exit 0 }
}

# --------------------------------------------------------------------------
# 4. 읽기 전용 Bash 허용목록 (careful-guard 위험패턴과 대칭적 접근 — 여긴 허용목록)
# --------------------------------------------------------------------------
function script:Test-ReadonlyBash {
    param([string]$Cmd)
    $ic = [System.Text.RegularExpressions.RegexOptions]::IgnoreCase
    if ([regex]::IsMatch($Cmd, '^\s*(ls|cat|pwd|echo|head|tail|wc|find|file|which|type|env|printenv|grep|jq|diff)(\s|$)', $ic)) { return $true }
    if ([regex]::IsMatch($Cmd, '^\s*git\s+(status|log|diff|show|branch|remote|blame)(\s|$)', $ic)) { return $true }
    if ([regex]::IsMatch($Cmd, '^\s*cargo\s+(check|test|build|clippy)(\s|$)', $ic)) { return $true }
    return $false
}

$denyReason = ''
switch ($tool) {
    { $_ -in @('Write','Edit','MultiEdit') } {
        $denyReason = '계획 확정 전 변경 차단 — plan_mode=on 상태입니다. 계획을 확정(/bathos plan off)한 뒤 다시 시도하세요. (BATHOS plan_mode는 Claude Code 네이티브 plan mode와 별개입니다.)'
    }
    'Bash' {
        if ([string]::IsNullOrEmpty($cmd)) { exit 0 }  # 명령 미확인 → 통과(보수적, careful-guard와 동일 관례)
        if (Test-ReadonlyBash -Cmd $cmd) { exit 0 }     # 읽기 전용 명령은 계획 모드에서도 허용 (AC2)
        $denyReason = '계획 확정 전 변경성 명령 차단 — plan_mode=on 상태입니다. 읽기 전용 명령(ls/cat/git status 등)만 허용됩니다. 계획을 확정(/bathos plan off)한 뒤 다시 시도하세요.'
    }
    default { exit 0 }
}

# --------------------------------------------------------------------------
# 5. 감사 기록 (단일 writer 경유, fail-safe)
# --------------------------------------------------------------------------
$targetExcerpt = $filePath
if ([string]::IsNullOrEmpty($targetExcerpt)) { $targetExcerpt = $cmd }
if ($targetExcerpt.Length -gt 150) { $targetExcerpt = $targetExcerpt.Substring(0, 150) }
$targetExcerpt = ($targetExcerpt -replace '["\\\r\n\t]', ' ')

$actor = $env:BATHOS_ROLE; if ([string]::IsNullOrEmpty($actor)) { $actor = 'hook:plan-gate' }
Add-BathosAudit -Context $ctx -Action 'plan.gated' -Target $targetExcerpt -Actor $actor

# --------------------------------------------------------------------------
# 6. deny 응답 — PreToolUse stdout JSON (권한모드 우회 불가, feasibility 확인됨)
#    ⚠️  exit 0 유지 — 차단은 permissionDecision:"deny" JSON이 담당한다.
# --------------------------------------------------------------------------
$outObj = [PSCustomObject]@{
    hookSpecificOutput = [PSCustomObject]@{
        hookEventName            = 'PreToolUse'
        permissionDecision        = 'deny'
        permissionDecisionReason  = '[bathos plan-gate] x ' + $denyReason
    }
}
$outObj | ConvertTo-Json -Depth 10 -Compress

Write-HookError ('[bathos plan-gate] x ' + $denyReason)
exit 0
