# ---------------------------------------------------------------------------
# BATHOS session-report.ps1  —  SessionEnd 훅  (Windows PowerShell 포트)
# 원본: .claude/hooks/session-report.sh
# 세션 종료(특히 /exit) 시 이 세션의 작업 사항을 최종 보고서 HTML로 생성·저장한다.
# 파일명 규약: <YYYY-MM-DD>-taskreport-<N>.html  (그날의 순번 N = 기존 최대+1)
# 저장 위치: <project>/.agent-team/12-report/
# 원천: _state/{SESSION-SNAPSHOT.md, wave-log.md, manifest.json} (모델이 세션 중 갱신).
# 철칙: fail-safe — 어떤 경우에도 종료를 막지 않으며 항상 exit 0. 날조 금지(디스크 사실만).
# 이식 메모:
#   - self-contained(_common.ps1 dot-source 안 함) — SessionEnd 견고성 최우선.
#   - 전체를 try/catch로 감싸 어떤 예외에도 exit 0 보장(bash `set +e` 등가).
#   - DATE/TS는 원본과 동일하게 로컬 시간(UTC 아님). PROJ 해석도 원본 방식
#     (CLAUDE_PROJECT_DIR 우선, 없으면 현재 디렉터리)을 그대로 미러.
#   - 감사 append는 원본과 동일하게 actor="hook", `-s <STATE>` 단축 플래그로 호출.
# ---------------------------------------------------------------------------
$ErrorActionPreference = 'Continue'

