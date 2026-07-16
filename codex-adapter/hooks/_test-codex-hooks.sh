#!/usr/bin/env bash
# =============================================================================
# BATHOS P5 — codex-adapter/hooks/_test-codex-hooks.sh
# 시뮬레이션 테스트 하네스: pretooluse-gate.sh · stop-save.sh를 실제 Codex
# 설치 없이 검증한다. 가짜 stdin JSON을 파이프하고, 스텁 bathos로 verdict를
# 재생해 exit code·stderr·부수효과(파일)를 단정(assert)한다.
#
# 설계 원본: .agent-team/04-architecture/w2-runtime-p4-design-kr.md §B5 (James)
# 자리매김: .claude/hooks/_test-hooks.sh와 동일(수동 실행 검증 스크립트).
# bash 3.2 호환(카운터 변수 + 함수, 연관배열 금지).
#
# P5 갱신(2026-07-16, Codex v0.144.5 macos-x86_64 실측 반영):
# 픽스처 stdin의 tool_name을 실측값(shell/exec_command/apply_patch)으로
# 교체하고, B-15~B-18을 추가해 "Bash는 존재하지 않는다"는 실측 사실이
# 회귀하지 않음을 계약화한다(B-3은 이제 실측 tool_name인 shell로 웨이브
# 진입 명령을 검증하고, B-15는 옛 P4 가정이던 Bash가 더 이상 필요/유효하지
# 않음 — 오지 않는 값이므로 무해 통과만 확인 — 을 별도로 남겨 둔다).
#
# 실행: bash codex-adapter/hooks/_test-codex-hooks.sh
# =============================================================================
set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
HOOKS_DIR="$SCRIPT_DIR"

TMPDIR_BASE="$(mktemp -d)"
cleanup() { rm -rf "$TMPDIR_BASE"; }
trap cleanup EXIT

RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; NC='\033[0m'
PASS_COUNT=0
FAIL_COUNT=0

assert_exit() {
  local desc="$1" expected="$2" actual="$3"
  if [ "$actual" -eq "$expected" ] 2>/dev/null; then
    printf "${GREEN}[PASS]${NC} %s (exit=%d)\n" "$desc" "$actual"
    PASS_COUNT=$((PASS_COUNT + 1))
  else
    printf "${RED}[FAIL]${NC} %s — 예상 exit=%d, 실제 exit=%d\n" "$desc" "$expected" "$actual"
    FAIL_COUNT=$((FAIL_COUNT + 1))
  fi
}

assert_true() {
  # $1=desc $2=condition(0/1, already-evaluated as shell truthiness via [ ] outside)
  local desc="$1" ok="$2"
  if [ "$ok" = "1" ]; then
    printf "${GREEN}[PASS]${NC} %s\n" "$desc"
    PASS_COUNT=$((PASS_COUNT + 1))
  else
    printf "${RED}[FAIL]${NC} %s\n" "$desc"
    FAIL_COUNT=$((FAIL_COUNT + 1))
  fi
}

# --------------------------------------------------------------------------
# 스텁 bathos 생성 헬퍼 — $1=STUB_DIR $2=verdict("PASS"|"CONCERNS"|"FAIL"|"EMPTY")
# --------------------------------------------------------------------------
make_stub_bathos() {
  local stub_dir="$1" verdict="$2"
  mkdir -p "$stub_dir"
  printf '%s' "$verdict" > "$stub_dir/verdict"
  cat > "$stub_dir/bathos" <<'STUB_EOF'
#!/usr/bin/env bash
# 스텁 bathos — 호출 기록 + verdict 재생 (실제 Codex/BATHOS 빌드 불요)
STUB_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
echo "$*" >> "$STUB_DIR/calls.log"
case "$*" in
  *"gate show"*)
     V="$(cat "$STUB_DIR/verdict" 2>/dev/null)"
     if [ "$V" = "EMPTY" ]; then exit 0; fi
     printf '{"decided":"2026-07-16T00:00:00Z","facilitator":"Paul","gate_type":"Implementation","issues_critical":0,"issues_total":1,"verdict":"%s","wave_id":"W3"}\n' "$V"
     ;;
  *"state show"*)
     printf '{"project_id":"stub","stub":true}\n'
     ;;
  *"audit append"*)
     exit 0
     ;;
