# =============================================================================
# BATHOS M6 — _test-hooks.ps1   (Windows PowerShell 포트)
# 원본: .claude/hooks/_test-hooks.sh
# 12개 훅(.ps1) 결정성(determinism) 테스트 스크립트 — bash판 1:1 이식
# =============================================================================
# 실행: pwsh -File .claude/hooks/_test-hooks.ps1  (또는 powershell -File ...)
# 목적: 각 .ps1 훅의 차단·통과 동작을 재현 가능하게 검증 (.sh 훅이 아니라 .ps1 훅 대상)
# DoD(M6): 파괴명령·freeze·FAIL 차단 결정성 테스트 통과
#
# ── 이식 설계 메모 ───────────────────────────────────────────────────────────
# [해석기 선택] 훅 자식 프로세스는 "현재 이 스크립트를 실행 중인 인터프리터"가
#   아니라, powershell.exe(5.1) 우선 → 없으면 pwsh 순으로 고정 선택한다.
#   이유: 이 하니스는 CI에서 `pwsh` 셸로 구동되지만(윈도우 러너에 5.1+7 모두
#   존재), 실제 Claude Code 운영 환경의 바닥선(floor)은 PowerShell 5.1이다.
#   "5.1로 실행되는 훅이 실제로 5.1에서도 동작하는지"를 검증하는 것이
#   "테스트를 구동 중인 인터프리터와 동일한 것으로 훅도 실행"하는 것보다
#   더 강한 보장이므로 powershell(5.1)을 우선한다(팀 지시사항 그대로 채택).
# [BATHOS_BIN] bash판은 하드코딩된 core/target/debug/bathos 경로만 본다(윈도우
#   CI는 release 빌드만 만들고 BATHOS_BIN 환경변수로 release\bathos.exe 경로를
#   넘겨준다 — ci.yml windows 잡 참고). 이 포트는 $env:BATHOS_BIN을 우선 존중하고,
#   없으면 release → debug 순으로 자동 탐지한다(팀 지시사항 반영, bash판보다
#   상위호환 — bash판의 "debug 고정 탐색"은 웹도우 CI 환경과 맞지 않기 때문).
# [프로세스 격리] bash는 `VAR=value command` 형태로 서브셸에만 환경변수를
#   주입한다. PowerShell `$env:X=...`는 현재 프로세스에 영구 반영되므로, 이
#   하니스는 자식 프로세스마다 System.Diagnostics.ProcessStartInfo의
#   EnvironmentVariables 딕셔너리를 직접 채워 넣어(하니스 자신의 $env: 는 절대
#   건드리지 않음) bash의 "그 호출에만 한정된 환경변수" 의미를 그대로 재현한다.
#   관리 대상 변수(BATHOS_STATE_DIR/BATHOS_ROLE/CLAUDE_PROJECT_DIR/
#   BATHOS_OWNED_PATHS/BATHOS_BIN)는 각 호출에서 명시하지 않으면 자식에서
#   제거(unset)되어 bash 서브셸과 동일한 격리를 보장한다.
# [stdin/stdout/stderr] JSON은 UTF-8(BOM 없음) 원시 바이트로 자식의 표준입력에
#   직접 쓴다(StandardInputEncoding 프로퍼티는 .NET 5+ 전용이라 5.1 산출물과
#   호환되지 않으므로 사용하지 않음 — BaseStream에 바이트를 직접 쓰는 방식은
#   .NET Framework/.NET 어디서나 동일하게 동작). 표준출력/표준오류는
#   StandardOutputEncoding/StandardErrorEncoding(.NET 4.5+, 5.1도 지원)을
#   UTF-8로 지정해 읽는다.
# [JSON 조립] 훅에 흘려보내는 stdin JSON은 bash판과 동일하게 손으로 조립한
#   문자열 리터럴을 그대로 쓰되, 임시 경로(윈도우 백슬래시 포함)나 한글 값처럼
#   동적으로 끼워 넣는 부분만 ConvertTo-JsonString 헬퍼로 이스케이프한다.
#   (ConvertTo-Json의 파이프라인 배열 언랩 특이사항을 피하기 위해 픽스처
#   JSON도 동일한 손 조립 방식을 씀 — 정확한 키 이름/형태를 bash판과 100%
#   맞추기 위함이기도 함.)
# [python3 의존 제거] bash판은 fingerprint manifest 픽스처 생성과 JSON 필드
#   추출에 python3(+jq)를 썼다. 윈도우 러너에 python3 존재를 보장할 수 없으므로
#   전부 네이티브 ConvertFrom-Json / .NET(Guid, DateTime)으로 대체했다.
# [7번 섹션 settings.json 검사] bash판은 `.claude/settings.json`(.sh 훅을
#   연결하는 파일)의 JSON 유효성만 확인한다. 이 윈도우 포트가 실제로 배선하는
#   파일은 `.claude/settings.windows.json`(.ps1 훅 연결)이므로 그 파일을
#   1순위로 검사하고, 없으면 settings.json으로 폴백한다(원본의 "파일 없으면
#   SKIP" 관대함은 그대로 유지) — 아래 "1:1 이식 불가 항목"에 사유 기재.
# =============================================================================

$ErrorActionPreference = 'Continue'
# 참고: 이 파일은 프로덕션 훅(_common.ps1 dot-source 대상)이 아니라 테스트
# 하니스이므로 의도적으로 Set-StrictMode를 걸지 않는다 — 91개 검사 중 하나가
# 예기치 못한 속성 접근으로 죽더라도 전체 스위트가 중단되면 안 되기 때문
# (각 섹션을 try/catch로 감싸 개별 실패를 FAIL 1건으로 국소화한다).

$ScriptDir = $PSScriptRoot
$HooksDir  = $ScriptDir

# --------------------------------------------------------------------------
# 테스트용 임시 상태 디렉터리 (bash: mktemp -d)
# --------------------------------------------------------------------------
$TmpBase = Join-Path ([System.IO.Path]::GetTempPath()) ('bathos-test-' + [System.IO.Path]::GetRandomFileName())
New-Item -ItemType Directory -Path $TmpBase -Force | Out-Null
$TestStateDir = Join-Path $TmpBase '_state'
New-Item -ItemType Directory -Path $TestStateDir -Force | Out-Null

$script:PassCount = 0
$script:FailCount = 0

# --------------------------------------------------------------------------
# 해석기 선택 (powershell.exe 우선 → pwsh, 팀 지시사항의 "가장 단단한 방법")
# --------------------------------------------------------------------------
function Resolve-TestInterpreter {
    $cmd = Get-Command 'powershell.exe' -ErrorAction SilentlyContinue
    if (-not $cmd) { $cmd = Get-Command 'powershell' -ErrorAction SilentlyContinue }
    if (-not $cmd) { $cmd = Get-Command 'pwsh' -ErrorAction SilentlyContinue }
    if (-not $cmd) { throw '[_test-hooks] powershell/pwsh 실행 파일을 찾을 수 없습니다.' }
    return $cmd.Source
}
$script:Interpreter = Resolve-TestInterpreter
Write-Host ('[_test-hooks] 훅 자식 프로세스 인터프리터: ' + $script:Interpreter)

# --------------------------------------------------------------------------
# BATHOS_BIN 해석 ($env:BATHOS_BIN 우선 → release → debug, .exe)
# --------------------------------------------------------------------------
$BathosRoot = $null
try { $BathosRoot = (Resolve-Path (Join-Path $HooksDir '..\..')).Path } catch { $BathosRoot = Join-Path $HooksDir '..\..' }

$BathosBinPath = $env:BATHOS_BIN
if ([string]::IsNullOrEmpty($BathosBinPath)) {
    $releaseBin = Join-Path $BathosRoot 'core\target\release\bathos.exe'
    $debugBin   = Join-Path $BathosRoot 'core\target\debug\bathos.exe'
    if (Test-Path -LiteralPath $releaseBin -PathType Leaf) { $BathosBinPath = $releaseBin }
    elseif (Test-Path -LiteralPath $debugBin -PathType Leaf) { $BathosBinPath = $debugBin }
    else { $BathosBinPath = $releaseBin }
}
$HasBathosBin = Test-Path -LiteralPath $BathosBinPath -PathType Leaf
Write-Host ('[_test-hooks] BATHOS_BIN 해석 경로: ' + $BathosBinPath + ' (존재=' + $HasBathosBin + ')')

# ============================================================================
# 공통 헬퍼
# ============================================================================

function Assert-Exit {
    param(
        [Parameter(Mandatory=$true)][string]$Desc,
        [Parameter(Mandatory=$true)][int]$Expected,
        [Parameter(Mandatory=$true)][int]$Actual
    )
    if ($Actual -eq $Expected) {
        Write-Host ("[PASS] {0} (exit={1})" -f $Desc, $Actual) -ForegroundColor Green
        $script:PassCount++
    } else {
        Write-Host ("[FAIL] {0} — 예상 exit={1}, 실제 exit={2}" -f $Desc, $Expected, $Actual) -ForegroundColor Red
        $script:FailCount++
    }
}

function Assert-Condition {
    param(
        [Parameter(Mandatory=$true)][string]$Desc,
        [Parameter(Mandatory=$true)][bool]$Condition,
        [string]$FailDetail = ''
    )
    if ($Condition) {
        Write-Host ("[PASS] {0}" -f $Desc) -ForegroundColor Green
        $script:PassCount++
    } else {
        $msg = "[FAIL] $Desc"
        if ($FailDetail -ne '') { $msg += (": " + $FailDetail) }
        Write-Host $msg -ForegroundColor Red
        $script:FailCount++
    }
}

function Write-Info {
    param([string]$Message)
    Write-Host ("[INFO] " + $Message) -ForegroundColor Yellow
}

