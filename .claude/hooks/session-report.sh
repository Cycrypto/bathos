#!/usr/bin/env bash
# ---------------------------------------------------------------------------
# BATHOS session-report.sh  —  SessionEnd 훅
# 세션 종료(특히 /exit) 시 이 세션의 작업 사항을 최종 보고서 HTML로 생성·저장한다.
# 파일명 규약: <YYYY-MM-DD>-taskreport-<N>.html  (그날의 순번 N = 기존 최대+1)
# 저장 위치: <project>/.agent-team/12-report/
# 원천: _state/{SESSION-SNAPSHOT.md, wave-log.md, manifest.json} (모델이 세션 중 갱신).
# 철칙: fail-safe — 어떤 경우에도 종료를 막지 않으며 항상 exit 0. 날조 금지(디스크 사실만).
# ---------------------------------------------------------------------------
set +e

PROJ="${CLAUDE_PROJECT_DIR:-$(pwd)}"
STATE="$PROJ/.agent-team/_state"
OUTDIR="$PROJ/.agent-team/12-report"

# BATHOS 프로젝트가 아니면(=_state 없음) 조용히 통과
[ -d "$STATE" ] || exit 0

mkdir -p "$OUTDIR" 2>/dev/null

DATE="$(date +%Y-%m-%d)"
N=1
while [ -e "$OUTDIR/${DATE}-taskreport-${N}.html" ]; do N=$((N+1)); done
FILE="$OUTDIR/${DATE}-taskreport-${N}.html"
TS="$(date '+%Y-%m-%d %H:%M:%S %Z')"

# HTML 이스케이프(& < >)
esc(){ sed -e 's/&/\&amp;/g' -e 's/</\&lt;/g' -e 's/>/\&gt;/g'; }

# ── 원천 추출 (없으면 빈 문자열) ─────────────────────────────────────────────
CURSTATE="$(awk '/^## ★/{f=1;next} f&&/^---[[:space:]]*$/{exit} f' "$STATE/SESSION-SNAPSHOT.md" 2>/dev/null | esc)"
[ -z "$CURSTATE" ] && CURSTATE="(SESSION-SNAPSHOT.md의 '★ 현재 상태' 없음)"

WLOG="$(tail -n 80 "$STATE/wave-log.md" 2>/dev/null | esc)"
[ -z "$WLOG" ] && WLOG="(wave-log.md 없음)"

MANI="$(grep -oE '"status"[[:space:]]*:[[:space:]]*"[^"]*"' "$STATE/manifest.json" 2>/dev/null | tail -1 | esc)"
[ -z "$MANI" ] && MANI="(manifest status 없음)"

# 프로젝트 식별자는 manifest.json에서 읽는다(하드코딩 금지 — 다른 프로젝트로 복사돼도 정확).
PROJID="$(grep -oE '"project_id"[[:space:]]*:[[:space:]]*"[^"]*"' "$STATE/manifest.json" 2>/dev/null | head -1 | sed -E 's/.*"([^"]+)"$/\1/' | esc)"
[ -z "$PROJID" ] && PROJID="bathos"

# git 상태 (repo면)
if git -C "$PROJ" rev-parse --is-inside-work-tree >/dev/null 2>&1; then
  GIT="$(git -C "$PROJ" status --porcelain 2>/dev/null | wc -l | tr -d ' ') changed · $(git -C "$PROJ" log -1 --oneline 2>/dev/null | esc)"
else
  GIT="git repo 아님(파일 직접 저장)"
fi

# .agent-team 산출물 인벤토리
INV="$(cd "$PROJ/.agent-team" 2>/dev/null && for d in */; do n=$(find "$d" -type f 2>/dev/null | grep -v '\.orig$' | wc -l | tr -d ' '); [ "$n" -gt 0 ] && printf '%s (%s)  ' "${d%/}" "$n"; done)"
[ -z "$INV" ] && INV="(인벤토리 없음)"

# ── 리포트 작성 ──────────────────────────────────────────────────────────────
cat > "$FILE" <<HTMLEOF
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
  <div class="meta">생성 <b>${TS}</b> · 프로젝트 <b>${PROJID}</b> · manifest ${MANI}</div>

  <h2><span class="n">1</span>현재 상태 (세션 종료 시점)</h2>
  <div class="rule"></div>
  <div class="card state"><pre>${CURSTATE}</pre></div>

  <h2><span class="n">2</span>최근 작업 로그 (wave-log 말미)</h2>
  <div class="rule"></div>
  <div class="card"><pre>${WLOG}</pre></div>

  <h2><span class="n">3</span>산출물 · 환경</h2>
  <div class="rule"></div>
  <div class="card">
    <div class="kv"><b>산출물 인벤토리:</b> ${INV}</div>
    <div class="kv" style="margin-top:8px"><b>git:</b> ${GIT}</div>
  </div>

  <footer>
    <span>BATHOS · βάθος — 표층이 아닌 깊이 · SessionEnd 훅 자동 산출</span>
    <span>${DATE}-taskreport-${N}.html</span>
  </footer>
</div>
</body>
</html>
HTMLEOF

# 감사 기록은 반드시 CLI(bathos audit append)를 경유한다 — 직접 printf는 hash_self 누락으로
# verify_chain을 항상 깨뜨린다(B-1 회귀). BIN 미해결이면 조용히 생략(fail-safe, 비차단).
BIN="${BATHOS_BIN:-}"
[ -z "$BIN" ] && command -v bathos >/dev/null 2>&1 && BIN="$(command -v bathos)"
[ -z "$BIN" ] && [ -x "$PROJ/core/target/release/bathos" ] && BIN="$PROJ/core/target/release/bathos"
[ -z "$BIN" ] && [ -x "$PROJ/core/target/debug/bathos" ] && BIN="$PROJ/core/target/debug/bathos"
[ -n "$BIN" ] && "$BIN" -s "$STATE" audit append --actor hook --action session.report --target "${DATE}-taskreport-${N}.html" >/dev/null 2>&1

exit 0
