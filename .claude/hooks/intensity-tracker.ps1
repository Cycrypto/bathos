# =============================================================================
# BATHOS Dynamis A5 — intensity-tracker.ps1   (Windows PowerShell 포트, SS4 · CF-A5, Could)
# 원본: .claude/hooks/intensity-tracker.sh
# UserPromptSubmit: "/bathos intensity <lite|full|ultra|off>" 파싱·상태 갱신
# =============================================================================
# 계약:  story-a5-intensity-toggle-kr.md, schema-extensions-kr.md §3
# 불변식(§5-6, LD-4): intensity ≠ Lv0~4. Lv=프로젝트/작업 규모(라우터,
#   manifest.current_level), intensity=세션 공격성(즉시 토글, session-flags.json).
#   이 훅은 plan_mode 필드를 절대 읽거나 쓰지 않는다(병합 기록 시 자연 보존됨 —
#   plan_mode 자체를 조회/판단에 사용하지 않음으로써 혼선 방지 불변식 유지).
# 안전:  advisory 전용(도구 차단 없음) — 항상 exit 0.
# DoD:   유효 레벨 → 기록+확인 문구(Lv 라우팅과 별개 명시, AC1/AC3).
#        레벨 누락 → 도움말(현재값+4레벨 설명), 변경 없음(AC4).
#        알 수 없는 값 → 에러+이전값 유지(AC4).
#        쓰기 실패 → 경고+세션 한정 임시 적용 표시(AC4, fail-safe).
# 이식 메모:
#   - jq 유무 분기 제거: 네이티브 JSON 병합 기록으로 plan_mode 등 다른 필드를
#     항상 보존한다(plan-toggle.ps1과 대칭 — bash의 "jq 없으면 최소 스키마로
#     덮어써 유실" 폴백은 이식하지 않음, 상위호환).
# =============================================================================

. (Join-Path $PSScriptRoot '_common.ps1')

$ctx = Get-BathosContext
$sessionFlagsPath = Join-Path $ctx.StateDir 'session-flags.json'

$ValidLevels = @('lite', 'full', 'ultra', 'off')

# --------------------------------------------------------------------------
# 1. stdin JSON 파싱: 사용자 프롬프트 원문 추출
# --------------------------------------------------------------------------
$parsed = Read-HookInput
$prompt = ''
if ($null -ne $parsed) {
    $p = Get-JsonField $parsed @('prompt'); if ($null -ne $p) { $prompt = [string]$p }
}
if ([string]::IsNullOrEmpty($prompt)) { exit 0 }

$ic = [System.Text.RegularExpressions.RegexOptions]::IgnoreCase

# 이 프롬프트에 /bathos intensity 커맨드 자체가 없으면 조용히 통과(advisory)
if (-not [regex]::IsMatch($prompt, '/bathos\s+intensity(\s|$)', $ic)) { exit 0 }

# 레벨 인자 추출(있으면). 없으면 빈 문자열 → 도움말 분기. 마지막 일치 사용.
$level = ''
$ms = [regex]::Matches($prompt, '/bathos\s+intensity\s*([a-zA-Z]*)', $ic)
if ($ms.Count -gt 0) {
    $level = $ms[$ms.Count - 1].Groups[1].Value.ToLowerInvariant()
}

# --------------------------------------------------------------------------
# 2. session-flags.json intensity 필드 읽기/쓰기 헬퍼 (plan-toggle.ps1과 대칭)
# --------------------------------------------------------------------------
function script:Set-NoteProp {
    param($Obj, [string]$Name, $Value)
    $Obj | Add-Member -NotePropertyName $Name -NotePropertyValue $Value -Force | Out-Null
}

function script:Read-Intensity {
    if (-not (Test-Path -LiteralPath $sessionFlagsPath)) { return 'full' }  # 기본값(schema-extensions §3)
    try {
        $obj = (Get-Content -LiteralPath $sessionFlagsPath -Raw) | ConvertFrom-Json
    } catch { return 'full' }
    if ($null -eq $obj) { return 'full' }
    $prop = $obj.PSObject.Properties['intensity']
    if ($null -eq $prop -or $null -eq $prop.Value -or ([string]$prop.Value) -eq '') { return 'full' }
    return [string]$prop.Value
}