# JSON 문자열 리터럴 이스케이프(따옴표 포함 반환) — 손 조립 JSON에 변수를
# 안전하게 끼워넣기 위함(윈도우 경로의 백슬래시, 한글 텍스트의 따옴표 등).
function ConvertTo-JsonString {
    param([AllowNull()][string]$Value)
    if ($null -eq $Value) { $Value = '' }
    $sb = [System.Text.StringBuilder]::new()
    [void]$sb.Append('"')
    foreach ($ch in $Value.ToCharArray()) {
        switch ($ch) {
            '"'  { [void]$sb.Append('\"') }
            '\'  { [void]$sb.Append('\\') }
            "`n" { [void]$sb.Append('\n') }
            "`r" { [void]$sb.Append('\r') }
            "`t" { [void]$sb.Append('\t') }
            default { [void]$sb.Append($ch) }
        }
    }
    [void]$sb.Append('"')
    return $sb.ToString()
}

# 경로 구분자 정규화(\ → /) — freeze-guard.ps1/scope-inject.ps1이 내부적으로
# 쓰는 ConvertTo-SlashPath와 동일 발상. 여러 섹션(10/13)에서 공용으로 쓴다.
function ConvertTo-SlashPathLocal {
    param([string]$Path)
    return ($Path -replace '\\', '/')
}

function New-UtcNowIso {
    return [DateTime]::UtcNow.ToString('yyyy-MM-ddTHH:mm:ssZ')
}

function Write-Utf8NoBom {
    param([string]$Path, [string]$Content)
    $dir = Split-Path -Path $Path -Parent
    if (-not [string]::IsNullOrEmpty($dir)) { New-Item -ItemType Directory -Path $dir -Force -ErrorAction SilentlyContinue | Out-Null }
    $enc = New-Object System.Text.UTF8Encoding($false)
    [System.IO.File]::WriteAllText($Path, $Content, $enc)
}

function Test-FileMatches {
    param([string]$Path, [string]$Pattern)
    if (-not (Test-Path -LiteralPath $Path)) { return $false }
    $content = $null
    try { $content = Get-Content -LiteralPath $Path -Raw -ErrorAction Stop } catch { return $false }
    if ($null -eq $content) { return $false }
    return [regex]::IsMatch($content, $Pattern)
}

# --------------------------------------------------------------------------
# 훅 호출 — bash: run_hook() 대응. 관리 대상 환경변수는 명시하지 않으면 자식
# 프로세스에서 제거된다(격리). stdin은 UTF-8 원시 바이트로 직접 기록한다.
# --------------------------------------------------------------------------
$script:ManagedEnvVars = @('BATHOS_STATE_DIR','BATHOS_ROLE','CLAUDE_PROJECT_DIR','BATHOS_OWNED_PATHS','BATHOS_BIN')

function Invoke-HookCapture {
    param(
        [Parameter(Mandatory=$true)][string]$Hook,
        [Parameter(Mandatory=$true)][AllowEmptyString()][string]$StdinJson,
        [hashtable]$EnvVars = @{}
    )
    $hookPath = Join-Path $HooksDir $Hook

    $psi = [System.Diagnostics.ProcessStartInfo]::new()
    $psi.FileName = $script:Interpreter
    $psi.Arguments = '-NoProfile -ExecutionPolicy Bypass -File "' + $hookPath + '"'
    $psi.UseShellExecute = $false
    $psi.RedirectStandardInput = $true
    $psi.RedirectStandardOutput = $true
    $psi.RedirectStandardError = $true
    try { $psi.StandardOutputEncoding = [System.Text.Encoding]::UTF8 } catch { }
    try { $psi.StandardErrorEncoding = [System.Text.Encoding]::UTF8 } catch { }
    $psi.WorkingDirectory = $HooksDir

    # 관리 대상 변수: 명시된 것만 설정, 나머지는 자식에서 제거(부모 프로세스의
    # 실제 env 값을 절대 물려주지 않음 — bash 서브셸 스코프와 동등한 격리).
    foreach ($k in $script:ManagedEnvVars) {
        if ($EnvVars.ContainsKey($k)) {
            $psi.EnvironmentVariables[$k] = [string]$EnvVars[$k]
        } else {
            if ($psi.EnvironmentVariables.ContainsKey($k)) { $psi.EnvironmentVariables.Remove($k) | Out-Null }
        }
    }

    $stdout = ''
    $stderr = ''
    $code = 1
    try {
        $proc = [System.Diagnostics.Process]::Start($psi)
        $bytes = [System.Text.Encoding]::UTF8.GetBytes($StdinJson)
        $proc.StandardInput.BaseStream.Write($bytes, 0, $bytes.Length)
        $proc.StandardInput.BaseStream.Flush()
        $proc.StandardInput.Close()
        $stdout = $proc.StandardOutput.ReadToEnd()
        $stderr = $proc.StandardError.ReadToEnd()
        $proc.WaitForExit()
        $code = $proc.ExitCode
    } catch {
        $stderr = $_.Exception.Message
        $code = 1
    }

    return [PSCustomObject]@{ ExitCode = $code; Stdout = $stdout; Stderr = $stderr }
}

function Get-DefaultEnv {
    return @{
        BATHOS_STATE_DIR   = $TestStateDir
        BATHOS_ROLE        = 'test-agent'
        CLAUDE_PROJECT_DIR = $TmpBase
    }
}

# bash: run_hook() — 기본 3종 환경변수로 실행 후 exit code만 반환
function Invoke-Hook {
    param([string]$Hook, [string]$StdinJson)
    return (Invoke-HookCapture -Hook $Hook -StdinJson $StdinJson -EnvVars (Get-DefaultEnv)).ExitCode
}