esac
exit 0
STUB_EOF
  chmod +x "$stub_dir/bathos"
}

# --------------------------------------------------------------------------
# 픽스처 프로젝트 디렉터리 생성 — $1=이름 -> stdout에 절대경로
# --------------------------------------------------------------------------
make_fixture_project() {
  local name="$1"
  local dir="$TMPDIR_BASE/proj-$name"
  mkdir -p "$dir/.agent-team/_state"
  printf '# seed\n' > "$dir/.agent-team/_state/SESSION-SNAPSHOT.md"
  printf '%s' "$dir"
}

# --------------------------------------------------------------------------
# 훅 실행 헬퍼 — $1=hook_script $2=stdin_json (env는 호출부에서 export)
# 표준출력에 "EXIT<tab>STDERR_B64" 형태로 반환(멀티라인 stderr 보존 위해 base64).
# --------------------------------------------------------------------------
run_hook() {
  local hook="$1" stdin_json="$2"
  local out ec
  out="$(printf '%s' "$stdin_json" | bash "$hook" 2>&1 1>/dev/null)"
  ec=$?
  printf '%d\t%s' "$ec" "$(printf '%s' "$out" | base64 | tr -d '\n')"
}

get_exit() { printf '%s' "$1" | cut -f1; }
get_stderr() { printf '%s' "$1" | cut -f2- | base64 -d 2>/dev/null; }

# ==========================================================================
# 픽스처 stdin JSON (하네스 내 printf 상수, <TMP_PROJ>를 실경로로 치환)
# tool_name은 Codex v0.144.5(macos-x86_64) 실측값만 사용한다: shell,
# exec_command, apply_patch. `Bash`는 존재하지 않는 값이며(P5에서 확정),
# 그 사실 자체를 검증하는 픽스처는 별도로 json_legacy_bash_wave()에 둔다.
# ==========================================================================
json_write_src() {
  printf '{"session_id":"s1","turn_id":"t1","hook_event_name":"PreToolUse","tool_name":"apply_patch","cwd":"%s","tool_input":{"patch":"*** Update File: core/crates/bathos-cli/src/main.rs"}}' "$1"
}
json_shell_wave() {
  # T2: 실측 tool_name "shell"의 tool_input.command가 웨이브 진입 명령.
  printf '{"session_id":"s1","turn_id":"t1","hook_event_name":"PreToolUse","tool_name":"shell","cwd":"%s","tool_input":{"command":"bathos wave advance --to W5"}}' "$1"
}
json_exec_command_wave() {
  # T2: 실측 tool_name "exec_command"(shell과 별개 값)로도 동일하게 발화해야 한다.
  printf '{"session_id":"s1","turn_id":"t1","hook_event_name":"PreToolUse","tool_name":"exec_command","cwd":"%s","tool_input":{"command":"bathos wave advance --to W5"}}' "$1"
}
json_bash_benign() {
  printf '{"session_id":"s1","turn_id":"t1","hook_event_name":"PreToolUse","tool_name":"shell","cwd":"%s","tool_input":{"command":"ls -la"}}' "$1"
}
json_write_docs() {
  printf '{"session_id":"s1","turn_id":"t1","hook_event_name":"PreToolUse","tool_name":"apply_patch","cwd":"%s","tool_input":{"patch":"*** Update File: .agent-team/08-impl-notes/backend.md"}}' "$1"
}
json_shell_write_src() {
  # T1: apply_patch가 아니라 shell 도구가 sed -i로 소스 경로를 직접 쓰는 경우
  # (§4 T1 케이스가 apply_patch 외 shell/exec_command도 포함하는지 검증).
  printf '{"session_id":"s1","turn_id":"t1","hook_event_name":"PreToolUse","tool_name":"shell","cwd":"%s","tool_input":{"command":"sed -i \\"\\" \\"s/x/y/\\" core/crates/bathos-cli/src/main.rs"}}' "$1"
}
json_legacy_bash_wave() {
  # P4가 가정했던(그러나 실제로는 존재하지 않는) tool_name "Bash"로 동일한
  # 웨이브 진입 명령을 보낸다 — Bash는 케이스문에 없으므로 TRIGGER가 비고
  # bathos 호출 없이 즉시 exit 0이어야 한다(= 옛 가정이 남아 있어도 무해).
  printf '{"session_id":"s1","turn_id":"t1","hook_event_name":"PreToolUse","tool_name":"Bash","cwd":"%s","tool_input":{"command":"bathos wave advance --to W5"}}' "$1"
}
json_unrelated_tool() {
  # 게이트 트리거와 무관한 tool_name(가상의 read 계열 도구) -> 항상 통과.
  printf '{"session_id":"s1","turn_id":"t1","hook_event_name":"PreToolUse","tool_name":"read_file","cwd":"%s","tool_input":{"path":"README.md"}}' "$1"
}
json_stop() {
  printf '{"session_id":"s1","turn_id":"t9","hook_event_name":"Stop","stop_hook_active":false,"cwd":"%s","last_assistant_message":"done"}' "$1"
}
json_stop_active() {
  printf '{"session_id":"s1","turn_id":"t9","hook_event_name":"Stop","stop_hook_active":true,"cwd":"%s","last_assistant_message":"done"}' "$1"
}
JSON_GARBAGE='not json at all'

