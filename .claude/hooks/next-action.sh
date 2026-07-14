#!/usr/bin/env bash
# =============================================================================
# BATHOS M6 — next-action.sh   [Dynamis A3 — 서킷브레이커 3종 추가]
# TeammateIdle: 남은 태스크 / 다음 웨이브 안내 + 침묵·무한루프 능동 안전망
# =============================================================================
# 계약:  api-contracts.md §D (TeammateIdle next-action), §B-3 (웨이브 전이 메시지)
# DoD:   idle 에이전트에게 다음 unblocked 태스크 또는 웨이브 안내 출력(기존, 회귀 금지)
# 참고:  기존 안내 로직은 차단하지 않음(exit 0 고정) — 서킷브레이커도 동일(유도만,
#        실제 프로세스 종료는 상위 오케스트레이터/사용자 판단, 이 훅은 신호만 냄).
#
# [Dynamis A3 신규 — SS2 · CF-A3, open-swe #7 사상]
#   `_state/session-flags.json`의 `counters.<role>` 서브객체를 판정한다:
#     - empty_msg_streak: 이 훅(TeammateIdle) 호출마다 +1. audit-log.sh(PostToolUse)가
#       도구 호출 시 0으로 리셋 — "도구 호출 없이 idle만 반복"을 정확히 포착한다.
#     - model_calls: audit-log.sh가 도구 호출마다 +1(모델 호출수 정밀 계측 훅이
#       Claude Code에 없어 "도구 호출 수"로 근사 — risk-log C-9, 정직 고지).
#     - fail_streak: audit-log.sh가 tool_response 실패 시 +1, 성공 시 0.
#   임계 초과 시 재주입/중단 유도 메시지를 출력하고 `bathos audit append`로 기록한다.
#   thresholds 기본값: empty_msg_max=2, model_call_max=200, fail_streak_max=3
#   (session-flags.json에 없으면 이 기본값 사용 — AC3).
#   fail-safe: jq 없거나 session-flags.json 없으면 서킷브레이커 로직 전체를
#   건너뛰고 기존 안내만 수행한다(AC3, AC4 회귀 없음).
# =============================================================================
set -uo pipefail

# --------------------------------------------------------------------------
# 1. 경로 해석
# --------------------------------------------------------------------------
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BATHOS_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
PROJECT_DIR="${CLAUDE_PROJECT_DIR:-$BATHOS_ROOT}"
STATE_DIR="${BATHOS_STATE_DIR:-$PROJECT_DIR/.agent-team/_state}"
MANIFEST="$STATE_DIR/manifest.json"
WAVE_LOG="$STATE_DIR/wave-log.md"
SESSION_FLAGS="$STATE_DIR/session-flags.json"
BATHOS_BIN="${BATHOS_BIN:-$BATHOS_ROOT/core/target/debug/bathos}"

# --------------------------------------------------------------------------
# 2. stdin JSON 파싱: 역할·남은 태스크 추출
# --------------------------------------------------------------------------
INPUT="$(cat || true)"
ROLE=""
REMAINING_TASKS=""

if command -v jq >/dev/null 2>&1; then
  ROLE="$(printf '%s' "$INPUT" | jq -r '.role // empty' 2>/dev/null || true)"
  REMAINING_TASKS="$(printf '%s' "$INPUT" | jq -r '
    .remaining_tasks // []
    | if length > 0 then map("  - " + .) | join("\n") else "" end
  ' 2>/dev/null || true)"
fi
ROLE="${ROLE:-${BATHOS_ROLE:-팀원}}"

# --------------------------------------------------------------------------
# 3. manifest.json 기반 웨이브 상태 파악 (기존 로직 — 회귀 금지)
# --------------------------------------------------------------------------
CURRENT_WAVE=""
NEXT_WAVE=""
GATE_VERDICT=""
WAVE_STATUS_SUMMARY=""

