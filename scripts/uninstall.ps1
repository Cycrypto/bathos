<#
=============================================================================
BATHOS Dynamis — scripts/uninstall.ps1  (Windows PowerShell 포트)
원본: scripts/uninstall.sh
CF-B4 / SS11 (Should) — 플러그인 폴더 "밖" 외부 상태 정리 1급 스크립트.

★★★ 실행순서 계약(핵심 엣지케이스, ponytail 확인 사실) ★★★
  호스트의 플러그인 제거 명령은 이 스크립트 파일 자체를 먼저 지운다.
  반드시 "호스트 제거 명령보다 먼저" 이 스크립트를 실행해야 한다.
  (스크립트가 이미 사라진 뒤라면 아래 §README "수동 정리 경로"를 따를 것 —
   docs/uninstall-kr.md 참조)

정리 대상(명시적 목록 — 암묵 글롭 삭제 금지, careful 정신 계승):
  1. ${BathosStateDir}/session-flags.json   (plan_mode/intensity 세션 플래그)
  2. ${BathosHome}/.claude/settings.json 의 statusLine 엔트리
     (bathos 관련 엔트리만 정밀 제거 — 파일 전체 재작성 금지, .bak 백업 후 수정)
  3. ${BathosConfigDir}/*                   (존재 시에만 — 전역 설정 잔여물)

명시적으로 건드리지 않는 것(SSOT 보호):
  - _state/manifest.json (프로젝트 SSOT, 삭제 대상 아님)
  - _state/audit-log.jsonl (append-only 감사 이력, 삭제 대상 아님)

UX 규약(ux-flow-map Flow C, design-handoff §2-4 그대로 구현):
  - dry-run 기본(인자 없으면 계획만 출력)
  - 파괴적 실행 앞단 강한 경고 + 항목별 결과 로그
  - `y/N`(기본 N) 또는 -Yes
  - 부분 실패는 성공/실패를 분리 표기(전체를 죽이지 않음)
  - 완료 후 "다음 행동" 안내(막다른 골목 금지)

exit: 0=정상 완료(dry-run 포함), 1=사용자 취소 또는 부분 실패 존재

타깃: Windows PowerShell 5.1 하한 + PowerShell 7+ 호환(삼항/`??` 미사용).
테스트 가능성을 위해 전부 매개변수/환경변수로 오버라이드 가능(실제 $HOME을
건드리지 않고 픽스처로 검증할 수 있어야 한다 — bash판과 동일 관례).
=============================================================================
#>

param(
    [Alias('apply')]
    [switch] $Apply,

    [Alias('yes', 'y')]
    [switch] $Yes,

    [Alias('dry-run')]
    [switch] $DryRunSwitch,

    [Alias('help', 'h')]
    [switch] $Help,

    # 테스트/오버라이드용(bash판 BATHOS_ROOT/BATHOS_STATE_DIR/BATHOS_HOME/BATHOS_CONFIG_DIR 등가)
    [string] $BathosRoot = '',
    [string] $BathosStateDir = '',
    [string] $BathosHome = '',
    [string] $BathosConfigDir = ''
)

Set-StrictMode -Version 2.0
$ErrorActionPreference = 'Stop'

$Prefix = '[bathos uninstall]'
function Info  { param([string] $m) Write-Host "$Prefix $m" }
function Warn2 { param([string] $m) Write-Host "$Prefix ! $m" -ForegroundColor Yellow }
function OkMsg { param([string] $m) Write-Host "$Prefix OK $m" -ForegroundColor Green }
function FailMsg { param([string] $m) Write-Host "$Prefix FAIL $m" -ForegroundColor Red }

if ($Help) {
    Write-Host @'
사용법: uninstall.ps1 [-Apply] [-Yes] [-DryRunSwitch]
  (인자 없음)   기본값 = dry-run: 삭제 대상 계획만 출력, 아무것도 지우지 않음
  -Apply        실제 삭제 진행(대화형 y/N 확인, 기본 N)
  -Yes, -y      -Apply와 함께(또는 단독) 사용 시 확인 없이 즉시 삭제
  -DryRunSwitch 명시적 dry-run(기본값과 동일, 문서화 목적)
'@
    exit 0
}

# --- 경로 해석 (bash판 SCRIPT_DIR -> BATHOS_ROOT 관례 계승) -------------------
$ScriptDir = $PSScriptRoot
if ([string]::IsNullOrEmpty($BathosRoot)) {
    $envRoot = $env:BATHOS_ROOT
    if (-not [string]::IsNullOrEmpty($envRoot)) {
        $BathosRoot = $envRoot
    } else {
        $BathosRoot = (Resolve-Path (Join-Path $ScriptDir '..')).Path
    }
}

if ([string]::IsNullOrEmpty($BathosStateDir)) {
    $envState = $env:BATHOS_STATE_DIR
    if (-not [string]::IsNullOrEmpty($envState)) { $BathosStateDir = $envState }
    else { $BathosStateDir = Join-Path $BathosRoot '_state' }
}

if ([string]::IsNullOrEmpty($BathosHome)) {
    $envHome = $env:BATHOS_HOME
    if (-not [string]::IsNullOrEmpty($envHome)) { $BathosHome = $envHome }
    else {
        $h = $env:USERPROFILE
        if ([string]::IsNullOrEmpty($h)) { $h = $HOME }
        $BathosHome = $h
    }
}

if ([string]::IsNullOrEmpty($BathosConfigDir)) {
    $envCfg = $env:BATHOS_CONFIG_DIR
    if (-not [string]::IsNullOrEmpty($envCfg)) { $BathosConfigDir = $envCfg }
    else { $BathosConfigDir = Join-Path $BathosHome '.config\bathos' }
}

# --Yes 는 --Apply 를 함의(bash판: --yes|-y) DRY_RUN=0)
$DryRun = $true
if ($Apply) { $DryRun = $false }
if ($Yes) { $DryRun = $false }
if ($DryRunSwitch) { $DryRun = $true }

Warn2 '되돌릴 수 없음 - 호스트의 플러그인 제거 명령보다 먼저 이 스크립트를 실행하세요.'
Warn2 '  (호스트 제거가 먼저 실행되면 이 스크립트 파일 자체가 함께 삭제됩니다.)'

# --- 대상 계획 수립 ----------------------------------------------------------
$PlanDesc = New-Object System.Collections.ArrayList
$PlanKind = New-Object System.Collections.ArrayList   # file | jsonkey | dircontents
$PlanPath = New-Object System.Collections.ArrayList

$SessionFlags = Join-Path $BathosStateDir 'session-flags.json'
if (Test-Path -LiteralPath $SessionFlags -PathType Leaf) {
    [void]$PlanDesc.Add("세션 플래그(plan/intensity): $SessionFlags")
    [void]$PlanKind.Add('file')
    [void]$PlanPath.Add($SessionFlags)
}

$SettingsJson = Join-Path $BathosHome '.claude\settings.json'
if (Test-Path -LiteralPath $SettingsJson -PathType Leaf) {
    [void]$PlanDesc.Add("전역 settings.json의 statusLine 엔트리(bathos 관련분만): $SettingsJson")
    [void]$PlanKind.Add('jsonkey')
    [void]$PlanPath.Add($SettingsJson)
}

if (Test-Path -LiteralPath $BathosConfigDir -PathType Container) {
    [void]$PlanDesc.Add("전역 설정 디렉터리 내용물: $BathosConfigDir\*")
    [void]$PlanKind.Add('dircontents')
    [void]$PlanPath.Add($BathosConfigDir)
}

$Total = $PlanDesc.Count

if ($Total -eq 0) {
    OkMsg '외부 상태 없음 - 호스트 제거만 하면 됩니다.'
    Info '다음 행동: 호스트의 플러그인 제거 명령을 실행하세요.'
    exit 0
}

Info "삭제 계획 (${Total}건):"
for ($i = 0; $i -lt $Total; $i++) {
    Write-Host "$Prefix   - $($PlanDesc[$i])"
}

if ($DryRun) {
    Info '(dry-run) 아무것도 삭제하지 않았습니다. 실제 삭제: -Apply [-Yes]'
    exit 0
}

if (-not $Yes) {
    $reply = Read-Host "$Prefix 위 $Total 건을 정말 삭제하시겠습니까? 되돌릴 수 없습니다. [y/N]"
    if ($reply -notmatch '^(y|Y|yes|YES)$') {
        Warn2 '사용자 취소 - 아무것도 삭제하지 않았습니다.'
        exit 1
    }
}

# --- 실제 삭제 실행 -----------------------------------------------------------
$Script:Success = 0
$Script:Failed = 0

function Remove-BathosFile {
    param([string] $Path)
    try {
        Remove-Item -LiteralPath $Path -Force -ErrorAction Stop
        OkMsg "[removed] $Path"
        $Script:Success++
    } catch {
        FailMsg "[fail: 권한/경로 문제] $Path"
        $Script:Failed++
    }
}

function Remove-BathosDirContents {
    param([string] $Dir)
    $entries = Get-ChildItem -LiteralPath $Dir -Force -ErrorAction SilentlyContinue
    foreach ($entry in $entries) {
        try {
            Remove-Item -LiteralPath $entry.FullName -Recurse -Force -ErrorAction Stop
            OkMsg "[removed] $($entry.FullName)"
            $Script:Success++
        } catch {
            FailMsg "[fail: 권한/경로 문제] $($entry.FullName)"
            $Script:Failed++
        }
    }
}

function Remove-BathosStatusLineKey {
    param([string] $Path)

    $raw = Get-Content -LiteralPath $Path -Raw -ErrorAction SilentlyContinue
    if ([string]::IsNullOrEmpty($raw)) {
        Warn2 "[skip: 읽기 실패] $Path"
        return
    }

    try {
        $json = $raw | ConvertFrom-Json
    } catch {
        Warn2 "[skip: JSON 파싱 실패] $Path - 수동 제거 안내: `"statusLine`" 키를 열어 bathos 관련 항목만 지우세요."
        return
    }

    $cmdVal = ''
    if ($json.PSObject.Properties['statusLine'] -and $json.statusLine.PSObject.Properties['command']) {
        $cmdVal = [string]$json.statusLine.command
    }

    if ($cmdVal -notmatch '(?i)bathos') {
        Warn2 "[skip: bathos 관련 아님] $Path - statusLine이 있으나 bathos 참조가 확인되지 않아 건드리지 않음."
        return
    }

    $backup = "$Path.bak"
    try {
        Copy-Item -LiteralPath $Path -Destination $backup -Force -ErrorAction Stop
    } catch {
        FailMsg "[fail: 백업 실패] $Path"
        $Script:Failed++
        return
    }

    try {
        $json.PSObject.Properties.Remove('statusLine')
        $outText = $json | ConvertTo-Json -Depth 20
        # UTF-8(BOM 없음)로 기록 — freeze-guard.ps1과 동일 관례
        $enc = New-Object System.Text.UTF8Encoding($false)
        [System.IO.File]::WriteAllText($Path, $outText, $enc)
        OkMsg "[removed] $Path 의 statusLine 엔트리 (백업: $backup)"
        $Script:Success++
    } catch {
        FailMsg "[fail: JSON 편집 실패] $Path (백업은 $backup 에 보존됨)"
        $Script:Failed++
    }
}

for ($i = 0; $i -lt $Total; $i++) {
    $kind = $PlanKind[$i]
    $path = $PlanPath[$i]
    switch ($kind) {
        'file'        { Remove-BathosFile $path }
        'jsonkey'     { Remove-BathosStatusLineKey $path }
        'dircontents' { Remove-BathosDirContents $path }
    }
}

Info "결과: 성공 $($Script:Success)건 / 실패 $($Script:Failed)건"
if ($Script:Failed -gt 0) {
    Warn2 '일부 항목 정리 실패 - 다음 행동: 위 [fail] 항목을 수동으로 확인/제거한 뒤, 그래도 호스트 제거는 진행 가능합니다.'
    exit 1
}

OkMsg '외부 상태 정리 완료.'
Info '다음 행동: 이제 호스트의 플러그인 제거 명령을 실행하세요.'
exit 0