# ==========================================================================
# B-1 ~ B-10: pretooluse-gate.sh
# ==========================================================================
printf '\n=== pretooluse-gate.sh (B-1 ~ B-10) ===\n'

# --- B-1: J_WRITE_SRC, verdict=PASS -> exit 0, stderr에 BLOCKED 없음 ---
P1="$(make_fixture_project b1)"
STUB1="$TMPDIR_BASE/stub-b1"; make_stub_bathos "$STUB1" "PASS"
RES="$(BATHOS_BIN="$STUB1/bathos" run_hook "$HOOKS_DIR/pretooluse-gate.sh" "$(json_write_src "$P1")")"
EC="$(get_exit "$RES")"; ERR="$(get_stderr "$RES")"
assert_exit "B-1 소스쓰기 + PASS -> 통과" 0 "$EC"
if printf '%s' "$ERR" | grep -q 'BLOCKED'; then
  assert_true "B-1 stderr에 BLOCKED 없음" 0
else
  assert_true "B-1 stderr에 BLOCKED 없음" 1
fi

# --- B-2: J_WRITE_SRC, verdict=FAIL -> exit 2, stderr에 BLOCKED·FAIL ---
P2="$(make_fixture_project b2)"
STUB2="$TMPDIR_BASE/stub-b2"; make_stub_bathos "$STUB2" "FAIL"
RES="$(BATHOS_BIN="$STUB2/bathos" run_hook "$HOOKS_DIR/pretooluse-gate.sh" "$(json_write_src "$P2")")"
EC="$(get_exit "$RES")"; ERR="$(get_stderr "$RES")"
assert_exit "B-2 소스쓰기 + FAIL -> 차단(exit 2)" 2 "$EC"
if printf '%s' "$ERR" | grep -q 'BLOCKED' && printf '%s' "$ERR" | grep -q 'FAIL'; then
  assert_true "B-2 stderr에 BLOCKED·FAIL 포함" 1
else
  assert_true "B-2 stderr에 BLOCKED·FAIL 포함" 0
fi

# --- B-3: J_SHELL_WAVE(tool_name=shell, 실측), verdict=FAIL -> exit 2, stderr에 wave-entry-command ---
P3="$(make_fixture_project b3)"
STUB3="$TMPDIR_BASE/stub-b3"; make_stub_bathos "$STUB3" "FAIL"
RES="$(BATHOS_BIN="$STUB3/bathos" run_hook "$HOOKS_DIR/pretooluse-gate.sh" "$(json_shell_wave "$P3")")"
EC="$(get_exit "$RES")"; ERR="$(get_stderr "$RES")"
assert_exit "B-3 웨이브진입명령(tool_name=shell) + FAIL -> 차단(exit 2)" 2 "$EC"
if printf '%s' "$ERR" | grep -q 'wave-entry-command'; then
  assert_true "B-3 stderr에 wave-entry-command 포함" 1
