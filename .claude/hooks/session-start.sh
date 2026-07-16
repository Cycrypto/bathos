#!/usr/bin/env bash
# ---------------------------------------------------------------------------
# BATHOS  session-start.sh  —  SessionStart 훅
# 세션 시작 시각을 마커 파일에 기록한다. task-report(6항목 중 "작업 시작 시각")의
# 정확도를 위해 존재한다. generate-task-report.sh 가 이 마커를 최우선으로 읽는다.
#
# 마커 : <project>/.agent-team/_state/session-start.marker  (ephemeral·gitignore)
# 동작 : source(startup/clear) = 새 세션 → 새 시각으로 덮어씀
#        source(resume/compact) = 진행 중 재개 → 기존 마커 보존(있으면 그대로)
# 철칙 : fail-safe — 세션 시작을 절대 막지 않으며 항상 exit 0. 날조 금지(실제 시각만).
# ---------------------------------------------------------------------------
set +e

PROJ="${CLAUDE_PROJECT_DIR:-$(pwd)}"
STATE="$PROJ/.agent-team/_state"
# BATHOS 프로젝트가 아니면(=_state 없음) 조용히 통과
[ -d "$STATE" ] || exit 0

MARKER="$STATE/session-start.marker"
IN="$(cat 2>/dev/null)"
SRC="$(printf '%s' "$IN" | grep -oE '"source"[[:space:]]*:[[:space:]]*"[^"]*"' | head -1 | sed -E 's/.*"([^"]+)"$/\1/')"
NOW="$(date '+%Y-%m-%d %H:%M:%S %Z')"

case "$SRC" in
  resume|compact)
    # 진행 중 세션의 연속 — 원래 시작 시각을 유지(없을 때만 기록)
    [ -s "$MARKER" ] || printf '%s\n' "$NOW" > "$MARKER" ;;
  *)
    # startup·clear·미상 — 새 세션의 시작으로 간주하고 갱신
    printf '%s\n' "$NOW" > "$MARKER" ;;
esac

exit 0
