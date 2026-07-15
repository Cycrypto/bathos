<#
=============================================================================
BATHOS Dynamis — check-versions.ps1  (Windows PowerShell 포트)
원본: scripts/check-versions.sh
CF-B3 / US7 AC2 — 전 배포 매니페스트가 단일 semver에 핀 되었는지 검사한다.
ponytail 교훈: 무언 버전 노후화를 changelog 각주가 아니라 CI 실패로 만든다.

검사 대상: dist/**/plugin.json , dist/**/manifest.json
  (marketplace.json은 검사 제외 — 자체 버전 필드를 갖지 않는 설계, plugin.json을
   포인터로만 참조하므로 version 드리프트 대상이 아니다.)

기준(SSOT): dist/VERSION 1줄(semver). 이 파일이 축 B 배포 레이어의 단일 버전
  핀이다. (주의: core/crates의 Rust 엔진 버전(Phillip 소유, VERSION 파일 별도)과
  *의도적으로 분리*된 축이다 — 배포 패키징 버전과 엔진 크레이트 버전은 다른 축.
  통합이 필요하면 Paul 확인 후 재설계.)

부가: git 태그가 존재하면(예: v0.2.0) 태그-버전 정합도 검사(US7 AC2 후단).

exit: 0=전부 정합, 1=불일치 발견(어떤 파일의 어떤 값이 다른지 명시 + 다음 행동)

타깃: Windows PowerShell 5.1 하한 + PowerShell 7+ 호환(삼항/`??` 미사용).
JSON은 ConvertFrom-Json 네이티브 사용(jq 불필요 — bash판의 jq 의존 제거).
=============================================================================
#>

param(
    [string] $BathosRoot = '',
    [Alias('help', 'h')]
    [switch] $Help
)

Set-StrictMode -Version 2.0
$ErrorActionPreference = 'Stop'

if ($Help) {
    Write-Host 'Usage: check-versions.ps1 [-BathosRoot <dir>]'
    exit 0
}

# --- 경로 해석 (bash판 관례 계승: ScriptDir -> BathosRoot) -------------------
$ScriptDir = $PSScriptRoot
if ([string]::IsNullOrEmpty($BathosRoot)) {
    $envRoot = $env:BATHOS_ROOT
    if (-not [string]::IsNullOrEmpty($envRoot)) { $BathosRoot = $envRoot }
    else { $BathosRoot = (Resolve-Path (Join-Path $ScriptDir '..')).Path }
}
$DistDir = Join-Path $BathosRoot 'dist'
$VersionFile = Join-Path $DistDir 'VERSION'

$Script:Fail = $false
function Info  { param([string] $m) Write-Host "[bathos check-versions] $m" }
function Warn2 { param([string] $m) Write-Host "[bathos check-versions] ! $m" -ForegroundColor Yellow }
function ErrorMsg { param([string] $m) Write-Host "[bathos check-versions] X $m" -ForegroundColor Red; $Script:Fail = $true }
function OkMsg { param([string] $m) Write-Host "[bathos check-versions] OK $m" -ForegroundColor Green }

if (-not (Test-Path -LiteralPath $VersionFile -PathType Leaf)) {
    ErrorMsg "기준 파일 없음: $VersionFile - 다음 행동: dist/VERSION을 생성하고 semver 한 줄을 기록하세요."
    exit 1
}

$CanonVersion = ((Get-Content -LiteralPath $VersionFile -Raw) -replace '\s', '')
if ([string]::IsNullOrEmpty($CanonVersion)) {
    ErrorMsg 'dist/VERSION이 비어 있음 - 다음 행동: semver 값을 기록하세요.'
    exit 1
}
Info "기준 버전(dist/VERSION): $CanonVersion"

# --- 필드 추출 (네이티브 JSON 파싱, 실패 시 정규식 폴백) ----------------------
function Get-VersionField {
    param([string] $Path)
    try {
        $obj = (Get-Content -LiteralPath $Path -Raw) | ConvertFrom-Json
        if ($obj.PSObject.Properties['version']) { return [string]$obj.version }
        return ''
    } catch {
        $raw = Get-Content -LiteralPath $Path -Raw -ErrorAction SilentlyContinue
        if ([string]::IsNullOrEmpty($raw)) { return '' }
        $m = [regex]::Match($raw, '"version"\s*:\s*"([^"]+)"')
        if ($m.Success) { return $m.Groups[1].Value }
        return ''
    }
}

# --- 검사 대상 수집 ----------------------------------------------------------
$Targets = @()
if (Test-Path -LiteralPath $DistDir -PathType Container) {
    $Targets = Get-ChildItem -LiteralPath $DistDir -Recurse -File -ErrorAction SilentlyContinue |
        Where-Object { $_.Name -eq 'plugin.json' -or $_.Name -eq 'manifest.json' }
}

if (-not $Targets -or $Targets.Count -eq 0) {
    # B3 CONCERNS 명시 처리: 검사 대상 0건 = 통과 + 안내(실패로 오인 금지)
    OkMsg '검사 대상 0건 (dist/**/{plugin,manifest}.json 없음) - 통과. 다음 행동: 매니페스트 추가 시 자동으로 검사됩니다.'
    exit 0
}

$Checked = 0
foreach ($f in $Targets) {
    $rel = $f.FullName.Substring($BathosRoot.Length).TrimStart('\', '/')
    $v = Get-VersionField $f.FullName
    $Checked++
    if ([string]::IsNullOrEmpty($v)) {
        ErrorMsg "$rel - version 필드 없음. 다음 행동: `"version`": `"$CanonVersion`"을 추가하세요."
        continue
    }
    if ($v -ne $CanonVersion) {
        ErrorMsg "$rel - version=$v (기대: $CanonVersion). 다음 행동: dist/VERSION과 동일하게 맞추세요."
    }
}

# --- git 태그 정합(태그가 있을 때만, 없으면 스킵 — fail-open) -------------------
if (Get-Command git -ErrorAction SilentlyContinue) {
    $isRepo = $false
    try {
        Push-Location $BathosRoot
        & git rev-parse --git-dir *> $null
        if ($LASTEXITCODE -eq 0) { $isRepo = $true }
    } finally {
        Pop-Location
    }

    if ($isRepo) {
        $tags = @()
        try {
            Push-Location $BathosRoot
            $tags = & git tag --points-at HEAD 2>$null
        } finally {
            Pop-Location
        }
        $latestTag = $null
        foreach ($t in $tags) {
            if ($t -match '^v?\d+\.\d+\.\d+$') { $latestTag = $t; break }
        }
        if ($null -ne $latestTag) {
            $tagVersion = $latestTag -replace '^v', ''
            if ($tagVersion -ne $CanonVersion) {
                ErrorMsg "git 태그($latestTag) != dist/VERSION($CanonVersion). 다음 행동: 태그 재발행 또는 VERSION 갱신."
            } else {
                OkMsg "git 태그($latestTag) 정합"
            }
        }
    } else {
        Warn2 'git 저장소 아님 - 태그 정합 검사 스킵(fail-open, 비차단).'
    }
} else {
    Warn2 'git 없음 - 태그 정합 검사 스킵(fail-open, 비차단).'
}

if (-not $Script:Fail) {
    OkMsg "${Checked}개 매니페스트 전부 ${CanonVersion}로 정합"
    exit 0
} else {
    Write-Host '[bathos check-versions] X 버전 불일치 발견 - 위 항목을 수정한 뒤 재실행하세요.' -ForegroundColor Red
    exit 1
}