if [[ -f "$MANIFEST" && -r "$MANIFEST" ]] && command -v jq >/dev/null 2>&1; then
  # H-7 수정(2026-06-30): .waves_status(존재하지 않는 필드) →
  #   (.waves // []) | map(select(.status=="active")) | .[0].wave_id
  CURRENT_WAVE="$(jq -r '
    (.waves // [])
    | map(select(.status == "active"))
    | .[0].wave_id // ""
  ' "$MANIFEST" 2>/dev/null || true)"

  # H-6 수정(2026-06-30, next-action.sh도 동일 패턴): .wave → .wave_id
  GATE_VERDICT="$(jq -r '
    (.gates // [])
    | map(select(.gate_type == "Implementation" or .wave_id == "W3"))
    | last
    | .verdict // "없음"
  ' "$MANIFEST" 2>/dev/null || echo "없음")"

  # H-7 수정: .waves_status(없는 필드) → (.waves // []) 배열 순회
  WAVE_STATUS_SUMMARY="$(jq -r '
    (.waves // [])
    | map("  " + .wave_id + ": " + (.status // "unknown"))
    | join("\n")
  ' "$MANIFEST" 2>/dev/null || true)"
fi

# --------------------------------------------------------------------------
# 4. 다음 권장 행동 결정 (기존 로직 — 회귀 금지)
# --------------------------------------------------------------------------
_get_next_action() {
  # 웨이브별 다음 행동 매핑 (api-contracts B-3 on_complete 인터페이스)
  case "$CURRENT_WAVE" in
    *w0*|*wave0*|*W0*|*analysis*)
      echo "W0 Analysis 완료 → W1 Discovery 시작 (/wave1-discovery)" ;;
    *w1*|*wave1*|*W1*|*discovery*)
      echo "W1 Discovery 완료 → W2 기획·아키텍처·디자인 시작 (/wave2-design)" ;;
    *w2*|*wave2*|*W2*|*design*)
      echo "W2 완료 → W3 Story Engineering & Readiness Gate 시작 (/wave3-story-gate)" ;;
    *w3*|*wave3*|*W3*|*story*)
      case "$GATE_VERDICT" in
        PASS|CONCERNS)
          echo "W3 게이트 $GATE_VERDICT → 구현 웨이브 시작 (/wave4-implement)" ;;
        FAIL)
          echo "W3 게이트 FAIL → W2로 반려 후 보완·재게이트 필요" ;;
        *)
          echo "W3 진행 중 → 게이트 판정 완료 후 구현 웨이브 또는 W2 반려" ;;
      esac ;;
    *w4*|*wave4*|*W4*|*ip*|*research*)
      echo "W4 IP/Research 완료 (플러그) → W6 검증·문서화 또는 Martin 취합 대기" ;;
    *w5*|*wave5*|*W5*|*implement*)
      echo "구현 웨이브 완료 → W6 검증·문서화·리포트 시작 (/wave5-verify-report)" ;;
    *w6*|*wave6*|*W6*|*verify*|*report*)
      echo "W6 진행 중 → Thomas·Timothy·Matthias 완료 후 Martin 취합 → 최종 confirm" ;;
    *)
      echo "현재 웨이브 상태를 확인하고 다음 unblocked 태스크를 self-claim 하세요." ;;
  esac
}

NEXT_ACTION="$(_get_next_action)"

