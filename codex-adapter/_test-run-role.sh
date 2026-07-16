#!/usr/bin/env bash
# =============================================================================
# BATHOS  codex-adapter/_test-run-role.sh  —  run-role.sh 스모크 테스트 (T8)
#
# 실제 Codex CLI 설치·라이브 인증 세션 없이 `run-role.sh`(§A3.3)의 계약만
# 검증한다: 스텁 `codex`(login status/exec를 흉내내는 가짜 바이너리)와 최소
# 픽스처 프로젝트(.claude/agents/_base/*.md, ETHOS.md, .codex/agents/*.toml)를
# 만들어 각 exit code 분기가 설계대로 발화하는지 단정한다.
#
# 자리매김: codex-adapter/hooks/_test-codex-hooks.sh와 동일 스타일(카운터+함수,
# bash 3.2 호환, 연관배열 금지). run-role.sh는 hooks/ 밖(codex-adapter/ 최상위)
# 이므로 테스트도 같은 위치에 둔다.
#
# 검증 매트릭스(design §B5 T8 + 보일더오션 확장):
#   T8a codex 미설치(PATH 조작)                → exit 3 E-CODEX-ABSENT
#   T8b codex 있음 + .toml 부재                 → exit 1 (안내 메시지 포함, 자동생성 안 함)
#   T8c codex login status가 "미인증" 패턴      → exit 4 E-CODEX-AUTH (선제 차단)
#   T8d login status 서브커맨드 자체가 실패     → 비차단(경고만) + 계속 진행
#   T8e 정상 픽스처 + --dry-run                 → exit 0, 프롬프트 보존, inbox DRY-RUN
#   T8f 정상 픽스처 + 실행(exec 성공 스텁)      → exit 0, inbox DONE
#   T8g codex exec가 인증 실패로 종료           → exit 4 E-CODEX-AUTH(사후 판정)
#   T8h codex exec가 일반 오류로 종료           → exit 1, inbox FAIL
#   T8i role-slug 형식 불량                      → exit 1 E-CODEX-ROLE-INVALID
#   T8j task-file 부재                           → exit 1
#
# 실행: bash codex-adapter/_test-run-role.sh
# =============================================================================
set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
RUN_ROLE="$SCRIPT_DIR/run-role.sh"

RED='\033[0;31m'; GREEN='\033[0;32m'; NC='\033[0m'
PASS_COUNT=0
FAIL_COUNT=0

ok()  { printf "${GREEN}[PASS]${NC} %s\n" "$*"; PASS_COUNT=$((PASS_COUNT+1)); }
bad() { printf "${RED}[FAIL]${NC} %s\n" "$*" >&2; FAIL_COUNT=$((FAIL_COUNT+1)); }

assert_exit() {
  local desc="$1" expected="$2" actual="$3"
  if [ "$actual" = "$expected" ]; then
    ok "$desc (exit=$actual)"
  else
    bad "$desc — 예상 exit=$expected, 실제 exit=$actual"
  fi
}

assert_contains() {
  local desc="$1" haystack="$2" needle="$3"
  if printf '%s' "$haystack" | grep -qF "$needle"; then
    ok "$desc"
  else
    bad "$desc — '$needle' 를 출력에서 찾지 못함: $haystack"
  fi
}

WORK="$(mktemp -d 2>/dev/null || mktemp -d -t bathosrunrole)"
cleanup() { rm -rf "$WORK" >/dev/null 2>&1 || true; }
trap cleanup EXIT

# --------------------------------------------------------------------------
# 픽스처 프로젝트 — 최소한의 agent base md + ETHOS + task-file
# --------------------------------------------------------------------------
PROJ="$WORK/fixture-project"
mkdir -p "$PROJ/.claude/agents/_base" "$PROJ/.codex/agents"
cat > "$PROJ/ETHOS.md" <<'EOF'
# 빌더 ETHOS (테스트 픽스처 요약본)
User Sovereignty / Boil the Ocean / Search Before Building.
EOF
cat > "$PROJ/.claude/agents/_base/08-test-role.md" <<'EOF'
---
role_number: 8
name: test-role
slug: test-role
model: claude-sonnet-5
spawnable: true
---