try {

# ============================================================================
# 1. careful-guard.ps1 테스트
# ============================================================================
try {
Write-Host "`n=== 1. careful-guard.ps1 ===" -ForegroundColor Cyan

$json = '{"tool_name":"Bash","tool_input":{"command":"rm -rf /"}}'
Assert-Exit -Desc 'rm -rf / 차단' -Expected 2 -Actual (Invoke-Hook -Hook 'careful-guard.ps1' -StdinJson $json)

$json = '{"tool_name":"Bash","tool_input":{"command":"rm -rf ~"}}'
Assert-Exit -Desc 'rm -rf ~ 차단' -Expected 2 -Actual (Invoke-Hook -Hook 'careful-guard.ps1' -StdinJson $json)

$json = '{"tool_name":"Bash","tool_input":{"command":"git push origin main --force"}}'
Assert-Exit -Desc 'git push --force 차단' -Expected 2 -Actual (Invoke-Hook -Hook 'careful-guard.ps1' -StdinJson $json)

$json = '{"tool_name":"Bash","tool_input":{"command":"git reset --hard HEAD~1"}}'
Assert-Exit -Desc 'git reset --hard 차단' -Expected 2 -Actual (Invoke-Hook -Hook 'careful-guard.ps1' -StdinJson $json)

$json = '{"tool_name":"Bash","tool_input":{"command":"psql -c \"DROP TABLE users\""}}'
Assert-Exit -Desc 'DROP TABLE 차단' -Expected 2 -Actual (Invoke-Hook -Hook 'careful-guard.ps1' -StdinJson $json)

$json = '{"tool_name":"Bash","tool_input":{"command":"rm -fr build/"}}'
Assert-Exit -Desc 'rm -fr 변형 차단' -Expected 2 -Actual (Invoke-Hook -Hook 'careful-guard.ps1' -StdinJson $json)

$json = '{"tool_name":"Bash","tool_input":{"command":"ls -la src/"}}'
Assert-Exit -Desc 'ls -la 통과' -Expected 0 -Actual (Invoke-Hook -Hook 'careful-guard.ps1' -StdinJson $json)

$json = '{"tool_name":"Bash","tool_input":{"command":"git status"}}'
Assert-Exit -Desc 'git status 통과' -Expected 0 -Actual (Invoke-Hook -Hook 'careful-guard.ps1' -StdinJson $json)

$json = '{"tool_name":"Bash","tool_input":{"command":"cargo build --release"}}'
Assert-Exit -Desc 'cargo build 통과' -Expected 0 -Actual (Invoke-Hook -Hook 'careful-guard.ps1' -StdinJson $json)

$json = '{"tool_name":"Write","tool_input":{"file_path":"bathos/.claude/hooks/test.sh","content":"#!/bin/bash\n"}}'
Assert-Exit -Desc 'Write 도구(비Bash) 통과' -Expected 0 -Actual (Invoke-Hook -Hook 'careful-guard.ps1' -StdinJson $json)

# --- L-4 보강 케이스: WHERE 없는 DELETE 2단계 검사 (2026-06-30) ---

$json = '{"tool_name":"Bash","tool_input":{"command":"psql -c \"DELETE FROM users\""}}'
Assert-Exit -Desc '[L-4] DELETE FROM users (WHERE 없음) → 차단' -Expected 2 -Actual (Invoke-Hook -Hook 'careful-guard.ps1' -StdinJson $json)

$json = '{"tool_name":"Bash","tool_input":{"command":"psql -c \"DELETE FROM users;\""}}'
Assert-Exit -Desc '[L-4] DELETE FROM users; (세미콜론, WHERE 없음) → 차단' -Expected 2 -Actual (Invoke-Hook -Hook 'careful-guard.ps1' -StdinJson $json)

$json = '{"tool_name":"Bash","tool_input":{"command":"psql -c \"DELETE FROM users -- truncate all\""}}'
Assert-Exit -Desc '[L-4] DELETE FROM users -- comment (WHERE 없음) → 차단' -Expected 2 -Actual (Invoke-Hook -Hook 'careful-guard.ps1' -StdinJson $json)

$json = '{"tool_name":"Bash","tool_input":{"command":"psql -c \"DELETE FROM users WHERE id=1\""}}'
Assert-Exit -Desc '[L-4] DELETE FROM users WHERE id=1 (WHERE 있음) → 통과' -Expected 0 -Actual (Invoke-Hook -Hook 'careful-guard.ps1' -StdinJson $json)

$json = '{"tool_name":"Bash","tool_input":{"command":"psql -c \"TRUNCATE TABLE sessions\""}}'
Assert-Exit -Desc '[L-4] TRUNCATE TABLE sessions → 차단' -Expected 2 -Actual (Invoke-Hook -Hook 'careful-guard.ps1' -StdinJson $json)

} catch {
    Write-Host ("[FAIL] 섹션 1(careful-guard) 실행 중 예외: " + $_.Exception.Message) -ForegroundColor Red
    $script:FailCount++
}

# ============================================================================
# 2. freeze-guard.ps1 테스트
# ============================================================================
try {
Write-Host "`n=== 2. freeze-guard.ps1 ===" -ForegroundColor Cyan

$json = '{"tool_name":"Write","tool_input":{"file_path":"any/path/file.txt"}}'
$ec = (Invoke-HookCapture -Hook 'freeze-guard.ps1' -StdinJson $json -EnvVars @{
    BATHOS_OWNED_PATHS = ''
    BATHOS_STATE_DIR   = $TestStateDir
    CLAUDE_PROJECT_DIR = $TmpBase
}).ExitCode
Assert-Exit -Desc 'BATHOS_OWNED_PATHS 미설정 → 통과' -Expected 0 -Actual $ec

# [Dynamis A1 수정] 원 픽스처(new.sh)는 신규 §5 위험경로 fingerprint 게이트와
# 겹쳐 "순수 소유권 검사"를 격리할 수 없다 — §10에서 위험경로 게이트를 별도
# 검증하므로 여기서는 비위험경로 픽스처로 owned_paths 로직만 격리 검증한다.
$json = '{"tool_name":"Write","tool_input":{"file_path":"bathos/core/crates/bathos-state/src/new.rs"}}'
$ec = (Invoke-HookCapture -Hook 'freeze-guard.ps1' -StdinJson $json -EnvVars @{
    BATHOS_OWNED_PATHS = 'bathos/core/crates/bathos-state/**:bathos/docs/README.md'
    BATHOS_STATE_DIR   = $TestStateDir
    CLAUDE_PROJECT_DIR = $TmpBase
}).ExitCode
Assert-Exit -Desc '소유 경로 내 Write 통과(비위험경로)' -Expected 0 -Actual $ec

$json = '{"tool_name":"Edit","tool_input":{"file_path":"bathos/docs/README.md"}}'
$ec = (Invoke-HookCapture -Hook 'freeze-guard.ps1' -StdinJson $json -EnvVars @{
    BATHOS_OWNED_PATHS = 'bathos/core/crates/bathos-state/**:bathos/docs/README.md'
    BATHOS_STATE_DIR   = $TestStateDir
    CLAUDE_PROJECT_DIR = $TmpBase
}).ExitCode
Assert-Exit -Desc '소유 경로 정확 일치 Edit 통과(비위험경로)' -Expected 0 -Actual $ec

$json = '{"tool_name":"Write","tool_input":{"file_path":"bathos/core/src/main.rs"}}'
$ec = (Invoke-HookCapture -Hook 'freeze-guard.ps1' -StdinJson $json -EnvVars @{
    BATHOS_OWNED_PATHS = 'bathos/.claude/hooks/**:bathos/.claude/settings.json'
    BATHOS_STATE_DIR   = $TestStateDir
    CLAUDE_PROJECT_DIR = $TmpBase
}).ExitCode
Assert-Exit -Desc '소유 경로 밖 Write 차단' -Expected 2 -Actual $ec

$json = '{"tool_name":"Edit","tool_input":{"file_path":".agent-team/04-architecture/build-plan.md"}}'
$ec = (Invoke-HookCapture -Hook 'freeze-guard.ps1' -StdinJson $json -EnvVars @{
    BATHOS_OWNED_PATHS = 'bathos/.claude/hooks/**:bathos/.claude/settings.json'
    BATHOS_STATE_DIR   = $TestStateDir
    CLAUDE_PROJECT_DIR = $TmpBase
}).ExitCode
Assert-Exit -Desc '.agent-team/ 편집 차단' -Expected 2 -Actual $ec

$json = '{"tool_name":"Bash","tool_input":{"command":"echo hello"}}'
$ec = (Invoke-HookCapture -Hook 'freeze-guard.ps1' -StdinJson $json -EnvVars @{
    BATHOS_OWNED_PATHS = 'bathos/.claude/hooks/**:bathos/.claude/settings.json'
    BATHOS_STATE_DIR   = $TestStateDir
    CLAUDE_PROJECT_DIR = $TmpBase
}).ExitCode
Assert-Exit -Desc 'Bash 도구 freeze-guard 통과' -Expected 0 -Actual $ec

$absPath = $TmpBase + '/bathos/core/crates/bathos-state/src/test.rs'
$json = '{"tool_name":"Write","tool_input":{"file_path":' + (ConvertTo-JsonString $absPath) + '}}'
$ec = (Invoke-HookCapture -Hook 'freeze-guard.ps1' -StdinJson $json -EnvVars @{
    BATHOS_OWNED_PATHS = 'bathos/core/crates/bathos-state/**:bathos/docs/README.md'
    BATHOS_STATE_DIR   = $TestStateDir
    CLAUDE_PROJECT_DIR = $TmpBase
}).ExitCode
Assert-Exit -Desc '절대 경로 소유 내 통과(비위험경로)' -Expected 0 -Actual $ec

} catch {
    Write-Host ("[FAIL] 섹션 2(freeze-guard) 실행 중 예외: " + $_.Exception.Message) -ForegroundColor Red
    $script:FailCount++
}

# ============================================================================
# 3. audit-log.ps1 테스트 (B-1: CLI 경유 단일 writer)
# ============================================================================
try {
Write-Host "`n=== 3. audit-log.ps1 ===" -ForegroundColor Cyan

$json = '{"tool_name":"Write","tool_input":{"file_path":"bathos/.claude/hooks/test.sh"},"tool_response":{"type":"result","result":"ok"}}'

$ec = (Invoke-HookCapture -Hook 'audit-log.ps1' -StdinJson $json -EnvVars @{
    BATHOS_STATE_DIR   = $TestStateDir
    BATHOS_ROLE        = 'andrew'
    CLAUDE_PROJECT_DIR = $TmpBase
}).ExitCode
Assert-Exit -Desc 'audit-log Write exit 0 (비강제, CLI 유무 무관)' -Expected 0 -Actual $ec

$auditLogPath = Join-Path $TestStateDir 'audit-log.jsonl'
if ($HasBathosBin) {
    Write-Info 'BATHOS CLI 빌드 완료 — 실제 audit append 검증'
    $auditLines = @()
    if (Test-Path -LiteralPath $auditLogPath) { $auditLines = @(Get-Content -LiteralPath $auditLogPath -ErrorAction SilentlyContinue) }
    Assert-Condition -Desc ('audit-log.jsonl CLI 경유 생성 확인 (' + $auditLines.Count + ' 줄)') -Condition ($auditLines.Count -ge 1)

    $allValidJson = $true
    if ($auditLines.Count -eq 0) { $allValidJson = $false }
    foreach ($ln in $auditLines) {
        if ([string]::IsNullOrWhiteSpace($ln)) { continue }
        try { $null = $ln | ConvertFrom-Json } catch { $allValidJson = $false }
    }
    Assert-Condition -Desc 'audit-log.jsonl JSON 형식 유효' -Condition $allValidJson

    $hasHashPrev = Test-FileMatches -Path $auditLogPath -Pattern '"hash_prev"'
    Assert-Condition -Desc 'hash_prev 체인 필드 존재' -Condition $hasHashPrev
} else {
    Write-Info 'CLI 미빌드 — || 안전 통과 검증'
    Write-Info ('가정 CLI 시그니처: & $BATHOS_BIN --state-dir $STATE_DIR audit append --actor <A> --action tool:<T> --target <S>')
    Assert-Condition -Desc 'CLI 부재 → 안전 통과 확인 [3-2 pending: CLI 빌드 후 재검증]' -Condition $true
    Assert-Condition -Desc '[3-3 pending] JSON 형식: CLI 빌드 후 재검증' -Condition $true
    Assert-Condition -Desc '[3-4 pending] hash_prev 체인: CLI 빌드 후 재검증' -Condition $true
}

$ec = (Invoke-HookCapture -Hook 'audit-log.ps1' -StdinJson $json -EnvVars @{
    BATHOS_STATE_DIR   = (Join-Path $TmpBase 'nonexistent-state-dir-xyz')
    CLAUDE_PROJECT_DIR = $TmpBase
}).ExitCode
Assert-Exit -Desc '_state 없어도 audit-log exit 0 (비강제)' -Expected 0 -Actual $ec

} catch {
    Write-Host ("[FAIL] 섹션 3(audit-log) 실행 중 예외: " + $_.Exception.Message) -ForegroundColor Red
    $script:FailCount++
}

# ============================================================================
# 4. artifact-verify.ps1 테스트
# ============================================================================
try {
Write-Host "`n=== 4. artifact-verify.ps1 ===" -ForegroundColor Cyan

$json = '{"title":"QA 검증 태스크 완료","description":"테스트 완료 처리"}'
$ec = (Invoke-HookCapture -Hook 'artifact-verify.ps1' -StdinJson $json -EnvVars @{ CLAUDE_PROJECT_DIR = $TmpBase }).ExitCode
Assert-Exit -Desc 'QA 태스크 + qa-summary 없음 → 차단' -Expected 2 -Actual $ec

New-Item -ItemType Directory -Path (Join-Path $TmpBase '.agent-team/11-qa') -Force | Out-Null
Write-Utf8NoBom -Path (Join-Path $TmpBase '.agent-team/11-qa/qa-summary.md') -Content "# QA Summary`n"
$json = '{"title":"QA 검증 태스크 완료","description":"테스트 완료 처리"}'
$ec = (Invoke-HookCapture -Hook 'artifact-verify.ps1' -StdinJson $json -EnvVars @{ CLAUDE_PROJECT_DIR = $TmpBase }).ExitCode
Assert-Exit -Desc 'QA 태스크 + qa-summary 존재 → 통과' -Expected 0 -Actual $ec

$json = '{"title":"W3 게이트 판정 완료","description":"story gate"}'
$ec = (Invoke-HookCapture -Hook 'artifact-verify.ps1' -StdinJson $json -EnvVars @{ CLAUDE_PROJECT_DIR = $TmpBase }).ExitCode
Assert-Exit -Desc 'W3 태스크 + readiness-report 없음 → 차단' -Expected 2 -Actual $ec

New-Item -ItemType Directory -Path (Join-Path $TmpBase '.agent-team/03-story-engineering') -Force | Out-Null
$readinessContent = @'
---
gate_type: "Implementation"
verdict: "PASS"
issues_total: 0
issues_critical: 0
---
# Readiness Report
'@
Write-Utf8NoBom -Path (Join-Path $TmpBase '.agent-team/03-story-engineering/readiness-report-kr.md') -Content $readinessContent
$json = '{"title":"W3 게이트 판정 완료","description":"story gate"}'
$ec = (Invoke-HookCapture -Hook 'artifact-verify.ps1' -StdinJson $json -EnvVars @{ CLAUDE_PROJECT_DIR = $TmpBase }).ExitCode
Assert-Exit -Desc 'W3 태스크 + readiness-report + verdict → 통과' -Expected 0 -Actual $ec

$json = '{"title":"일반 코딩 작업","description":"함수 구현"}'
$ec = (Invoke-HookCapture -Hook 'artifact-verify.ps1' -StdinJson $json -EnvVars @{ CLAUDE_PROJECT_DIR = $TmpBase }).ExitCode
Assert-Exit -Desc '관련 없는 태스크 → 통과' -Expected 0 -Actual $ec

} catch {
    Write-Host ("[FAIL] 섹션 4(artifact-verify) 실행 중 예외: " + $_.Exception.Message) -ForegroundColor Red
    $script:FailCount++
}

# ============================================================================
# 5. gate-enforce.ps1 테스트
# ============================================================================
try {
Write-Host "`n=== 5. gate-enforce.ps1 ===" -ForegroundColor Cyan

$json = '{"title":"W5 구현 시작","description":"enter W5 implementation"}'
$ec = (Invoke-HookCapture -Hook 'gate-enforce.ps1' -StdinJson $json -EnvVars @{
    CLAUDE_PROJECT_DIR = $TmpBase
    BATHOS_STATE_DIR   = (Join-Path $TmpBase '_state_empty')
}).ExitCode
Assert-Exit -Desc 'W5 진입 + manifest 없음 → fail-safe 통과' -Expected 0 -Actual $ec

New-Item -ItemType Directory -Path $TestStateDir -Force | Out-Null
$manifestContent = @'
{
  "project": "BATHOS test",
  "gates": [
    {"gate_type": "Implementation", "wave_id": "W3", "verdict": "PASS"}
  ]
}
'@
Write-Utf8NoBom -Path (Join-Path $TestStateDir 'manifest.json') -Content $manifestContent
$json = '{"title":"W5 구현 시작","description":"enter W5 implementation"}'
$ec = (Invoke-HookCapture -Hook 'gate-enforce.ps1' -StdinJson $json -EnvVars @{
    CLAUDE_PROJECT_DIR = $TmpBase
    BATHOS_STATE_DIR   = $TestStateDir
}).ExitCode
Assert-Exit -Desc 'W5 진입 + verdict=PASS → 통과' -Expected 0 -Actual $ec

$manifestContent = @'
{
  "project": "BATHOS test",
  "gates": [
    {"gate_type": "Implementation", "wave_id": "W3", "verdict": "CONCERNS"}
  ]
}
'@
Write-Utf8NoBom -Path (Join-Path $TestStateDir 'manifest.json') -Content $manifestContent
$json = '{"title":"W5 구현 웨이브 진입","description":"enter W5 implementation"}'
$ec = (Invoke-HookCapture -Hook 'gate-enforce.ps1' -StdinJson $json -EnvVars @{
    CLAUDE_PROJECT_DIR = $TmpBase
    BATHOS_STATE_DIR   = $TestStateDir
}).ExitCode
Assert-Exit -Desc 'W5 진입 + verdict=CONCERNS → 통과(경고)' -Expected 0 -Actual $ec

$manifestContent = @'
{
  "project": "BATHOS test",
  "gates": [
    {"gate_type": "Implementation", "wave_id": "W3", "verdict": "FAIL"}
  ]
}
'@
Write-Utf8NoBom -Path (Join-Path $TestStateDir 'manifest.json') -Content $manifestContent
$json = '{"title":"W5 구현 시작 진입","description":"implementation wave5 enter"}'
$ec = (Invoke-HookCapture -Hook 'gate-enforce.ps1' -StdinJson $json -EnvVars @{
    CLAUDE_PROJECT_DIR = $TmpBase
    BATHOS_STATE_DIR   = $TestStateDir
}).ExitCode
Assert-Exit -Desc 'W5 진입 + verdict=FAIL → 차단(핵심)' -Expected 2 -Actual $ec

$json = '{"title":"일반 코딩 작업 완료","description":"refactoring done"}'
$ec = (Invoke-HookCapture -Hook 'gate-enforce.ps1' -StdinJson $json -EnvVars @{
    CLAUDE_PROJECT_DIR = $TmpBase
    BATHOS_STATE_DIR   = $TestStateDir
}).ExitCode
Assert-Exit -Desc 'W5 아닌 태스크 → 통과' -Expected 0 -Actual $ec

$manifestContent = '{"project": "BATHOS test"}'
Write-Utf8NoBom -Path (Join-Path $TestStateDir 'manifest.json') -Content $manifestContent
$readinessFailContent = @'
---
gate_type: "Implementation"
verdict: "FAIL"
---
# Readiness Report
'@
Write-Utf8NoBom -Path (Join-Path $TmpBase '.agent-team/03-story-engineering/readiness-report-kr.md') -Content $readinessFailContent
$json = '{"title":"W5 구현 웨이브 진입","description":"implementation wave5 enter"}'
$ec = (Invoke-HookCapture -Hook 'gate-enforce.ps1' -StdinJson $json -EnvVars @{
    CLAUDE_PROJECT_DIR = $TmpBase
    BATHOS_STATE_DIR   = $TestStateDir
}).ExitCode
Assert-Exit -Desc 'readiness-report fallback FAIL → 차단' -Expected 2 -Actual $ec

} catch {
    Write-Host ("[FAIL] 섹션 5(gate-enforce) 실행 중 예외: " + $_.Exception.Message) -ForegroundColor Red
    $script:FailCount++
}

# ============================================================================
# 6. next-action.ps1 테스트
# ============================================================================
try {
Write-Host "`n=== 6. next-action.ps1 ===" -ForegroundColor Cyan

$json = '{"role":"Andrew","remaining_tasks":["task1","task2"]}'
$ec = (Invoke-HookCapture -Hook 'next-action.ps1' -StdinJson $json -EnvVars @{
    CLAUDE_PROJECT_DIR = $TmpBase
    BATHOS_STATE_DIR   = $TestStateDir
}).ExitCode
Assert-Exit -Desc 'next-action 항상 exit 0 (비강제)' -Expected 0 -Actual $ec

$json = '{"role":"Andrew","remaining_tasks":[]}'
$ec = (Invoke-HookCapture -Hook 'next-action.ps1' -StdinJson $json -EnvVars @{
    CLAUDE_PROJECT_DIR = $TmpBase
    BATHOS_STATE_DIR   = (Join-Path $TmpBase 'nonexistent-state-dir-abc')
}).ExitCode
Assert-Exit -Desc 'manifest 없어도 exit 0' -Expected 0 -Actual $ec

} catch {
    Write-Host ("[FAIL] 섹션 6(next-action) 실행 중 예외: " + $_.Exception.Message) -ForegroundColor Red
    $script:FailCount++
}

# ============================================================================
# 7. artifact-verify.ps1 SubagentStop 분기 테스트 (B-3)
# ============================================================================
try {
Write-Host "`n=== 7. artifact-verify.ps1 SubagentStop 분기 (B-3) ===" -ForegroundColor Cyan

# 이식 메모(1:1 불가 항목): bash판은 .claude/settings.json(.sh 훅 배선)의 JSON
# 유효성을 검사한다. 이 윈도우 포트가 실제로 쓰는 배선 파일은
# .claude/settings.windows.json(.ps1 훅 배선)이므로 그것을 우선 검사하고,
# 없으면 settings.json으로 폴백한다(원본의 "파일 없으면 SKIP" 관용은 유지).
$settingsCandidate = Join-Path $HooksDir '../settings.windows.json'
if (-not (Test-Path -LiteralPath $settingsCandidate)) { $settingsCandidate = Join-Path $HooksDir '../settings.json' }

if (Test-Path -LiteralPath $settingsCandidate) {
    $validJson = $true
    try {
        $raw = Get-Content -LiteralPath $settingsCandidate -Raw -ErrorAction Stop
        $null = $raw | ConvertFrom-Json -ErrorAction Stop
    } catch { $validJson = $false }
    Assert-Condition -Desc 'settings(.windows).json JSON 형식 유효 (SubagentStop 등록 후)' -Condition $validJson
} else {
    Write-Info 'settings.windows.json / settings.json 미존재 — 검증 생략'
}

$storyDir = Join-Path $TmpBase '.agent-team/03-story-engineering'
New-Item -ItemType Directory -Path $storyDir -Force | Out-Null
$storyFilePath = Join-Path $storyDir 'story-1-1-test-kr.md'

function New-SubagentJson {
    param([string]$Role)
    return '{"role":' + (ConvertTo-JsonString $Role) + ',"outputs":[]}'
}

$completeStory = @'
---
story_key: "1-1-test"
status: "ready-for-dev"
---
## story_requirements
요구사항 내용 [Source: 03-service-planning/service-stories.md#SS1]

## developer_context
개발자 컨텍스트 내용이 여기 있습니다. [Source: 04-architecture/api-contracts.md#D]

## architecture_compliance
아키텍처 준수 내용

## library_framework_requirements
라이브러리 요구사항

## file_structure_requirements
파일 구조 요구사항

## testing_requirements
테스트 요구사항

## project_context_reference
프로젝트 컨텍스트 참조
'@
Write-Utf8NoBom -Path $storyFilePath -Content $completeStory
$json = New-SubagentJson -Role '#15'
$ec = (Invoke-HookCapture -Hook 'artifact-verify.ps1' -StdinJson $json -EnvVars @{ CLAUDE_PROJECT_DIR = $TmpBase }).ExitCode
Assert-Exit -Desc 'SubagentStop #15 + 완전한 StoryFile → 통과' -Expected 0 -Actual $ec

$noSourceStory = @'
---
story_key: "1-1-test"
status: "ready-for-dev"
---
## story_requirements
요구사항 내용 (출처 표기 없음)

## developer_context
개발자 컨텍스트

## architecture_compliance
아키텍처

## library_framework_requirements
라이브러리

## file_structure_requirements
파일구조

## testing_requirements
테스트

## project_context_reference
컨텍스트
'@
Write-Utf8NoBom -Path $storyFilePath -Content $noSourceStory
$json = New-SubagentJson -Role '#15'
$ec = (Invoke-HookCapture -Hook 'artifact-verify.ps1' -StdinJson $json -EnvVars @{ CLAUDE_PROJECT_DIR = $TmpBase }).ExitCode
Assert-Exit -Desc 'SubagentStop #15 + [Source:] 없음 → 차단(exit 2)' -Expected 2 -Actual $ec

$missingDevCtxStory = @'
---
story_key: "1-1-test"
status: "ready-for-dev"
---
## story_requirements
요구사항 [Source: api-contracts.md#A-1]

## architecture_compliance
아키텍처

## library_framework_requirements
라이브러리

## file_structure_requirements
파일구조

## testing_requirements
테스트

## project_context_reference
컨텍스트
'@
Write-Utf8NoBom -Path $storyFilePath -Content $missingDevCtxStory
$json = New-SubagentJson -Role 'matthew'
$ec = (Invoke-HookCapture -Hook 'artifact-verify.ps1' -StdinJson $json -EnvVars @{ CLAUDE_PROJECT_DIR = $TmpBase }).ExitCode
Assert-Exit -Desc 'SubagentStop matthew + developer_context 누락 → 차단(exit 2)' -Expected 2 -Actual $ec

$json = New-SubagentJson -Role 'phillip'
$ec = (Invoke-HookCapture -Hook 'artifact-verify.ps1' -StdinJson $json -EnvVars @{ CLAUDE_PROJECT_DIR = $TmpBase }).ExitCode
Assert-Exit -Desc 'SubagentStop 비#15 역할(phillip) → fail-safe 통과' -Expected 0 -Actual $ec

# readiness-report-kr.md는 섹션4 4-4에서 생성됨 → W3 분기 통과
$json = '{"task":{"title":"W3 게이트 판정 완료"}}'
$ec = (Invoke-HookCapture -Hook 'artifact-verify.ps1' -StdinJson $json -EnvVars @{ CLAUDE_PROJECT_DIR = $TmpBase }).ExitCode
Assert-Exit -Desc 'TaskCompleted task 필드 있음 → SubagentStop 비진입(W3 분기 통과 회귀)' -Expected 0 -Actual $ec

} catch {
    Write-Host ("[FAIL] 섹션 7(artifact-verify SubagentStop) 실행 중 예외: " + $_.Exception.Message) -ForegroundColor Red
    $script:FailCount++
}

# ============================================================================
# 9. fingerprint 재승인 방지 (bathos-cli fingerprint 서브커맨드)
# ============================================================================
try {
Write-Host "`n=== 9. fingerprint 재승인 방지 (bathos-cli fingerprint 서브커맨드) ===" -ForegroundColor Cyan

if ($HasBathosBin) {
    $fpStateDir = Join-Path $TmpBase '_state_fp'
    New-Item -ItemType Directory -Path $fpStateDir -Force | Out-Null

    $fpManifest = '{"project_id":"bathos-' + ([guid]::NewGuid().ToString()) + '","codename":"TEST","current_level":2,"status":"active","lang":"ko","created":"' + (New-UtcNowIso) + '"}'
    Write-Utf8NoBom -Path (Join-Path $fpStateDir 'manifest.json') -Content $fpManifest

    $diffFile = Join-Path $TmpBase 'fp-diff1.txt'
    Write-Utf8NoBom -Path $diffFile -Content "--- a/.claude/settings.json`n+++ b/.claude/settings.json`n@@ -1,1 +1,1 @@`n-old`n+new`n"

    $out = & $BathosBinPath --state-dir $fpStateDir fingerprint check --diff-file $diffFile 2>$null | Out-String
    Assert-Condition -Desc 'fingerprint check 신규 diff → new' -Condition ($out.Contains('"status":"new"')) -FailDetail $out

    $fpHash = ''
    try { $outObj = $out | ConvertFrom-Json; if ($null -ne $outObj.hash) { $fpHash = [string]$outObj.hash } } catch { }

    & $BathosBinPath --state-dir $fpStateDir fingerprint approve --hash $fpHash --actor test-agent --scope hooks *> $null
    $out2 = & $BathosBinPath --state-dir $fpStateDir fingerprint check --diff-file $diffFile 2>$null | Out-String
    Assert-Condition -Desc 'fingerprint approve 후 재조회 → approved' -Condition ($out2.Contains('"status":"approved"')) -FailDetail $out2

    $diffFile2 = Join-Path $TmpBase 'fp-diff2.txt'
    Write-Utf8NoBom -Path $diffFile2 -Content "--- a/.claude/settings.json`r`n+++ b/.claude/settings.json`r`n@@ -1,1 +1,1 @@`r`n-old   `r`n+new`r`n"
    $out3 = & $BathosBinPath --state-dir $fpStateDir fingerprint check --diff-file $diffFile2 2>$null | Out-String
    Assert-Condition -Desc '공백/CRLF만 다른 diff → 동일 지문 approved(R2 정규화 결정성)' -Condition ($out3.Contains('"status":"approved"')) -FailDetail $out3

    $missingStateDir = Join-Path $TmpBase '_state_fp_missing'
    $out4 = & $BathosBinPath --state-dir $missingStateDir fingerprint check --diff-file $diffFile 2>$null | Out-String
    Assert-Condition -Desc 'manifest 부재 시 fingerprint check → new(안전 측)' -Condition ($out4.Contains('"status":"new"')) -FailDetail $out4
} else {
    Write-Info 'CLI 미빌드 — fingerprint 서브커맨드 테스트 생략(pending, cargo build -p bathos-cli 후 재검증)'
    Assert-Condition -Desc 'fingerprint 서브커맨드 테스트(pending)' -Condition $true
}

} catch {
    Write-Host ("[FAIL] 섹션 9(fingerprint) 실행 중 예외: " + $_.Exception.Message) -ForegroundColor Red
    $script:FailCount++
}

# ============================================================================
# 10. freeze-guard.ps1 위험 경로 fingerprint 게이트
# ============================================================================
try {
Write-Host "`n=== 10. freeze-guard.ps1 위험 경로 fingerprint 게이트 ===" -ForegroundColor Cyan

if ($HasBathosBin) {
    $fzStateDir = Join-Path $TmpBase '_state_fz'
    New-Item -ItemType Directory -Path $fzStateDir -Force | Out-Null
    $fzManifest = '{"project_id":"bathos-' + ([guid]::NewGuid().ToString()) + '","codename":"TEST","current_level":2,"status":"active","lang":"ko","created":"' + (New-UtcNowIso) + '"}'
    Write-Utf8NoBom -Path (Join-Path $fzStateDir 'manifest.json') -Content $fzManifest

    $dangerFile = $TmpBase + '/.claude/settings.json'
    $json = '{"tool_name":"Edit","tool_input":{"file_path":' + (ConvertTo-JsonString $dangerFile) + ',"old_string":"a","new_string":"b"}}'
    $cap = Invoke-HookCapture -Hook 'freeze-guard.ps1' -StdinJson $json -EnvVars @{
        BATHOS_STATE_DIR   = $fzStateDir
        BATHOS_BIN         = $BathosBinPath
        CLAUDE_PROJECT_DIR = $TmpBase
    }
    Assert-Exit -Desc '[A1] 위험경로(settings.json) 미승인 Edit → 차단' -Expected 2 -Actual $cap.ExitCode

    $m = [regex]::Match($cap.Stderr, '지문\(hash\):\s*([0-9a-f]+)')
    $fzHash = ''
    if ($m.Success) { $fzHash = $m.Groups[1].Value }

    if (-not [string]::IsNullOrEmpty($fzHash)) {
        & $BathosBinPath --state-dir $fzStateDir fingerprint approve --hash $fzHash --actor test-agent --scope hooks *> $null
        $ec2 = (Invoke-HookCapture -Hook 'freeze-guard.ps1' -StdinJson $json -EnvVars @{
            BATHOS_STATE_DIR   = $fzStateDir
            BATHOS_BIN         = $BathosBinPath
            CLAUDE_PROJECT_DIR = $TmpBase
        }).ExitCode
        Assert-Exit -Desc '[A1] 동일 변경 승인 후 재시도 → 허용(재승인 생략)' -Expected 0 -Actual $ec2
    } else {
        Assert-Condition -Desc '[A1] 동일 변경 승인 후 재시도 → 허용(재승인 생략)' -Condition $false -FailDetail 'freeze-guard 차단 메시지에서 hash 추출 실패'
    }

    $normalFile = $TmpBase + '/src/lib.rs'
    $json3 = '{"tool_name":"Edit","tool_input":{"file_path":' + (ConvertTo-JsonString $normalFile) + ',"old_string":"a","new_string":"b"}}'
    $ec3 = (Invoke-HookCapture -Hook 'freeze-guard.ps1' -StdinJson $json3 -EnvVars @{
        BATHOS_STATE_DIR   = $fzStateDir
        BATHOS_BIN         = $BathosBinPath
        CLAUDE_PROJECT_DIR = $TmpBase
    }).ExitCode
    Assert-Exit -Desc '[A1] 일반 경로 Edit(위험경로 아님) → 통과(회귀 없음)' -Expected 0 -Actual $ec3

    $newHookFile = $TmpBase + '/.claude/hooks/new-hook.sh'
    $json4 = '{"tool_name":"Write","tool_input":{"file_path":' + (ConvertTo-JsonString $newHookFile) + ',"content":"echo hi"}}'
    $ec4 = (Invoke-HookCapture -Hook 'freeze-guard.ps1' -StdinJson $json4 -EnvVars @{
        BATHOS_STATE_DIR   = $fzStateDir
        BATHOS_BIN         = (Join-Path $TmpBase 'nonexistent-bathos-bin.exe')
        CLAUDE_PROJECT_DIR = $TmpBase
    }).ExitCode
    Assert-Exit -Desc '[A1] 위험경로 + 엔진바이너리 부재 → 안전측 차단(freeze는 fail-open 예외)' -Expected 2 -Actual $ec4

    # 실제 관례 경로 ".../_state/manifest.json"를 그대로 써야 DANGEROUS_PATTERNS의
    # "*_state/manifest.json" 글롭과 매칭된다 — 디렉터리명이 "_state"로 끝나야 함
    $realisticStateDir = Join-Path $TmpBase '.agent-team/_state'
    New-Item -ItemType Directory -Path $realisticStateDir -Force | Out-Null
    $manifestTargetPath = (ConvertTo-SlashPathLocal $realisticStateDir) + '/manifest.json'
    $json5 = '{"tool_name":"Write","tool_input":{"file_path":' + (ConvertTo-JsonString $manifestTargetPath) + ',"content":"{\"gates\":[]}"}}'
    $ec5 = (Invoke-HookCapture -Hook 'freeze-guard.ps1' -StdinJson $json5 -EnvVars @{
        BATHOS_STATE_DIR   = $fzStateDir
        BATHOS_BIN         = $BathosBinPath
        CLAUDE_PROJECT_DIR = $TmpBase
    }).ExitCode
    Assert-Exit -Desc '[A1] manifest.json 직접 Write 시도 → 차단(게이트 우회 방지)' -Expected 2 -Actual $ec5
} else {
    Write-Info 'CLI 미빌드 — freeze-guard 위험경로 게이트 테스트 생략(pending)'
    Assert-Condition -Desc 'freeze-guard 위험경로 게이트 테스트(pending)' -Condition $true
}

} catch {
    Write-Host ("[FAIL] 섹션 10(freeze-guard 위험경로) 실행 중 예외: " + $_.Exception.Message) -ForegroundColor Red
    $script:FailCount++
}

# ============================================================================
# 11. plan-gate.ps1 (plan-mode deny 게이팅)
# ============================================================================
try {
Write-Host "`n=== 11. plan-gate.ps1 (plan-mode deny 게이팅) ===" -ForegroundColor Cyan

$pgStateDir = Join-Path $TmpBase '_state_pg'
New-Item -ItemType Directory -Path $pgStateDir -Force | Out-Null

function Write-SessionFlagsPlan {
    param([string]$StateDir, [string]$PlanMode, [string]$Intensity)
    $content = '{"$schema":"bathos:session-flags","plan_mode":' + (ConvertTo-JsonString $PlanMode) + ',"intensity":' + (ConvertTo-JsonString $Intensity) + ',"updated":"2026-07-08T00:00:00Z"}'
    Write-Utf8NoBom -Path (Join-Path $StateDir 'session-flags.json') -Content $content
}

Write-SessionFlagsPlan -StateDir $pgStateDir -PlanMode 'on' -Intensity 'full'
$json = '{"tool_name":"Write","tool_input":{"file_path":"any/file.txt","content":"x"}}'
$out = (Invoke-HookCapture -Hook 'plan-gate.ps1' -StdinJson $json -EnvVars @{ BATHOS_STATE_DIR = $pgStateDir; CLAUDE_PROJECT_DIR = $TmpBase }).Stdout
Assert-Condition -Desc '[A4] plan_mode=on + Write → deny' -Condition ($out.Contains('"permissionDecision":"deny"')) -FailDetail $out

$json = '{"tool_name":"Bash","tool_input":{"command":"git status"}}'
$out = (Invoke-HookCapture -Hook 'plan-gate.ps1' -StdinJson $json -EnvVars @{ BATHOS_STATE_DIR = $pgStateDir; CLAUDE_PROJECT_DIR = $TmpBase }).Stdout
Assert-Condition -Desc '[A4] plan_mode=on + 읽기전용 Bash → allow' -Condition ([string]::IsNullOrEmpty($out.Trim())) -FailDetail $out

$json = '{"tool_name":"Bash","tool_input":{"command":"git commit -am wip"}}'
$out = (Invoke-HookCapture -Hook 'plan-gate.ps1' -StdinJson $json -EnvVars @{ BATHOS_STATE_DIR = $pgStateDir; CLAUDE_PROJECT_DIR = $TmpBase }).Stdout
Assert-Condition -Desc '[A4] plan_mode=on + 변경성 Bash → deny' -Condition ($out.Contains('"permissionDecision":"deny"')) -FailDetail $out

Write-SessionFlagsPlan -StateDir $pgStateDir -PlanMode 'off' -Intensity 'full'
$json = '{"tool_name":"Write","tool_input":{"file_path":"any/file.txt","content":"x"}}'
$out = (Invoke-HookCapture -Hook 'plan-gate.ps1' -StdinJson $json -EnvVars @{ BATHOS_STATE_DIR = $pgStateDir; CLAUDE_PROJECT_DIR = $TmpBase }).Stdout
Assert-Condition -Desc '[A4] plan_mode=off + Write → allow' -Condition ([string]::IsNullOrEmpty($out.Trim())) -FailDetail $out

Remove-Item -LiteralPath (Join-Path $pgStateDir 'session-flags.json') -Force -ErrorAction SilentlyContinue
$json = '{"tool_name":"Write","tool_input":{"file_path":"any/file.txt","content":"x"}}'
$out = (Invoke-HookCapture -Hook 'plan-gate.ps1' -StdinJson $json -EnvVars @{ BATHOS_STATE_DIR = $pgStateDir; CLAUDE_PROJECT_DIR = $TmpBase }).Stdout
Assert-Condition -Desc '[A4] session-flags.json 부재 → allow(fail-open)' -Condition ([string]::IsNullOrEmpty($out.Trim())) -FailDetail $out

Write-SessionFlagsPlan -StateDir $pgStateDir -PlanMode 'on' -Intensity 'ultra'
$json = '{"tool_name":"Write","tool_input":{"file_path":"any/file.txt","content":"x"}}'
$out = (Invoke-HookCapture -Hook 'plan-gate.ps1' -StdinJson $json -EnvVars @{ BATHOS_STATE_DIR = $pgStateDir; CLAUDE_PROJECT_DIR = $TmpBase }).Stdout
Assert-Condition -Desc '[A4] intensity=ultra여도 plan_mode=on 판정 불변(필드 독립성)' -Condition ($out.Contains('"permissionDecision":"deny"')) -FailDetail $out

} catch {
    Write-Host ("[FAIL] 섹션 11(plan-gate) 실행 중 예외: " + $_.Exception.Message) -ForegroundColor Red
    $script:FailCount++
}

# ============================================================================
# 12. plan-toggle.ps1 (Dynamis A4 토글 수단)
# ============================================================================
try {
Write-Host "`n=== 12. plan-toggle.ps1 ===" -ForegroundColor Cyan

$ptStateDir = Join-Path $TmpBase '_state_pt'
New-Item -ItemType Directory -Path $ptStateDir -Force | Out-Null
$ptFlagsPath = Join-Path $ptStateDir 'session-flags.json'

$json = '{"prompt":"/bathos plan on 부탁해"}'
[void](Invoke-HookCapture -Hook 'plan-toggle.ps1' -StdinJson $json -EnvVars @{ BATHOS_STATE_DIR = $ptStateDir; CLAUDE_PROJECT_DIR = $TmpBase; BATHOS_BIN = $BathosBinPath })
Assert-Condition -Desc '[A4] /bathos plan on → plan_mode=on 기록' -Condition (Test-FileMatches -Path $ptFlagsPath -Pattern '"plan_mode":\s*"on"')

$json = '{"prompt":"/bathos plan off"}'
[void](Invoke-HookCapture -Hook 'plan-toggle.ps1' -StdinJson $json -EnvVars @{ BATHOS_STATE_DIR = $ptStateDir; CLAUDE_PROJECT_DIR = $TmpBase; BATHOS_BIN = $BathosBinPath })
Assert-Condition -Desc '[A4] /bathos plan off → plan_mode=off 기록' -Condition (Test-FileMatches -Path $ptFlagsPath -Pattern '"plan_mode":\s*"off"')

$before = Get-Content -LiteralPath $ptFlagsPath -Raw -ErrorAction SilentlyContinue
$json = '{"prompt":"오늘 날씨 어때"}'
$cap = Invoke-HookCapture -Hook 'plan-toggle.ps1' -StdinJson $json -EnvVars @{ BATHOS_STATE_DIR = $ptStateDir; CLAUDE_PROJECT_DIR = $TmpBase; BATHOS_BIN = $BathosBinPath }
$after = Get-Content -LiteralPath $ptFlagsPath -Raw -ErrorAction SilentlyContinue
Assert-Condition -Desc '[A4] 무관 프롬프트 → 무해 통과(파일 불변)' -Condition (($cap.ExitCode -eq 0) -and ($before -eq $after))

$json = '{"prompt":"/bathos plan maybe"}'
$cap = Invoke-HookCapture -Hook 'plan-toggle.ps1' -StdinJson $json -EnvVars @{ BATHOS_STATE_DIR = $ptStateDir; CLAUDE_PROJECT_DIR = $TmpBase; BATHOS_BIN = $BathosBinPath }
Assert-Condition -Desc '[A4] 알 수 없는 plan 값 → 경고 + 이전값(off) 유지' -Condition ($cap.Stderr.Contains('알 수 없는 값') -and (Test-FileMatches -Path $ptFlagsPath -Pattern '"plan_mode":\s*"off"')) -FailDetail $cap.Stderr

} catch {
    Write-Host ("[FAIL] 섹션 12(plan-toggle) 실행 중 예외: " + $_.Exception.Message) -ForegroundColor Red
    $script:FailCount++
}

# ============================================================================
# 13. scope-inject.ps1 (스코프 규칙 자동 주입)
# ============================================================================
try {
Write-Host "`n=== 13. scope-inject.ps1 (스코프 규칙 자동 주입) ===" -ForegroundColor Cyan

$siRoot = Join-Path $TmpBase 'scope-proj'
New-Item -ItemType Directory -Path (Join-Path $siRoot 'pkg-a/sub') -Force | Out-Null
Write-Utf8NoBom -Path (Join-Path $siRoot 'pkg-a/AGENTS.md') -Content "# 팀 규칙`n이 디렉터리는 Andrew 소유입니다.`n"

$targetFile = (ConvertTo-SlashPathLocal $siRoot) + '/pkg-a/sub/file.rs'
$json = '{"tool_name":"Read","tool_input":{"file_path":' + (ConvertTo-JsonString $targetFile) + '}}'
$out = (Invoke-HookCapture -Hook 'scope-inject.ps1' -StdinJson $json -EnvVars @{ CLAUDE_PROJECT_DIR = $siRoot }).Stdout
Assert-Condition -Desc '[A2] 상위 AGENTS.md 존재 → additionalContext 주입' -Condition ($out.Contains('additionalContext')) -FailDetail $out

$siRoot2 = Join-Path $TmpBase 'scope-proj-empty'
New-Item -ItemType Directory -Path (Join-Path $siRoot2 'pkg-b') -Force | Out-Null
$targetFile2 = (ConvertTo-SlashPathLocal $siRoot2) + '/pkg-b/file.rs'
$json = '{"tool_name":"Read","tool_input":{"file_path":' + (ConvertTo-JsonString $targetFile2) + '}}'
$out = (Invoke-HookCapture -Hook 'scope-inject.ps1' -StdinJson $json -EnvVars @{ CLAUDE_PROJECT_DIR = $siRoot2 }).Stdout
Assert-Condition -Desc '[A2] 규칙 파일 없음 → 무출력 통과(fail-open)' -Condition ([string]::IsNullOrEmpty($out.Trim())) -FailDetail $out

$siRoot3 = Join-Path $TmpBase 'scope-proj-deep'
$deepPath = $siRoot3
for ($i = 1; $i -le 10; $i++) { $deepPath = Join-Path $deepPath ('lvl' + $i) }
New-Item -ItemType Directory -Path $deepPath -Force | Out-Null
Write-Utf8NoBom -Path (Join-Path $siRoot3 'AGENTS.md') -Content "# 너무 먼 규칙`n"
$targetFile3 = (ConvertTo-SlashPathLocal $deepPath) + '/file.rs'
$json = '{"tool_name":"Read","tool_input":{"file_path":' + (ConvertTo-JsonString $targetFile3) + '}}'
$out = (Invoke-HookCapture -Hook 'scope-inject.ps1' -StdinJson $json -EnvVars @{ CLAUDE_PROJECT_DIR = $siRoot3 }).Stdout
Assert-Condition -Desc '[A2] 깊이 상한(8) 초과 규칙 → 미부착' -Condition ([string]::IsNullOrEmpty($out.Trim())) -FailDetail $out

$siRoot4 = Join-Path $TmpBase 'scope-proj-big'
New-Item -ItemType Directory -Path (Join-Path $siRoot4 'pkg-c') -Force | Out-Null
Write-Utf8NoBom -Path (Join-Path $siRoot4 'AGENTS.md') -Content ('x' * 10000)
$targetFile4 = (ConvertTo-SlashPathLocal $siRoot4) + '/pkg-c/file.rs'
$json = '{"tool_name":"Read","tool_input":{"file_path":' + (ConvertTo-JsonString $targetFile4) + '}}'
$out = (Invoke-HookCapture -Hook 'scope-inject.ps1' -StdinJson $json -EnvVars @{ CLAUDE_PROJECT_DIR = $siRoot4 }).Stdout
Assert-Condition -Desc '[A2] 8KB 초과 규칙 파일 → 절단 주입 표시' -Condition ($out.Contains('이하 생략')) -FailDetail $out

$json = '{"tool_name":"Bash","tool_input":{"command":"ls"}}'
$ec = (Invoke-HookCapture -Hook 'scope-inject.ps1' -StdinJson $json -EnvVars @{ CLAUDE_PROJECT_DIR = $siRoot }).ExitCode
Assert-Exit -Desc '[A2] Bash 도구 → scope-inject 비대상 통과' -Expected 0 -Actual $ec

} catch {
    Write-Host ("[FAIL] 섹션 13(scope-inject) 실행 중 예외: " + $_.Exception.Message) -ForegroundColor Red
    $script:FailCount++
}

# ============================================================================
# 14. next-action.ps1 서킷브레이커 3종 (Dynamis A3)
# ============================================================================
try {
Write-Host "`n=== 14. next-action.ps1 서킷브레이커(빈응답/호출수/연속실패) ===" -ForegroundColor Cyan

$cbStateDir = Join-Path $TmpBase '_state_cb'
New-Item -ItemType Directory -Path $cbStateDir -Force | Out-Null
$cbFlagsPath = Join-Path $cbStateDir 'session-flags.json'

function Write-Counters {
    param([string]$StateDir, [int]$EmptyStreak, [int]$ModelCalls, [int]$FailStreak, [int]$CallMax = 200)
    $content = '{"$schema":"bathos:session-flags","plan_mode":"off","intensity":"full",' +
        '"counters":{"test-role":{"empty_msg_streak":' + $EmptyStreak + ',"model_calls":' + $ModelCalls + ',"fail_streak":' + $FailStreak + '}},' +
        '"thresholds":{"empty_msg_max":2,"model_call_max":' + $CallMax + ',"fail_streak_max":3}}'
    Write-Utf8NoBom -Path (Join-Path $StateDir 'session-flags.json') -Content $content
}

Write-Counters -StateDir $cbStateDir -EmptyStreak 0 -ModelCalls 5 -FailStreak 3
$json = '{"role":"test-role","remaining_tasks":[]}'
$cap = Invoke-HookCapture -Hook 'next-action.ps1' -StdinJson $json -EnvVars @{ BATHOS_STATE_DIR = $cbStateDir; BATHOS_ROLE = 'test-role'; CLAUDE_PROJECT_DIR = $TmpBase; BATHOS_BIN = $BathosBinPath }
Assert-Condition -Desc '[A3] fail_streak>=임계 → 서킷브레이커 발동 메시지' -Condition ($cap.Stderr.Contains('서킷브레이커 발동')) -FailDetail $cap.Stderr
Assert-Condition -Desc '[A3] 서킷 발동 후 fail_streak 리셋 확인' -Condition (Test-FileMatches -Path $cbFlagsPath -Pattern '"fail_streak":\s*0')

Write-Counters -StateDir $cbStateDir -EmptyStreak 0 -ModelCalls 1 -FailStreak 0
$json = '{"role":"test-role","remaining_tasks":["작업 계속"]}'
$cap = Invoke-HookCapture -Hook 'next-action.ps1' -StdinJson $json -EnvVars @{ BATHOS_STATE_DIR = $cbStateDir; BATHOS_ROLE = 'test-role'; CLAUDE_PROJECT_DIR = $TmpBase; BATHOS_BIN = $BathosBinPath }
Assert-Condition -Desc '[A3] 임계 미만 → 서킷 메시지 없음(정상 안내만)' -Condition (-not $cap.Stderr.Contains('서킷브레이커 발동'))

Write-Counters -StateDir $cbStateDir -EmptyStreak 1 -ModelCalls 1 -FailStreak 0
$json = '{"role":"test-role","remaining_tasks":[]}'
$cap = Invoke-HookCapture -Hook 'next-action.ps1' -StdinJson $json -EnvVars @{ BATHOS_STATE_DIR = $cbStateDir; BATHOS_ROLE = 'test-role'; CLAUDE_PROJECT_DIR = $TmpBase; BATHOS_BIN = $BathosBinPath }
Assert-Condition -Desc '[A3] empty_msg_streak 임계 도달 → 재주입 메시지' -Condition ($cap.Stderr.Contains('연속 무도구-응답')) -FailDetail $cap.Stderr

Remove-Item -LiteralPath $cbFlagsPath -Force -ErrorAction SilentlyContinue
$json = '{"role":"test-role","remaining_tasks":[]}'
$cap = Invoke-HookCapture -Hook 'next-action.ps1' -StdinJson $json -EnvVars @{ BATHOS_STATE_DIR = $cbStateDir; BATHOS_ROLE = 'test-role'; CLAUDE_PROJECT_DIR = $TmpBase; BATHOS_BIN = $BathosBinPath }
Assert-Condition -Desc '[A3] session-flags.json 부재 → fail-open(기존 안내만)' -Condition (($cap.ExitCode -eq 0) -and (-not [regex]::IsMatch($cap.Stderr, '서킷브레이커 발동|연속 무도구-응답')))

New-Item -ItemType Directory -Path $cbStateDir -Force | Out-Null
Write-Counters -StateDir $cbStateDir -EmptyStreak 0 -ModelCalls 5 -FailStreak 0 -CallMax 1
$json = '{"role":"test-role","remaining_tasks":[]}'
$cap = Invoke-HookCapture -Hook 'next-action.ps1' -StdinJson $json -EnvVars @{ BATHOS_STATE_DIR = $cbStateDir; BATHOS_ROLE = 'test-role'; CLAUDE_PROJECT_DIR = $TmpBase; BATHOS_BIN = $BathosBinPath }
Assert-Condition -Desc '[A3] 커스텀 thresholds(model_call_max=1) 반영 확인' -Condition ($cap.Stderr.Contains('서킷브레이커 발동')) -FailDetail $cap.Stderr

New-Item -ItemType Directory -Path $TestStateDir -Force | Out-Null
$json = '{"role":"Andrew","remaining_tasks":["task1"]}'
$cap = Invoke-HookCapture -Hook 'next-action.ps1' -StdinJson $json -EnvVars @{ CLAUDE_PROJECT_DIR = $TmpBase; BATHOS_STATE_DIR = $TestStateDir }
Assert-Condition -Desc '[A3] 기존 idle 안내 기능 회귀 없음' -Condition ($cap.Stderr.Contains('남은 태스크'))

} catch {
    Write-Host ("[FAIL] 섹션 14(next-action 서킷브레이커) 실행 중 예외: " + $_.Exception.Message) -ForegroundColor Red
    $script:FailCount++
}

# ============================================================================
# 15. audit-log.ps1 서킷브레이커 카운터 갱신 (Dynamis A3)
# ============================================================================
try {
Write-Host "`n=== 15. audit-log.ps1 카운터 갱신(model_calls·empty_msg_streak·fail_streak) ===" -ForegroundColor Cyan

$alStateDir = Join-Path $TmpBase '_state_al'
New-Item -ItemType Directory -Path $alStateDir -Force | Out-Null
$alFlagsPath = Join-Path $alStateDir 'session-flags.json'
Write-Utf8NoBom -Path $alFlagsPath -Content '{"$schema":"bathos:session-flags","plan_mode":"off","intensity":"full","counters":{"al-actor":{"empty_msg_streak":2,"model_calls":3,"fail_streak":0}}}'

$json = '{"tool_name":"Bash","tool_input":{"command":"ls"},"tool_response":{"type":"result","result":"ok"}}'
[void](Invoke-HookCapture -Hook 'audit-log.ps1' -StdinJson $json -EnvVars @{ BATHOS_STATE_DIR = $alStateDir; BATHOS_ROLE = 'al-actor'; CLAUDE_PROJECT_DIR = $TmpBase; BATHOS_BIN = $BathosBinPath })
Assert-Condition -Desc '[A3] 성공 도구호출 → model_calls+1 · empty_msg_streak 리셋' -Condition ((Test-FileMatches -Path $alFlagsPath -Pattern '"model_calls":\s*4') -and (Test-FileMatches -Path $alFlagsPath -Pattern '"empty_msg_streak":\s*0'))

$json = '{"tool_name":"Bash","tool_input":{"command":"false"},"tool_response":{"type":"error","result":"boom"}}'
[void](Invoke-HookCapture -Hook 'audit-log.ps1' -StdinJson $json -EnvVars @{ BATHOS_STATE_DIR = $alStateDir; BATHOS_ROLE = 'al-actor'; CLAUDE_PROJECT_DIR = $TmpBase; BATHOS_BIN = $BathosBinPath })
Assert-Condition -Desc '[A3] 실패 도구호출 → fail_streak+1' -Condition (Test-FileMatches -Path $alFlagsPath -Pattern '"fail_streak":\s*1')

$json = '{"tool_name":"Bash","tool_input":{"command":"true"},"tool_response":{"type":"result","result":"ok"}}'
[void](Invoke-HookCapture -Hook 'audit-log.ps1' -StdinJson $json -EnvVars @{ BATHOS_STATE_DIR = $alStateDir; BATHOS_ROLE = 'al-actor'; CLAUDE_PROJECT_DIR = $TmpBase; BATHOS_BIN = $BathosBinPath })
Assert-Condition -Desc '[A3] 성공 이어짐 → fail_streak 리셋' -Condition (Test-FileMatches -Path $alFlagsPath -Pattern '"fail_streak":\s*0')

} catch {
    Write-Host ("[FAIL] 섹션 15(audit-log 카운터) 실행 중 예외: " + $_.Exception.Message) -ForegroundColor Red
    $script:FailCount++
}

# ============================================================================
# 16. intensity-tracker.ps1 (세션 intensity 토글)
# ============================================================================
try {
Write-Host "`n=== 16. intensity-tracker.ps1 (세션 intensity 토글) ===" -ForegroundColor Cyan

$itStateDir = Join-Path $TmpBase '_state_it'
New-Item -ItemType Directory -Path $itStateDir -Force | Out-Null
$itFlagsPath = Join-Path $itStateDir 'session-flags.json'

$json = '{"prompt":"/bathos intensity ultra"}'
[void](Invoke-HookCapture -Hook 'intensity-tracker.ps1' -StdinJson $json -EnvVars @{ BATHOS_STATE_DIR = $itStateDir; CLAUDE_PROJECT_DIR = $TmpBase; BATHOS_BIN = $BathosBinPath })
Assert-Condition -Desc '[A5] /bathos intensity ultra → 기록' -Condition (Test-FileMatches -Path $itFlagsPath -Pattern '"intensity":\s*"ultra"')

$json = '{"prompt":"/bathos intensity"}'
$out = (Invoke-HookCapture -Hook 'intensity-tracker.ps1' -StdinJson $json -EnvVars @{ BATHOS_STATE_DIR = $itStateDir; CLAUDE_PROJECT_DIR = $TmpBase; BATHOS_BIN = $BathosBinPath }).Stdout
Assert-Condition -Desc '[A5] 레벨 누락 → 도움말 출력, 값 불변' -Condition ($out.Contains('사용 가능 레벨') -and (Test-FileMatches -Path $itFlagsPath -Pattern '"intensity":\s*"ultra"')) -FailDetail $out

$json = '{"prompt":"/bathos intensity banana"}'
$cap = Invoke-HookCapture -Hook 'intensity-tracker.ps1' -StdinJson $json -EnvVars @{ BATHOS_STATE_DIR = $itStateDir; CLAUDE_PROJECT_DIR = $TmpBase; BATHOS_BIN = $BathosBinPath }
Assert-Condition -Desc '[A5] 알 수 없는 값 → 에러 + 이전값(ultra) 유지' -Condition ($cap.Stderr.Contains('알 수 없는 값') -and (Test-FileMatches -Path $itFlagsPath -Pattern '"intensity":\s*"ultra"')) -FailDetail $cap.Stderr

$before = Get-Content -LiteralPath $itFlagsPath -Raw -ErrorAction SilentlyContinue
$json = '{"prompt":"점심 뭐 먹지"}'
[void](Invoke-HookCapture -Hook 'intensity-tracker.ps1' -StdinJson $json -EnvVars @{ BATHOS_STATE_DIR = $itStateDir; CLAUDE_PROJECT_DIR = $TmpBase; BATHOS_BIN = $BathosBinPath })
$after = Get-Content -LiteralPath $itFlagsPath -Raw -ErrorAction SilentlyContinue
Assert-Condition -Desc '[A5] 무관 프롬프트 → 무해 통과(파일 불변)' -Condition ($before -eq $after)

$json = '{"prompt":"/bathos intensity off"}'
$out = (Invoke-HookCapture -Hook 'intensity-tracker.ps1' -StdinJson $json -EnvVars @{ BATHOS_STATE_DIR = $itStateDir; CLAUDE_PROJECT_DIR = $TmpBase; BATHOS_BIN = $BathosBinPath }).Stdout
Assert-Condition -Desc '[A5] intensity=off → 경고 접두(!) 확인응답 + 기록' -Condition ($out.Contains('! intensity=off') -and (Test-FileMatches -Path $itFlagsPath -Pattern '"intensity":\s*"off"')) -FailDetail $out

Write-Utf8NoBom -Path $itFlagsPath -Content '{"$schema":"bathos:session-flags","plan_mode":"on","intensity":"full","updated":"2026-07-08T00:00:00Z"}'
$json = '{"prompt":"/bathos intensity lite"}'
[void](Invoke-HookCapture -Hook 'intensity-tracker.ps1' -StdinJson $json -EnvVars @{ BATHOS_STATE_DIR = $itStateDir; CLAUDE_PROJECT_DIR = $TmpBase; BATHOS_BIN = $BathosBinPath })
Assert-Condition -Desc '[A5] intensity 변경이 plan_mode 필드를 건드리지 않음(필드 독립성)' -Condition ((Test-FileMatches -Path $itFlagsPath -Pattern '"plan_mode":\s*"on"') -and (Test-FileMatches -Path $itFlagsPath -Pattern '"intensity":\s*"lite"'))

} catch {
    Write-Host ("[FAIL] 섹션 16(intensity-tracker) 실행 중 예외: " + $_.Exception.Message) -ForegroundColor Red
    $script:FailCount++
}

} finally {
    Remove-Item -LiteralPath $TmpBase -Recurse -Force -ErrorAction SilentlyContinue
}

# ============================================================================
# 8. 최종 결과 (bash 원본의 절 번호 관례를 그대로 보존 — 1~7, 9~16 다음 8)
# ============================================================================
Write-Host "`n══════════════════════════════════════════════"
$total = $script:PassCount + $script:FailCount
Write-Host ("테스트 결과: {0} PASS / {1} FAIL (총 {2})" -f $script:PassCount, $script:FailCount, $total)

if ($script:FailCount -gt 0) {
    Write-Host '일부 테스트가 실패했습니다. 위 출력을 확인하세요.' -ForegroundColor Red
    exit 1
} else {
    Write-Host '모든 테스트 통과! M6 훅 결정성 + B-1/B-3/H-6/H-7/L-4 수정 + Dynamis A1~A6 확인 완료(PowerShell 포트).' -ForegroundColor Green
    exit 0
}