else
  assert_true "B-3 stderr에 wave-entry-command 포함" 0
fi

# --- B-4: J_WRITE_SRC, verdict=CONCERNS -> exit 0, stderr에 CONCERNS ---
P4="$(make_fixture_project b4)"
STUB4="$TMPDIR_BASE/stub-b4"; make_stub_bathos "$STUB4" "CONCERNS"
RES="$(BATHOS_BIN="$STUB4/bathos" run_hook "$HOOKS_DIR/pretooluse-gate.sh" "$(json_write_src "$P4")")"
EC="$(get_exit "$RES")"; ERR="$(get_stderr "$RES")"
assert_exit "B-4 소스쓰기 + CONCERNS -> 통과" 0 "$EC"
if printf '%s' "$ERR" | grep -q 'CONCERNS'; then
  assert_true "B-4 stderr에 CONCERNS 포함" 1
else
  assert_true "B-4 stderr에 CONCERNS 포함" 0
fi

# --- B-5: J_BASH_BENIGN(tool_name=shell, 무해 명령), verdict=FAIL(스텁) -> exit 0, calls.log 비어있음(비트리거는 bathos 미호출) ---
P5="$(make_fixture_project b5)"
STUB5="$TMPDIR_BASE/stub-b5"; make_stub_bathos "$STUB5" "FAIL"
RES="$(BATHOS_BIN="$STUB5/bathos" run_hook "$HOOKS_DIR/pretooluse-gate.sh" "$(json_bash_benign "$P5")")"
EC="$(get_exit "$RES")"
assert_exit "B-5 무해 shell 명령 -> 통과" 0 "$EC"
if [ ! -s "$STUB5/calls.log" ]; then
  assert_true "B-5 calls.log 비어있음(bathos 미호출)" 1
else
  assert_true "B-5 calls.log 비어있음(bathos 미호출)" 0
fi

# --- B-6: J_WRITE_SRC, BATHOS_BIN=/nonexistent + manifest/report도 없음 -> exit 0, fail-safe 경고 ---
P6="$(make_fixture_project b6)"
RES="$(BATHOS_BIN=/nonexistent run_hook "$HOOKS_DIR/pretooluse-gate.sh" "$(json_write_src "$P6")")"
EC="$(get_exit "$RES")"; ERR="$(get_stderr "$RES")"
assert_exit "B-6 bathos 없음 -> fail-safe 통과" 0 "$EC"
if printf '%s' "$ERR" | grep -q '확인 불가'; then
  assert_true "B-6 stderr에 fail-safe 경고" 1
else
  assert_true "B-6 stderr에 fail-safe 경고" 0
fi

# --- B-7: J_WRITE_SRC, verdict=EMPTY(게이트 미존재) -> exit 0, stderr에 "verdict 확인 불가" ---
P7="$(make_fixture_project b7)"
STUB7="$TMPDIR_BASE/stub-b7"; make_stub_bathos "$STUB7" "EMPTY"
RES="$(BATHOS_BIN="$STUB7/bathos" run_hook "$HOOKS_DIR/pretooluse-gate.sh" "$(json_write_src "$P7")")"
EC="$(get_exit "$RES")"; ERR="$(get_stderr "$RES")"
assert_exit "B-7 게이트 미존재(EMPTY) -> 통과" 0 "$EC"
if printf '%s' "$ERR" | grep -q '확인 불가'; then
  assert_true "B-7 stderr에 verdict 확인 불가" 1
else
  assert_true "B-7 stderr에 verdict 확인 불가" 0
fi

