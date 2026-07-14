#!/usr/bin/env bash
# =============================================================================
# BATHOS Dynamis A4 — plan-toggle.sh   (SS5 · CF-A4, LD-4 승격)
# UserPromptSubmit: "/bathos plan <on|off>" 파싱 → session-flags.json 갱신
# =============================================================================
# 계약:  story-a4-plan-gate-kr.md §4 "토글 수단 1개(구현 방식은 Phillip 판단)"
#        → intensity-tracker.sh(A5)와 대칭적인 UserPromptSubmit 파싱 방식으로 구현.
# DoD:   AC4 — 토글 수단 존재, off 후 변경 도구 즉시 허용.
#        AC5 — intensity 필드는 절대 읽거나 쓰지 않는다(같은 파일, 별도 필드).
# 안전:  이 훅은 advisory(차단 없음) — 항상 exit 0.
# =============================================================================
set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BATHOS_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
PROJECT_DIR="${CLAUDE_PROJECT_DIR:-$BATHOS_ROOT}"
STATE_DIR="${BATHOS_STATE_DIR:-$PROJECT_DIR/.agent-team/_state}"
BATHOS_BIN="${BATHOS_BIN:-$BATHOS_ROOT/core/target/debug/bathos}"
SESSION_FLAGS="$STATE_DIR/session-flags.json"

# --------------------------------------------------------------------------
# 1. stdin JSON 파싱: 사용자 프롬프트 원문 추출
# --------------------------------------------------------------------------
INPUT="$(cat || true)"
PROMPT=""
if command -v jq >/dev/null 2>&1; then
  PROMPT="$(printf '%s' "$INPUT" | jq -r '.prompt // empty' 2>/dev/null || true)"
else
  PROMPT="$INPUT"
fi
[[ -z "$PROMPT" ]] && exit 0

# --------------------------------------------------------------------------
# 2. "/bathos plan <on|off>" 패턴 감지 (대소문자 무관, 앞뒤 공백 허용)
# --------------------------------------------------------------------------
LEVEL="$(printf '%s' "$PROMPT" | grep -Eio '/bathos[[:space:]]+plan[[:space:]]+[a-z]+' \
  | tail -1 | awk '{print tolower($3)}' || true)"

# 명령 자체가 없으면(다른 프롬프트) 조용히 통과 — advisory hook
[[ -z "$LEVEL" ]] && exit 0

# --------------------------------------------------------------------------
# 3. session-flags.json 필드 갱신 헬퍼 (jq 우선, 없으면 최소 스키마로 재작성)
#    ⚠️  jq 부재 시 counters/thresholds 등 다른 필드는 보존되지 않는다(문서화된
#    한계 — session-flags.json은 감사 대상 SSOT가 아니므로 완전성보다 fail-safe
#    우선). [Source: backend.md "session-flags 쓰기 한계"]
# --------------------------------------------------------------------------
_write_plan_mode() {
  local value="$1"
  mkdir -p "$STATE_DIR" 2>/dev/null || true
  local now
  now="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
  if command -v jq >/dev/null 2>&1; then
    local tmp="$SESSION_FLAGS.tmp.$$"
    if [[ -f "$SESSION_FLAGS" ]]; then
      jq --arg v "$value" --arg u "$now" '.plan_mode = $v | .updated = $u' \
        "$SESSION_FLAGS" > "$tmp" 2>/dev/null
    else
      jq -n --arg v "$value" --arg u "$now" \
        '{"$schema":"bathos:session-flags","plan_mode":$v,"intensity":"full","updated":$u}' \
        > "$tmp" 2>/dev/null
    fi
    if [[ -s "$tmp" ]]; then
      mv "$tmp" "$SESSION_FLAGS"
      return 0
    fi
    rm -f "$tmp"
    return 1
  else
    printf '{"$schema":"bathos:session-flags","plan_mode":"%s","intensity":"full","updated":"%s"}\n' \
      "$value" "$now" > "$SESSION_FLAGS" 2>/dev/null
  fi
}

case "$LEVEL" in
  on)
    if _write_plan_mode "on"; then
      printf '[bathos plan-toggle] . plan_mode=on 적용 — 계획 확정 전까지 Write/Edit/MultiEdit/변경성 Bash가 차단됩니다.\n'
      "$BATHOS_BIN" --state-dir "$STATE_DIR" audit append \
        --actor "${BATHOS_ROLE:-hook:plan-toggle}" --action "plan.toggle" --target "on" 2>/dev/null || true
    else
      printf '[bathos plan-toggle] ! plan_mode=on 기록 실패(파일 쓰기 오류) — 다음 행동: STATE_DIR 쓰기 권한을 확인하세요.\n' >&2
    fi
    ;;
  off)
    if _write_plan_mode "off"; then
      printf '[bathos plan-toggle] . plan_mode=off 적용 — 변경 도구가 즉시 허용됩니다.\n'
      "$BATHOS_BIN" --state-dir "$STATE_DIR" audit append \
        --actor "${BATHOS_ROLE:-hook:plan-toggle}" --action "plan.toggle" --target "off" 2>/dev/null || true
    else
      printf '[bathos plan-toggle] ! plan_mode=off 기록 실패(파일 쓰기 오류) — 다음 행동: STATE_DIR 쓰기 권한을 확인하세요.\n' >&2
    fi
    ;;
  *)
    printf '[bathos plan-toggle] ! 알 수 없는 값 "%s" — on|off 중 하나를 사용하세요. (이전 값 유지)\n' "$LEVEL" >&2
    ;;
esac

exit 0
