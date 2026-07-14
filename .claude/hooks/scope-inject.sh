#!/usr/bin/env bash
# =============================================================================
# BATHOS Dynamis A2 — scope-inject.sh   (SS3 · CF-A2)
# PreToolUse(Read/Edit/Write/MultiEdit): 상위 디렉터리 스코프 규칙 자동 부착(advisory)
# =============================================================================
# 계약:  story-a2-scope-inject-kr.md, hook-and-gate-design-kr.md §2
# 역할 분리(핵심): scope-inject = 가시화(advisory, 차단 안 함) / freeze-guard = 강제(차단).
#   이 훅은 어떤 경우에도 exit 2를 반환하지 않는다 — 항상 exit 0.
# open-swe 사상: SubdirAgentsReadMiddleware(파일 read 시 상위 AGENTS.md 자동 부착)
#
# DoD: 상위에 AGENTS.md/OWNERS/CLAUDE.md 있으면 additionalContext로 주입(AC1).
#      없으면 무출력 통과(AC2, fail-open). 탐색 깊이 상한 8(AC3).
#      8KB 초과 파일은 절단 주입(AC4). jq 없으면 grep 폴백(AC5).
# =============================================================================
set -uo pipefail

# --------------------------------------------------------------------------
# 1. 경로 해석 (기존 훅과 동일 골격)
# --------------------------------------------------------------------------
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BATHOS_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
PROJECT_DIR="${CLAUDE_PROJECT_DIR:-$BATHOS_ROOT}"

MAX_DEPTH=8
RULE_FILENAMES=("AGENTS.md" "OWNERS" "CLAUDE.md")
MAX_BYTES=8192   # AC4: 8KB 초과 시 절단

# --------------------------------------------------------------------------
# 2. stdin JSON 파싱: 대상 파일 경로 추출 (freeze-guard.sh와 동일 골격)
# --------------------------------------------------------------------------
INPUT="$(cat || true)"
FILE_PATH=""
TOOL=""

if command -v jq >/dev/null 2>&1; then
  TOOL="$(printf '%s' "$INPUT" | jq -r '.tool_name // empty' 2>/dev/null || true)"
  FILE_PATH="$(printf '%s' "$INPUT" | jq -r '.tool_input.file_path // empty' 2>/dev/null || true)"
else
  FILE_PATH="$(printf '%s' "$INPUT" | grep -o '"file_path"[[:space:]]*:[[:space:]]*"[^"]*"' 2>/dev/null \
    | head -1 | sed 's/.*"file_path"[[:space:]]*:[[:space:]]*"\([^"]*\)"/\1/' || true)"
  TOOL="$(printf '%s' "$INPUT" | grep -o '"tool_name"[[:space:]]*:[[:space:]]*"[^"]*"' 2>/dev/null \
    | head -1 | sed 's/.*"tool_name"[[:space:]]*:[[:space:]]*"\([^"]*\)"/\1/' || true)"
fi

case "$TOOL" in
  Read|Edit|Write|MultiEdit) ;;
  "") ;;                        # 도구명 미확인 → 보수적으로 계속
  *) exit 0 ;;                  # 그 외 도구는 통과(advisory 전용 훅이므로 조용히)
esac

[[ -z "$FILE_PATH" ]] && exit 0