# Test Role [base]

이것은 run-role.sh 테스트용 최소 역할 지침이다.
EOF
TASK_FILE="$WORK/task-brief.md"
cat > "$TASK_FILE" <<'EOF'
# 테스트 태스크 브리프
입력 경로: (없음) / 소유 경로: (없음) / DoD: 아무 것도 안 해도 통과.
EOF

TOML_PATH="$PROJ/.codex/agents/test-role.toml"

# --------------------------------------------------------------------------
# 스텁 codex 생성 헬퍼 — $1=STUB_DIR $2=login_mode(ok|authfail|unsupported)
#                        $3=exec_mode(ok|authfail|generic-fail)
# --------------------------------------------------------------------------
make_stub_codex() {
  local stub_dir="$1" login_mode="$2" exec_mode="$3"
  mkdir -p "$stub_dir"
  cat > "$stub_dir/codex" <<STUB_EOF
#!/usr/bin/env bash
if [ "\$1" = "login" ] && [ "\$2" = "status" ]; then
  case "$login_mode" in
    ok) echo "Logged in as test@example.com"; exit 0 ;;
    authfail) echo "Error: not logged in. Run 'codex login' first."; exit 1 ;;
    unsupported) echo "error: unrecognized subcommand 'login'"; exit 2 ;;
  esac
fi
if [ "\$1" = "exec" ]; then
  case "$exec_mode" in
    ok) echo "codex exec stub: 완료"; exit 0 ;;
    authfail) echo "Error: unauthorized — please login again."; exit 1 ;;
    generic-fail) echo "Error: something went wrong (stub)"; exit 7 ;;
  esac
fi
echo "unhandled stub invocation: \$*" >&2
exit 9
STUB_EOF
  chmod +x "$stub_dir/codex"
}

# --------------------------------------------------------------------------
# T8a: codex 미설치(PATH 조작) → exit 3 E-CODEX-ABSENT
# --------------------------------------------------------------------------
printf '\n== T8a: codex 부재 → exit 3 ==\n'
OUT="$(env -i PATH="/usr/bin:/bin" HOME="$WORK" "$RUN_ROLE" test-role "$TASK_FILE" --project "$PROJ" 2>&1)"; RC=$?
assert_exit "codex 미설치 시 exit 3" 3 "$RC"
assert_contains "E-CODEX-ABSENT 메시지 출력" "$OUT" "E-CODEX-ABSENT"
INBOX="$PROJ/.agent-team/_state/panes/inbox"
if ls "$INBOX"/codex-test-role-*.txt >/dev/null 2>&1; then
  ok "T8a inbox FAIL 마커 생성됨"
  rm -f "$INBOX"/codex-test-role-*.txt
else
  bad "T8a inbox 마커 미생성"
fi

# --------------------------------------------------------------------------
# T8b: codex 있음(login ok) + .toml 부재 → exit 1
# --------------------------------------------------------------------------
printf '\n== T8b: codex 있음 + .toml 부재 → exit 1 ==\n'
STUB1="$WORK/stub1"
make_stub_codex "$STUB1" ok ok
OUT="$(env -i PATH="$STUB1:/usr/bin:/bin" HOME="$WORK" "$RUN_ROLE" test-role "$TASK_FILE" --project "$PROJ" 2>&1)"; RC=$?
assert_exit "TOML 부재 시 exit 1" 1 "$RC"
assert_contains "E-CODEX-TOML-ABSENT 메시지 출력" "$OUT" "E-CODEX-TOML-ABSENT"
assert_contains "to-codex.sh 안내(자동실행 아님) 포함" "$OUT" "to-codex.sh"
rm -f "$INBOX"/codex-test-role-*.txt 2>/dev/null || true

# 이제부터의 테스트를 위해 .toml을 마련한다(정상 케이스 검증에 필요).
cat > "$TOML_PATH" <<'EOF'
name = "test-role"
description = "test"
developer_instructions = """
test
"""
EOF