# ============================================================================
# 5. [Dynamis A3] 서킷브레이커 3종 판정 — jq·session-flags.json 필요(fail-open)
# ============================================================================
CIRCUIT_MESSAGE=""
if command -v jq >/dev/null 2>&1 && [[ -f "$SESSION_FLAGS" ]]; then
  # 안전한 jq 키(공백/특수문자 방지) — role을 그대로 객체 키로 사용
  READ_OUT="$(jq -r --arg role "$ROLE" '
    {
      empty_msg_streak: (.counters[$role].empty_msg_streak // 0),
      model_calls: (.counters[$role].model_calls // 0),
      fail_streak: (.counters[$role].fail_streak // 0),
      empty_msg_max: (.thresholds.empty_msg_max // 2),
      model_call_max: (.thresholds.model_call_max // 200),
      fail_streak_max: (.thresholds.fail_streak_max // 3)
    } | [.empty_msg_streak, .model_calls, .fail_streak, .empty_msg_max, .model_call_max, .fail_streak_max]
    | @tsv
  ' "$SESSION_FLAGS" 2>/dev/null || true)"

  if [[ -n "$READ_OUT" ]]; then
    IFS=$'\t' read -r EMPTY_STREAK MODEL_CALLS FAIL_STREAK EMPTY_MAX CALL_MAX FAIL_MAX <<< "$READ_OUT"

    # 5-a. 이번 idle 자체를 "도구 호출 없는 턴"으로 집계 — +1
    NEW_EMPTY_STREAK=$((EMPTY_STREAK + 1))
    RESET_EMPTY=0
    RESET_CIRCUIT=0

    if [[ "$NEW_EMPTY_STREAK" -ge "$EMPTY_MAX" ]]; then
      CIRCUIT_MESSAGE="${CIRCUIT_MESSAGE}[bathos next-action] ! 연속 무도구-응답 ${NEW_EMPTY_STREAK}회(임계 ${EMPTY_MAX}) 감지 — 재주입: 다음 unblocked 태스크를 self-claim 하거나 진행 상황을 보고하세요.\n"
      RESET_EMPTY=1
      "$BATHOS_BIN" --state-dir "$STATE_DIR" audit append \
        --actor "${BATHOS_ROLE:-hook:next-action}" --action "circuit.reinject" --target "$ROLE" 2>/dev/null || true
    fi

    if [[ "$MODEL_CALLS" -gt "$CALL_MAX" ]] || [[ "$FAIL_STREAK" -ge "$FAIL_MAX" ]]; then
      CIRCUIT_MESSAGE="${CIRCUIT_MESSAGE}[bathos next-action] x 서킷브레이커 발동(model_calls=${MODEL_CALLS}/${CALL_MAX}, fail_streak=${FAIL_STREAK}/${FAIL_MAX}) — 작업을 중단하고 현재까지 결과 1단락 + 리스크를 Paul에게 보고하세요.\n"
      RESET_CIRCUIT=1
      "$BATHOS_BIN" --state-dir "$STATE_DIR" audit append \
        --actor "${BATHOS_ROLE:-hook:next-action}" --action "circuit.open" --target "$ROLE" 2>/dev/null || true
    fi

    # 카운터 갱신(신호를 낸 항목은 리셋 — 반복 스팸 방지, 나머지는 갱신값 유지)
    TMP_FLAGS="$SESSION_FLAGS.tmp.$$"
    FINAL_EMPTY=$(( RESET_EMPTY == 1 ? 0 : NEW_EMPTY_STREAK ))
    FINAL_FAIL=$(( RESET_CIRCUIT == 1 ? 0 : FAIL_STREAK ))
    jq --arg role "$ROLE" \
       --argjson empty "$FINAL_EMPTY" \
       --argjson fail "$FINAL_FAIL" \
       '.counters[$role].empty_msg_streak = $empty | .counters[$role].fail_streak = $fail' \
       "$SESSION_FLAGS" > "$TMP_FLAGS" 2>/dev/null
    if [[ -s "$TMP_FLAGS" ]]; then
      mv "$TMP_FLAGS" "$SESSION_FLAGS"
    else
      rm -f "$TMP_FLAGS"
    fi
  fi
fi

# --------------------------------------------------------------------------
# 6. 안내 메시지 출력 (기존 + 서킷브레이커 메시지)
# --------------------------------------------------------------------------
printf '\n[bathos next-action] . %s — idle 상태 안내\n' "$ROLE" >&2

if [[ -n "$CIRCUIT_MESSAGE" ]]; then
  printf '%b' "$CIRCUIT_MESSAGE" >&2
fi

if [[ -n "$REMAINING_TASKS" ]]; then
  printf '[bathos next-action] 남은 태스크:\n%s\n' "$REMAINING_TASKS" >&2
  printf '[bathos next-action] → 다음 unblocked 태스크를 self-claim 하여 계속 진행하세요.\n' >&2
else
  printf '[bathos next-action] 할당된 남은 태스크가 없습니다.\n' >&2
fi

if [[ -n "$NEXT_ACTION" ]]; then
  printf '[bathos next-action] 권장 다음 행동: %s\n' "$NEXT_ACTION" >&2
fi

if [[ -n "$CURRENT_WAVE" ]]; then
  printf '[bathos next-action] 현재 웨이브: %s | W3 게이트: %s\n' "$CURRENT_WAVE" "$GATE_VERDICT" >&2
fi

if [[ -n "$WAVE_STATUS_SUMMARY" ]]; then
  printf '[bathos next-action] 웨이브 상태:\n%s\n' "$WAVE_STATUS_SUMMARY" >&2
fi

printf '[bathos next-action] 종료 보고 형식: 핵심 결과 1단락 + 미해결 리스크를 Paul에게 보내세요.\n' >&2

# 비강제: 항상 통과(서킷브레이커도 "유도"이지 강제 종료가 아님 — §3 경량 제약)
exit 0