# --------------------------------------------------------------------------
# 3. 대상 디렉터리 절대경로화 (파일이 아직 없어도 됨 — Write 신규 생성 케이스)
# --------------------------------------------------------------------------
TARGET_DIR="$(dirname "$FILE_PATH")"
if [[ "$TARGET_DIR" != /* ]]; then
  TARGET_DIR="$PROJECT_DIR/$TARGET_DIR"
fi
if [[ -d "$TARGET_DIR" ]]; then
  TARGET_DIR="$(cd "$TARGET_DIR" 2>/dev/null && pwd || printf '%s' "$TARGET_DIR")"
fi
PROJECT_DIR_ABS="$(cd "$PROJECT_DIR" 2>/dev/null && pwd || printf '%s' "$PROJECT_DIR")"

# --------------------------------------------------------------------------
# 4. 상위 디렉터리를 거슬러 올라가며 규칙 파일 수집
#    - 프로젝트 루트를 넘어서지 않는다(안전 경계 — 무관한 상위 디렉터리 스캔 방지).
#    - 깊이 상한 MAX_DEPTH(8) — AC3.
#    - 근접(target 디렉터리에 가까움) 우선순위가 있음을 마지막에 명시(AC1).
# --------------------------------------------------------------------------
declare -a FOUND_NEAR_TO_ROOT=()  # 근접 → 루트 순으로 채움. 표시 시 reverse.
CUR="$TARGET_DIR"
DEPTH=0
while [[ "$DEPTH" -lt "$MAX_DEPTH" ]]; do
  for fname in "${RULE_FILENAMES[@]}"; do
    if [[ -f "$CUR/$fname" ]]; then
      FOUND_NEAR_TO_ROOT+=("$CUR/$fname")
    fi
  done
  # 프로젝트 루트에 도달했으면 종료(그 이상은 스캔하지 않음 — 안전 경계)
  if [[ "$CUR" == "$PROJECT_DIR_ABS" ]]; then
    break
  fi
  PARENT="$(dirname "$CUR")"
  # 파일시스템 루트에 도달하거나 더 이상 올라갈 수 없으면 종료
  if [[ "$PARENT" == "$CUR" ]]; then
    break
  fi
  CUR="$PARENT"
  DEPTH=$((DEPTH + 1))
done

# 아무 규칙 파일도 없으면 무해 통과 (AC2, fail-open)
if [[ "${#FOUND_NEAR_TO_ROOT[@]}" -eq 0 ]]; then
  exit 0
fi

# --------------------------------------------------------------------------
# 5. 동일 세션 내 반복 주입 억제(캐시) — CONCERNS 대응, 구현 재량
#    Claude Code 세션은 훅 호출마다 새 bash 프로세스이므로 진짜 "세션 상태"가
#    없다. 대신 부모 프로세스(Claude Code 본체) PID를 세션 프록시로 사용해
#    동일 세션 동안은 동일 규칙 파일 집합을 1회만 주입한다(컨텍스트 비대 방지).
#    한계(문서화): PPID가 재사용되거나 조기 회수되면 오탐 가능 — 비차단 advisory
#    훅이므로 최악의 경우도 "정보 재주입 생략"에 그친다(안전).
# --------------------------------------------------------------------------
CACHE_FILE="${TMPDIR:-/tmp}/bathos-scope-inject-seen-${PPID}"
SIGNATURE="$(printf '%s\n' "${FOUND_NEAR_TO_ROOT[@]}" | tr '\n' '|')"
if [[ -f "$CACHE_FILE" ]] && grep -qF "$SIGNATURE" "$CACHE_FILE" 2>/dev/null; then
  exit 0  # 이미 이번 세션에서 동일 규칙 집합을 주입함 — 재주입 생략
fi
printf '%s\n' "$SIGNATURE" >> "$CACHE_FILE" 2>/dev/null || true

# --------------------------------------------------------------------------
# 6. additionalContext 텍스트 조립 (루트 → 근접 순, AC1 — 근접 우선 명시)
# --------------------------------------------------------------------------
CONTEXT_LINES=()
CONTEXT_LINES+=("[스코프 규칙] 아래 상위 디렉터리에 적용 규칙 파일이 있습니다(루트→근접 순, 근접 파일이 우선):")

# 배열을 역순(루트가 먼저 오도록)으로 순회
for (( idx=${#FOUND_NEAR_TO_ROOT[@]}-1; idx>=0; idx-- )); do
  rule_path="${FOUND_NEAR_TO_ROOT[$idx]}"
  rel_path="${rule_path#$PROJECT_DIR_ABS/}"
  size=$(wc -c < "$rule_path" 2>/dev/null || echo 0)

  CONTEXT_LINES+=("--- $rel_path ---")
  if [[ "$size" -gt "$MAX_BYTES" ]]; then
    # AC4: 8KB 초과 → 절단 주입(사실 명시)
    excerpt="$(head -c "$MAX_BYTES" "$rule_path" 2>/dev/null)"
    CONTEXT_LINES+=("$excerpt")
    CONTEXT_LINES+=("...(이하 생략 — 원본 ${size}바이트 중 ${MAX_BYTES}바이트만 표시. 전문은 $rel_path 참고)")
  else
    content="$(cat "$rule_path" 2>/dev/null)"
    CONTEXT_LINES+=("$content")
  fi
done

ADDITIONAL_CONTEXT="$(printf '%s\n' "${CONTEXT_LINES[@]}")"

# --------------------------------------------------------------------------
# 7. stdout JSON 출력 (advisory — 차단 없음, hookSpecificOutput.additionalContext)
# --------------------------------------------------------------------------
if command -v jq >/dev/null 2>&1; then
  jq -n --arg ctx "$ADDITIONAL_CONTEXT" \
    '{"hookSpecificOutput":{"hookEventName":"PreToolUse","additionalContext":$ctx}}'
else
  # jq 없으면 최소 이스케이프로 수동 조립(AC5 — 완벽한 JSON 이스케이프는 아니나
  # 개행/따옴표만 처리하는 보수적 폴백. 실패해도 이 훅은 차단하지 않는다).
  escaped="$(printf '%s' "$ADDITIONAL_CONTEXT" | sed 's/\\/\\\\/g; s/"/\\"/g' | awk '{printf "%s\\n", $0}')"
  printf '{"hookSpecificOutput":{"hookEventName":"PreToolUse","additionalContext":"%s"}}\n' "$escaped"
fi

exit 0
