#!/usr/bin/env bash
# =============================================================================
# BATHOS M6 — gate-enforce.sh
# TaskCompleted: W3 게이트 미통과(FAIL) 시 구현 웨이브 진입 물리 차단
# =============================================================================
# 계약:  api-contracts.md §D (TaskCompleted gate-enforce), §C-2 (게이트 응답)
# 예외:  exceptions.md §1 E-GATE-FAIL, E-GATE-LOOP
# DoD:   구현 진입 시도 + W3 게이트 FAIL → exit 2(차단); PASS/CONCERNS → exit 0
#
# [Dynamis A1 — 동작 완전 보존 회귀 금지 대상]
#   이 훅은 story-a1-guard-hardening-kr.md AC1의 회귀 금지 대상이다. 로직은
#   원본과 동일하게 유지한다. fingerprint 재승인 방지(SS1)는 freeze-guard.sh가
#   담당하며 이 훅과는 결선이 불필요하다(설계 시 확인 — hook-and-gate-design-kr.md
#   §1a "신규 훅 불필요 — 기존 확장"은 careful/freeze만 해당).
#
#   부수 계약(schema-extensions-kr.md §2b, C1 선결): 리뷰 finding 결과 라벨은
#   `bathos audit append --action "finding.resolved" --target "F-000N:<resolution>"`
#   형식으로 기존 `bathos audit append` 인터페이스를 그대로 사용해 기록 가능함을
#   확인했다(신규 코드 불필요 — 기존 --action/--target 자유 문자열 파라미터로 충분).
#   Stephen의 continual-learning 스킬(CF-C1)이 이 이벤트를 audit-log.jsonl에서
#   grep/jq로 소싱한다. [Source: schema-extensions-kr.md §2b]
# =============================================================================
#
# ⚙️  B3 의존 동작 가정 (2026-06-30 현재 B2 페이즈):
#   `bathos gate show` 서브커맨드는 B3에서 구현 예정.
#   현재는 manifest.json의 gates[] 또는 readiness-report-kr.md를 직접 읽어
#   verdict를 판단한다. B3에서 `bathos gate show`가 구현되면 §6의 코드 블록을
#   활성화하면 된다 — 인터페이스는 완성 상태.
#
# ⚠️  fail-safe 원칙:
#   - 바이너리/서브커맨드 없음      → 경고 후 통과 (비차단)
#   - manifest.json 읽기 실패       → 경고 후 통과 (비차단)
#   - 구현 진입 태스크 아닌 경우    → 조용히 통과
#   - W3 verdict == FAIL 확인 시   → exit 2 차단 (유일 차단 조건)
# =============================================================================
set -uo pipefail

# --------------------------------------------------------------------------
# 1. 경로 해석
# --------------------------------------------------------------------------
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BATHOS_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
PROJECT_DIR="${CLAUDE_PROJECT_DIR:-$BATHOS_ROOT}"
STATE_DIR="${BATHOS_STATE_DIR:-$PROJECT_DIR/.agent-team/_state}"
ART_DIR="${PROJECT_DIR}/.agent-team"
BATHOS_BIN="${BATHOS_BIN:-$BATHOS_ROOT/core/target/debug/bathos}"

# --------------------------------------------------------------------------
# 2. stdin JSON 파싱: 태스크 제목 추출
# --------------------------------------------------------------------------
INPUT="$(cat || true)"
TASK_TITLE=""
TASK_DESC=""

if command -v jq >/dev/null 2>&1; then
  TASK_TITLE="$(printf '%s' "$INPUT" | jq -r '.task.title // .title // empty' 2>/dev/null || true)"
  TASK_DESC="$(printf '%s' "$INPUT" | jq -r '.task.description // .description // empty' 2>/dev/null || true)"
else
  TASK_TITLE="$(printf '%s' "$INPUT" | grep -o '"title"[[:space:]]*:[[:space:]]*"[^"]*"' 2>/dev/null \
    | head -1 | sed 's/.*"\([^"]*\)".*/\1/' || true)"
fi

# --------------------------------------------------------------------------
# 3. 구현 웨이브 진입 태스크 감지 (본 프로젝트에서 "구현" = charter W4, 표준명칭 W5)
# --------------------------------------------------------------------------
# W5 진입 시도를 나타내는 키워드 (태스크 제목/설명에서 탐지)
is_w5_entry() {
  local combined="$TASK_TITLE $TASK_DESC"
  printf '%s' "$combined" | grep -Eiq \
    'W5|wave.5|wave5|구현.*시작|implementation.*start|implement.*wave|구현.*웨이브|W5.*진입|enter.*W5'
}

if ! is_w5_entry; then
  # 구현 진입 태스크가 아닌 경우: 조용히 통과
  exit 0
fi

# --------------------------------------------------------------------------
# 4. W3 게이트 verdict 판정 (우선순위: bathos gate show > manifest > report)
# --------------------------------------------------------------------------
W3_VERDICT=""
VERDICT_SOURCE=""

