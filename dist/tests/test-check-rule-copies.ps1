<#
=============================================================================
BATHOS Dynamis — dist/tests/test-check-rule-copies.ps1  (Windows PowerShell 포트)
원본: dist/tests/test-check-rule-copies.sh
Story B3 §5 픽스처 4종(자기검증) — -CheckCopies 모드의 세 분기(byte/invariant/
제외 목록)를 전부 격리된 임시 디렉터리에서 검증한다. 실제
scripts/drift-exclusions.json·dist/copies-manifest.json은 건드리지 않는다
(BathosRoot/BathosCopiesManifest/BathosDriftExclusions 매개변수로 격리).

  ① 일부러 드리프트시킨 복제본(byte 모드) -> 실패
  ② 정합 상태(byte 모드) -> 통과
  ③ invariant 문구 누락 -> 실패
  ④ 제외 목록 항목의 드리프트 -> 통과(제외 동작 확인)

타깃: Windows PowerShell 5.1 하한 + PowerShell 7+ 호환(삼항/`??` 미사용).
=============================================================================
#>

Set-StrictMode -Version 2.0
$ErrorActionPreference = 'Stop'

$ScriptDir = $PSScriptRoot
$CheckScript = (Resolve-Path (Join-Path $ScriptDir '..\..\scripts\check-rule-copies.ps1')).Path

# 검사 대상 스크립트는 실패 시 exit 1을 호출한다 — 같은 프로세스에서 직접
# 호출(dot-source/call operator)하면 이 테스트 러너 자체가 종료돼버리므로,
# bash판(`bash "$CHECK_SCRIPT"` = 서브프로세스)과 동등하게 자식 프로세스로
# 실행한다. 현재 실행 중인 PowerShell 실행파일(pwsh.exe 또는 powershell.exe)을
# 그대로 재사용해 5.1/7+ 어느 쪽에서 테스트를 돌리든 일관되게 동작시킨다.
$PwshExe = [System.Diagnostics.Process]::GetCurrentProcess().MainModule.FileName

$Script:Fail = $false
function Pass2 { param([string] $m) Write-Host "[PASS] $m" }
function Fail2 { param([string] $m) Write-Host "[FAIL] $m"; $Script:Fail = $true }

function Invoke-Check {
    param([string] $Root)
    $out = & $PwshExe -NoProfile -NoLogo -NonInteractive -File $CheckScript `
        -CheckCopies `
        -BathosRoot $Root `
        -BathosCopiesManifest (Join-Path $Root 'dist\copies-manifest.json') `
        -BathosDriftExclusions (Join-Path $Root 'drift-exclusions.json') 2>&1
    return [PSCustomObject]@{ Output = ($out | Out-String); ExitCode = $LASTEXITCODE }
}

function New-FixtureRoot {
    $root = Join-Path ([System.IO.Path]::GetTempPath()) ('bathos-crc-' + [System.Guid]::NewGuid().ToString('N'))
    New-Item -ItemType Directory -Force -Path (Join-Path $root 'dist') | Out-Null
    New-Item -ItemType Directory -Force -Path (Join-Path $root 'canon') | Out-Null
    New-Item -ItemType Directory -Force -Path (Join-Path $root 'copy') | Out-Null
    return $root
}

function Set-Utf8NoBom {
    param([string] $Path, [string] $Content)
    $enc = New-Object System.Text.UTF8Encoding($false)
    [System.IO.File]::WriteAllText($Path, $Content, $enc)
}