# --------------------------------------------------------------------------
# T8c: login status가 명백한 미인증 패턴 → exit 4 (선제 차단, exec 호출 전)
# --------------------------------------------------------------------------
printf '\n== T8c: login status 미인증 패턴 → exit 4 ==\n'
STUB2="$WORK/stub2"
make_stub_codex "$STUB2" authfail ok
OUT="$(env -i PATH="$STUB2:/usr/bin:/bin" HOME="$WORK" "$RUN_ROLE" test-role "$TASK_FILE" --project "$PROJ" 2>&1)"; RC=$?
assert_exit "미인증 패턴 시 exit 4" 4 "$RC"
assert_contains "E-CODEX-AUTH 메시지 출력" "$OUT" "E-CODEX-AUTH"
rm -f "$INBOX"/codex-test-role-*.txt 2>/dev/null || true

# --------------------------------------------------------------------------
# T8d: login status 서브커맨드 자체가 미지원 → 비차단, dry-run으로 계속 진행
# --------------------------------------------------------------------------
printf '\n== T8d: login status 미지원(unsupported) → 비차단, 계속 진행(--dry-run) ==\n'
STUB3="$WORK/stub3"
make_stub_codex "$STUB3" unsupported ok
OUT="$(env -i PATH="$STUB3:/usr/bin:/bin" HOME="$WORK" "$RUN_ROLE" test-role "$TASK_FILE" --project "$PROJ" --dry-run 2>&1)"; RC=$?
assert_exit "login status 미지원 시에도 dry-run은 exit 0" 0 "$RC"
assert_contains "login status 확인 불가 경고 출력(비차단)" "$OUT" "확인 불가"
rm -f "$INBOX"/codex-test-role-*.txt 2>/dev/null || true

# --------------------------------------------------------------------------
# T8e: 정상 픽스처 + --dry-run → exit 0, 프롬프트 보존, inbox DRY-RUN
# --------------------------------------------------------------------------
printf '\n== T8e: 정상 + --dry-run → exit 0 ==\n'
STUB4="$WORK/stub4"
make_stub_codex "$STUB4" ok ok
OUT="$(env -i PATH="$STUB4:/usr/bin:/bin" HOME="$WORK" "$RUN_ROLE" test-role "$TASK_FILE" --project "$PROJ" --dry-run 2>&1)"; RC=$?
assert_exit "정상 dry-run exit 0" 0 "$RC"
assert_contains "dry-run 안내 출력" "$OUT" "dry-run"
KEPT="$(printf '%s' "$OUT" | grep -o '/[^ ]*\.kept' | head -1)"
if [ -n "$KEPT" ] && [ -f "$KEPT" ]; then
  ok "보존된 프롬프트 파일 실존: $(basename "$KEPT")"
  if grep -q "Test Role" "$KEPT" && grep -q "User Sovereignty" "$KEPT" && grep -q "테스트 태스크 브리프" "$KEPT"; then
    ok "프롬프트에 역할 지침+ETHOS+task-file 3요소 모두 포함"
  else
    bad "프롬프트 3요소 조합 불완전"
  fi
  rm -f "$KEPT" 2>/dev/null || true
else
  bad "보존된 프롬프트 파일을 출력에서 찾지 못함"
fi
if ls "$INBOX"/codex-test-role-*.txt >/dev/null 2>&1; then
  MARKER_CONTENT="$(cat "$INBOX"/codex-test-role-*.txt | head -1)"
  assert_contains "inbox 마커 상태=DRY-RUN" "$MARKER_CONTENT" "DRY-RUN"
  rm -f "$INBOX"/codex-test-role-*.txt
else
  bad "T8e inbox 마커 미생성"
fi

