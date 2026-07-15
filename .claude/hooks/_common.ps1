# =============================================================================
# BATHOS — _common.ps1   (Windows PowerShell 포트 · 공유 헬퍼)
# 대응 원본: .claude/hooks/*.sh 의 공통 프리앰블(경로 해석·stdin 파싱·감사 append·
#            stderr 피드백)을 함수로 팩터링한 것. 각 *.ps1 훅이 dot-source 한다.
# -----------------------------------------------------------------------------
# 타깃: Windows PowerShell 5.1 하한(추가 설치 0) + PowerShell 7+ 호환.
#   - 삼항 연산자(`? :`)·널병합(`??`) 신문법 회피(5.1 미지원분).
#   - JSON은 네이티브 ConvertFrom-Json 사용(jq 불필요 — bash판의 jq 의존 제거).
# 계약 등가성: bash 프리앰블과 동일한 경로 변수·감사 writer(CLI 단일 writer)·
#   exit 코드(0 통과 / 2 차단) 규약을 보존한다.
# 안전: 헬퍼는 예외를 삼켜 훅을 절대 지연/오차단하지 않는다(엔진 부재 시 fail-safe).
#       단, freeze-guard 계열의 "판정 불가 → 차단(안전 측)" 정책은 각 훅이 결정한다.
# =============================================================================

Set-StrictMode -Version 2.0
$ErrorActionPreference = 'Stop'

