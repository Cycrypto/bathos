#!/usr/bin/env bash
# ---------------------------------------------------------------------------
# BATHOS session-end.sh  —  SessionEnd 훅 오케스트레이터
# 세션 종료(특히 /exit) 시 정해진 순서를 강제한다:
#   [1] 세션 작업 저장(mechanical save)  →  [2] 작업 리포트 생성  →  [3] 종료 진행
# 내장 /exit는 모델 턴을 주지 않으므로 이 순서는 셸에서 보장한다.
# (서술 스냅샷 '★ 현재 상태'의 최신 narrative는 세션 중 모델이 /save-session 등으로 갱신.)
# 철칙: fail-safe — 종료를 막지 않으며 항상 exit 0. 날조 금지(디스크 사실만).
# ---------------------------------------------------------------------------
set +e

PROJ="${CLAUDE_PROJECT_DIR:-$(pwd)}"
STATE="$PROJ/.agent-team/_state"

# BATHOS 프로젝트가 아니면 조용히 통과
[ -d "$STATE" ] || exit 0

DATE="$(date +%Y-%m-%d)"
TS="$(date '+%Y-%m-%d %H:%M:%S %Z')"

# ===== [1] 세션 작업 저장 (mechanical) =====================================
# 1a. 서술 스냅샷을 날짜 아카이브로 보존(세션 중 모델이 갱신한 최신본을 고정)
[ -f "$STATE/SESSION-SNAPSHOT.md" ] && cp -f "$STATE/SESSION-SNAPSHOT.md" "$STATE/SESSION-SNAPSHOT-$DATE.md" 2>/dev/null

# 1b. 엔진 상태 SSOT 덤프(엔진 스키마 프로젝트일 때만 성공; 운영용 manifest면 실패 → 정리)
BIN="${BATHOS_BIN:-}"
[ -z "$BIN" ] && command -v bathos >/dev/null 2>&1 && BIN="$(command -v bathos)"
[ -z "$BIN" ] && [ -x "$PROJ/core/target/release/bathos" ] && BIN="$PROJ/core/target/release/bathos"
[ -z "$BIN" ] && [ -x "$PROJ/core/target/debug/bathos" ] && BIN="$PROJ/core/target/debug/bathos"
if [ -n "$BIN" ]; then
  if "$BIN" -s "$STATE" state show > "$STATE/session-state.json" 2>/dev/null; then :; else rm -f "$STATE/session-state.json"; fi
fi

# 1c. 종료 스탬프(감사/추적용, 비차단)
printf 'session closed: %s\n' "$TS" >> "$STATE/session-close.log" 2>/dev/null
# 감사 기록은 반드시 CLI(bathos audit append)를 경유한다 — 직접 printf는 hash_self 누락·genesis 형식 불일치로
# verify_chain을 항상 깨뜨린다(B-1 회귀). BIN 미해결이면 조용히 생략(fail-safe, 종료 비차단).
[ -n "$BIN" ] && "$BIN" -s "$STATE" audit append --actor hook --action session.save --target session-end >/dev/null 2>&1

# ===== [2] 작업 리포트 생성 =================================================
# 저장이 끝난 뒤 리포트를 만든다(리포트는 위에서 고정된 _state를 읽는다).
if [ -x "$PROJ/.claude/hooks/session-report.sh" ]; then
  bash "$PROJ/.claude/hooks/session-report.sh"
fi

# ===== [3] 종료 진행 ========================================================
# (SessionEnd 훅은 종료를 막을 수 없다 — 여기서 반환하면 세션이 닫힌다.)
exit 0