function script:Write-Intensity {
    param([string]$Value)

    New-Item -ItemType Directory -Path $ctx.StateDir -Force -ErrorAction SilentlyContinue | Out-Null
    $now = Get-UtcIsoNow

    $obj = $null
    if (Test-Path -LiteralPath $sessionFlagsPath) {
        try { $obj = (Get-Content -LiteralPath $sessionFlagsPath -Raw) | ConvertFrom-Json } catch { $obj = $null }
    }
    if ($null -eq $obj) {
        $obj = [PSCustomObject]@{
            '$schema'  = 'bathos:session-flags'
            plan_mode  = 'off'
            intensity  = $Value
            updated    = $now
        }
    } else {
        # AC5(LD-4): plan_mode 필드는 건드리지 않는다 — intensity/updated만 갱신.
        Set-NoteProp $obj 'intensity' $Value
        Set-NoteProp $obj 'updated' $now
    }

    try {
        $json = $obj | ConvertTo-Json -Depth 20
        $tmp = [System.IO.Path]::GetTempFileName()
        $enc = New-Object System.Text.UTF8Encoding($false)
        [System.IO.File]::WriteAllText($tmp, $json, $enc)
        Move-Item -LiteralPath $tmp -Destination $sessionFlagsPath -Force
        return $true
    } catch {
        return $false
    }
}

$current = Read-Intensity

# --------------------------------------------------------------------------
# 3. 4가지 상태 분기 (ux-flow-map-kr.md Flow A 정본)
# --------------------------------------------------------------------------

# 3-a. 레벨 누락 → 도움말(현재값 + 4레벨 설명), 변경 없음
if ([string]::IsNullOrEmpty($level)) {
    $help = @(
        ('[bathos intensity] 현재 intensity=' + $current + ' (Lv 라우팅과 별개 — 변경 없음)')
        '[bathos intensity] 사용 가능 레벨:'
        '  lite  — 최소 개입(빠른 반복)'
        '  full  — 표준 심화 안전망(기본값)'
        '  ultra — 최대 심화(엄격 게이팅)'
        '  off   — 심화 안전망 전면 비활성(경고: 상시 주의 필요)'
        '[bathos intensity] 사용: /bathos intensity <lite|full|ultra|off>'
    ) -join "`n"
    Write-Output $help
    exit 0
}

# 3-b. 알 수 없는 값 → 에러 + 이전값 유지
if ($ValidLevels -notcontains $level) {
    Write-HookError ('[bathos intensity] ! 알 수 없는 값 "' + $level + '" — lite|full|ultra|off 중 하나여야 합니다. (이전값 유지: ' + $current + ')')
    exit 0
}

# 3-c/3-d. 유효 레벨 → 기록 시도
if (Write-Intensity -Value $level) {
    if ($level -eq 'off') {
        Write-Output '[bathos intensity] ! intensity=off 적용 (Lv 라우팅과 별개) — 심화 안전망이 전면 비활성 상태입니다. 상시 주의하세요.'
    } else {
        Write-Output ('[bathos intensity] . intensity=' + $level + ' 적용 (Lv 라우팅과 별개)')
    }
    $actor = $env:BATHOS_ROLE; if ([string]::IsNullOrEmpty($actor)) { $actor = 'hook:intensity-tracker' }
    Add-BathosAudit -Context $ctx -Action 'intensity.set' -Target $level -Actor $actor
} else {
    # 쓰기 실패 → 경고 + "세션 한정 임시 적용" 안내(fail-safe, AC4).
    # 정직성 고지: 훅은 프로세스 간 상태를 공유하지 않으므로 "임시 적용"은 이번
    # 확인 응답에 한정된 표시일 뿐, 다음 훅 호출부터는 디스크 값(미기록이므로
    # 이전값)이 다시 적용된다. 완전한 세션 메모리는 이 아키텍처의 알려진 한계다.
    Write-HookError ('[bathos intensity] ! intensity=' + $level + ' 기록 실패(파일 쓰기 오류) — 이번 응답에 한해 임시 적용 표시, 지속 저장은 실패했습니다. STATE_DIR 쓰기 권한을 확인하세요.')
}

exit 0
