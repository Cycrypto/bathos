#!/usr/bin/env bash
# =============================================================================
# BATHOS P4 — codex-adapter/hooks/pretooluse-gate.sh
# Codex PreToolUse 훅: W3 Implementation 게이트가 FAIL인 동안 "구현 웨이브(W5)
# 진입"을 뜻하는 도구 호출만 exit 2로 물리 차단한다. 그 외 모든 상황은 통과.
#
# 설계 원본: .agent-team/04-architecture/w2-runtime-p4-design-kr.md §B2 (James)
# 의미론은 .claude/hooks/gate-enforce.sh(Claude Code용)와 문자 그대로 동일하게
# 유지한다(ADR-P4-2) — 런타임이 달라도 게이트 정책은 하나다.
#
# stdin/stdout/exit 계약:
#   stdin  = Codex PreToolUse JSON 1개(session_id/turn_id/tool_name/tool_input/
#            cwd/hook_event_name 등). 비-JSON/빈 입력 -> fail-safe exit 0.
#   stdout = 사용 안 함(비움 유지 — exit 0에서 무출력=성공).
#   stderr = 차단 사유·경고만.
#   exit   = 0(통과) | 2(차단). 그 외 코드는 절대 사용하지 않는다 — 내부 오류도
#            0으로 수렴(훅 오류가 개발 전체를 막으면 안 된다는 careful 원칙).
#
# 트리거는 "모든 도구 호출마다 게이트를 조회"하지 않는다(불필요한 지연·감사
# 소음) — T1(제품 소스 쓰기) 또는 T2(웨이브 진입 명령) 감지 시에만 bathos를
# 호출한다(ADR-P4-1). 비트리거는 아래 §3에서 bathos 호출 전에 즉시 exit 0.
# =============================================================================
set -uo pipefail

PROG="pretooluse-gate.sh"

# --------------------------------------------------------------------------
# 1. stdin 전량 읽기 + 평탄화 (E-B1: 멀티라인/CRLF stdin 대응)
# --------------------------------------------------------------------------
INPUT="$(cat 2>/dev/null || true)"
INPUT_FLAT="$(printf '%s' "$INPUT" | tr '\n\r' '  ')"

# --------------------------------------------------------------------------
# 2. 공통 JSON 파싱 함수 (§C0 — jq 금지, 네이티브 grep/sed만)
# --------------------------------------------------------------------------
# 최상위 문자열 필드 추출. 값 내 이스케이프 따옴표(\")는 미지원(알려진 한계) —
# 우리가 읽는 필드(hook_event_name/tool_name/cwd)는 런타임 생성 식별자·경로라
# 이스케이프 포함 가능성이 사실상 없다. 오파싱 시에도 fail-safe 통과로
# 수렴하므로(차단 아님) 안전한 방향이다.
_jstr() {
  printf '%s' "$INPUT_FLAT" | grep -o "\"$1\"[[:space:]]*:[[:space:]]*\"[^\"]*\"" \
    | head -1 | sed 's/.*:[[:space:]]*"\(.*\)"$/\1/'
}

EVENT="$(_jstr hook_event_name)"
TOOL="$(_jstr tool_name)"
CWD="$(_jstr cwd)"

# 잘못 배선돼도(다른 이벤트에 물려도) 무해하게 통과.
if [ -n "$EVENT" ] && [ "$EVENT" != "PreToolUse" ]; then
  exit 0
fi

# --------------------------------------------------------------------------
# 3. 경로 해석 (§B1 — Codex는 CLAUDE_PROJECT_DIR을 주지 않으므로 stdin cwd 사용)
# --------------------------------------------------------------------------
PROJECT_DIR="${BATHOS_PROJECT_DIR:-${CWD:-$PWD}}"
STATE_DIR="${BATHOS_STATE_DIR:-$PROJECT_DIR/.agent-team/_state}"

# BATHOS 프로젝트가 아니면(=_state 없음) 조용히 통과 — 다른 프로젝트에서
# Codex를 써도 이 훅이 방해하지 않는다(session-start.sh와 동일 원칙).
if [ ! -d "$STATE_DIR" ]; then
  exit 0
fi

