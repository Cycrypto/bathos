#!/usr/bin/env bash
# =============================================================================
# BATHOS M6 — freeze-guard.sh   [Dynamis A1 — 위험 경로 fingerprint 게이트 추가]
# PreToolUse(Write/Edit/MultiEdit): owned_paths 밖 편집 + 위험경로 미승인 변경 차단
# =============================================================================
# 계약:  api-contracts.md §D (PreToolUse freeze), B-1 (owned_paths 불변식)
# 예외:  exceptions.md §5 E-FREEZE-VIOLATION, §4 E-PATH-COLLISION, E-FINGERPRINT-MISS
# DoD:   owned_paths 밖 편집 → exit 2(차단); owned_paths 내 또는 미설정 → exit 0
# 설정:  BATHOS_OWNED_PATHS  콜론(:) 구분 허용 경로 패턴 목록
#          예) "bathos/.claude/hooks/**:bathos/.claude/settings.json"
#          미설정 시: freeze 비활성(모든 경로 허용) — 개발 편의
#        BATHOS_ROLE  현재 에이전트 역할명 (감사로그 actor 필드)
# 안전:  거짓양성(과차단) > 거짓음성(놓침) 원칙
#
# [Dynamis A1 신규 — SS1 · CF-A1, risk-log A-3 재기술]
#   위험 경로(.claude/settings.json · .claude/hooks/* · .github/workflows/* ·
#   _state/manifest.json)로의 Write/Edit/MultiEdit는 owned_paths 소속 여부와
#   **무관하게** 별도의 fingerprint 승인 게이트를 통과해야 한다.
#   - 승인 캐시 히트(`bathos fingerprint check`) → 이 게이트는 통과, 이후 기존
#     owned_paths 검사로 진행.
#   - 미승인(신규 diff) 또는 엔진 바이너리/jq 부재로 판정 불가 → **차단**
#     (freeze 계열은 "거짓양성 > 거짓음성" 원칙이므로 판정 불가 시 안전 측으로
#     기운다 — 다른 신규 훅의 fail-open과 다름, 의도된 설계).
#   - AC2 재기술(risk-log A-3): 이 경로는 Write/Edit/MultiEdit에 대해 **100% 차단**을
#     보장한다(구조적으로 우회 불가 — PreToolUse가 모든 호출을 가로챈다). Bash를 통한
#     우회는 careful-guard.sh가 "확장분에 한해" 차단하며 완전성을 주장하지 않는다.
# =============================================================================
set -uo pipefail

# --------------------------------------------------------------------------
# 1. 경로 해석
# --------------------------------------------------------------------------
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BATHOS_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
PROJECT_DIR="${CLAUDE_PROJECT_DIR:-$BATHOS_ROOT}"
STATE_DIR="${BATHOS_STATE_DIR:-$PROJECT_DIR/.agent-team/_state}"
# B-1 수정(2026-06-30): Rust CLI 단일 writer 경유. 직접 printf 제거로 AuditEntry 비호환 방지.
BATHOS_BIN="${BATHOS_BIN:-$BATHOS_ROOT/core/target/debug/bathos}"

# --------------------------------------------------------------------------
# 2. stdin JSON 파싱: 편집 대상 파일 경로 + (위험경로용) 변경 내용 추출
# --------------------------------------------------------------------------
INPUT="$(cat || true)"
FILE_PATH=""
TOOL=""

if command -v jq >/dev/null 2>&1; then
  TOOL="$(printf '%s' "$INPUT" | jq -r '.tool_name // empty' 2>/dev/null || true)"
  # Write / Edit / MultiEdit 모두 file_path 필드를 사용
  FILE_PATH="$(printf '%s' "$INPUT" | jq -r '.tool_input.file_path // empty' 2>/dev/null || true)"