# --- (1) byte 모드 드리프트 -> 실패 -------------------------------------------
$root = New-FixtureRoot
Set-Utf8NoBom (Join-Path $root 'canon\rule.md') "canonical content line one`n"
Set-Utf8NoBom (Join-Path $root 'copy\rule.md') "DRIFTED content line one`n"
Set-Utf8NoBom (Join-Path $root 'dist\copies-manifest.json') '{"copies":[{"source":"canon/rule.md","copy":"copy/rule.md","mode":"byte"}]}'
Set-Utf8NoBom (Join-Path $root 'drift-exclusions.json') '{"exclusions":[]}'
$r1 = Invoke-Check $root
if ($r1.ExitCode -eq 0) {
    Fail2 "(1) byte 드리프트인데 통과함: $($r1.Output)"
} else {
    Pass2 '(1) byte 드리프트 검출 -> 실패(exit!=0)'
}
Remove-Item -LiteralPath $root -Recurse -Force -ErrorAction SilentlyContinue

# --- (2) byte 모드 정합 -> 통과 -----------------------------------------------
$root = New-FixtureRoot
Set-Utf8NoBom (Join-Path $root 'canon\rule.md') "canonical content line one`n"
Copy-Item -LiteralPath (Join-Path $root 'canon\rule.md') -Destination (Join-Path $root 'copy\rule.md') -Force
Set-Utf8NoBom (Join-Path $root 'dist\copies-manifest.json') '{"copies":[{"source":"canon/rule.md","copy":"copy/rule.md","mode":"byte"}]}'
Set-Utf8NoBom (Join-Path $root 'drift-exclusions.json') '{"exclusions":[]}'
$r2 = Invoke-Check $root
if ($r2.ExitCode -eq 0) {
    Pass2 '(2) byte 정합 -> 통과'
} else {
    Fail2 "(2) byte 정합인데 실패함: $($r2.Output)"
}
Remove-Item -LiteralPath $root -Recurse -Force -ErrorAction SilentlyContinue

# --- (3) invariant 문구 누락 -> 실패 ------------------------------------------
$root = New-FixtureRoot
Set-Utf8NoBom (Join-Path $root 'copy\skill.md') "some skill body without the required phrase`n"
Set-Utf8NoBom (Join-Path $root 'dist\copies-manifest.json') '{"copies":[{"source":"copy/skill.md","copy":"copy/skill.md","mode":"invariant","invariant":["## Boundaries"]}]}'
Set-Utf8NoBom (Join-Path $root 'drift-exclusions.json') '{"exclusions":[]}'
$r3 = Invoke-Check $root
if ($r3.ExitCode -eq 0) {
    Fail2 "(3) invariant 누락인데 통과함: $($r3.Output)"
} else {
    Pass2 '(3) invariant 문구 누락 검출 -> 실패'
}
Remove-Item -LiteralPath $root -Recurse -Force -ErrorAction SilentlyContinue

# --- (4) 제외 목록 항목의 드리프트 -> 통과(제외 동작) --------------------------
$root = New-FixtureRoot
Set-Utf8NoBom (Join-Path $root 'canon\rule.md') "canonical content line one`n"
Set-Utf8NoBom (Join-Path $root 'copy\rule-es.md') "DRIFTED but excluded`n"
Set-Utf8NoBom (Join-Path $root 'dist\copies-manifest.json') '{"copies":[{"source":"canon/rule.md","copy":"copy/rule-es.md","mode":"byte"}]}'
Set-Utf8NoBom (Join-Path $root 'drift-exclusions.json') '{"exclusions":[{"path":"copy/rule-es.md","reason":"테스트용 i18n 예외"}]}'
$r4 = Invoke-Check $root
if ($r4.ExitCode -eq 0) {
    Pass2 '(4) 제외 목록 항목은 드리프트해도 통과(제외 동작 확인)'
} else {
    Fail2 "(4) 제외 목록이 동작하지 않음: $($r4.Output)"
}
Remove-Item -LiteralPath $root -Recurse -Force -ErrorAction SilentlyContinue

if (-not $Script:Fail) {
    Write-Host '[bathos test-check-rule-copies] OK 전체 통과'
    exit 0
} else {
    Write-Host '[bathos test-check-rule-copies] FAIL 실패 항목 존재'
    exit 1
}
