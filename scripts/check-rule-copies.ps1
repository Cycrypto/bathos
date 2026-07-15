<#
=============================================================================
BATHOS Dynamis — check-rule-copies.ps1  (Windows PowerShell 포트)
원본: scripts/check-rule-copies.sh

두 가지 모드를 지원한다(둘 다 CF-B1/B3 뒷받침):

  -DupScan       (기본)  risk-log A-4 필수 조치 — B1/B2 AC1 "무중복" 즉시 검증.
                  canonical 소스(역할/스킬/커맨드 산문)의 유의미한 줄이
                  dist/**·docs/** 안에 통째로 재복제되지 않았는지 검사.

  -CheckCopies   CF-B3 AC1 본체 — instruction-only 계층으로 "의도적으로"
                  복제된 규칙 텍스트(dist/copies-manifest.json에 등록된 항목만)의
                  drift를 검사한다. 짧은 복제=byte diff, 긴 본문=invariant 부분문자열.
                  등록된 복제가 0건이면 통과+안내(실패 아님 — B3 CONCERNS 명시 처리).

공통 관례: 기존 훅 관례 계승
  ① ScriptDir -> BathosRoot 경로 해석
  ② JSON은 ConvertFrom-Json 네이티브 사용(jq 불필요)
  ③ fail-safe: 검사 대상 자체가 없으면(디렉터리 부재 등) 통과 + 안내(과차단 금지)

exit: 0=문제 없음, 1=드리프트/중복 발견(무엇이 어디서 발견됐는지 + 다음 행동 포함)

타깃: Windows PowerShell 5.1 하한 + PowerShell 7+ 호환(삼항/`??` 미사용).
=============================================================================
#>

param(
    [switch] $DupScan,
    [switch] $CheckCopies,
    [Alias('help', 'h')]
    [switch] $Help,

    [string] $BathosRoot = '',
    [string] $BathosCopiesManifest = '',
    [string] $BathosDriftExclusions = ''
)

Set-StrictMode -Version 2.0
$ErrorActionPreference = 'Stop'

if ($Help) {
    Write-Host @'
사용법: check-rule-copies.ps1 [-DupScan|-CheckCopies]
  -DupScan      (기본) canonical 산문이 dist/·docs/ 안에 재복제되지 않았는지 검사(B1/B2 AC1, A-4)
  -CheckCopies  등록된 instruction-only 복제본의 drift 검사(B3 AC1)
'@
    exit 0
}

$Mode = 'dup-scan'
if ($CheckCopies) { $Mode = 'check-copies' }
if ($DupScan) { $Mode = 'dup-scan' }

$ScriptDir = $PSScriptRoot
if ([string]::IsNullOrEmpty($BathosRoot)) {
    $envRoot = $env:BATHOS_ROOT
    if (-not [string]::IsNullOrEmpty($envRoot)) { $BathosRoot = $envRoot }
    else { $BathosRoot = (Resolve-Path (Join-Path $ScriptDir '..')).Path }
}

$Script:Fail = $false
function Info  { param([string] $m) Write-Host "[bathos check-rule-copies] $m" }
function Warn2 { param([string] $m) Write-Host "[bathos check-rule-copies] ! $m" -ForegroundColor Yellow }
function ErrorMsg { param([string] $m) Write-Host "[bathos check-rule-copies] X $m" -ForegroundColor Red; $Script:Fail = $true }
function OkMsg { param([string] $m) Write-Host "[bathos check-rule-copies] OK $m" -ForegroundColor Green }

# --- canonical 소스 루트 해석 (제품 트리 우선, 없으면 원본 bathos/ 폴백) --------
# LD-5: 최종 조립(Paul) 전에는 제품 트리에 .claude/agents 등이 아직 없을 수 있다.
# 이 경우 원본 bathos/를 read-only 참조로 폴백해 검증한다(임시 동작).
function Resolve-CanonRoot {
    $primaryAgents = Join-Path $BathosRoot '.claude\agents'
    if (Test-Path -LiteralPath $primaryAgents -PathType Container) {
        return (Join-Path $BathosRoot '.claude')
    }
    $orig = Join-Path $BathosRoot '..\..\bathos\.claude'
    if (Test-Path -LiteralPath $orig -PathType Container) {
        Write-Host '[bathos check-rule-copies] ! 제품 트리에 .claude/agents 없음 - 원본 bathos/.claude 폴백 검사 중(LD-5 최종 조립 전 임시 동작)' -ForegroundColor Yellow
        return (Resolve-Path $orig).Path
    }
    return ''
}

if ($Mode -eq 'dup-scan') {
    $CanonClaude = Resolve-CanonRoot
    if ([string]::IsNullOrEmpty($CanonClaude)) {
        Warn2 'canonical 소스(.claude/agents)를 찾을 수 없음 - 검사 대상 없음(fail-safe 통과). 다음 행동: 최종 조립 후 재실행.'
        exit 0
    }

    $CanonDirs = New-Object System.Collections.ArrayList
    foreach ($sub in @('agents', 'skills', 'commands')) {
        $d = Join-Path $CanonClaude $sub
        if (Test-Path -LiteralPath $d -PathType Container) { [void]$CanonDirs.Add($d) }
    }

    $CanonFiles = New-Object System.Collections.ArrayList
    foreach ($d in $CanonDirs) {
        $found = Get-ChildItem -LiteralPath $d -Recurse -File -Filter '*.md' -ErrorAction SilentlyContinue
        foreach ($f in $found) { [void]$CanonFiles.Add($f.FullName) }
    }

    if ($CanonFiles.Count -eq 0) {
        OkMsg 'canonical 산문 파일 0건 - 검사 대상 없음(통과)'
        exit 0
    }

    $SearchDirs = New-Object System.Collections.ArrayList
    $distDir = Join-Path $BathosRoot 'dist'
    $docsDir = Join-Path $BathosRoot 'docs'
    if (Test-Path -LiteralPath $distDir -PathType Container) { [void]$SearchDirs.Add($distDir) }
    if (Test-Path -LiteralPath $docsDir -PathType Container) { [void]$SearchDirs.Add($docsDir) }

    if ($SearchDirs.Count -eq 0) {
        OkMsg 'dist/·docs/ 없음 - 검사 대상 없음(통과)'
        exit 0
    }

    $MinLen = 60   # 이 길이 미만 줄은 우연 일치 가능성이 높아 비교 제외(오탐 방지)
    $LinesScanned = 0
    $DupesFound = 0

    foreach ($f in $CanonFiles) {
        $relF = $f.Substring($BathosRoot.Length).TrimStart('\', '/')
        $lines = Get-Content -LiteralPath $f -ErrorAction SilentlyContinue
        foreach ($line in $lines) {
            if ([string]::IsNullOrEmpty($line)) { continue }
            if ($line -match '^\s*#+\s') { continue }
            if ($line -eq '---') { continue }
            if ($line.Length -lt $MinLen) { continue }
            $LinesScanned++
            foreach ($d in $SearchDirs) {
                $hits = Get-ChildItem -LiteralPath $d -Recurse -File -ErrorAction SilentlyContinue |
                    Select-String -Pattern $line -SimpleMatch -List -ErrorAction SilentlyContinue
                if ($hits) {
                    foreach ($h in $hits) {
                        $DupesFound++
                        ErrorMsg "산문 재복제 의심: '$relF' 의 한 줄이 다음에 그대로 존재함 -> $($h.Path)"
                        $excerpt = $line
                        if ($excerpt.Length -gt 80) { $excerpt = $excerpt.Substring(0, 80) }
                        ErrorMsg "  내용(일부): $excerpt..."
                    }
                }
            }
        }
    }

    if (-not $Script:Fail) {
        OkMsg "무중복 확인 - canonical 파일 $($CanonFiles.Count)개, 비교 줄 $LinesScanned 개, 재복제 0건"
        exit 0
    } else {
        ErrorMsg "재복제 ${DupesFound}건 발견. 다음 행동: 해당 표면을 canonical 경로 포인터 참조로 교체하세요(CF-B1 위반=게이트 FAIL 사유)."
        exit 1
    }
}

if ($Mode -eq 'check-copies') {
    if ([string]::IsNullOrEmpty($BathosCopiesManifest)) {
        $envManifest = $env:BATHOS_COPIES_MANIFEST
        if (-not [string]::IsNullOrEmpty($envManifest)) { $BathosCopiesManifest = $envManifest }
        else { $BathosCopiesManifest = Join-Path $BathosRoot 'dist\copies-manifest.json' }
    }
    if ([string]::IsNullOrEmpty($BathosDriftExclusions)) {
        $envExcl = $env:BATHOS_DRIFT_EXCLUSIONS
        if (-not [string]::IsNullOrEmpty($envExcl)) { $BathosDriftExclusions = $envExcl }
        else { $BathosDriftExclusions = Join-Path $ScriptDir 'drift-exclusions.json' }
    }

    if (-not (Test-Path -LiteralPath $BathosCopiesManifest -PathType Leaf)) {
        OkMsg 'copies-manifest.json 없음 - 검사 대상 0건(통과). 다음 행동: instruction-only 어댑터 추가 시 dist/copies-manifest.json에 등록하세요.'
        exit 0
    }

    $manifest = $null
    try {
        $manifest = (Get-Content -LiteralPath $BathosCopiesManifest -Raw) | ConvertFrom-Json
    } catch {
        ErrorMsg "copies-manifest.json 파싱 실패: $BathosCopiesManifest"
        exit 1
    }

    $copies = @()
    if ($manifest.PSObject.Properties['copies']) { $copies = @($manifest.copies) }
    $count = $copies.Count

    if ($count -eq 0) {
        OkMsg '등록된 복제본 0건 - 통과(현재 범위는 Claude Code 정본 + MCP 포인터만 구현, B3 CONCERNS 명시 처리)'
        exit 0
    }

    $exclusions = @()
    if (Test-Path -LiteralPath $BathosDriftExclusions -PathType Leaf) {
        try {
            $exJson = (Get-Content -LiteralPath $BathosDriftExclusions -Raw) | ConvertFrom-Json
            if ($exJson.PSObject.Properties['exclusions']) { $exclusions = @($exJson.exclusions) }
        } catch { $exclusions = @() }
    }

    function Test-IsExcluded {
        param([string] $Rel)
        foreach ($ex in $exclusions) {
            if ($ex.PSObject.Properties['path'] -and ($ex.path -eq $Rel)) { return $true }
        }
        return $false
    }

    function Get-NormalizedContent {
        param([string] $Path)
        # fingerprint 정규화(간이판): 후행공백 제거 + CRLF->LF.
        $lines = Get-Content -LiteralPath $Path -ErrorAction SilentlyContinue
        $normed = foreach ($l in $lines) { $l -replace '\s+$', '' }
        return ($normed -join "`n")
    }

    for ($i = 0; $i -lt $count; $i++) {
        $entry = $copies[$i]
        $srcRel = [string]$entry.source
        $copyRel = [string]$entry.copy
        $mode = [string]$entry.mode

        if (Test-IsExcluded $copyRel) {
            Info "제외됨(exclusions): $copyRel"
            continue
        }

        $src = Join-Path $BathosRoot $srcRel
        $copy = Join-Path $BathosRoot $copyRel

        if ((-not (Test-Path -LiteralPath $src -PathType Leaf)) -or (-not (Test-Path -LiteralPath $copy -PathType Leaf))) {
            ErrorMsg "$copyRel - 소스 또는 복제본 파일 부재(source=$srcRel). 다음 행동: 경로를 확인하세요."
            continue
        }

        if ($mode -eq 'byte') {
            $srcNorm = Get-NormalizedContent $src
            $copyNorm = Get-NormalizedContent $copy
            if ($srcNorm -ne $copyNorm) {
                ErrorMsg "$copyRel - canonical($srcRel)과 drift 발견(byte 모드). 다음 행동: 복제본을 정본과 동기화하세요."
            }
        } elseif ($mode -eq 'invariant') {
            $invariants = @()
            if ($entry.PSObject.Properties['invariant']) { $invariants = @($entry.invariant) }
            $copyText = Get-Content -LiteralPath $copy -Raw -ErrorAction SilentlyContinue
            foreach ($needle in $invariants) {
                $safeCopyText = $copyText
                if ($null -eq $safeCopyText) { $safeCopyText = '' }
                if (-not $safeCopyText.Contains([string]$needle)) {
                    ErrorMsg "$copyRel - invariant 문구 누락: `"$needle`". 다음 행동: 복제본에 해당 문구를 복원하세요."
                }
            }
        } else {
            ErrorMsg "$copyRel - 알 수 없는 mode: $mode (byte|invariant만 지원)"
        }
    }

    if (-not $Script:Fail) {
        OkMsg "등록된 복제본 $count 건 전부 drift 없음"
        exit 0
    } else {
        exit 1
    }
}