# bathos 바이너리 후보 순회 — 첫 실행가능 파일을 채택.
BATHOS_BIN="${BATHOS_BIN:-}"
if [ -z "$BATHOS_BIN" ]; then
  for cand in \
    "$PROJECT_DIR/core/target/release/bathos" \
    "$PROJECT_DIR/core/target/debug/bathos"
  do
    if [ -x "$cand" ]; then BATHOS_BIN="$cand"; break; fi
  done
  if [ -z "$BATHOS_BIN" ] && command -v bathos >/dev/null 2>&1; then
    BATHOS_BIN="$(command -v bathos)"
  fi
fi

# --------------------------------------------------------------------------
# 4. 트리거 판정 (bathos 호출 전, 저비용 — T1: 소스 쓰기 / T2: 웨이브 진입 명령)
# --------------------------------------------------------------------------
# T1 소스 경로 패턴(env로 오버라이드 가능). ".agent-team/" 전용 언급은 제외
# (문서·산출물 쓰기는 게이트 대상이 아니다).
SRC_ERE="${BATHOS_GATE_SRC_ERE:-(^|[^A-Za-z0-9_./-])(src|core/crates|core/src|codex-adapter/hooks)/}"

# T2 웨이브 진입 명령 ERE (gate-enforce.sh is_w5_entry와 정렬).
W5_ENTRY_ERE='bathos[[:space:]]+wave[[:space:]]+(advance|start|enter)|wave.?5|wave.?4-implement|W5.*(진입|시작)|implement(ation)?[[:space:]]+(start|begin)'

TRIGGER=""

case "$TOOL" in
  apply_patch|write_file|edit|create_file)
    # 예외: ".agent-team/" 만 언급되면 트리거 아님(문서·산출물 쓰기는 게이트
    # 대상이 아니다) — ".agent-team/<...>" 경로 세그먼트를 원문에서 지운 뒤에도
    # SRC_ERE가 매치되면, 그 매치는 .agent-team/ 바깥의 진짜 소스 경로라는 뜻.
    # 과탐(문서 안에 우연히 "src/" 문자열)은 FAIL 상태에서만 영향이 있으므로
    # careful 원칙(과차단 > 놓침)에 따라 수용한다(ADR-P4-1).
    NON_AGENT_TEAM_FLAT="$(printf '%s' "$INPUT_FLAT" | sed -E 's#\.agent-team/[A-Za-z0-9_./-]*##g')"
    if printf '%s' "$NON_AGENT_TEAM_FLAT" | grep -Eq "$SRC_ERE"; then
      TRIGGER="source-write"
    fi
    ;;
esac

if [ -z "$TRIGGER" ]; then
  case "$TOOL" in
    Bash|bash|shell|local_shell|exec_command)
      if printf '%s' "$INPUT_FLAT" | grep -Eiq "$W5_ENTRY_ERE"; then
        TRIGGER="wave-entry-command"
      fi
      ;;
  esac
fi

# 비트리거 -> 즉시 exit 0, bathos를 호출하지 않는다(불필요한 프로세스 스폰·
# 감사 소음 방지 — 테스트 B-5가 이를 계약화한다).
if [ -z "$TRIGGER" ]; then
  exit 0
fi

# --------------------------------------------------------------------------
# 5. verdict 3단 조회 (gate-enforce.sh §4와 동일 우선순위)
# --------------------------------------------------------------------------
W3_VERDICT=""
VERDICT_SOURCE=""

# 5-a. bathos gate show — 1순위(바이너리 SSOT)
if [ -n "$BATHOS_BIN" ] && [ -x "$BATHOS_BIN" ]; then
  GATE_OUTPUT="$("$BATHOS_BIN" -s "$STATE_DIR" gate show 2>/dev/null || true)"
  if [ -n "$GATE_OUTPUT" ]; then
    W3_VERDICT="$(printf '%s' "$GATE_OUTPUT" \
      | grep -o '"verdict"[[:space:]]*:[[:space:]]*"[^"]*"' \
      | head -1 | sed 's/.*"\([^"]*\)"$/\1/')"
    [ -n "$W3_VERDICT" ] && VERDICT_SOURCE="bathos gate show"
  fi
fi

