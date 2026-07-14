#!/usr/bin/env bash
# =============================================================================
# BATHOS M6 — audit-log.sh   [Dynamis A3 — 서킷브레이커 카운터 갱신 추가]
# PostToolUse: 모든 도구 사용을 _state/audit-log.jsonl에 append (hash 체인)
# =============================================================================
# 계약:  api-contracts.md §D (PostToolUse audit), §A-7 (감사로그 스키마)
# 예외:  exceptions.md §6 E-AUDIT-TAMPER (hash_prev 체인 무결성)
# DoD:   도구 사용마다 JSONL 1행 append; hash chain 유지; 비강제(오류 시 통과)
# 형식:  {"seq":N,"ts":"ISO","actor":"...","action":"tool:name","target":"...","hash_prev":"sha256:..."}
# 참고:  이 훅은 차단하지 않음(exit 0 고정). 감사 목적 전용.
#
# [Dynamis A3 신규 — SS2 · CF-A3]
#   이 훅(PostToolUse = "도구를 실제로 호출함")은 next-action.sh(TeammateIdle)의
#   서킷브레이커 3종이 참조하는 `_state/session-flags.json`의 `counters.<actor>`를
#   갱신하는 유일한 지점이다:
#     - empty_msg_streak → 0 리셋(도구를 호출했으므로 "무도구-응답"이 아님)
#     - model_calls → +1 (risk-log C-9: Claude Code에는 "모델 호출수"를 직접 세는
#       훅이 없다 — 이 값은 **도구 호출 수의 근사치**다. 정직 고지: 도구를 호출하지
#       않는 순수 텍스트 응답 턴은 집계되지 않는다. 완전한 모델 호출 계측은 범위
#       밖이며 이 근사가 최선이다.)
#     - fail_streak → tool_response 실패 시 +1, 성공 시 0
#   fail-safe: jq 없으면 카운터 갱신을 건너뛴다(기존 audit append 동작은 계속됨).
# =============================================================================
set -uo pipefail

# --------------------------------------------------------------------------
# 1. 경로 해석
# --------------------------------------------------------------------------
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BATHOS_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
PROJECT_DIR="${CLAUDE_PROJECT_DIR:-$BATHOS_ROOT}"
STATE_DIR="${BATHOS_STATE_DIR:-$PROJECT_DIR/.agent-team/_state}"
# B-1 수정(2026-06-30): Rust CLI 단일 writer 경유. 직접 printf 제거로 AuditEntry 역직렬화 실패 방지.
BATHOS_BIN="${BATHOS_BIN:-$BATHOS_ROOT/core/target/debug/bathos}"
SESSION_FLAGS="$STATE_DIR/session-flags.json"

# _state 디렉터리가 없으면 감사 스킵 (비강제)
if [[ ! -d "$STATE_DIR" ]]; then
  exit 0
fi

# --------------------------------------------------------------------------
# 2. stdin JSON 파싱
# --------------------------------------------------------------------------
INPUT="$(cat || true)"
TOOL_NAME=""
TARGET=""
RESULT_SUMMARY=""

if command -v jq >/dev/null 2>&1; then
  TOOL_NAME="$(printf '%s' "$INPUT" | jq -r '.tool_name // empty' 2>/dev/null || true)"

  # 도구별 target 추출 (파일 경로 또는 명령 요약)
  case "$TOOL_NAME" in
    Write|Edit|MultiEdit)
      TARGET="$(printf '%s' "$INPUT" | jq -r '.tool_input.file_path // empty' 2>/dev/null || true)"
      ;;
    Read|Grep|Glob)
      TARGET="$(printf '%s' "$INPUT" | jq -r '
        .tool_input.file_path //
        .tool_input.pattern //
        .tool_input.path //
        empty
      ' 2>/dev/null || true)"
      ;;
    Bash)
      # 명령 앞 80자만 기록 (민감정보 최소화)
      TARGET="$(printf '%s' "$INPUT" | jq -r '.tool_input.command // empty' 2>/dev/null \
        | head -c 80 | tr '"\\\n\r\t' "    " || true)"
      ;;
    WebFetch|WebSearch)
      TARGET="$(printf '%s' "$INPUT" | jq -r '.tool_input.url // .tool_input.query // empty' 2>/dev/null || true)"
      ;;
    *)
      TARGET="$(printf '%s' "$INPUT" | jq -r '.tool_input | keys[0] // empty' 2>/dev/null || true)"
      ;;
  esac

  # 결과 요약 (성공/실패 여부만)
  RESULT_SUMMARY="$(printf '%s' "$INPUT" | jq -r '
    if .tool_response.type == "result" then "ok"
    elif .tool_response.type == "error" then "error"
    else "unknown"
    end
  ' 2>/dev/null || echo "unknown")"