# -----------------------------------------------------------------------------
# 1. 경로 해석 — bash: SCRIPT_DIR / BATHOS_ROOT / PROJECT_DIR / STATE_DIR / BATHOS_BIN
#    $PSScriptRoot = 이 _common.ps1 이 있는 .claude/hooks 디렉터리(dot-source 시에도
#    호출 파일이 아니라 이 파일 기준으로 확장됨 — bash BASH_SOURCE 등가).
# -----------------------------------------------------------------------------
function Get-BathosContext {
    $scriptDir  = $PSScriptRoot
    $bathosRoot = (Resolve-Path (Join-Path $scriptDir '..\..')).Path

    $projectDir = $env:CLAUDE_PROJECT_DIR
    if ([string]::IsNullOrEmpty($projectDir)) { $projectDir = $bathosRoot }

    $stateDir = $env:BATHOS_STATE_DIR
    if ([string]::IsNullOrEmpty($stateDir)) {
        $stateDir = Join-Path $projectDir '.agent-team\_state'
    }

    # 엔진 바이너리 해석: 환경변수 우선 → release → debug (Windows=.exe).
    # bash판은 debug를 기본으로 삼지만, 실제 운영은 release라 release를 먼저 탐색한다.
    $bin = $env:BATHOS_BIN
    if ([string]::IsNullOrEmpty($bin)) {
        $exe = 'bathos'
        if ($env:OS -eq 'Windows_NT') { $exe = 'bathos.exe' }
        $release = Join-Path $bathosRoot ('core\target\release\' + $exe)
        $debug   = Join-Path $bathosRoot ('core\target\debug\'   + $exe)
        if (Test-Path -LiteralPath $release)   { $bin = $release }
        elseif (Test-Path -LiteralPath $debug) { $bin = $debug }
        else { $bin = $release }  # 부재 시에도 경로는 반환(각 훅이 존재검사)
    }

    return [PSCustomObject]@{
        ScriptDir  = $scriptDir
        BathosRoot = $bathosRoot
        ProjectDir = $projectDir
        StateDir   = $stateDir
        Bin        = $bin
    }
}

# -----------------------------------------------------------------------------
# 2. stdin JSON 파싱 — bash: INPUT="$(cat)"; jq -r '...'
#    Claude Code 는 도구호출 JSON을 훅의 stdin으로 전달(전 플랫폼 공통 — 문서 확인).
#    반환: 파싱된 PSCustomObject, 또는 입력없음/파싱실패 시 $null.
# -----------------------------------------------------------------------------
function Read-HookInput {
    $raw = ''
    try { $raw = [Console]::In.ReadToEnd() } catch { $raw = '' }
    if ([string]::IsNullOrWhiteSpace($raw)) { return $null }
    try { return ($raw | ConvertFrom-Json) } catch { return $null }
}

# 원문 문자열이 필요할 때(정규식 grep 등 fallback)를 위해 원문+파싱을 함께 반환.
function Read-HookInputRaw {
    $raw = ''
    try { $raw = [Console]::In.ReadToEnd() } catch { $raw = '' }
    $obj = $null
    if (-not [string]::IsNullOrWhiteSpace($raw)) {
        try { $obj = ($raw | ConvertFrom-Json) } catch { $obj = $null }
    }
    return [PSCustomObject]@{ Raw = $raw; Json = $obj }
}

# 안전한 중첩 프로퍼티 접근 — bash: jq -r '.a.b // empty'
#   예: Get-JsonField $input @('tool_input','command')
function Get-JsonField {
    param(
        [Parameter(Mandatory=$true)] $Object,
        [Parameter(Mandatory=$true)] [string[]] $Path
    )
    $cur = $Object
    foreach ($seg in $Path) {
        if ($null -eq $cur) { return $null }
        $prop = $cur.PSObject.Properties[$seg]
        if ($null -eq $prop) { return $null }
        $cur = $prop.Value
    }
    return $cur
}

# -----------------------------------------------------------------------------
# 3. 감사 로그 append — bash: "$BATHOS_BIN" --state-dir "$STATE_DIR" audit append ...
#    Rust CLI 단일 writer 경유(B-1). 바이너리 없으면 조용히 통과(훅 차단 금지).
# -----------------------------------------------------------------------------
function Add-BathosAudit {
    param(
        [Parameter(Mandatory=$true)] $Context,
        [Parameter(Mandatory=$true)] [string] $Action,
        [Parameter(Mandatory=$true)] [string] $Target,
        [string] $Actor = ''
    )
    if ([string]::IsNullOrEmpty($Actor)) {
        $Actor = $env:BATHOS_ROLE
        if ([string]::IsNullOrEmpty($Actor)) { $Actor = 'agent' }
    }
    if (-not (Test-Path -LiteralPath $Context.Bin)) { return }
    # 민감정보 최소화: 앞 200자 + 제어문자 치환
    $safe = $Target
    if ($null -eq $safe) { $safe = '' }
    if ($safe.Length -gt 200) { $safe = $safe.Substring(0, 200) }
    $safe = ($safe -replace '["\\\r\n\t]', ' ')
    try {
        & $Context.Bin --state-dir $Context.StateDir audit append `
            --actor $Actor --action $Action --target $safe 2>$null | Out-Null
    } catch { }  # 훅은 절대 감사 실패로 차단/지연하지 않음
}

# -----------------------------------------------------------------------------
# 4. stderr 피드백 — bash: printf '...' >&2  (차단 사유 등)
#    PowerShell Write-Error 는 노이즈가 커서 [Console]::Error 직접 사용.
# -----------------------------------------------------------------------------
function Write-HookError {
    param([Parameter(Mandatory=$true)] [AllowEmptyString()] [string] $Message)
    [Console]::Error.WriteLine($Message)
}

# -----------------------------------------------------------------------------
# 5. glob 매칭 헬퍼 — bash: [[ "$path" == $pattern ]] / prefix/**
#    PowerShell -like 는 *,? 와일드카드 지원. "prefix/**" 는 prefix 이하 전체.
#    경로 구분자는 정규화(\ → /)해 패턴을 슬래시 기준으로 비교.
# -----------------------------------------------------------------------------
function ConvertTo-SlashPath {
    param([AllowEmptyString()] [string] $Path)
    if ([string]::IsNullOrEmpty($Path)) { return '' }
    return ($Path -replace '\\', '/')
}

function Test-GlobMatch {
    param(
        [Parameter(Mandatory=$true)] [string] $Path,
        [Parameter(Mandatory=$true)] [string] $Pattern
    )
    $p   = ConvertTo-SlashPath $Path
    $pat = ConvertTo-SlashPath $Pattern
    if ($pat.EndsWith('/**')) {
        $prefix = $pat.Substring(0, $pat.Length - 2)  # "prefix/"
        return $p.StartsWith($prefix)
    }
    return ($p -like $pat)
}

# -----------------------------------------------------------------------------
# 6. UTC ISO 타임스탬프 — bash: date -u +%Y-%m-%dT%H:%M:%SZ
# -----------------------------------------------------------------------------
function Get-UtcIsoNow {
    return [DateTime]::UtcNow.ToString('yyyy-MM-ddTHH:mm:ssZ')
}