# --------------------------------------------------------------------------
# T8f: 정상 픽스처 + 실제 실행(exec 성공 스텁) → exit 0, inbox DONE
# --------------------------------------------------------------------------
printf '\n== T8f: 정상 + exec 성공 스텁 → exit 0 ==\n'
OUT="$(env -i PATH="$STUB4:/usr/bin:/bin" HOME="$WORK" "$RUN_ROLE" test-role "$TASK_FILE" --project "$PROJ" 2>&1)"; RC=$?
assert_exit "정상 실행 exit 0" 0 "$RC"
if ls "$INBOX"/codex-test-role-*.txt >/dev/null 2>&1; then
  MARKER_CONTENT="$(cat "$INBOX"/codex-test-role-*.txt | head -1)"
  assert_contains "inbox 마커 상태=DONE" "$MARKER_CONTENT" "DONE"
  rm -f "$INBOX"/codex-test-role-*.txt
else
  bad "T8f inbox 마커 미생성"
fi

# --------------------------------------------------------------------------
# T8g: codex exec가 인증 실패로 종료 → exit 4 (사후 판정)
# --------------------------------------------------------------------------
printf '\n== T8g: exec 인증 실패 → exit 4(사후 판정) ==\n'
STUB5="$WORK/stub5"
make_stub_codex "$STUB5" ok authfail
OUT="$(env -i PATH="$STUB5:/usr/bin:/bin" HOME="$WORK" "$RUN_ROLE" test-role "$TASK_FILE" --project "$PROJ" 2>&1)"; RC=$?
assert_exit "exec 인증 실패 시 exit 4" 4 "$RC"
assert_contains "E-CODEX-AUTH(사후) 메시지 출력" "$OUT" "E-CODEX-AUTH"
rm -f "$INBOX"/codex-test-role-*.txt 2>/dev/null || true

# --------------------------------------------------------------------------
# T8h: codex exec가 일반 오류로 종료 → exit 1
# --------------------------------------------------------------------------
printf '\n== T8h: exec 일반 오류 → exit 1 ==\n'
STUB6="$WORK/stub6"
make_stub_codex "$STUB6" ok generic-fail
OUT="$(env -i PATH="$STUB6:/usr/bin:/bin" HOME="$WORK" "$RUN_ROLE" test-role "$TASK_FILE" --project "$PROJ" 2>&1)"; RC=$?
assert_exit "exec 일반 오류 시 exit 1" 1 "$RC"
if ls "$INBOX"/codex-test-role-*.txt >/dev/null 2>&1; then
  MARKER_CONTENT="$(cat "$INBOX"/codex-test-role-*.txt | head -1)"
  assert_contains "inbox 마커 상태=FAIL" "$MARKER_CONTENT" "FAIL"
  rm -f "$INBOX"/codex-test-role-*.txt
else
  bad "T8h inbox 마커 미생성"
fi

# --------------------------------------------------------------------------
# T8i: role-slug 형식 불량 → exit 1
# --------------------------------------------------------------------------
printf '\n== T8i: role-slug 형식 불량 → exit 1 ==\n'
OUT="$(env -i PATH="$STUB4:/usr/bin:/bin" HOME="$WORK" "$RUN_ROLE" "bad slug!" "$TASK_FILE" --project "$PROJ" 2>&1)"; RC=$?
assert_exit "bad slug 형식 시 exit 1" 1 "$RC"
assert_contains "E-CODEX-ROLE-INVALID 메시지 출력" "$OUT" "E-CODEX-ROLE-INVALID"

# --------------------------------------------------------------------------
# T8j: task-file 부재 → exit 1
# --------------------------------------------------------------------------
printf '\n== T8j: task-file 부재 → exit 1 ==\n'
OUT="$(env -i PATH="$STUB4:/usr/bin:/bin" HOME="$WORK" "$RUN_ROLE" test-role "$WORK/no-such-task.md" --project "$PROJ" 2>&1)"; RC=$?
assert_exit "task-file 부재 시 exit 1" 1 "$RC"

printf '\n=========================================\n'
printf '결과: PASS=%d FAIL=%d\n' "$PASS_COUNT" "$FAIL_COUNT"
if [ "$FAIL_COUNT" -eq 0 ]; then
  printf '전체 통과\n'
  exit 0
else
  printf '실패 항목 있음\n' >&2
  exit 1
fi
