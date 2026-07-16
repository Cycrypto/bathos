#!/usr/bin/env bash
# ---------------------------------------------------------------------------
# BATHOS  result_report/generate-task-report.sh
# 한 세션의 작업 사항을 self-contained HTML 리포트로 저장한다.
#
# 파일명 규약 : task_report_<YYYYMMDD>_<HHMMSS>_session_no<NN>.html
#   - <YYYYMMDD>_<HHMMSS> = 생성(=작업 종료) 시각
#   - <NN>                = 세션 순번(첫 세션부터 incremental, 2자리 zero-pad)
#                           result_report/ 안의 기존 리포트 최대 순번 + 1로 자동 결정
# 저장 위치   : <project>/result_report/
#
# 필수 포함 6항목:
#   1) 작업 시작 시각  2) 작업 종료 시각  3) 총 작업 시간
#   4) 작업 핵심 사항  5) 중요 이슈       6) git commit/push/PR/merge 히스토리
#
# 서술형 항목(시작시각·핵심사항·이슈)은 콘텐츠 파일에서 읽는다(모델/작성자가 갱신).
#   기본 경로: result_report/.session-content.md  (또는 1번째 인자로 경로 지정)
#   구획: <!-- START -->..<!-- /START -->  <!-- SUMMARY -->..  <!-- ISSUES -->..
# git 히스토리(6번)는 저장소에서 자동 수집한다.
#
# 철칙: 날조 금지(디스크·git 사실만). start 시각 미기재 시 "(미기재)"로 표기.
# ---------------------------------------------------------------------------
set +e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJ="$(cd "$SCRIPT_DIR/.." && pwd)"
OUTDIR="$SCRIPT_DIR"
CONTENT="${1:-$SCRIPT_DIR/.session-content.md}"