# --- B-8: J_WRITE_DOCS, verdict=FAIL -> exit 0, .agent-team/ 전용 쓰기는 비트리거 ---
P8="$(make_fixture_project b8)"
STUB8="$TMPDIR_BASE/stub-b8"; make_stub_bathos "$STUB8" "FAIL"
RES="$(BATHOS_BIN="$STUB8/bathos" run_hook "$HOOKS_DIR/pretooluse-gate.sh" "$(json_write_docs "$P8")")"
EC="$(get_exit "$RES")"
assert_exit "B-8 .agent-team/ 전용 쓰기 -> 비트리거 통과" 0 "$EC"

# --- B-9: J_GARBAGE, verdict=FAIL -> exit 0, 파싱 실패 fail-safe ---
STUB9="$TMPDIR_BASE/stub-b9"; make_stub_bathos "$STUB9" "FAIL"
RES="$(BATHOS_BIN="$STUB9/bathos" run_hook "$HOOKS_DIR/pretooluse-gate.sh" "$JSON_GARBAGE")"
EC="$(get_exit "$RES")"
assert_exit "B-9 비-JSON 입력 -> fail-safe 통과" 0 "$EC"

# --- B-10: J_WRITE_SRC, _state 없는 cwd -> exit 0, 비-BATHOS 프로젝트 통과 ---
NO_STATE_DIR="$TMPDIR_BASE/proj-b10-no-state"
mkdir -p "$NO_STATE_DIR"
STUB10="$TMPDIR_BASE/stub-b10"; make_stub_bathos "$STUB10" "FAIL"
RES="$(BATHOS_BIN="$STUB10/bathos" run_hook "$HOOKS_DIR/pretooluse-gate.sh" "$(json_write_src "$NO_STATE_DIR")")"
EC="$(get_exit "$RES")"
assert_exit "B-10 _state 없는 cwd -> 비-BATHOS 프로젝트 통과" 0 "$EC"

# --------------------------------------------------------------------------
# B-15 ~ B-18: P5 실측 회귀(Codex v0.144.5) — tool_name 하드닝 계약화
# --------------------------------------------------------------------------

# --- B-15: J_EXEC_COMMAND_WAVE(tool_name=exec_command), verdict=FAIL -> exit 2 ---
# (a) 웨이브 진입 명령이 exec_command로 와도 shell과 동일하게 차단되어야 한다.
P15="$(make_fixture_project b15)"
STUB15="$TMPDIR_BASE/stub-b15"; make_stub_bathos "$STUB15" "FAIL"
RES="$(BATHOS_BIN="$STUB15/bathos" run_hook "$HOOKS_DIR/pretooluse-gate.sh" "$(json_exec_command_wave "$P15")")"
EC="$(get_exit "$RES")"; ERR="$(get_stderr "$RES")"
assert_exit "B-15 웨이브진입명령(tool_name=exec_command) + FAIL -> 차단(exit 2)" 2 "$EC"
if printf '%s' "$ERR" | grep -q 'wave-entry-command'; then
  assert_true "B-15 stderr에 wave-entry-command 포함" 1
else
  assert_true "B-15 stderr에 wave-entry-command 포함" 0
fi

# --- B-16: J_SHELL_WRITE_SRC(tool_name=shell, sed -i 로 src/ 직접 쓰기), verdict=FAIL -> exit 2 ---
# (b) apply_patch가 아니어도 shell이 소스 경로를 직접 쓰면 T1(source-write)로
# 차단되어야 한다(§4 T1 케이스가 apply_patch 외 shell/exec_command도 포함).
P16="$(make_fixture_project b16)"
STUB16="$TMPDIR_BASE/stub-b16"; make_stub_bathos "$STUB16" "FAIL"
RES="$(BATHOS_BIN="$STUB16/bathos" run_hook "$HOOKS_DIR/pretooluse-gate.sh" "$(json_shell_write_src "$P16")")"
EC="$(get_exit "$RES")"; ERR="$(get_stderr "$RES")"
assert_exit "B-16 shell의 소스직접쓰기(sed -i) + FAIL -> 차단(exit 2)" 2 "$EC"
if printf '%s' "$ERR" | grep -q 'source-write'; then
  assert_true "B-16 stderr에 source-write 포함" 1
else
  assert_true "B-16 stderr에 source-write 포함" 0
fi

