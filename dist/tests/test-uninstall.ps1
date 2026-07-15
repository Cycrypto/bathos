<#
=============================================================================
BATHOS Dynamis — dist/tests/test-uninstall.ps1  (Windows PowerShell 포트)
원본: dist/tests/test-uninstall.sh
Story B4 §5 "dry-run 모드가 곧 테스트 하니스다" — 픽스처 4종.
  ① 픽스처 외부 상태 생성 -> dry-run 출력이 전 대상을 나열
  ② -Yes 실행 -> 실제 제거 + [removed] 로그
  ③ 대상 없음 -> 빈 상태 안내
  ④ 쓰기 불가 파일 1개 -> 부분 실패 분리 표기

실제 $HOME/_state를 절대 건드리지 않는다 — 전부 격리된 임시 디렉터리로
BATHOS_ROOT/BATHOS_STATE_DIR/BATHOS_HOME/BATHOS_CONFIG_DIR 환경변수를
오버라이드한다(scripts/uninstall.ps1은 매개변수가 비어 있으면 이 환경변수를
읽는다 — bash판과 동일한 오버라이드 관례).

이식 메모(④): bash판은 대상 디렉터리 자체를 chmod 555로 잠가 두 번째 항목
(설정 statusLine 편집, 특히 .bak 백업 생성)을 실패시킨다. Windows NTFS에서는
디렉터리 읽기전용 속성이 쓰기를 막지 않고, ACL 조작(icacls/Set-Acl)은
테스트 환경(권한/소유자)에 따라 불안정하다. 대신 대상 파일 자체를
IsReadOnly=$true 로 표시해 "백업은 성공하지만 원본 파일 교체 시 쓰기가
실패"하는 동일 범주의 부분 실패(분리 표기)를 유도한다 — 실패 지점은
다르지만 "부분 실패가 성공/실패로 분리 표기되는가"라는 테스트 취지는 동일.

타깃: Windows PowerShell 5.1 하한 + PowerShell 7+ 호환(삼항/`??` 미사용).
=============================================================================
#>

Set-StrictMode -Version 2.0
$ErrorActionPreference = 'Stop'

$ScriptDir = $PSScriptRoot
$Uninstall = (Resolve-Path (Join-Path $ScriptDir '..\..\scripts\uninstall.ps1')).Path

# 검사 대상 스크립트가 실패 시 exit 1을 호출하므로, bash판(서브셸)과 동등하게
# 자식 프로세스로 실행한다(같은 프로세스에서 직접 호출하면 이 러너가 죽는다).
$PwshExe = [System.Diagnostics.Process]::GetCurrentProcess().MainModule.FileName

$Script:Fail = $false
function Pass2 { param([string] $m) Write-Host "[PASS] $m" }
function Fail2 { param([string] $m) Write-Host "[FAIL] $m"; $Script:Fail = $true }

function New-FixtureRoot {
    return Join-Path ([System.IO.Path]::GetTempPath()) ('bathos-uninstall-' + [System.Guid]::NewGuid().ToString('N'))
}

function Invoke-Uninstall {
    param(
        [string] $Root,
        [string] $State,
        [string] $HomeDir,
        [string[]] $ExtraArgs = @()
    )
    $prevRoot = $env:BATHOS_ROOT
    $prevState = $env:BATHOS_STATE_DIR
    $prevHome = $env:BATHOS_HOME
    $env:BATHOS_ROOT = $Root
    $env:BATHOS_STATE_DIR = $State
    $env:BATHOS_HOME = $HomeDir
    try {
        $out = & $PwshExe -NoProfile -NoLogo -NonInteractive -File $Uninstall @ExtraArgs 2>&1
        $rc = $LASTEXITCODE
    } finally {
        $env:BATHOS_ROOT = $prevRoot
        $env:BATHOS_STATE_DIR = $prevState
        $env:BATHOS_HOME = $prevHome
    }
    return [PSCustomObject]@{ Output = ($out | Out-String); ExitCode = $rc }
}

# ---------------------------------------------------------------------------
# ① dry-run: 외부 상태가 있으면 전 대상을 나열하고 아무것도 지우지 않는다
# ---------------------------------------------------------------------------
function Test-DryRunListsAll {
    $root = New-FixtureRoot
    $state = Join-Path $root '_state'; New-Item -ItemType Directory -Force -Path $state | Out-Null
    $home = Join-Path $root 'home'; New-Item -ItemType Directory -Force -Path (Join-Path $home '.claude') | Out-Null
    Set-Content -LiteralPath (Join-Path $state 'session-flags.json') -Value '{"plan_mode": true}' -Encoding ascii
    Set-Content -LiteralPath (Join-Path $home '.claude\settings.json') -Value '{"statusLine": {"command": "bathos statusline"}}' -Encoding ascii

    $r = Invoke-Uninstall -Root $root -State $state -HomeDir $home

    if (($r.ExitCode -eq 0) -and ($r.Output -match 'session-flags\.json') -and ($r.Output -match 'dry-run')) {
        Pass2 '① dry-run이 대상을 나열하고 exit 0'
    } else {
        Fail2 "① dry-run 출력 이상: $($r.Output)"
    }

    if (Test-Path -LiteralPath (Join-Path $state 'session-flags.json') -PathType Leaf) {
        Pass2 '① dry-run은 실제로 삭제하지 않음'
    } else {
        Fail2 '① dry-run인데 파일이 삭제됨(위험한 버그)'
    }

    Remove-Item -LiteralPath $root -Recurse -Force -ErrorAction SilentlyContinue
}

