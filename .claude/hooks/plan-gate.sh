#!/usr/bin/env bash
# =============================================================================
# BATHOS Dynamis A4 — plan-gate.sh   (SS5 · CF-A4, LD-4 승격)
# PreToolUse(Write/Edit/MultiEdit/Bash): plan_mode=on일 때 변경 도구 deny 게이팅
# =============================================================================
# 계약:  project-context-kr.md §4(훅 계약 5칙), story-a4-plan-gate-kr.md
# 예외:  exceptions.md E-PLAN-GATED (신규)
#
# 배경(검증된 사실, 재조사 불요 — feasibility-cf-a4-kr.md):
#   - "모델이 보는 도구 목록을 런타임에 숨기는 것"은 Claude Code에서 불가하다.
#   - `PreToolUse` 훅의 stdout JSON `hookSpecificOutput.permissionDecision:"deny"`는
#     권한모드 검사보다 먼저 발화하므로 `--dangerously-skip-permissions`로도
#     **우회 불가**(deny > defer > ask > allow). 도구 은닉보다 강한 물리 차단이다.
#   - plan 상태는 Claude Code 네이티브 plan mode에 의존하지 않고 BATHOS 자체
#     플래그(`_state/session-flags.json`의 `plan_mode`)로 관리한다(LD-4).
#
# DoD: plan_mode=="on" + Write/Edit/MultiEdit → deny.
#      plan_mode=="on" + 변경성 Bash → deny. 읽기 전용 Bash → allow.
#      plan_mode=="off"/필드 미설정/파일 부재 → 전부 allow(fail-open).
#      intensity 필드는 읽지도 쓰지도 않는다(LD-4 혼선 방지 불변식).
# =============================================================================
set -uo pipefail

# --------------------------------------------------------------------------
# 1. 경로 해석 (기존 훅과 동일 골격)
# --------------------------------------------------------------------------
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BATHOS_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
PROJECT_DIR="${CLAUDE_PROJECT_DIR:-$BATHOS_ROOT}"
STATE_DIR="${BATHOS_STATE_DIR:-$PROJECT_DIR/.agent-team/_state}"
BATHOS_BIN="${BATHOS_BIN:-$BATHOS_ROOT/core/target/debug/bathos}"
SESSION_FLAGS="$STATE_DIR/session-flags.json"

# --------------------------------------------------------------------------
# 2. session-flags.json 필드 읽기 헬퍼 (jq 우선, grep 폴백, 파일 없으면 기본값)
# --------------------------------------------------------------------------
_read_flag() {
  local key="$1" default="$2"
  if [[ ! -f "$SESSION_FLAGS" ]]; then
    printf '%s' "$default"
    return
  fi
  if command -v jq >/dev/null 2>&1; then
    local v
    v="$(jq -r --arg k "$key" '.[$k] // empty' "$SESSION_FLAGS" 2>/dev/null || true)"
    printf '%s' "${v:-$default}"
  else
    local v
    v="$(grep -o "\"$key\"[[:space:]]*:[[:space:]]*\"[^\"]*\"" "$SESSION_FLAGS" 2>/dev/null \
      | head -1 | sed 's/.*:[[:space:]]*"\([^"]*\)"/\1/' || true)"
    printf '%s' "${v:-$default}"
  fi
}

PLAN_MODE="$(_read_flag "plan_mode" "off")"

# fail-open: off/미설정/파일부재 → 즉시 통과 (AC3)
if [[ "$PLAN_MODE" != "on" ]]; then
  exit 0
fi

# --------------------------------------------------------------------------
# 3. stdin JSON 파싱
# --------------------------------------------------------------------------
INPUT="$(cat || true)"
TOOL=""
CMD=""
FILE_PATH=""

