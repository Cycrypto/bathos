#!/usr/bin/env bash
# =============================================================================
# BATHOS Dynamis A5 — intensity-tracker.sh   (SS4 · CF-A5, Could)
# UserPromptSubmit: "/bathos intensity <lite|full|ultra|off>" 파싱·상태 갱신
# =============================================================================
# 계약:  story-a5-intensity-toggle-kr.md, schema-extensions-kr.md §3
# 불변식(§5-6, LD-4): intensity ≠ Lv0~4. Lv=프로젝트/작업 규모(라우터,
#   manifest.current_level), intensity=세션 공격성(즉시 토글, session-flags.json).
#   이 훅은 plan_mode 필드를 절대 읽거나 쓰지 않는다(혼선 방지 불변식).
# 안전:  advisory 전용(도구 차단 없음) — 항상 exit 0.
# DoD:   유효 레벨 → 기록+확인 문구(Lv 라우팅과 별개 명시, AC1/AC3).
#        레벨 누락 → 도움말(현재값+4레벨 설명), 변경 없음(AC4).
#        알 수 없는 값 → 에러+이전값 유지(AC4).
#        쓰기 실패 → 경고+세션 한정 임시 적용 표시(AC4, fail-safe).
# =============================================================================
set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BATHOS_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
PROJECT_DIR="${CLAUDE_PROJECT_DIR:-$BATHOS_ROOT}"
STATE_DIR="${BATHOS_STATE_DIR:-$PROJECT_DIR/.agent-team/_state}"
BATHOS_BIN="${BATHOS_BIN:-$BATHOS_ROOT/core/target/debug/bathos}"
SESSION_FLAGS="$STATE_DIR/session-flags.json"

VALID_LEVELS=("lite" "full" "ultra" "off")

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

# 이 프롬프트에 /bathos intensity 커맨드 자체가 없으면 조용히 통과(advisory)
if ! printf '%s' "$PROMPT" | grep -Eiq '/bathos[[:space:]]+intensity([[:space:]]|$)'; then
  exit 0
fi

# 레벨 인자 추출(있으면). 없으면 빈 문자열 → 도움말 분기.
LEVEL="$(printf '%s' "$PROMPT" | grep -Eio '/bathos[[:space:]]+intensity[[:space:]]*[a-z]*' \
  | tail -1 | awk '{print tolower($3)}' || true)"

# --------------------------------------------------------------------------
# 2. session-flags.json 필드 읽기/쓰기 헬퍼 (plan-toggle.sh와 대칭 — 중복은
#    기존 코드베이스 관례(공유 lib 없음)를 계승한 의도적 선택, impl-notes 기록)
# --------------------------------------------------------------------------
_read_intensity() {
  if [[ ! -f "$SESSION_FLAGS" ]]; then
    printf 'full'  # 기본값(schema-extensions §3)
    return
  fi
  if command -v jq >/dev/null 2>&1; then
    local v
    v="$(jq -r '.intensity // empty' "$SESSION_FLAGS" 2>/dev/null || true)"
    printf '%s' "${v:-full}"
  else
    local v
    v="$(grep -o '"intensity"[[:space:]]*:[[:space:]]*"[^"]*"' "$SESSION_FLAGS" 2>/dev/null \
      | head -1 | sed 's/.*:[[:space:]]*"\([^"]*\)"/\1/' || true)"
    printf '%s' "${v:-full}"
  fi
}

