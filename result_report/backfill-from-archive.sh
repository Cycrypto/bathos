#!/usr/bin/env bash
# ---------------------------------------------------------------------------
# BATHOS  result_report/backfill-from-archive.sh
# 과거 세션 리포트(.agent-team/12-report/*-taskreport-*.html)를 새 result_report/
# 형식으로 소급(backfill) 생성한다. 시간순으로 session_no 01 부터 채번한다.
# 이후 generate-task-report.sh 는 자동으로 그 다음 번호(예: 12)를 이어받는다.
#
# 소급 리포트의 정확도(날조 금지 — 원본에 있는 사실만):
#   - 작업 종료 시각 = 원본 리포트의 "생성" 시각(=세션 종료 시 자동 생성됐음)
#   - 작업 시작 시각 / 총 작업 시간 = (소급 — 당시 미기록) 로 명시
#   - 작업 핵심 사항 = 원본의 "현재 상태" 블록 verbatim(이미 이스케이프됨)
#   - 중요 이슈 = (소급 — 당시 별도 이슈 로그 없음) + 원본 파일명 포인터
#   - git 히스토리 = 원본 리포트에 기록된 git 라인(당시 상태) — 현재 git 로그 아님
#
# 사용: bash backfill-from-archive.sh          (기존 리포트 있으면 중단)
#       bash backfill-from-archive.sh --force  (강제 진행)
# ---------------------------------------------------------------------------
set +e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJ="$(cd "$SCRIPT_DIR/.." && pwd)"
OUTDIR="$SCRIPT_DIR"
ARCHDIR="$PROJ/.agent-team/12-report"

[ -d "$ARCHDIR" ] || { echo "아카이브 디렉토리 없음: $ARCHDIR"; exit 1; }

existing=$(ls "$OUTDIR"/task_report_*_session_no*.html 2>/dev/null | wc -l | tr -d ' ')
if [ "$existing" -gt 0 ] && [ "$1" != "--force" ]; then
  echo "result_report/ 에 이미 $existing 개 리포트 존재 → 중복 방지로 중단. 강제하려면 --force"; exit 1
fi

PROJID="$(grep -oE '"project_id"[[:space:]]*:[[:space:]]*"[^"]*"' "$PROJ/.agent-team/_state/manifest.json" 2>/dev/null | head -1 | sed -E 's/.*"([^"]+)"$/\1/')"
[ -z "$PROJID" ] && PROJID="$(basename "$PROJ")"

NN=0
for a in $(ls "$ARCHDIR"/*-taskreport-*.html 2>/dev/null | sort); do
  NN=$((NN + 1)); NNP="$(printf '%02d' "$NN")"

  END="$(grep -oE '생성 <b>[^<]*</b>' "$a" | head -1 | sed -E 's/생성 <b>([^<]*)<\/b>/\1/')"
  [ -z "$END" ] && END="(미상)"
  STAMP="$(printf '%s' "$END" | sed -E 's/^([0-9]{4})-([0-9]{2})-([0-9]{2}) ([0-9]{2}):([0-9]{2}):([0-9]{2}).*/\1\2\3_\4\5\6/')"
  case "$STAMP" in *[!0-9_]*|"") STAMP="unknown${NNP}";; esac

  # 원본 "현재 상태" 섹션의 <pre>..</pre> 내용을 verbatim 추출(이미 escape 되어 있음)
  SUMMARY="$(awk '/현재 상태/{a=1} a&&/<pre>/{p=1} p{print} p&&/<\/pre>/{exit}' "$a" \
             | sed -E 's/.*<pre>//; s/<\/pre>.*//')"
  [ -z "$SUMMARY" ] && SUMMARY="(원본 '현재 상태' 추출 실패 — 원본 참조)"

  GIT="$(grep -oE '<b>git:</b>[^<]*' "$a" | head -1 | sed -E 's/<b>git:<\/b>[[:space:]]*//')"
  [ -z "$GIT" ] && GIT="(원본 git 기록 없음)"

  ARCHNAME="$(basename "$a")"
  FILE="$OUTDIR/task_report_${STAMP}_session_no${NNP}.html"

  cat > "$FILE" <<HTMLEOF
