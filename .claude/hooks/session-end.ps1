# =============================================================================
# BATHOS session-end.ps1   (Windows PowerShell 포트)
# 원본: .claude/hooks/session-end.sh
# SessionEnd 훅 오케스트레이터 — 세션 종료(특히 /exit) 시 정해진 순서를 강제한다:
#   [1] 세션 작업 저장(mechanical save)  →  [2] 작업 리포트 생성  →  [3] 종료 진행
# 내장 /exit는 모델 턴을 주지 않으므로 이 순서는 훅 스크립트에서 보장한다.
# (서술 스냅샷 '★ 현재 상태'의 최신 narrative는 세션 중 모델이 /save-session 등으로 갱신.)
# 철칙: fail-safe — 종료를 막지 않으며 항상 exit 0. 날조 금지(디스크 사실만).
# =============================================================================
# 이식 메모:
#   - PROJ 해석은 원본과 동일하게 CLAUDE_PROJECT_DIR 우선, 없으면 현재 작업
#     디렉터리(pwd)를 사용한다(Get-BathosContext의 "BathosRoot 폴백"과는 다름 —
#     이 훅만 원본 bash의 PROJ 해석을 그대로 재현).
#   - `bathos -s <STATE>` 단축 플래그(--state-dir 아님)를 원본 그대로 보존한다.
#   - bash `date`(로컬 시간, %Z)는 정확한 1:1 대응이 없어 UTC 오프셋(zzz)으로
#     근사한다 — 로그 타임스탬프 용도이므로 기능상 영향 없음(문서화된 근사).
#   - session-report.sh 대신 session-report.ps1을 경로로 참조한다(별도 포트 담당).
# =============================================================================

. (Join-Path $PSScriptRoot '_common.ps1')

try {
    $proj = $env:CLAUDE_PROJECT_DIR
    if ([string]::IsNullOrEmpty($proj)) { $proj = (Get-Location).Path }
    $state = Join-Path $proj '.agent-team/_state'

    # BATHOS 프로젝트가 아니면 조용히 통과
    if (-not (Test-Path -LiteralPath $state -PathType Container)) { exit 0 }

    $date = (Get-Date).ToString('yyyy-MM-dd')
    $ts   = (Get-Date).ToString('yyyy-MM-dd HH:mm:ss zzz')

    # ===== [1] 세션 작업 저장 (mechanical) ==================================
    # 1a. 서술 스냅샷을 날짜 아카이브로 보존(세션 중 모델이 갱신한 최신본을 고정)
    try {
        $snapshot = Join-Path $state 'SESSION-SNAPSHOT.md'
        if (Test-Path -LiteralPath $snapshot -PathType Leaf) {
            $archive = Join-Path $state ('SESSION-SNAPSHOT-' + $date + '.md')
            Copy-Item -LiteralPath $snapshot -Destination $archive -Force -ErrorAction SilentlyContinue
        }
    } catch { }

    # 1b. 엔진 상태 SSOT 덤프(엔진 스키마 프로젝트일 때만 성공; 운영용 manifest면 실패 → 정리)
    $bin = $env:BATHOS_BIN
    if ([string]::IsNullOrEmpty($bin)) {
        $cmdInfo = Get-Command 'bathos' -ErrorAction SilentlyContinue
        if ($null -ne $cmdInfo) { $bin = $cmdInfo.Source }
    }
    if ([string]::IsNullOrEmpty($bin)) {
        $exeName = 'bathos'
        if ($env:OS -eq 'Windows_NT') { $exeName = 'bathos.exe' }
        $releaseBin = Join-Path $proj ('core/target/release/' + $exeName)
        $debugBin   = Join-Path $proj ('core/target/debug/' + $exeName)
        if (Test-Path -LiteralPath $releaseBin) { $bin = $releaseBin }
        elseif (Test-Path -LiteralPath $debugBin) { $bin = $debugBin }
    }

    if (-not [string]::IsNullOrEmpty($bin) -and (Test-Path -LiteralPath $bin)) {
        $sessionStatePath = Join-Path $state 'session-state.json'
        try {
            $output = & $bin -s $state state show 2>$null
            if ($LASTEXITCODE -eq 0 -and $null -ne $output) {
                $text = ($output -join "`n")
                $enc = New-Object System.Text.UTF8Encoding($false)
                [System.IO.File]::WriteAllText($sessionStatePath, $text, $enc)
            } else {
                Remove-Item -LiteralPath $sessionStatePath -Force -ErrorAction SilentlyContinue
            }
        } catch {
            Remove-Item -LiteralPath $sessionStatePath -Force -ErrorAction SilentlyContinue
        }

        # 1c. 종료 스탬프(감사/추적용, 비차단)
        try {
            $closeLog = Join-Path $state 'session-close.log'
            Add-Content -LiteralPath $closeLog -Value ('session closed: ' + $ts) -ErrorAction SilentlyContinue
        } catch { }

        # 감사 기록은 반드시 CLI(bathos audit append)를 경유한다 — 직접 기록은 hash_self
        # 누락·genesis 형식 불일치로 verify_chain을 항상 깨뜨린다(B-1 회귀).
        try {
            & $bin -s $state audit append --actor hook --action session.save --target session-end 2>$null 1>$null
        } catch { }
    } else {
        # BIN 미해결이면 종료 스탬프만 시도(fail-safe, 종료 비차단)
        try {
            $closeLog = Join-Path $state 'session-close.log'
            Add-Content -LiteralPath $closeLog -Value ('session closed: ' + $ts) -ErrorAction SilentlyContinue
        } catch { }
    }

    # ===== [2] 작업 리포트 생성 =============================================
    # 저장이 끝난 뒤 리포트를 만든다(리포트는 위에서 고정된 _state를 읽는다).
    try {
        $reportScript = Join-Path $proj '.claude/hooks/session-report.ps1'
        if (Test-Path -LiteralPath $reportScript -PathType Leaf) {
            & $reportScript
        }
    } catch { }
} catch {
    # 최상위 fail-safe: 어떤 예외가 나도 세션 종료를 막지 않는다
} finally {
    # ===== [3] 종료 진행 ====================================================
    # (SessionEnd 훅은 종료를 막을 수 없다 — 여기서 반환하면 세션이 닫힌다.)
    exit 0
}
