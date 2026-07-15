<#
=============================================================================
BATHOS Dynamis — dist/tests/test-host-detect.ps1  (Windows PowerShell 포트)
원본: dist/tests/test-host-detect.sh
Story B5 §5 스모크 테스트: 기본값=claude, 미지원 호스트=NotImplemented(exit 2).

Pester 등 외부 모듈 의존 없이 단순 assert 방식으로 작성한다(요청 관례 준수).
타깃: Windows PowerShell 5.1 하한 + PowerShell 7+ 호환(삼항/`??` 미사용).
=============================================================================
#>

Set-StrictMode -Version 2.0
$ErrorActionPreference = 'Stop'

$ScriptDir = $PSScriptRoot
. (Join-Path $ScriptDir '..\lib\host-detect.ps1')

$Script:Fail = $false
function Assert-Eq {
    param([string] $Desc, [string] $Expected, [string] $Actual)
    if ($Expected -eq $Actual) {
        Write-Host "[PASS] $Desc"
    } else {
        Write-Host "[FAIL] $Desc - 기대=$Expected 실제=$Actual"
        $Script:Fail = $true
    }
}

# 1) 기본값(환경변수 없음) -> claude
$env:PLUGIN_DATA = $null
$env:COPILOT_PLUGIN_DATA = $null
$env:BATHOS_FORCE_HOST = $null
Remove-Item Env:\PLUGIN_DATA -ErrorAction SilentlyContinue
Remove-Item Env:\COPILOT_PLUGIN_DATA -ErrorAction SilentlyContinue
Remove-Item Env:\BATHOS_FORCE_HOST -ErrorAction SilentlyContinue

$got = Get-BathosHost
Assert-Eq '기본 호스트 판별' 'claude' $got

# 2) claude 경로는 payload를 그대로 통과
$result = Write-BathosHookOutput -HostName 'claude' -Payload '{"decision":"allow"}'
Assert-Eq 'claude 출력 통과(exit)' '0' ([string]$result.ExitCode)
Assert-Eq 'claude 출력 내용' '{"decision":"allow"}' ([string]$result.Output)

# 3) 미지원 호스트(codex) -> NotImplemented(exit 2), stdout(Output)은 비어야 함(오출력 금지)
$env:BATHOS_FORCE_HOST = 'codex'
$prevErr = [Console]::Error
$sw = New-Object System.IO.StringWriter
[Console]::SetError($sw)
try {
    $result2 = Write-BathosHookOutput -HostName 'codex' -Payload '{"decision":"allow"}'
} finally {
    [Console]::SetError($prevErr)
}
Assert-Eq 'codex 미구현 exit code' '2' ([string]$result2.ExitCode)
$out2 = $result2.Output
if ($null -eq $out2) { $out2 = '' }
Assert-Eq 'codex 미구현 stdout 비어있음(오출력 금지)' '' $out2
Remove-Item Env:\BATHOS_FORCE_HOST -ErrorAction SilentlyContinue

# 4) BATHOS_FORCE_HOST 오버라이드 동작 확인
$env:BATHOS_FORCE_HOST = 'copilot'
$got2 = Get-BathosHost
Assert-Eq '강제 오버라이드(copilot)' 'copilot' $got2
Remove-Item Env:\BATHOS_FORCE_HOST -ErrorAction SilentlyContinue

if (-not $Script:Fail) {
    Write-Host '[bathos test-host-detect] OK 전체 통과'
    exit 0
} else {
    Write-Host '[bathos test-host-detect] FAIL 실패 항목 존재'
    exit 1
}