# 5-b. manifest.json — jq 없이는 gates[]에서 "마지막 Implementation" 선택이
# 신뢰 불가하므로 비단순 파싱은 하지 않는다(설계 결정: 부정확한 파싱으로
# 오차단하느니 fail-safe). "gate_type":"Implementation" 존재 시에만 그
# 언저리의 verdict를 보수적으로 시도하고, 애매하면 포기(빈 값)한다.
MANIFEST="$STATE_DIR/manifest.json"
if [ -z "$W3_VERDICT" ] && [ -f "$MANIFEST" ] && [ -r "$MANIFEST" ]; then
  if grep -q '"gate_type"[[:space:]]*:[[:space:]]*"Implementation"' "$MANIFEST" 2>/dev/null; then
    CAND="$(tr -d '\n' < "$MANIFEST" \
      | grep -o '"gate_type"[[:space:]]*:[[:space:]]*"Implementation"[^}]*"verdict"[[:space:]]*:[[:space:]]*"[^"]*"' \
      | head -1 \
      | grep -o '"verdict"[[:space:]]*:[[:space:]]*"[^"]*"' \
      | tail -1 \
      | sed 's/.*"\([^"]*\)"$/\1/' || true)"
    if printf '%s' "$CAND" | grep -Eq '^(PASS|CONCERNS|FAIL)$'; then
      W3_VERDICT="$CAND"
      VERDICT_SOURCE="manifest.json"
    fi
  fi
fi

# 5-c. readiness-report-kr.md 프론트매터 — 최후 수단(gate-enforce.sh §4-c와 동일 sed)
READINESS_REPORT="$PROJECT_DIR/.agent-team/03-story-engineering/readiness-report-kr.md"
if [ -z "$W3_VERDICT" ] && [ -f "$READINESS_REPORT" ] && [ -r "$READINESS_REPORT" ]; then
  W3_VERDICT="$(grep -m1 'verdict:[[:space:]]*' "$READINESS_REPORT" 2>/dev/null \
    | sed 's/.*verdict:[[:space:]]*["'"'"']*\([A-Z]*\)["'"'"']*.*/\1/' \
    | grep -E '^(PASS|CONCERNS|FAIL)$' || true)"
  [ -n "$W3_VERDICT" ] && VERDICT_SOURCE="readiness-report-kr.md"
fi

# --------------------------------------------------------------------------
# 6. 감사 로그 append 헬퍼 (FAIL 차단 시에만, best-effort — careful-guard.sh §5 패턴 재사용)
# --------------------------------------------------------------------------
_append_audit_block() {
  [ -n "$BATHOS_BIN" ] && [ -x "$BATHOS_BIN" ] || return 0
  "$BATHOS_BIN" -s "$STATE_DIR" audit append \
    --actor "hook:codex-gate" \
    --action "block" \
    --target "$TRIGGER:$TOOL" 2>/dev/null || true
}

# --------------------------------------------------------------------------
# 7. verdict 부재 시 fail-safe
# --------------------------------------------------------------------------
if [ -z "$W3_VERDICT" ]; then
  printf '[codex-gate] verdict 확인 불가 — fail-safe 통과 (bathos/state/report 미존재)\n' >&2
  exit 0
fi

# --------------------------------------------------------------------------
# 8. verdict 기반 판정 (ADR-P4-2: FAIL만 차단, 그 외 전부 fail-safe 통과)
# --------------------------------------------------------------------------
case "$W3_VERDICT" in
  PASS)
    printf '[codex-gate] W3 게이트 PASS (%s) — 구현 웨이브 진입 허용\n' "$VERDICT_SOURCE" >&2
    exit 0
    ;;
  CONCERNS)
    printf '[codex-gate] W3 CONCERNS(%s) — 비차단 리스크, _state/ 확인\n' "$VERDICT_SOURCE" >&2
    exit 0
    ;;
  FAIL)
    _append_audit_block
    cat >&2 <<EOF
[bathos codex-gate] BLOCKED: W3 Implementation gate is FAIL (source: $VERDICT_SOURCE).
Trigger: $TRIGGER (tool: $TOOL)
구현 웨이브(W5) 진입이 차단되었습니다.
1) .agent-team/03-story-engineering/readiness-report-kr.md의 critical issues 확인
2) W2 산출물 보완 후 재게이트 (bathos gate verdict ...)
3) 재게이트 PASS/CONCERNS 후 이 작업을 다시 시도
참고: w3-story-engine-design.md §5 · exceptions.md §1 E-GATE-FAIL
EOF
    exit 2
    ;;
  *)
    printf '[codex-gate] 미지 verdict "%s" — 보수적 통과\n' "$W3_VERDICT" >&2
    exit 0
    ;;
esac