# ── 세션 순번 자동 증가 (기존 리포트 최대 + 1) ────────────────────────────────
N=0
for f in "$OUTDIR"/task_report_*_session_no*.html; do
  [ -e "$f" ] || continue
  num="$(printf '%s' "$f" | sed -E 's/.*_session_no([0-9]+)\.html$/\1/')"
  case "$num" in ''|*[!0-9]*) continue;; esac
  num=$((10#$num))
  [ "$num" -gt "$N" ] && N=$num
done
N=$((N + 1))
NN="$(printf '%02d' "$N")"

# ── 시각 ─────────────────────────────────────────────────────────────────────
END_TS="$(date '+%Y-%m-%d %H:%M:%S %Z')"
STAMP="$(date '+%Y%m%d_%H%M%S')"
FILE="$OUTDIR/task_report_${STAMP}_session_no${NN}.html"

# ── HTML 이스케이프 & 초경량 markdown → HTML ─────────────────────────────────
esc(){ sed -e 's/&/\&amp;/g' -e 's/</\&lt;/g' -e 's/>/\&gt;/g'; }
# 콘텐츠 파일에서 <!-- TAG -->..<!-- /TAG --> 구간 추출
extract(){ awk -v t="$1" '$0 ~ ("<!-- "t" -->"){f=1;next} $0 ~ ("<!-- /"t" -->"){f=0} f' "$CONTENT" 2>/dev/null; }
# 불릿(- )·굵게(**..**)만 지원하는 경량 렌더러(먼저 esc → 태그 주입 → 목록 묶음)
md2html(){
  esc | sed -E 's/\*\*([^*]+)\*\*/<b>\1<\/b>/g' | awk '
    /^[[:space:]]*-[[:space:]]/ { if(!u){print "<ul>";u=1} sub(/^[[:space:]]*-[[:space:]]/,""); print "<li>"$0"</li>"; next }
    { if(u){print "</ul>";u=0} if($0 ~ /^[[:space:]]*$/) next; print "<p>"$0"</p>" }
    END{ if(u) print "</ul>" }'
}

# ── 서술형 콘텐츠 로드 ────────────────────────────────────────────────────────
# 작업 시작 시각 우선순위: (1) SessionStart 훅이 남긴 마커(정확·자동) →
#                          (2) .session-content.md 의 START 구획(수동/폴백) → (3) 미기재
MARKER="$PROJ/.agent-team/_state/session-start.marker"
START_RAW=""
[ -s "$MARKER" ] && START_RAW="$(sed '/^[[:space:]]*$/d' "$MARKER" | head -1)"
[ -z "$START_RAW" ] && START_RAW="$(extract START | sed '/^[[:space:]]*$/d' | head -1)"
[ -z "$START_RAW" ] && START_RAW="(미기재)"
SUMMARY_HTML="$(extract SUMMARY | md2html)"
[ -z "$SUMMARY_HTML" ] && SUMMARY_HTML="<p>(핵심 사항 미기재 — result_report/.session-content.md의 SUMMARY 구획을 채우세요)</p>"
ISSUES_HTML="$(extract ISSUES | md2html)"
[ -z "$ISSUES_HTML" ] && ISSUES_HTML="<p>(중요 이슈 없음/미기재)</p>"

# ── 총 작업 시간 계산 (GNU date -d / BSD date -j 모두 대응) ───────────────────
to_epoch(){
  local s; s="$(printf '%s' "$1" | sed -E 's/[[:space:]]*[A-Za-z]{2,4}$//')" # 끝의 tz명 제거
  date -d "$s" +%s 2>/dev/null || date -j -f "%Y-%m-%d %H:%M:%S" "$s" +%s 2>/dev/null
}
DUR="(계산 불가 — 시작 시각 미기재)"
if [ "$START_RAW" != "(미기재)" ]; then
  se="$(to_epoch "$START_RAW")"; ee="$(to_epoch "$END_TS")"
  if [ -n "$se" ] && [ -n "$ee" ] && [ "$ee" -ge "$se" ] 2>/dev/null; then
    d=$((ee - se)); DUR="$((d/3600))시간 $(((d%3600)/60))분 $((d%60))초"
  fi
fi

# ── 프로젝트 식별자(있으면 manifest에서) ─────────────────────────────────────
PROJID="$(grep -oE '"project_id"[[:space:]]*:[[:space:]]*"[^"]*"' "$PROJ/.agent-team/_state/manifest.json" 2>/dev/null | head -1 | sed -E 's/.*"([^"]+)"$/\1/')"
[ -z "$PROJID" ] && PROJID="$(basename "$PROJ")"

# ── 6) git commit/push/PR/merge 히스토리 자동 수집 ───────────────────────────
if git -C "$PROJ" rev-parse --is-inside-work-tree >/dev/null 2>&1; then
  BR="$(git -C "$PROJ" branch --show-current 2>/dev/null)"
  SYNC="$(git -C "$PROJ" status -sb 2>/dev/null | head -1 | sed 's/^## //')"
  COMMITS="$(git -C "$PROJ" log --oneline --decorate -15 2>/dev/null | esc)"
  MERGES="$(git -C "$PROJ" log --merges --oneline -10 2>/dev/null | esc)"
  [ -z "$MERGES" ] && MERGES="(merge 커밋 없음)"
  # push 상태: 로컬 브랜치가 upstream 대비 ahead/behind인지 (=미push 여부의 프록시)
  PUSH="$(git -C "$PROJ" for-each-ref --format='%(refname:short) → %(upstream:short) [%(upstream:track)]' refs/heads 2>/dev/null | esc)"
  [ -z "$PUSH" ] && PUSH="(upstream 추적 브랜치 없음)"
  if command -v gh >/dev/null 2>&1; then
    PRS="$(gh pr list --repo "$(git -C "$PROJ" remote get-url origin 2>/dev/null)" --state all --limit 15 \
            --json number,state,title,headRefName,baseRefName \
            --template '{{range .}}#{{.number}} [{{.state}}] {{.headRefName}}→{{.baseRefName}} · {{.title}}{{"\n"}}{{end}}' 2>/dev/null | esc)"
    [ -z "$PRS" ] && PRS="(PR 없음 또는 gh 조회 실패)"
  else
    PRS="(gh CLI 미설치 — PR 히스토리 수집 생략)"
  fi
else
  BR="(git repo 아님)"; SYNC="-"; COMMITS="(git repo 아님)"; MERGES="-"; PUSH="-"; PRS="-"
fi

# ── 리포트 렌더 ──────────────────────────────────────────────────────────────
cat > "$FILE" <<HTMLEOF
<!doctype html>
<html lang="ko">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>BATHOS 작업 리포트 — session_no${NN}</title>
<style>
  :root{--abyss:#081215;--surface:#0f2226;--line:#1c3a40;--teal:#18b6c0;--teal-brand:#0e9aa1;--teal-soft:#7fd6dc;--ink:#e9f3f4;--muted:#8fabb0;--dim:#5f7c81;--grn:#4fbf9a;
    --mono:ui-monospace,'SFMono-Regular','JetBrains Mono',Menlo,Consolas,monospace;
    --sans:'Pretendard','Pretendard Variable',-apple-system,BlinkMacSystemFont,'Apple SD Gothic Neo','Noto Sans KR',system-ui,sans-serif;}
  *{box-sizing:border-box}
  body{margin:0;font-family:var(--sans);color:var(--ink);line-height:1.65;
    background:radial-gradient(1000px 560px at 82% -10%,rgba(14,154,161,.16),transparent 60%),var(--abyss);-webkit-font-smoothing:antialiased}
  .wrap{max-width:920px;margin:0 auto;padding:52px 26px 72px}
  .eyebrow{font-family:var(--mono);font-size:.76rem;letter-spacing:.16em;text-transform:uppercase;color:var(--teal)}
  h1{font-size:clamp(1.8rem,4vw,2.6rem);margin:12px 0 8px;font-weight:800;letter-spacing:-.01em}
  .meta{font-family:var(--mono);color:var(--muted);font-size:.88rem}
  .meta b{color:var(--teal-soft)}
  h2{font-size:1.2rem;margin:38px 0 4px;font-weight:800}
  h2 .n{font-family:var(--mono);color:var(--teal);margin-right:.5em;font-size:.88em}
  .rule{height:1px;background:linear-gradient(90deg,var(--teal-brand),transparent);margin:9px 0 14px;opacity:.6}
  .card{background:var(--surface);border:1px solid var(--line);border-radius:12px;padding:16px 20px;color:var(--ink)}
  .card.state{border-left:3px solid var(--teal-brand)}
  .card p{margin:.35em 0}.card ul{margin:.35em 0 .35em 0;padding-left:1.25em}.card li{margin:.28em 0}
  .card b{color:#fff}
  pre{font-family:var(--mono);font-size:.82rem;line-height:1.7;color:#cfe9ec;white-space:pre-wrap;word-break:break-word;margin:0;overflow-x:auto}
  .times{display:grid;grid-template-columns:repeat(3,1fr);gap:12px;margin-top:4px}
  @media(max-width:640px){.times{grid-template-columns:1fr}}
  .tcard{background:var(--surface);border:1px solid var(--line);border-radius:12px;padding:14px 16px}
  .tcard .lbl{font-family:var(--mono);font-size:.72rem;letter-spacing:.1em;text-transform:uppercase;color:var(--teal)}
  .tcard .val{font-family:var(--mono);font-size:1.02rem;color:#fff;margin-top:6px;font-weight:700}
  .subh{font-family:var(--mono);font-size:.78rem;color:var(--teal-soft);margin:14px 0 6px;letter-spacing:.04em}
  footer{margin-top:46px;padding-top:16px;border-top:1px solid var(--line);color:var(--dim);font-family:var(--mono);font-size:.8rem;display:flex;justify-content:space-between;flex-wrap:wrap;gap:8px}
</style>
</head>
<body>
<div class="wrap">
  <div class="eyebrow">BATHOS Dynamis · 세션 작업 리포트</div>
  <h1>작업 리포트 · session_no${NN}</h1>
  <div class="meta">생성 <b>${END_TS}</b> · 프로젝트 <b>${PROJID}</b> · 세션 순번 <b>#${N}</b></div>

  <h2><span class="n">1–3</span>작업 시각 &amp; 총 소요</h2>
  <div class="rule"></div>
  <div class="times">
    <div class="tcard"><div class="lbl">작업 시작</div><div class="val">${START_RAW}</div></div>
    <div class="tcard"><div class="lbl">작업 종료</div><div class="val">${END_TS}</div></div>
    <div class="tcard"><div class="lbl">총 작업 시간</div><div class="val">${DUR}</div></div>
  </div>

  <h2><span class="n">4</span>작업 핵심 사항</h2>
  <div class="rule"></div>
  <div class="card state">${SUMMARY_HTML}</div>

  <h2><span class="n">5</span>중요 이슈</h2>
  <div class="rule"></div>
  <div class="card">${ISSUES_HTML}</div>

  <h2><span class="n">6</span>git commit / push / PR / merge 히스토리</h2>
  <div class="rule"></div>
  <div class="card">
    <div class="subh">현재 브랜치 · 동기화</div>
    <pre>${BR}   ·   ${SYNC}</pre>
    <div class="subh" style="margin-top:14px">최근 커밋 (commit, 최신 15)</div>
    <pre>${COMMITS}</pre>
    <div class="subh" style="margin-top:14px">merge 히스토리 (최신 10)</div>
    <pre>${MERGES}</pre>
    <div class="subh" style="margin-top:14px">push 상태 (로컬 → upstream [track])</div>
    <pre>${PUSH}</pre>
    <div class="subh" style="margin-top:14px">Pull Request (전체 상태)</div>
    <pre>${PRS}</pre>
  </div>

  <footer>
    <span>BATHOS · βάθος — 표층이 아닌 깊이</span>
    <span>task_report_${STAMP}_session_no${NN}.html</span>
  </footer>
</div>
</body>
</html>
HTMLEOF

echo "생성됨: $FILE"

# 감사 기록(있으면) — 직접 append 금지, CLI 경유(hash_self 보존)
BIN="${BATHOS_BIN:-}"
[ -z "$BIN" ] && command -v bathos >/dev/null 2>&1 && BIN="$(command -v bathos)"
[ -z "$BIN" ] && [ -x "$PROJ/core/target/release/bathos" ] && BIN="$PROJ/core/target/release/bathos"
[ -n "$BIN" ] && [ -d "$PROJ/.agent-team/_state" ] && \
  "$BIN" -s "$PROJ/.agent-team/_state" audit append --actor hook --action taskreport.generate --target "$(basename "$FILE")" >/dev/null 2>&1

exit 0