else
  # jq 없으면 원문에서 file_path 값을 단순 grep(완벽하지 않으나 보수적)
  FILE_PATH="$(printf '%s' "$INPUT" | grep -o '"file_path"[[:space:]]*:[[:space:]]*"[^"]*"' 2>/dev/null \
    | head -1 | sed 's/.*"file_path"[[:space:]]*:[[:space:]]*"\([^"]*\)"/\1/' || true)"
  TOOL="$(printf '%s' "$INPUT" | grep -o '"tool_name"[[:space:]]*:[[:space:]]*"[^"]*"' 2>/dev/null \
    | head -1 | sed 's/.*"tool_name"[[:space:]]*:[[:space:]]*"\([^"]*\)"/\1/' || true)"
fi

# 파일 경로를 수반하지 않는 도구(Bash 등)는 통과
# freeze-guard는 Write/Edit/MultiEdit 파일 쓰기에 특화
case "$TOOL" in
  Write|Edit|MultiEdit) ;;          # 이 도구들만 검사
  "")                   ;;          # 도구명 미확인 → 보수적으로 계속
  *)                   exit 0 ;;    # 그 외 도구는 통과
esac

# 파일 경로가 없으면 통과
[[ -z "$FILE_PATH" ]] && exit 0

# --------------------------------------------------------------------------
# 3. 경로 정규화: PROJECT_DIR 기준 상대 경로로 변환
# --------------------------------------------------------------------------
# 절대 경로인 경우 PROJECT_DIR prefix 제거
if [[ "$FILE_PATH" == "$PROJECT_DIR/"* ]]; then
  REL_PATH="${FILE_PATH#$PROJECT_DIR/}"
elif [[ "$FILE_PATH" == "$BATHOS_ROOT/"* ]]; then
  REL_PATH="${FILE_PATH#$BATHOS_ROOT/}"
else
  # 이미 상대 경로이거나 경로 밖
  REL_PATH="$FILE_PATH"
fi

# --------------------------------------------------------------------------
# 4. 감사 로그 append 헬퍼
# --------------------------------------------------------------------------
_append_audit() {
  local action="$1"
  local safe_path
  safe_path="$(printf '%s' "$REL_PATH" | tr '"\\\n\r' "   ")"

  # B-1 수정(2026-06-30): Rust CLI 경유 단일 writer.
  # 직접 printf 기록 제거 — bash 포맷이 AuditEntry(hash_self 필수·genesis 접두사 불일치)와
  # 비호환이므로 CLI(bathos audit append)가 seq·hash_prev·hash_self를 일관 관리(SSOT).
  # 바이너리 없으면 조용히 통과(|| true — 훅 차단/지연 금지).
  "$BATHOS_BIN" --state-dir "$STATE_DIR" audit append \
    --actor "${BATHOS_ROLE:-hook:freeze}" \
    --action "$action" \
    --target "$safe_path" 2>/dev/null || true
}

# ============================================================================
# 5. [Dynamis A1] 위험 경로 fingerprint 게이트 — owned_paths 검사보다 먼저 수행
# ============================================================================
DANGEROUS_PATTERNS=(
  '*.claude/settings.json'
  '*.claude/hooks/*'
  '*.github/workflows/*'
  '*_state/manifest.json'
)

_is_dangerous_path() {
  local path="$1" pattern
  for pattern in "${DANGEROUS_PATTERNS[@]}"; do
    # shellcheck disable=SC2254
    [[ "$path" == $pattern ]] && return 0
  done
  return 1
}