# --- B-17: J_LEGACY_BASH_WAVE(tool_name="Bash", P4의 옛 가정), verdict=FAIL -> exit 0, bathos 미호출 ---
# (c) 과거 P4가 가정했던 "Bash"는 실측 결과 존재하지 않는 tool_name이다.
# 케이스문에서 완전히 제거됐어도(=더 이상 필요 없음) 여전히 무해(비트리거로
# 즉시 통과, bathos도 호출하지 않음)함을 계약화한다 — 회귀 시 이 값이 다시
# T2로 오인 매칭되면 fail-open 리스크가 재발하므로 명시적으로 감시한다.
P17="$(make_fixture_project b17)"
STUB17="$TMPDIR_BASE/stub-b17"; make_stub_bathos "$STUB17" "FAIL"
RES="$(BATHOS_BIN="$STUB17/bathos" run_hook "$HOOKS_DIR/pretooluse-gate.sh" "$(json_legacy_bash_wave "$P17")")"
EC="$(get_exit "$RES")"
assert_exit "B-17 옛 가정 tool_name=Bash + FAIL(verdict) -> 비트리거 통과(exit 0)" 0 "$EC"
if [ ! -s "$STUB17/calls.log" ]; then
  assert_true "B-17 calls.log 비어있음(Bash는 트리거 아님 -> bathos 미호출)" 1
else
  assert_true "B-17 calls.log 비어있음(Bash는 트리거 아님 -> bathos 미호출)" 0
fi

# --- B-18: J_UNRELATED_TOOL(tool_name=read_file), verdict=FAIL -> exit 0 ---
# (d) 게이트 트리거와 무관한 tool_name은 verdict와 무관하게 항상 통과.
P18="$(make_fixture_project b18)"
STUB18="$TMPDIR_BASE/stub-b18"; make_stub_bathos "$STUB18" "FAIL"
RES="$(BATHOS_BIN="$STUB18/bathos" run_hook "$HOOKS_DIR/pretooluse-gate.sh" "$(json_unrelated_tool "$P18")")"
EC="$(get_exit "$RES")"
assert_exit "B-18 무관 tool_name(read_file) -> 통과(exit 0)" 0 "$EC"
if [ ! -s "$STUB18/calls.log" ]; then
  assert_true "B-18 calls.log 비어있음(비트리거 -> bathos 미호출)" 1
else
  assert_true "B-18 calls.log 비어있음(비트리거 -> bathos 미호출)" 0
fi

# ==========================================================================
# B-11 ~ B-14: stop-save.sh
# ==========================================================================
printf '\n=== stop-save.sh (B-11 ~ B-14) ===\n'

# --- B-11: J_STOP -> session-state.json 존재·유효 JSON, 로그 dump=ok, 스냅샷 아카이브 생성 ---
P11="$(make_fixture_project b11)"
STUB11="$TMPDIR_BASE/stub-b11"; make_stub_bathos "$STUB11" "PASS"
RES="$(BATHOS_BIN="$STUB11/bathos" run_hook "$HOOKS_DIR/stop-save.sh" "$(json_stop "$P11")")"
EC="$(get_exit "$RES")"
assert_exit "B-11 Stop 저장 -> exit 0" 0 "$EC"

STATE_DIR_11="$P11/.agent-team/_state"
if [ -f "$STATE_DIR_11/session-state.json" ] && head -c1 "$STATE_DIR_11/session-state.json" | grep -q '{'; then
  assert_true "B-11 session-state.json 존재·유효 JSON({로 시작)" 1
else
  assert_true "B-11 session-state.json 존재·유효 JSON({로 시작)" 0
fi
if grep -q 'dump=ok' "$STATE_DIR_11/codex-stop-save.log" 2>/dev/null; then
  assert_true "B-11 codex-stop-save.log에 dump=ok" 1
else
  assert_true "B-11 codex-stop-save.log에 dump=ok" 0
fi
TODAY="$(date '+%Y-%m-%d')"
if [ -f "$STATE_DIR_11/SESSION-SNAPSHOT-$TODAY.md" ]; then
  assert_true "B-11 스냅샷 날짜 아카이브 생성" 1
