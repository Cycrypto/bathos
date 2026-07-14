#!/usr/bin/env bash
# =============================================================================
# BATHOS Dynamis — dist/tests/test-check-rule-copies.sh
# Story B3 §5 픽스처 4종(자기검증) — --check-copies 모드의 세 분기(byte/invariant/
# 제외 목록)를 전부 격리된 임시 디렉터리에서 검증한다. 실제
# scripts/drift-exclusions.json·dist/copies-manifest.json은 건드리지 않는다
# (BATHOS_ROOT·BATHOS_COPIES_MANIFEST·BATHOS_DRIFT_EXCLUSIONS 환경변수로 격리).
#
#   ① 일부러 드리프트시킨 복제본(byte 모드) -> 실패
#   ② 정합 상태(byte 모드) -> 통과
#   ③ invariant 문구 누락 -> 실패
#   ④ 제외 목록 항목의 드리프트 -> 통과(제외 동작 확인)
# =============================================================================
set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CHECK_SCRIPT="$SCRIPT_DIR/../../scripts/check-rule-copies.sh"

FAIL=0
pass() { printf '[PASS] %s\n' "$1"; }
fail() { printf '[FAIL] %s\n' "$1"; FAIL=1; }

run_check() {
  local root="$1"
  BATHOS_ROOT="$root" \
  BATHOS_COPIES_MANIFEST="$root/dist/copies-manifest.json" \
  BATHOS_DRIFT_EXCLUSIONS="$root/drift-exclusions.json" \
  bash "$CHECK_SCRIPT" --check-copies
}

setup_root() {
  local root
  root="$(mktemp -d)"
  mkdir -p "$root/dist" "$root/canon" "$root/copy"
  printf '%s\n' "$root"
}

# --- ① byte 모드 드리프트 -> 실패 -------------------------------------------
root="$(setup_root)"
echo "canonical content line one" > "$root/canon/rule.md"
echo "DRIFTED content line one" > "$root/copy/rule.md"
cat > "$root/dist/copies-manifest.json" <<EOF
{"copies":[{"source":"canon/rule.md","copy":"copy/rule.md","mode":"byte"}]}
EOF
echo '{"exclusions":[]}' > "$root/drift-exclusions.json"
if run_check "$root" >/tmp/out1.$$ 2>&1; then
  fail "① byte 드리프트인데 통과함: $(cat /tmp/out1.$$)"
else
  pass "① byte 드리프트 검출 -> 실패(exit!=0)"
fi
rm -rf "$root" /tmp/out1.$$

# --- ② byte 모드 정합 -> 통과 -----------------------------------------------
root="$(setup_root)"
echo "canonical content line one" > "$root/canon/rule.md"
cp "$root/canon/rule.md" "$root/copy/rule.md"
cat > "$root/dist/copies-manifest.json" <<EOF
{"copies":[{"source":"canon/rule.md","copy":"copy/rule.md","mode":"byte"}]}
EOF
echo '{"exclusions":[]}' > "$root/drift-exclusions.json"
if run_check "$root" >/tmp/out2.$$ 2>&1; then
  pass "② byte 정합 -> 통과"
else
  fail "② byte 정합인데 실패함: $(cat /tmp/out2.$$)"
fi
rm -rf "$root" /tmp/out2.$$

# --- ③ invariant 문구 누락 -> 실패 ------------------------------------------
root="$(setup_root)"
echo "some skill body without the required phrase" > "$root/copy/skill.md"
cat > "$root/dist/copies-manifest.json" <<EOF
{"copies":[{"source":"copy/skill.md","copy":"copy/skill.md","mode":"invariant","invariant":["## Boundaries"]}]}
EOF
echo '{"exclusions":[]}' > "$root/drift-exclusions.json"
if run_check "$root" >/tmp/out3.$$ 2>&1; then
  fail "③ invariant 누락인데 통과함: $(cat /tmp/out3.$$)"
else
  pass "③ invariant 문구 누락 검출 -> 실패"
fi
rm -rf "$root" /tmp/out3.$$

# --- ④ 제외 목록 항목의 드리프트 -> 통과(제외 동작) --------------------------
root="$(setup_root)"
echo "canonical content line one" > "$root/canon/rule.md"
echo "DRIFTED but excluded" > "$root/copy/rule-es.md"
cat > "$root/dist/copies-manifest.json" <<EOF
{"copies":[{"source":"canon/rule.md","copy":"copy/rule-es.md","mode":"byte"}]}
EOF
cat > "$root/drift-exclusions.json" <<EOF
{"exclusions":[{"path":"copy/rule-es.md","reason":"테스트용 i18n 예외"}]}
EOF
if run_check "$root" >/tmp/out4.$$ 2>&1; then
  pass "④ 제외 목록 항목은 드리프트해도 통과(제외 동작 확인)"
else
  fail "④ 제외 목록이 동작하지 않음: $(cat /tmp/out4.$$)"
fi
rm -rf "$root" /tmp/out4.$$

if [[ "$FAIL" -eq 0 ]]; then
  printf '[bathos test-check-rule-copies] ✓ 전체 통과\n'
  exit 0
else
  printf '[bathos test-check-rule-copies] ✗ 실패 항목 존재\n'
  exit 1
fi