if _is_dangerous_path "$REL_PATH"; then
  # jq 없으면 diff를 안전하게 재구성할 수 없다 → 판정 불가 → 차단(안전 측)
  if ! command -v jq >/dev/null 2>&1; then
    _append_audit "fingerprint-miss-no-jq"
    printf '\n[bathos freeze] x 위험 경로 변경 차단 — jq가 없어 승인 여부를 확인할 수 없습니다.\n' >&2
    printf '[bathos freeze] 대상: %s\n' "$REL_PATH" >&2
    printf '[bathos freeze] jq를 설치하거나 관리자가 수동 검토 후 승인하세요.\n' >&2
    exit 2
  fi

  # 5-a. tool_input에서 old_string/new_string(Edit) · edits[](MultiEdit) ·
  #      content(Write)로 "pseudo-diff"를 재구성한다. 실제 unified diff 형식은
  #      아니나(줄번호 등 생략), 지문 계산·재현성 목적엔 충분한 결정론적 표현이다.
  #      [Source: story-a1-guard-hardening-kr.md §2 "(추정) diff 획득 경로" 확정]
  TMP_DIFF="$(mktemp "${TMPDIR:-/tmp}/bathos-fp-diff.XXXXXX")"
  trap 'rm -f "$TMP_DIFF"' EXIT

  case "$TOOL" in
    Write)
      printf '%s' "$INPUT" | jq -r --arg relpath "$REL_PATH" '
        "--- /dev/null",
        ("+++ b/" + $relpath),
        "@@ new file @@",
        ((.tool_input.content // "") | split("\n")[] | "+" + .)
      ' > "$TMP_DIFF" 2>/dev/null
      ;;
    Edit)
      printf '%s' "$INPUT" | jq -r --arg relpath "$REL_PATH" '
        ("--- a/" + $relpath),
        ("+++ b/" + $relpath),
        "@@ edit @@",
        ((.tool_input.old_string // "") | split("\n")[] | "-" + .),
        ((.tool_input.new_string // "") | split("\n")[] | "+" + .)
      ' > "$TMP_DIFF" 2>/dev/null
      ;;
    MultiEdit)
      printf '%s' "$INPUT" | jq -r --arg relpath "$REL_PATH" '
        ("--- a/" + $relpath),
        ("+++ b/" + $relpath),
        ((.tool_input.edits // []) | to_entries[] |
          ("@@ edit " + (.key | tostring) + " @@"),
          ((.value.old_string // "") | split("\n")[] | "-" + .),
          ((.value.new_string // "") | split("\n")[] | "+" + .)
        )
      ' > "$TMP_DIFF" 2>/dev/null
      ;;
  esac

  # 5-b. 엔진 바이너리 부재 → 캐시 조회 불가 → 판정 불가 → 차단(안전 측)
  if [[ ! -x "$BATHOS_BIN" ]]; then
    _append_audit "fingerprint-miss-no-bin"
    printf '\n[bathos freeze] x 위험 경로 변경 차단 — 엔진 바이너리가 없어 승인 캐시를 확인할 수 없습니다.\n' >&2
    printf '[bathos freeze] 대상: %s (BATHOS_BIN=%s)\n' "$REL_PATH" "$BATHOS_BIN" >&2
    printf '[bathos freeze] cargo build -p bathos-cli 로 엔진을 빌드한 뒤 다시 시도하세요.\n' >&2
    exit 2
  fi

  FP_OUTPUT="$("$BATHOS_BIN" --state-dir "$STATE_DIR" fingerprint check --diff-file "$TMP_DIFF" 2>/dev/null || true)"
  FP_STATUS="$(printf '%s' "$FP_OUTPUT" | jq -r '.status // empty' 2>/dev/null || true)"
  FP_HASH="$(printf '%s' "$FP_OUTPUT" | jq -r '.hash // empty' 2>/dev/null || true)"

  if [[ "$FP_STATUS" == "approved" ]]; then
    _append_audit "fingerprint-hit"
    printf '[bathos freeze] . 위험 경로 변경 — 기승인 지문 확인(재승인 생략): %s\n' "$REL_PATH" >&2
    # 승인됐어도 owned_paths 검사는 계속 진행(§6) — 소유권과 위험승인은 별개 축.
  else
    _append_audit "fingerprint-miss"
    printf '\n[bathos freeze] x 위험 경로 미승인 변경 — 차단되었습니다.\n' >&2
    printf '[bathos freeze] 대상: %s\n' "$REL_PATH" >&2
    printf '[bathos freeze] 지문(hash): %s\n' "${FP_HASH:-(계산 실패)}" >&2
    printf '[bathos freeze] 승인 후 재시도하세요:\n' >&2
    printf '[bathos freeze]   "$BATHOS_BIN" --state-dir "%s" fingerprint approve --hash %s --actor <역할> --scope <hooks|ci|w5-entry 등>\n' \
      "$STATE_DIR" "${FP_HASH:-<hash>}" >&2
    printf '[bathos freeze] 동일 변경(공백/줄바꿈 차이 포함)은 승인 후 재승인 없이 통과합니다.\n' >&2
    printf '[bathos freeze] 참고: exceptions.md E-FINGERPRINT-MISS, story-a1-guard-hardening-kr.md AC2/AC3\n' >&2
    exit 2
  fi
fi

# --------------------------------------------------------------------------
# 6. BATHOS_OWNED_PATHS 미설정 시 freeze 비활성 (기존 소유권 검사 — 위험경로 게이트와 별개)
# --------------------------------------------------------------------------
OWNED_PATHS="${BATHOS_OWNED_PATHS:-}"
if [[ -z "$OWNED_PATHS" ]]; then
  # freeze 설정 없음 → 통과 (Paul이 스폰 시 소유 경로를 명시할 책임)
  exit 0
fi

# --------------------------------------------------------------------------
# 7. owned_paths 패턴 매칭 함수
#    패턴 형식:
#      "foo/bar/**"   → foo/bar/ 하위 모든 경로 허용 (prefix 매칭)
#      "foo/bar/*.md" → bash glob (단일 세그먼트 와일드카드)
#      "foo/bar.txt"  → 정확 일치
#    ⚠️  risk-log A-1(Thomas H-1): 이 매칭은 변수 패턴에 대해 brace 확장({a,b})을
#    하지 않는다(bash 제약, 실측 재현됨). 스폰 시 BATHOS_OWNED_PATHS는 **중괄호
#    없이 콜론 구분 전개형**으로 주입해야 한다(risk-log §C 확정값 참고).
# --------------------------------------------------------------------------
is_owned() {
  local path="$1"
  local IFS=':'
  local pattern
  for pattern in $OWNED_PATHS; do
    # 빈 패턴 스킵
    [[ -z "$pattern" ]] && continue

    if [[ "$pattern" == */** ]]; then
      # "prefix/**" → prefix 디렉터리 이하 모든 파일
      local prefix="${pattern%/**}/"
      [[ "$path" == "$prefix"* ]] && return 0
    else
      # bash glob 매칭 (extglob 불필요한 단순 패턴)
      # shellcheck disable=SC2254
      [[ "$path" == $pattern ]] && return 0
    fi
  done
  return 1
}

# --------------------------------------------------------------------------
# 8. 소유 경로 검사
# --------------------------------------------------------------------------
if is_owned "$REL_PATH"; then
  # 소유 경로 내 → 통과
  exit 0
fi

# 소유 경로 밖 → 차단
_append_audit "freeze-violation"

printf '\n[bathos freeze] x 소유 경로 밖 편집이 차단되었습니다.\n' >&2
printf '[bathos freeze] 대상 파일: %s\n' "$REL_PATH" >&2
printf '[bathos freeze] 허용 경로 (BATHOS_OWNED_PATHS):\n' >&2
# 각 허용 경로를 줄별로 출력
IFS=':' read -ra _PATHS <<< "$OWNED_PATHS"
for _p in "${_PATHS[@]}"; do
  [[ -n "$_p" ]] && printf '[bathos freeze]   - %s\n' "$_p" >&2
done
printf '[bathos freeze] 소유 경로 밖을 수정해야 한다면:\n' >&2
printf '[bathos freeze]   1. 해당 경로 소유자와 메시지로 합의하거나\n' >&2
printf '[bathos freeze]   2. Paul(리드)에게 보고하십시오.\n' >&2
printf '[bathos freeze] 참고: exceptions.md §5 E-FREEZE-VIOLATION, §4 E-PATH-COLLISION\n' >&2
exit 2  # ← 하드 차단
