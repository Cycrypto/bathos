#!/usr/bin/env bash
# =============================================================================
# BATHOS Dynamis — dist/tests/test-manifests.sh
# Story B1·B2 §5 "테스트 접근" 구현: 매니페스트 스키마 자기검증 + 포인터 유효성.
#
#   1) 공통 필드(name/version/sources/capability_tier) 존재 확인 + 산문 필드 없음
#      (behavior 텍스트가 통째로 박혀있지 않은지 — 필드 값 길이로 간이 검사)
#   2) sources.* 의 상대경로가 실제 존재하는 디렉터리를 가리키는지 확인
#      (깨진 포인터 = 실패). LD-5(최종 조립 전) 갭 처리: 제품 트리에 대상이 아직
#      없으면 원본 bathos/의 동일 상대경로로 폴백 확인 후 "조립 대기" 로 표기.
#   3) capability_tier 값이 폐쇄 어휘(full-hook/instruction-only/mcp) 안에 있는지
#
# marketplace.json은 sources/capability_tier가 없는 것이 정상 스키마이므로
# plugin.json/manifest.json만 검사한다(check-versions.sh와 동일 대상 규약).
# =============================================================================
set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BATHOS_ROOT="${BATHOS_ROOT:-$(cd "$SCRIPT_DIR/../.." && pwd)}"
DIST_DIR="$BATHOS_ROOT/dist"

FAIL=0
PASS_COUNT=0
info()  { printf '[test-manifests] %s\n' "$*"; }
error() { printf '[test-manifests] ✗ %s\n' "$*"; FAIL=1; }
ok()    { printf '[test-manifests] ✓ %s\n' "$*"; PASS_COUNT=$((PASS_COUNT + 1)); }

if ! command -v jq >/dev/null 2>&1; then
  info "jq 없음 — 스키마 검사는 스킵(fail-open, 비차단). 다음 행동: jq 설치 후 재실행."
  exit 0
fi

VALID_TIERS="full-hook instruction-only mcp"

TARGETS=()
while IFS= read -r -d '' f; do
  TARGETS+=("$f")
done < <(find "$DIST_DIR" -type f \( -name 'plugin.json' -o -name 'manifest.json' \) -print0 2>/dev/null)

if [[ "${#TARGETS[@]}" -eq 0 ]]; then
  info "검사 대상 0건 — 통과(안내)"
  exit 0
fi

for f in "${TARGETS[@]}"; do
  rel="${f#"$BATHOS_ROOT"/}"
  fdir="$(cd "$(dirname "$f")" && pwd)"

  # --- 1) 공통 필드 존재 -----------------------------------------------------
  name="$(jq -r '.name // empty' "$f")"
  version="$(jq -r '.version // empty' "$f")"
  tier="$(jq -r '.capability_tier // empty' "$f")"
  has_sources="$(jq -r 'has("sources")' "$f")"

  [[ -n "$name" ]] && ok "$rel — name 존재" || error "$rel — name 필드 없음"
  [[ -n "$version" ]] && ok "$rel — version 존재" || error "$rel — version 필드 없음"

  if [[ "$has_sources" != "true" ]]; then
    error "$rel — sources 필드 없음(포인터 규약 위반, CF-B1 AC1)"
  else
    ok "$rel — sources 필드 존재"
  fi

  # --- 2) 산문 필드 없음(간이 검사: description을 제외한 값 중 200자 초과 문자열
  #        필드가 없는지) — behavior 텍스트 임베드 금지(CF-B1 AC1 "<20줄 목표")
  long_field="$(jq -r '
    to_entries
    | map(select(.key != "sources" and .key != "_skeleton_note"))
    | map(select(.value | type == "string" and (length > 200)))
    | length
  ' "$f")"
  if [[ "$long_field" -gt 0 ]]; then
    error "$rel — 200자 초과 문자열 필드 발견(behavior 산문 임베드 의심)"
  else
    ok "$rel — 산문 임베드 없음(필드 길이 검사 통과)"
  fi

  # --- 3) capability_tier 폐쇄 어휘 검사 --------------------------------------
  if [[ -n "$tier" ]]; then
    if [[ " $VALID_TIERS " == *" $tier "* ]]; then
      ok "$rel — capability_tier=$tier (폐쇄 어휘 내)"
    else
      error "$rel — capability_tier='$tier' 는 폐쇄 어휘(full-hook/instruction-only/mcp) 밖"
    fi
  fi

  # --- 4) 포인터 유효성 -------------------------------------------------------
  if [[ "$has_sources" == "true" ]]; then
    for key in roles skills commands hooks; do
      ptr="$(jq -r ".sources.${key} // empty" "$f")"
      [[ -z "$ptr" ]] && continue
      target="$fdir/$ptr"
      if [[ -d "$target" ]]; then
        ok "$rel — sources.$key -> $ptr (존재)"
        continue
      fi
      # LD-5 폴백: 제품 트리 미조립 상태에서는 원본 bathos/ 동일 상대 위치로 확인.
      # (fdir가 dist/... 이므로, "target"의 .claude/xxx 접미사만 뽑아 원본과 대조)
      suffix="${ptr##*.claude/}"
      orig_target="$BATHOS_ROOT/../../bathos/.claude/$suffix"
      if [[ -d "$orig_target" ]]; then
        info "$rel — sources.$key -> $ptr : 제품 트리 미존재(LD-5 조립 대기), 원본 bathos/.claude/$suffix 로 확인됨(통과)"
        PASS_COUNT=$((PASS_COUNT + 1))
      else
        error "$rel — sources.$key -> $ptr : 대상 디렉터리 없음(제품 트리·원본 모두 부재 — 깨진 포인터)"
      fi
    done
  fi
done

info "총 ${PASS_COUNT}건 통과"
if [[ "$FAIL" -eq 0 ]]; then
  info "전체 통과"
  exit 0
else
  error "실패 항목 존재 — 위 목록을 확인하세요."
  exit 1
fi
