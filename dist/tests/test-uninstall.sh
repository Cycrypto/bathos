#!/usr/bin/env bash
# =============================================================================
# BATHOS Dynamis — dist/tests/test-uninstall.sh
# Story B4 §5 "dry-run 모드가 곧 테스트 하니스다" — 픽스처 4종.
#   ① 픽스처 외부 상태 생성 -> dry-run 출력이 전 대상을 나열
#   ② --yes 실행 -> 실제 제거 + [removed] 로그
#   ③ 대상 없음 -> 빈 상태 안내
#   ④ 쓰기 불가 파일 1개 -> 부분 실패 분리 표기
#
# 실제 $HOME/_state를 절대 건드리지 않는다 — 전부 격리된 임시 디렉터리로
# BATHOS_ROOT/BATHOS_STATE_DIR/BATHOS_HOME/BATHOS_CONFIG_DIR를 오버라이드한다.
# =============================================================================
set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
UNINSTALL="$SCRIPT_DIR/../../scripts/uninstall.sh"

FAIL=0
pass() { printf '[PASS] %s\n' "$1"; }
fail() { printf '[FAIL] %s\n' "$1"; FAIL=1; }

new_fixture_root() {
  mktemp -d
}

# ---------------------------------------------------------------------------
# ① dry-run: 외부 상태가 있으면 전 대상을 나열하고 아무것도 지우지 않는다
# ---------------------------------------------------------------------------
test_dry_run_lists_all() {
  local root state home
  root="$(new_fixture_root)"
  state="$root/_state"; mkdir -p "$state"
  home="$root/home"; mkdir -p "$home/.claude"
  echo '{"plan_mode": true}' > "$state/session-flags.json"
  echo '{"statusLine": {"command": "bathos statusline"}}' > "$home/.claude/settings.json"

  local out
  out="$(BATHOS_ROOT="$root" BATHOS_STATE_DIR="$state" BATHOS_HOME="$home" bash "$UNINSTALL" 2>&1)"
  local rc=$?

  if [[ "$rc" -eq 0 ]] && echo "$out" | grep -q "session-flags.json" && echo "$out" | grep -q "dry-run"; then
    pass "① dry-run이 대상을 나열하고 exit 0"
  else
    fail "① dry-run 출력 이상: $out"
  fi

  if [[ -f "$state/session-flags.json" ]]; then
    pass "① dry-run은 실제로 삭제하지 않음"
  else
    fail "① dry-run인데 파일이 삭제됨(위험한 버그)"
  fi

  rm -rf "$root"
}

# ---------------------------------------------------------------------------
# ② --yes 실행: 실제 제거 + [removed] 로그
# ---------------------------------------------------------------------------
test_yes_actually_removes() {
  local root state home
  root="$(new_fixture_root)"
  state="$root/_state"; mkdir -p "$state"
  home="$root/home"; mkdir -p "$home/.claude"
  echo '{"plan_mode": true}' > "$state/session-flags.json"

  local out
  out="$(BATHOS_ROOT="$root" BATHOS_STATE_DIR="$state" BATHOS_HOME="$home" bash "$UNINSTALL" --yes 2>&1)"
  local rc=$?

  if [[ "$rc" -eq 0 ]] && echo "$out" | grep -q "\[removed\]"; then
    pass "② --yes 실행이 [removed] 로그를 남김"
  else
    fail "② --yes 실행 출력 이상(exit=$rc): $out"
  fi

  if [[ ! -f "$state/session-flags.json" ]]; then
    pass "② --yes 실행 후 실제로 파일이 삭제됨"
  else
    fail "② --yes 실행했는데 파일이 남아있음"
  fi

  rm -rf "$root"
}

# ---------------------------------------------------------------------------
# ③ 대상 없음: 빈 상태 안내
# ---------------------------------------------------------------------------
test_empty_state_message() {
  local root state home
  root="$(new_fixture_root)"
  state="$root/_state"; mkdir -p "$state"
  home="$root/home"; mkdir -p "$home"

  local out
  out="$(BATHOS_ROOT="$root" BATHOS_STATE_DIR="$state" BATHOS_HOME="$home" bash "$UNINSTALL" 2>&1)"
  local rc=$?

  if [[ "$rc" -eq 0 ]] && echo "$out" | grep -q "외부 상태 없음"; then
    pass "③ 대상 없음일 때 빈 상태 안내 + exit 0"
  else
    fail "③ 빈 상태 처리 이상(exit=$rc): $out"
  fi

  rm -rf "$root"
}

# ---------------------------------------------------------------------------
# ④ 쓰기 불가 파일: 부분 실패 분리 표기(성공/실패 카운트 분리)
# ---------------------------------------------------------------------------
test_partial_failure_reported() {
  local root state home
  root="$(new_fixture_root)"
  state="$root/_state"; mkdir -p "$state"
  home="$root/home"; mkdir -p "$home/.claude"
  echo '{"plan_mode": true}' > "$state/session-flags.json"
  # 쓰기 불가 디렉터리를 만들어 두 번째 대상(설정 statusLine 편집)에서 실패를 유도.
  echo '{"statusLine": {"command": "bathos statusline"}}' > "$home/.claude/settings.json"
  chmod 555 "$home/.claude"   # 디렉터리 쓰기 금지 -> 백업 파일(.bak) 생성 실패 유도

  local out
  out="$(BATHOS_ROOT="$root" BATHOS_STATE_DIR="$state" BATHOS_HOME="$home" bash "$UNINSTALL" --yes 2>&1)"
  local rc=$?

  chmod 755 "$home/.claude"   # 정리 전 복구(rm -rf 위해)

  if echo "$out" | grep -q "\[fail" && echo "$out" | grep -q "\[removed\]"; then
    pass "④ 부분 실패가 성공/실패 분리 표기됨"
  else
    fail "④ 부분 실패 표기 이상(exit=$rc): $out"
  fi

  if [[ "$rc" -eq 1 ]]; then
    pass "④ 부분 실패 시 exit=1(비차단이나 신호는 남김)"
  else
    fail "④ 부분 실패인데 exit=$rc (기대 1)"
  fi

  rm -rf "$root"
}

test_dry_run_lists_all
test_yes_actually_removes
test_empty_state_message
test_partial_failure_reported

if [[ "$FAIL" -eq 0 ]]; then
  printf '[bathos test-uninstall] ✓ 전체 통과\n'
  exit 0
else
  printf '[bathos test-uninstall] ✗ 실패 항목 존재\n'
  exit 1
fi