<!doctype html>
<html lang="ko">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>BATHOS 작업 리포트 — session_no${NNP} (소급)</title>
<style>
  :root{--abyss:#081215;--surface:#0f2226;--line:#1c3a40;--teal:#18b6c0;--teal-brand:#0e9aa1;--teal-soft:#7fd6dc;--ink:#e9f3f4;--muted:#8fabb0;--dim:#5f7c81;--amber:#e0a94f;
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
  .badge{display:inline-block;font-family:var(--mono);font-size:.72rem;color:var(--amber);border:1px solid rgba(224,169,79,.45);border-radius:999px;padding:2px 10px;margin-left:8px}
  h2{font-size:1.2rem;margin:38px 0 4px;font-weight:800}
  h2 .n{font-family:var(--mono);color:var(--teal);margin-right:.5em;font-size:.88em}
  .rule{height:1px;background:linear-gradient(90deg,var(--teal-brand),transparent);margin:9px 0 14px;opacity:.6}
  .card{background:var(--surface);border:1px solid var(--line);border-radius:12px;padding:16px 20px;color:var(--ink)}
  .card.state{border-left:3px solid var(--teal-brand)}
  pre{font-family:var(--mono);font-size:.82rem;line-height:1.7;color:#cfe9ec;white-space:pre-wrap;word-break:break-word;margin:0;overflow-x:auto}
  .times{display:grid;grid-template-columns:repeat(3,1fr);gap:12px;margin-top:4px}
  @media(max-width:640px){.times{grid-template-columns:1fr}}
  .tcard{background:var(--surface);border:1px solid var(--line);border-radius:12px;padding:14px 16px}
  .tcard .lbl{font-family:var(--mono);font-size:.72rem;letter-spacing:.1em;text-transform:uppercase;color:var(--teal)}
  .tcard .val{font-family:var(--mono);font-size:1.02rem;color:#fff;margin-top:6px;font-weight:700}
  .tcard .val.dim{color:var(--dim);font-weight:500;font-size:.86rem}
  .subh{font-family:var(--mono);font-size:.78rem;color:var(--teal-soft);margin:14px 0 6px;letter-spacing:.04em}
  .note{font-family:var(--mono);font-size:.78rem;color:var(--amber);margin:0 0 14px}
  footer{margin-top:46px;padding-top:16px;border-top:1px solid var(--line);color:var(--dim);font-family:var(--mono);font-size:.8rem;display:flex;justify-content:space-between;flex-wrap:wrap;gap:8px}
</style>
</head>
<body>
<div class="wrap">
  <div class="eyebrow">BATHOS Dynamis · 세션 작업 리포트</div>
  <h1>작업 리포트 · session_no${NNP}<span class="badge">소급 백필 · RETROACTIVE</span></h1>
  <div class="meta">프로젝트 <b>${PROJID}</b> · 세션 순번 <b>#${NN}</b> · 원본 <b>${ARCHNAME}</b></div>
  <p class="note">※ 이 리포트는 과거 세션 아카이브에서 소급 생성됨 — 원본에 기록된 사실만 포함(시작 시각·소요 시간은 당시 미기록).</p>

  <h2><span class="n">1–3</span>작업 시각 &amp; 총 소요</h2>
  <div class="rule"></div>
  <div class="times">
    <div class="tcard"><div class="lbl">작업 시작</div><div class="val dim">(소급 — 당시 미기록)</div></div>
    <div class="tcard"><div class="lbl">작업 종료</div><div class="val">${END}</div></div>
    <div class="tcard"><div class="lbl">총 작업 시간</div><div class="val dim">(소급 — 산출 불가)</div></div>
  </div>

  <h2><span class="n">4</span>작업 핵심 사항 <span style="font-size:.7em;color:var(--dim)">(원본 '현재 상태')</span></h2>
  <div class="rule"></div>
  <div class="card state"><pre>${SUMMARY}</pre></div>

  <h2><span class="n">5</span>중요 이슈</h2>
  <div class="rule"></div>
  <div class="card"><pre>(소급 백필 — 당시 별도 이슈 로그 없음. 상세는 원본 리포트 ${ARCHNAME} 참조.)</pre></div>

  <h2><span class="n">6</span>git commit / push / PR / merge 히스토리 <span style="font-size:.7em;color:var(--dim)">(원본 기록 기준)</span></h2>
  <div class="rule"></div>
  <div class="card"><pre>${GIT}</pre></div>

  <footer>
    <span>BATHOS · βάθος — 표층이 아닌 깊이 · 소급 백필</span>
    <span>task_report_${STAMP}_session_no${NNP}.html</span>
  </footer>
</div>
</body>
</html>
HTMLEOF

  echo "백필: session_no${NNP}  ← ${ARCHNAME}  (종료=${END})"
done

echo "완료: 총 ${NN} 개 세션 소급 생성. 다음 세션은 generate-task-report.sh 가 $(printf '%02d' $((NN+1))) 부터 이어받습니다."
exit 0