# 4-a. [B3 활성] bathos gate show 서브커맨드 — 1순위 verdict 출처(바이너리 SSOT)
#      Phillip의 `bathos gate show`가 api-contracts §C-1/C-2 형식으로 검증됨(2026-06-30 Paul).
#      출력: {"gate_type":"Implementation","verdict":"PASS|CONCERNS|FAIL",...} / 게이트 없음=빈 출력 exit0.
# -----------------------------------------------------------------------
if [[ -x "$BATHOS_BIN" ]]; then
  GATE_OUTPUT="$("$BATHOS_BIN" \
    --state-dir "$STATE_DIR" \
    gate show 2>/dev/null || true)"
  if [[ -n "$GATE_OUTPUT" ]]; then
    # 예상 출력: {"gate_type":"Implementation","verdict":"PASS|CONCERNS|FAIL",...}
    if command -v jq >/dev/null 2>&1; then
      W3_VERDICT="$(printf '%s' "$GATE_OUTPUT" | \
        jq -r '.verdict // empty' 2>/dev/null || true)"
    else
      W3_VERDICT="$(printf '%s' "$GATE_OUTPUT" | \
        grep -o '"verdict"[[:space:]]*:[[:space:]]*"[^"]*"' 2>/dev/null \
        | head -1 | sed 's/.*"\([^"]*\)".*/\1/' || true)"
    fi
    [[ -n "$W3_VERDICT" ]] && VERDICT_SOURCE="bathos gate show"
  fi
fi
# -----------------------------------------------------------------------

# 4-b. manifest.json에서 직접 읽기 (B2 fail-safe 경로)
MANIFEST="$STATE_DIR/manifest.json"
if [[ -z "$W3_VERDICT" && -f "$MANIFEST" && -r "$MANIFEST" ]]; then
  if command -v jq >/dev/null 2>&1; then
    # gates[] 배열에서 Implementation 게이트의 가장 최신 verdict 추출
    # H-6 수정(2026-06-30): .wave → .wave_id (manifest 스키마 필드명 불일치 수정)
    W3_VERDICT="$(jq -r '
      ( .gates // [] )
      | map(select(.gate_type == "Implementation" or .wave_id == "W3"))
      | last
      | .verdict // empty
    ' "$MANIFEST" 2>/dev/null || true)"
    [[ -n "$W3_VERDICT" ]] && VERDICT_SOURCE="manifest.json"
  fi
fi

# 4-c. readiness-report-kr.md 프론트매터에서 직접 읽기 (최후 수단)
READINESS_REPORT="$ART_DIR/03-story-engineering/readiness-report-kr.md"
if [[ -z "$W3_VERDICT" && -f "$READINESS_REPORT" && -r "$READINESS_REPORT" ]]; then
  W3_VERDICT="$(grep -m1 'verdict:[[:space:]]*' "$READINESS_REPORT" 2>/dev/null \
    | sed 's/.*verdict:[[:space:]]*["'"'"']*\([A-Z]*\)["'"'"']*.*/\1/' \
    | grep -E '^(PASS|CONCERNS|FAIL)$' || true)"
  [[ -n "$W3_VERDICT" ]] && VERDICT_SOURCE="readiness-report-kr.md"
fi

# --------------------------------------------------------------------------
# 5. verdict 부재 시 fail-safe
# --------------------------------------------------------------------------
if [[ -z "$W3_VERDICT" ]]; then
  printf '\n[bathos gate-enforce] ! W3 게이트 verdict를 확인할 수 없습니다.\n' >&2
  printf '[bathos gate-enforce] (bathos gate show: B3 미구현 / manifest.json: 미존재 또는 gates[] 없음)\n' >&2
  printf '[bathos gate-enforce] 구현 웨이브 진입을 허용하되, W3 게이트를 별도로 확인하십시오.\n' >&2
  printf '[bathos gate-enforce] 참고: w3-story-engine-design.md §5, exceptions.md §1 E-GATE-FAIL\n' >&2
  # fail-safe: 차단하지 않고 경고만 (verdict 없는 상황은 초기 설정 시 흔함)
  exit 0
fi

# --------------------------------------------------------------------------
# 6. verdict 기반 판정
# --------------------------------------------------------------------------
case "$W3_VERDICT" in
  PASS)
    printf '[bathos gate-enforce] . W3 게이트 PASS (%s) — 구현 웨이브 진입 허용\n' "$VERDICT_SOURCE" >&2
    exit 0
    ;;
  CONCERNS)
    printf '[bathos gate-enforce] ! W3 게이트 CONCERNS (%s) — 리스크 로그 후 진행\n' "$VERDICT_SOURCE" >&2
    printf '[bathos gate-enforce] 비차단 리스크가 있습니다. _state/에 리스크 로그를 확인하세요.\n' >&2
    exit 0
    ;;
  FAIL)
    # ← 유일한 차단 조건 —————————————————————————
    printf '\n[bathos gate-enforce] x W3 게이트 FAIL — 구현 웨이브 진입이 차단되었습니다.\n' >&2
    printf '[bathos gate-enforce] verdict 출처: %s\n' "$VERDICT_SOURCE" >&2
    printf '[bathos gate-enforce] 이유: critical issue ≥ 1 (api-contracts §C-2)\n' >&2
    printf '\n[bathos gate-enforce] 다음 단계:\n' >&2
    printf '[bathos gate-enforce]   1. readiness-report-kr.md의 critical issues를 확인하세요.\n' >&2
    printf '[bathos gate-enforce]   2. W2(기획·아키텍처·디자인)로 반려하여 산출물을 보완하세요.\n' >&2
    printf '[bathos gate-enforce]   3. 재게이트를 통과한 후 구현 웨이브에 진입하세요.\n' >&2
    printf '[bathos gate-enforce]   4. regate_count ≥ 3이면 Paul에게 escalation 하세요.\n' >&2
    printf '[bathos gate-enforce] 참고: w3-story-engine-design.md §5, exceptions.md §1 E-GATE-FAIL\n' >&2
    exit 2  # ← 하드 차단
    ;;
  *)
    printf '[bathos gate-enforce] ! 알 수 없는 verdict 값: "%s" — 통과 처리(보수적)\n' "$W3_VERDICT" >&2
    exit 0
    ;;
esac