try {
    $PROJ = $env:CLAUDE_PROJECT_DIR
    if ([string]::IsNullOrEmpty($PROJ)) { $PROJ = (Get-Location).Path }
    $STATE  = Join-Path $PROJ '.agent-team\_state'
    $OUTDIR = Join-Path $PROJ '.agent-team\12-report'

    # BATHOS 프로젝트가 아니면(=_state 없음) 조용히 통과
    if (-not (Test-Path -LiteralPath $STATE -PathType Container)) { exit 0 }

    New-Item -ItemType Directory -Path $OUTDIR -Force -ErrorAction SilentlyContinue | Out-Null

    $DATE = (Get-Date).ToString('yyyy-MM-dd')
    $N = 1
    while (Test-Path -LiteralPath (Join-Path $OUTDIR ("$DATE-taskreport-$N.html"))) { $N++ }
    $FILE = Join-Path $OUTDIR ("$DATE-taskreport-$N.html")
    $tzAbbr = [System.TimeZoneInfo]::Local.Id
    $TS = (Get-Date).ToString('yyyy-MM-dd HH:mm:ss') + ' ' + $tzAbbr

    # HTML 이스케이프(& < >) — 원본 sed 순서(&부터) 보존
    function Esc([AllowEmptyString()][string]$s) {
        if ([string]::IsNullOrEmpty($s)) { return '' }
        return $s.Replace('&', '&amp;').Replace('<', '&lt;').Replace('>', '&gt;')
    }

    # ── 원천 추출 (없으면 빈 문자열) ──────────────────────────────────────────
    # CURSTATE: SESSION-SNAPSHOT.md의 "## ★" 다음 줄부터 "---" 이전까지(양끝 제외).
    $snap = Join-Path $STATE 'SESSION-SNAPSHOT.md'
    $curstate = ''
    if (Test-Path -LiteralPath $snap) {
        $collecting = $false
        $buf = New-Object System.Collections.Generic.List[string]
        foreach ($ln in (Get-Content -LiteralPath $snap -ErrorAction SilentlyContinue)) {
            if (-not $collecting) {
                if ($ln -match '^## ★') { $collecting = $true }
                continue
            }
            if ($ln -match '^---\s*$') { break }
            $buf.Add($ln)
        }
        $curstate = ($buf -join "`n")
    }
    $curstate = Esc $curstate
    if ([string]::IsNullOrEmpty($curstate)) { $curstate = "(SESSION-SNAPSHOT.md의 '★ 현재 상태' 없음)" }

    # WLOG: wave-log.md 말미 80줄
    $wlogPath = Join-Path $STATE 'wave-log.md'
    $wlog = ''
    if (Test-Path -LiteralPath $wlogPath) {
        $wlog = ((Get-Content -LiteralPath $wlogPath -Tail 80 -ErrorAction SilentlyContinue) -join "`n")
    }
    $wlog = Esc $wlog
    if ([string]::IsNullOrEmpty($wlog)) { $wlog = '(wave-log.md 없음)' }

    # manifest.json에서 status(마지막), project_id(첫번째)
    $manifest = Join-Path $STATE 'manifest.json'
    $mtext = ''
    if (Test-Path -LiteralPath $manifest) {
        try { $mtext = Get-Content -LiteralPath $manifest -Raw -ErrorAction Stop } catch { $mtext = '' }
    }
    $mani = ''
    if (-not [string]::IsNullOrEmpty($mtext)) {
        $mm = [regex]::Matches($mtext, '"status"\s*:\s*"[^"]*"')
        if ($mm.Count -gt 0) { $mani = $mm[$mm.Count - 1].Value }
    }
    $mani = Esc $mani
    if ([string]::IsNullOrEmpty($mani)) { $mani = '(manifest status 없음)' }

    # 프로젝트 식별자는 manifest.json에서 읽는다(하드코딩 금지).
    $projid = ''
    if (-not [string]::IsNullOrEmpty($mtext)) {
        $pm = [regex]::Match($mtext, '"project_id"\s*:\s*"([^"]*)"')
        if ($pm.Success) { $projid = $pm.Groups[1].Value }
    }
    $projid = Esc $projid
    if ([string]::IsNullOrEmpty($projid)) { $projid = 'bathos' }

    # git 상태 (repo면)
    $git = ''
    $isRepo = $false
    try {
        & git -C $PROJ rev-parse --is-inside-work-tree 2>$null | Out-Null
        if ($LASTEXITCODE -eq 0) { $isRepo = $true }
    } catch { $isRepo = $false }
    if ($isRepo) {
        $changed = @(& git -C $PROJ status --porcelain 2>$null).Count
        $lastCommit = ((& git -C $PROJ log -1 --oneline 2>$null) -join ' ').Trim()
        $git = ([string]$changed) + ' changed · ' + (Esc $lastCommit)
    } else {
        $git = 'git repo 아님(파일 직접 저장)'
    }

    # .agent-team 산출물 인벤토리
    $inv = ''
    $atDir = Join-Path $PROJ '.agent-team'
    if (Test-Path -LiteralPath $atDir -PathType Container) {
        foreach ($d in (Get-ChildItem -LiteralPath $atDir -Directory -ErrorAction SilentlyContinue)) {
            $cnt = @(Get-ChildItem -LiteralPath $d.FullName -Recurse -File -ErrorAction SilentlyContinue |
                Where-Object { $_.Name -notlike '*.orig' }).Count
            if ($cnt -gt 0) { $inv += ($d.Name + ' (' + $cnt + ')  ') }
        }
    }
    if ([string]::IsNullOrEmpty($inv)) { $inv = '(인벤토리 없음)' }

    # ── 리포트 작성 ────────────────────────────────────────────────────────────
    # PowerShell 이중따옴표 here-string은 ${VAR} 를 보간한다(원본 bash 템플릿과 동일 표기).
    # CSS 의 { } 는 보간 대상이 아니며($ 없음) 그대로 출력된다.
    $html = @"
<!doctype html>
<html lang="ko">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>BATHOS 작업 리포트 — ${DATE} #${N}</title>
<style>
  :root{--abyss:#081215;--surface:#0f2226;--line:#1c3a40;--teal:#18b6c0;--teal-brand:#0e9aa1;--teal-soft:#7fd6dc;--ink:#e9f3f4;--muted:#8fabb0;--dim:#5f7c81;--grn:#4fbf9a;
    --mono:ui-monospace,'SFMono-Regular','JetBrains Mono',Menlo,Consolas,monospace;
    --sans:'Pretendard','Pretendard Variable',-apple-system,BlinkMacSystemFont,'Apple SD Gothic Neo','Noto Sans KR',system-ui,sans-serif;}
  *{box-sizing:border-box}
  body{margin:0;font-family:var(--sans);color:var(--ink);line-height:1.6;
    background:radial-gradient(1000px 560px at 82% -10%,rgba(14,154,161,.16),transparent 60%),var(--abyss);-webkit-font-smoothing:antialiased}
  .wrap{max-width:900px;margin:0 auto;padding:52px 26px 72px}
  .eyebrow{font-family:var(--mono);font-size:.76rem;letter-spacing:.16em;text-transform:uppercase;color:var(--teal)}
  h1{font-size:clamp(1.8rem,4vw,2.6rem);margin:12px 0 8px;font-weight:800;letter-spacing:-.01em}
  .meta{font-family:var(--mono);color:var(--muted);font-size:.88rem}
  .meta b{color:var(--teal-soft)}
  h2{font-size:1.2rem;margin:38px 0 4px;font-weight:800}
  h2 .n{font-family:var(--mono);color:var(--teal);margin-right:.5em;font-size:.88em}
  .rule{height:1px;background:linear-gradient(90deg,var(--teal-brand),transparent);margin:9px 0 14px;opacity:.6}
  .card{background:var(--surface);border:1px solid var(--line);border-radius:12px;padding:16px 18px;color:var(--ink)}
  .card.state{border-left:3px solid var(--teal-brand)}
  pre{font-family:var(--mono);font-size:.84rem;line-height:1.7;color:#cfe9ec;white-space:pre-wrap;word-break:break-word;margin:0}
  .kv{font-family:var(--mono);font-size:.9rem;color:var(--muted)}
  .kv b{color:#fff}
  footer{margin-top:46px;padding-top:16px;border-top:1px solid var(--line);color:var(--dim);font-family:var(--mono);font-size:.8rem;display:flex;justify-content:space-between;flex-wrap:wrap;gap:8px}
  .auto{display:inline-block;font-family:var(--mono);font-size:.72rem;color:var(--grn);border:1px solid rgba(79,191,154,.4);border-radius:999px;padding:2px 10px;margin-left:8px}
</style>
</head>
<body>
<div class="wrap">
  <div class="eyebrow">BATHOS Dynamis · 작업 리포트<span class="auto">SessionEnd 자동 생성</span></div>
  <h1>${DATE} · 작업 리포트 #${N}</h1>
  <div class="meta">생성 <b>${TS}</b> · 프로젝트 <b>${projid}</b> · manifest ${mani}</div>

  <h2><span class="n">1</span>현재 상태 (세션 종료 시점)</h2>
  <div class="rule"></div>
  <div class="card state"><pre>${curstate}</pre></div>

  <h2><span class="n">2</span>최근 작업 로그 (wave-log 말미)</h2>
  <div class="rule"></div>
  <div class="card"><pre>${wlog}</pre></div>

  <h2><span class="n">3</span>산출물 · 환경</h2>
  <div class="rule"></div>
  <div class="card">
    <div class="kv"><b>산출물 인벤토리:</b> ${inv}</div>
    <div class="kv" style="margin-top:8px"><b>git:</b> ${git}</div>
  </div>

  <footer>
    <span>BATHOS · βάθος — 표층이 아닌 깊이 · SessionEnd 훅 자동 산출</span>
    <span>${DATE}-taskreport-${N}.html</span>
  </footer>
</div>
</body>
</html>
"@

    $enc = New-Object System.Text.UTF8Encoding($false)
    [System.IO.File]::WriteAllText($FILE, $html, $enc)

    # 감사 기록은 반드시 CLI(bathos audit append)를 경유한다(B-1 회귀 방지).
    # BIN 미해결이면 조용히 생략(fail-safe, 비차단).
    $BIN = $env:BATHOS_BIN
    $exe = 'bathos'; if ($env:OS -eq 'Windows_NT') { $exe = 'bathos.exe' }
    if ([string]::IsNullOrEmpty($BIN)) {
        $cmd = Get-Command $exe -ErrorAction SilentlyContinue
        if ($null -ne $cmd) { $BIN = $cmd.Source }
    }
    if ([string]::IsNullOrEmpty($BIN)) {
        $rel = Join-Path $PROJ ('core\target\release\' + $exe)
        if (Test-Path -LiteralPath $rel) { $BIN = $rel }
    }
    if ([string]::IsNullOrEmpty($BIN)) {
        $dbg = Join-Path $PROJ ('core\target\debug\' + $exe)
        if (Test-Path -LiteralPath $dbg) { $BIN = $dbg }
    }
    if (-not [string]::IsNullOrEmpty($BIN) -and (Test-Path -LiteralPath $BIN)) {
        try {
            & $BIN -s $STATE audit append --actor hook --action session.report --target ("$DATE-taskreport-$N.html") 2>$null | Out-Null
        } catch { }
    }
} catch {
    # fail-safe: 어떤 예외에도 종료를 막지 않는다
}

exit 0