_write_intensity() {
  local value="$1"
  mkdir -p "$STATE_DIR" 2>/dev/null || true
  local now
  now="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
  if command -v jq >/dev/null 2>&1; then
    local tmp="$SESSION_FLAGS.tmp.$$"
    if [[ -f "$SESSION_FLAGS" ]]; then
      # AC5(LD-4): plan_mode 필드는 건드리지 않는다 — jq는 지정 필드만 갱신.
      jq --arg v "$value" --arg u "$now" '.intensity = $v | .updated = $u' \
        "$SESSION_FLAGS" > "$tmp" 2>/dev/null
    else
      jq -n --arg v "$value" --arg u "$now" \
        '{"$schema":"bathos:session-flags","plan_mode":"off","intensity":$v,"updated":$u}' \
        > "$tmp" 2>/dev/null
    fi
    if [[ -s "$tmp" ]]; then
      mv "$tmp" "$SESSION_FLAGS"
      return 0
    fi
    rm -f "$tmp"
    return 1
  else
    # jq 없음: plan_mode 보존 시도(grep으로 기존 값 추출) 후 최소 스키마 재작성.
    local existing_plan="off"
    if [[ -f "$SESSION_FLAGS" ]]; then
      existing_plan="$(grep -o '"plan_mode"[[:space:]]*:[[:space:]]*"[^"]*"' "$SESSION_FLAGS" 2>/dev/null \
        | head -1 | sed 's/.*:[[:space:]]*"\([^"]*\)"/\1/' || echo "off")"
      [[ -z "$existing_plan" ]] && existing_plan="off"
    fi
    printf '{"$schema":"bathos:session-flags","plan_mode":"%s","intensity":"%s","updated":"%s"}\n' \
      "$existing_plan" "$value" "$now" > "$SESSION_FLAGS" 2>/dev/null || return 1
  fi
}

CURRENT="$(_read_intensity)"

# --------------------------------------------------------------------------
# 3. 4가지 상태 분기 (ux-flow-map-kr.md Flow A 정본)
# --------------------------------------------------------------------------

# 3-a. 레벨 누락 → 도움말(현재값 + 4레벨 설명), 변경 없음
if [[ -z "$LEVEL" ]]; then
  cat <<EOF
[bathos intensity] 현재 intensity=${CURRENT} (Lv 라우팅과 별개 — 변경 없음)
[bathos intensity] 사용 가능 레벨:
  lite  — 최소 개입(빠른 반복)
  full  — 표준 심화 안전망(기본값)
  ultra — 최대 심화(엄격 게이팅)
  off   — 심화 안전망 전면 비활성(경고: 상시 주의 필요)
[bathos intensity] 사용: /bathos intensity <lite|full|ultra|off>
EOF
  exit 0
fi

# 3-b. 알 수 없는 값 → 에러 + 이전값 유지
_is_valid_level() {
  local lvl="$1" v
  for v in "${VALID_LEVELS[@]}"; do
    [[ "$lvl" == "$v" ]] && return 0
  done
  return 1
}

if ! _is_valid_level "$LEVEL"; then
  printf '[bathos intensity] ! 알 수 없는 값 "%s" — lite|full|ultra|off 중 하나여야 합니다. (이전값 유지: %s)\n' \
    "$LEVEL" "$CURRENT" >&2
  exit 0
fi

# 3-c/3-d. 유효 레벨 → 기록 시도
if _write_intensity "$LEVEL"; then
  if [[ "$LEVEL" == "off" ]]; then
    printf '[bathos intensity] ! intensity=off 적용 (Lv 라우팅과 별개) — 심화 안전망이 전면 비활성 상태입니다. 상시 주의하세요.\n'
  else
    printf '[bathos intensity] . intensity=%s 적용 (Lv 라우팅과 별개)\n' "$LEVEL"
  fi
  "$BATHOS_BIN" --state-dir "$STATE_DIR" audit append \
    --actor "${BATHOS_ROLE:-hook:intensity-tracker}" --action "intensity.set" --target "$LEVEL" 2>/dev/null || true
else
  # 쓰기 실패 → 경고 + "세션 한정 임시 적용" 안내(fail-safe, AC4).
  # 정직성 고지: bash 훅은 프로세스 간 상태를 공유하지 않으므로 "임시 적용"은
  # 이번 확인 응답에 한정된 표시일 뿐, 다음 훅 호출부터는 디스크 값(미기록이므로
  # 이전값)이 다시 적용된다. 완전한 세션 메모리는 이 아키텍처의 알려진 한계다.
  printf '[bathos intensity] ! intensity=%s 기록 실패(파일 쓰기 오류) — 이번 응답에 한해 임시 적용 표시, 지속 저장은 실패했습니다. STATE_DIR 쓰기 권한을 확인하세요.\n' \
    "$LEVEL" >&2
fi

exit 0