if command -v jq >/dev/null 2>&1; then
  TOOL="$(printf '%s' "$INPUT" | jq -r '.tool_name // empty' 2>/dev/null || true)"
  CMD="$(printf '%s' "$INPUT" | jq -r '.tool_input.command // empty' 2>/dev/null || true)"
  FILE_PATH="$(printf '%s' "$INPUT" | jq -r '.tool_input.file_path // empty' 2>/dev/null || true)"
else
  TOOL="$(printf '%s' "$INPUT" | grep -o '"tool_name"[[:space:]]*:[[:space:]]*"[^"]*"' 2>/dev/null \
    | head -1 | sed 's/.*"\([^"]*\)".*/\1/' || true)"
  CMD="$INPUT"
fi

# 이 훅이 다루지 않는 도구는 통과 (matcher가 이미 좁히지만 방어적으로 재확인)
case "$TOOL" in
  Write|Edit|MultiEdit|Bash) ;;
  "") ;;   # 도구명 미확인 → 보수적으로 계속 진행(read-only 판별에서 걸러짐)
  *) exit 0 ;;
esac

# --------------------------------------------------------------------------
# 4. 읽기 전용 Bash 허용목록 (careful-guard 위험패턴과 대칭적 접근 — 여긴 허용목록)
# --------------------------------------------------------------------------
_is_readonly_bash() {
  local cmd="$1"
  printf '%s' "$cmd" | grep -Eiq \
    '^[[:space:]]*(ls|cat|pwd|echo|head|tail|wc|find|file|which|type|env|printenv|grep|jq|diff)([[:space:]]|$)' \
    && return 0
  printf '%s' "$cmd" | grep -Eiq \
    '^[[:space:]]*git[[:space:]]+(status|log|diff|show|branch|remote|blame)([[:space:]]|$)' \
    && return 0
  printf '%s' "$cmd" | grep -Eiq '^[[:space:]]*cargo[[:space:]]+(check|test|build|clippy)([[:space:]]|$)' \
    && return 0
  return 1
}

DENY_REASON=""
case "$TOOL" in
  Write|Edit|MultiEdit)
    DENY_REASON="계획 확정 전 변경 차단 — plan_mode=on 상태입니다. 계획을 확정(/bathos plan off)한 뒤 다시 시도하세요. (BATHOS plan_mode는 Claude Code 네이티브 plan mode와 별개입니다.)"
    ;;
  Bash)
    if [[ -z "$CMD" ]]; then
      exit 0  # 명령 미확인 → 통과(보수적, careful-guard와 동일 관례)
    fi
    if _is_readonly_bash "$CMD"; then
      exit 0  # 읽기 전용 명령은 계획 모드에서도 허용 (AC2)
    fi
    DENY_REASON="계획 확정 전 변경성 명령 차단 — plan_mode=on 상태입니다. 읽기 전용 명령(ls/cat/git status 등)만 허용됩니다. 계획을 확정(/bathos plan off)한 뒤 다시 시도하세요."
    ;;
  *)
    exit 0
    ;;
esac

# --------------------------------------------------------------------------
# 5. 감사 기록 (단일 writer 경유, fail-safe)
# --------------------------------------------------------------------------
_target_excerpt="${FILE_PATH:-$CMD}"
_target_excerpt="$(printf '%s' "$_target_excerpt" | head -c 150 | tr '"\\\n\r\t' "    ")"
"$BATHOS_BIN" --state-dir "$STATE_DIR" audit append \
  --actor "${BATHOS_ROLE:-hook:plan-gate}" \
  --action "plan.gated" \
  --target "$_target_excerpt" 2>/dev/null || true

# --------------------------------------------------------------------------
# 6. deny 응답 — PreToolUse stdout JSON (권한모드 우회 불가, feasibility 확인됨)
# --------------------------------------------------------------------------
printf '%s\n' "$(cat <<JSON
{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"deny","permissionDecisionReason":"[bathos plan-gate] x ${DENY_REASON}"}}
JSON
)"
printf '[bathos plan-gate] x %s\n' "$DENY_REASON" >&2
exit 0