# ---------------------------------------------------------------------------
# ② -Yes 실행: 실제 제거 + [removed] 로그
# ---------------------------------------------------------------------------
function Test-YesActuallyRemoves {
    $root = New-FixtureRoot
    $state = Join-Path $root '_state'; New-Item -ItemType Directory -Force -Path $state | Out-Null
    $home = Join-Path $root 'home'; New-Item -ItemType Directory -Force -Path (Join-Path $home '.claude') | Out-Null
    Set-Content -LiteralPath (Join-Path $state 'session-flags.json') -Value '{"plan_mode": true}' -Encoding ascii

    $r = Invoke-Uninstall -Root $root -State $state -HomeDir $home -ExtraArgs @('-Yes')

    if (($r.ExitCode -eq 0) -and ($r.Output -match '\[removed\]')) {
        Pass2 '② -Yes 실행이 [removed] 로그를 남김'
    } else {
        Fail2 "② -Yes 실행 출력 이상(exit=$($r.ExitCode)): $($r.Output)"
    }

    if (-not (Test-Path -LiteralPath (Join-Path $state 'session-flags.json') -PathType Leaf)) {
        Pass2 '② -Yes 실행 후 실제로 파일이 삭제됨'
    } else {
        Fail2 '② -Yes 실행했는데 파일이 남아있음'
    }

    Remove-Item -LiteralPath $root -Recurse -Force -ErrorAction SilentlyContinue
}

# ---------------------------------------------------------------------------
# ③ 대상 없음: 빈 상태 안내
# ---------------------------------------------------------------------------
function Test-EmptyStateMessage {
    $root = New-FixtureRoot
    $state = Join-Path $root '_state'; New-Item -ItemType Directory -Force -Path $state | Out-Null
    $home = Join-Path $root 'home'; New-Item -ItemType Directory -Force -Path $home | Out-Null

    $r = Invoke-Uninstall -Root $root -State $state -HomeDir $home

    if (($r.ExitCode -eq 0) -and ($r.Output -match '외부 상태 없음')) {
        Pass2 '③ 대상 없음일 때 빈 상태 안내 + exit 0'
    } else {
        Fail2 "③ 빈 상태 처리 이상(exit=$($r.ExitCode)): $($r.Output)"
    }

    Remove-Item -LiteralPath $root -Recurse -Force -ErrorAction SilentlyContinue
}

# ---------------------------------------------------------------------------
# ④ 쓰기 불가 파일: 부분 실패 분리 표기(성공/실패 카운트 분리)
# ---------------------------------------------------------------------------
function Test-PartialFailureReported {
    $root = New-FixtureRoot
    $state = Join-Path $root '_state'; New-Item -ItemType Directory -Force -Path $state | Out-Null
    $home = Join-Path $root 'home'; New-Item -ItemType Directory -Force -Path (Join-Path $home '.claude') | Out-Null
    Set-Content -LiteralPath (Join-Path $state 'session-flags.json') -Value '{"plan_mode": true}' -Encoding ascii
    $settingsPath = Join-Path $home '.claude\settings.json'
    Set-Content -LiteralPath $settingsPath -Value '{"statusLine": {"command": "bathos statusline"}}' -Encoding ascii
    # 대상 파일을 읽기전용으로 표시 -> 백업은 성공(원본이 읽기전용이어도 복사는 허용),
    # 원본 파일 교체(쓰기)는 실패 -> 부분 실패 유도(위 이식 메모 참고).
    Set-ItemProperty -LiteralPath $settingsPath -Name IsReadOnly -Value $true

    $r = Invoke-Uninstall -Root $root -State $state -HomeDir $home -ExtraArgs @('-Yes')

    # 정리 전 복구(삭제 위해 읽기전용 해제)
    if (Test-Path -LiteralPath $settingsPath) {
        Set-ItemProperty -LiteralPath $settingsPath -Name IsReadOnly -Value $false -ErrorAction SilentlyContinue
    }

    if (($r.Output -match '\[fail') -and ($r.Output -match '\[removed\]')) {
        Pass2 '④ 부분 실패가 성공/실패 분리 표기됨'
    } else {
        Fail2 "④ 부분 실패 표기 이상(exit=$($r.ExitCode)): $($r.Output)"
    }

    if ($r.ExitCode -eq 1) {
        Pass2 '④ 부분 실패 시 exit=1(비차단이나 신호는 남김)'
    } else {
        Fail2 "④ 부분 실패인데 exit=$($r.ExitCode) (기대 1)"
    }

    Remove-Item -LiteralPath $root -Recurse -Force -ErrorAction SilentlyContinue
}

Test-DryRunListsAll
Test-YesActuallyRemoves
Test-EmptyStateMessage
Test-PartialFailureReported

if (-not $Script:Fail) {
    Write-Host '[bathos test-uninstall] OK 전체 통과'
    exit 0
} else {
    Write-Host '[bathos test-uninstall] FAIL 실패 항목 존재'
    exit 1
}