else
  # jq 없으면 원문에서 최소 추출
  TOOL_NAME="$(printf '%s' "$INPUT" | grep -o '"tool_name"[[:space:]]*:[[:space:]]*"[^"]*"' 2>/dev/null \
    | head -1 | sed 's/.*"\([^"]*\)".*/\1/' || echo "unknown")"
  TARGET="(jq unavailable)"
  RESULT_SUMMARY="unknown"
fi

# 도구명 미확인 시 스킵 (비강제)
[[ -z "$TOOL_NAME" ]] && exit 0

ACTOR="${BATHOS_ROLE:-agent}"
SAFE_TARGET="$(printf '%s' "$TARGET" | head -c 200 | tr '"\\\n\r\t' "    ")"

# --------------------------------------------------------------------------
# 3. CLI 경유 감사 append (단일 writer 보장 — B-1 수정, 2026-06-30)
# --------------------------------------------------------------------------
# ■ 변경 이유: Rust AuditEntry(hash_self 필드 필수·genesis 형식 차이)와 bash
#   직접 printf 출력이 완전 비호환 → verify_chain() 항상 AuditChainBroken 반환.
#   bash가 CLI(bathos audit append)를 경유하면 단일 구현이 포맷을 보장한다.
# ■ seq 단조성·hash_prev 체인은 Rust CLI가 관리(SSOT).
# ■ 바이너리 없으면 조용히 통과 — 훅은 절대 차단/지연 금지(|| true).
"$BATHOS_BIN" --state-dir "$STATE_DIR" audit append \
  --actor "$ACTOR" \
  --action "tool:$TOOL_NAME" \
  --target "$SAFE_TARGET" 2>/dev/null || true

# ============================================================================
# 4. [Dynamis A3] 서킷브레이커 카운터 갱신 — jq 필요(fail-open: 없으면 스킵)
# ============================================================================
if command -v jq >/dev/null 2>&1; then
  TMP_FLAGS="$SESSION_FLAGS.tmp.$$"
  now="$(date -u +%Y-%m-%dT%H:%M:%SZ)"

  if [[ -f "$SESSION_FLAGS" ]]; then
    BASE_JSON="$(cat "$SESSION_FLAGS" 2>/dev/null || echo '{}')"
  else
    mkdir -p "$STATE_DIR" 2>/dev/null || true
    BASE_JSON='{"$schema":"bathos:session-flags","plan_mode":"off","intensity":"full"}'
  fi

  printf '%s' "$BASE_JSON" | jq \
    --arg actor "$ACTOR" \
    --arg result "$RESULT_SUMMARY" \
    --arg u "$now" '
      .counters[$actor].empty_msg_streak = 0
      | .counters[$actor].model_calls = ((.counters[$actor].model_calls // 0) + 1)
      | .counters[$actor].fail_streak = (if $result == "error"
          then ((.counters[$actor].fail_streak // 0) + 1)
          else 0 end)
      | .updated = $u
    ' > "$TMP_FLAGS" 2>/dev/null

  if [[ -s "$TMP_FLAGS" ]]; then
    mv "$TMP_FLAGS" "$SESSION_FLAGS"
  else
    rm -f "$TMP_FLAGS"
  fi
fi

# 비강제: 항상 통과
exit 0