else
  assert_true "B-11 스냅샷 날짜 아카이브 생성" 0
fi

# --- B-12: J_STOP, BATHOS_BIN=/nonexistent -> 로그 dump=no-bathos, session-state.json 미생성(기존본 미파괴) ---
P12="$(make_fixture_project b12)"
RES="$(BATHOS_BIN=/nonexistent run_hook "$HOOKS_DIR/stop-save.sh" "$(json_stop "$P12")")"
EC="$(get_exit "$RES")"
assert_exit "B-12 bathos 없음 -> exit 0" 0 "$EC"
STATE_DIR_12="$P12/.agent-team/_state"
if grep -q 'dump=no-bathos' "$STATE_DIR_12/codex-stop-save.log" 2>/dev/null; then
  assert_true "B-12 로그에 dump=no-bathos" 1
else
  assert_true "B-12 로그에 dump=no-bathos" 0
fi
if [ ! -f "$STATE_DIR_12/session-state.json" ]; then
  assert_true "B-12 session-state.json 미생성(기존본 미파괴)" 1
else
  assert_true "B-12 session-state.json 미생성(기존본 미파괴)" 0
fi

# --- B-13: J_STOP 2회 연속(디바운스 10s) -> 0,0 / 2회차 로그 dump=skip ---
P13="$(make_fixture_project b13)"
STUB13="$TMPDIR_BASE/stub-b13"; make_stub_bathos "$STUB13" "PASS"
RES1="$(BATHOS_BIN="$STUB13/bathos" run_hook "$HOOKS_DIR/stop-save.sh" "$(json_stop "$P13")")"
EC1="$(get_exit "$RES1")"
RES2="$(BATHOS_BIN="$STUB13/bathos" run_hook "$HOOKS_DIR/stop-save.sh" "$(json_stop "$P13")")"
EC2="$(get_exit "$RES2")"
assert_exit "B-13 1회차 -> exit 0" 0 "$EC1"
assert_exit "B-13 2회차(디바운스 내) -> exit 0" 0 "$EC2"
STATE_DIR_13="$P13/.agent-team/_state"
LAST_LINE="$(tail -1 "$STATE_DIR_13/codex-stop-save.log" 2>/dev/null)"
if printf '%s' "$LAST_LINE" | grep -q 'dump=skip'; then
  assert_true "B-13 2회차 로그 dump=skip" 1
else
  assert_true "B-13 2회차 로그 dump=skip (실제: $LAST_LINE)" 0
fi

# --- B-14: J_STOP_ACTIVE -> exit 0, 아무 파일도 변경 없음(루프 가드) ---
P14="$(make_fixture_project b14)"
STUB14="$TMPDIR_BASE/stub-b14"; make_stub_bathos "$STUB14" "PASS"
STATE_DIR_14="$P14/.agent-team/_state"
BEFORE_LISTING="$(ls -la "$STATE_DIR_14" 2>/dev/null)"
RES="$(BATHOS_BIN="$STUB14/bathos" run_hook "$HOOKS_DIR/stop-save.sh" "$(json_stop_active "$P14")")"
EC="$(get_exit "$RES")"
AFTER_LISTING="$(ls -la "$STATE_DIR_14" 2>/dev/null)"
assert_exit "B-14 stop_hook_active=true -> exit 0" 0 "$EC"
if [ "$BEFORE_LISTING" = "$AFTER_LISTING" ]; then
  assert_true "B-14 아무 파일도 변경 없음(루프 가드)" 1
else
  assert_true "B-14 아무 파일도 변경 없음(루프 가드)" 0
fi

# ==========================================================================
# 요약
# ==========================================================================
printf '\n=== 요약 ===\n'
TOTAL=$((PASS_COUNT + FAIL_COUNT))
printf 'PASS %d/%d\n' "$PASS_COUNT" "$TOTAL"
if [ "$FAIL_COUNT" -gt 0 ]; then
  printf "${RED}FAIL 있음: %d건${NC}\n" "$FAIL_COUNT"
  exit 1
fi
printf "${GREEN}전체 통과${NC}\n"
exit 0
