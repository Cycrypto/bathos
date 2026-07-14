#!/usr/bin/env bash
# =============================================================================
# BATHOS Dynamis — dist/tests/test-host-detect.sh
# Story B5 §5 스모크 테스트: 기본값=claude, 미지원 호스트=NotImplemented(exit 2).
# =============================================================================
set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=../lib/host-detect.sh
source "$SCRIPT_DIR/../lib/host-detect.sh"

FAIL=0
assert_eq() {
  local desc="$1" expected="$2" actual="$3"
  if [[ "$expected" == "$actual" ]]; then
    printf '[PASS] %s\n' "$desc"
  else
    printf '[FAIL] %s — 기대=%s 실제=%s\n' "$desc" "$expected" "$actual"
    FAIL=1
  fi
}

# 1) 기본값(환경변수 없음) -> claude
unset PLUGIN_DATA COPILOT_PLUGIN_DATA BATHOS_FORCE_HOST 2>/dev/null || true
got="$(bathos_detect_host)"
assert_eq "기본 호스트 판별" "claude" "$got"

# 2) claude 경로는 payload를 그대로 통과
out="$(bathos_write_hook_output claude '{"decision":"allow"}')"; rc=$?
assert_eq "claude 출력 통과(exit)" "0" "$rc"
assert_eq "claude 출력 내용" '{"decision":"allow"}' "$out"

# 3) 미지원 호스트(codex) -> NotImplemented(exit 2), stdout은 비어야 함(오출력 금지)
out="$(BATHOS_FORCE_HOST=codex bathos_write_hook_output codex '{"decision":"allow"}' 2>/dev/null)"
rc=$?
assert_eq "codex 미구현 exit code" "2" "$rc"
assert_eq "codex 미구현 stdout 비어있음(오출력 금지)" "" "$out"

# 4) BATHOS_FORCE_HOST 오버라이드 동작 확인
got="$(BATHOS_FORCE_HOST=copilot bathos_detect_host)"
assert_eq "강제 오버라이드(copilot)" "copilot" "$got"

if [[ "$FAIL" -eq 0 ]]; then
  printf '[bathos test-host-detect] ✓ 전체 통과\n'
  exit 0
else
  printf '[bathos test-host-detect] ✗ 실패 항목 존재\n'
  exit 1
fi
