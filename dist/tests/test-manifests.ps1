<#
=============================================================================
BATHOS Dynamis — dist/tests/test-manifests.ps1  (Windows PowerShell 포트)
원본: dist/tests/test-manifests.sh
Story B1·B2 §5 "테스트 접근" 구현: 매니페스트 스키마 자기검증 + 포인터 유효성.

  1) 공통 필드(name/version/sources/capability_tier) 존재 확인 + 산문 필드 없음
     (behavior 텍스트가 통째로 박혀있지 않은지 — 필드 값 길이로 간이 검사)
  2) sources.* 의 상대경로가 실제 존재하는 디렉터리를 가리키는지 확인
     (깨진 포인터 = 실패). LD-5(최종 조립 전) 갭 처리: 제품 트리에 대상이 아직
     없으면 원본 bathos/의 동일 상대경로로 폴백 확인 후 "조립 대기" 로 표기.
  3) capability_tier 값이 폐쇄 어휘(full-hook/instruction-only/mcp) 안에 있는지

marketplace.json은 sources/capability_tier가 없는 것이 정상 스키마이므로
plugin.json/manifest.json만 검사한다(check-versions.ps1과 동일 대상 규약).

타깃: Windows PowerShell 5.1 하한 + PowerShell 7+ 호환(삼항/`??` 미사용).
JSON은 ConvertFrom-Json 네이티브 사용(jq 불필요).
=============================================================================
#>

Set-StrictMode -Version 2.0
$ErrorActionPreference = 'Stop'

$ScriptDir = $PSScriptRoot
$BathosRoot = (Resolve-Path (Join-Path $ScriptDir '..\..')).Path
$DistDir = Join-Path $BathosRoot 'dist'

$Script:Fail = $false
$Script:PassCount = 0
function Info  { param([string] $m) Write-Host "[test-manifests] $m" }
function ErrorMsg { param([string] $m) Write-Host "[test-manifests] X $m" -ForegroundColor Red; $Script:Fail = $true }
function OkMsg { param([string] $m) Write-Host "[test-manifests] OK $m" -ForegroundColor Green; $Script:PassCount++ }

$ValidTiers = @('full-hook', 'instruction-only', 'mcp')

$Targets = @()
if (Test-Path -LiteralPath $DistDir -PathType Container) {
    $Targets = Get-ChildItem -LiteralPath $DistDir -Recurse -File -ErrorAction SilentlyContinue |
        Where-Object { $_.Name -eq 'plugin.json' -or $_.Name -eq 'manifest.json' }
}

if (-not $Targets -or $Targets.Count -eq 0) {
    Info '검사 대상 0건 - 통과(안내)'
    exit 0
}

foreach ($f in $Targets) {
    $rel = $f.FullName.Substring($BathosRoot.Length).TrimStart('\', '/')
    $fdir = $f.DirectoryName

    $obj = $null
    try {
        $obj = (Get-Content -LiteralPath $f.FullName -Raw) | ConvertFrom-Json
    } catch {
        ErrorMsg "$rel - JSON 파싱 실패"
        continue
    }

    # --- 1) 공통 필드 존재 -----------------------------------------------------
    $name = ''
    if ($obj.PSObject.Properties['name']) { $name = [string]$obj.name }
    $version = ''
    if ($obj.PSObject.Properties['version']) { $version = [string]$obj.version }
    $tier = ''
    if ($obj.PSObject.Properties['capability_tier']) { $tier = [string]$obj.capability_tier }
    $hasSources = $obj.PSObject.Properties['sources'] -ne $null

    if (-not [string]::IsNullOrEmpty($name)) { OkMsg "$rel - name 존재" } else { ErrorMsg "$rel - name 필드 없음" }
    if (-not [string]::IsNullOrEmpty($version)) { OkMsg "$rel - version 존재" } else { ErrorMsg "$rel - version 필드 없음" }

    if (-not $hasSources) {
        ErrorMsg "$rel - sources 필드 없음(포인터 규약 위반, CF-B1 AC1)"
    } else {
        OkMsg "$rel - sources 필드 존재"
    }

    # --- 2) 산문 필드 없음(간이 검사: sources·_skeleton_note 제외한 값 중
    #        200자 초과 문자열 필드가 없는지) — behavior 텍스트 임베드 금지
    $longFieldCount = 0
    foreach ($prop in $obj.PSObject.Properties) {
        if ($prop.Name -eq 'sources' -or $prop.Name -eq '_skeleton_note') { continue }
        if ($prop.Value -is [string] -and $prop.Value.Length -gt 200) { $longFieldCount++ }
    }
    if ($longFieldCount -gt 0) {
        ErrorMsg "$rel - 200자 초과 문자열 필드 발견(behavior 산문 임베드 의심)"
    } else {
        OkMsg "$rel - 산문 임베드 없음(필드 길이 검사 통과)"
    }

    # --- 3) capability_tier 폐쇄 어휘 검사 --------------------------------------
    if (-not [string]::IsNullOrEmpty($tier)) {
        if ($ValidTiers -contains $tier) {
            OkMsg "$rel - capability_tier=$tier (폐쇄 어휘 내)"
        } else {
            ErrorMsg "$rel - capability_tier='$tier' 는 폐쇄 어휘(full-hook/instruction-only/mcp) 밖"
        }
    }

    # --- 4) 포인터 유효성 -------------------------------------------------------
    if ($hasSources) {
        foreach ($key in @('roles', 'skills', 'commands', 'hooks')) {
            $ptr = ''
            if ($obj.sources.PSObject.Properties[$key]) { $ptr = [string]$obj.sources.$key }
            if ([string]::IsNullOrEmpty($ptr)) { continue }

            $target = Join-Path $fdir $ptr
            if (Test-Path -LiteralPath $target -PathType Container) {
                OkMsg "$rel - sources.$key -> $ptr (존재)"
                continue
            }

            # LD-5 폴백: 제품 트리 미조립 상태에서는 원본 bathos/ 동일 상대 위치로 확인.
            $idx = $ptr.IndexOf('.claude/')
            $suffix = $ptr
            if ($idx -ge 0) { $suffix = $ptr.Substring($idx + '.claude/'.Length) }
            $origTarget = Join-Path $BathosRoot ('..\..\bathos\.claude\' + $suffix)
            if (Test-Path -LiteralPath $origTarget -PathType Container) {
                Info "$rel - sources.$key -> $ptr : 제품 트리 미존재(LD-5 조립 대기), 원본 bathos/.claude/$suffix 로 확인됨(통과)"
                $Script:PassCount++
            } else {
                ErrorMsg "$rel - sources.$key -> $ptr : 대상 디렉터리 없음(제품 트리·원본 모두 부재 - 깨진 포인터)"
            }
        }
    }
}

Info "총 $($Script:PassCount)건 통과"
if (-not $Script:Fail) {
    Info '전체 통과'
    exit 0
} else {
    ErrorMsg '실패 항목 존재 - 위 목록을 확인하세요.'
    exit 1
}
