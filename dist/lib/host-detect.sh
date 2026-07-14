#!/usr/bin/env bash
# =============================================================================
# BATHOS Dynamis — dist/lib/host-detect.sh
# CF-B5 / SS9 (Could · 스텁만) — 런타임 host-detection + 출력분기 단일 모듈.
#
# ★ 이 파일은 스텁이다. 실제 Codex/Copilot 등 분기 로직은 미구현이다(LD-2:
#   Claude Code 외 full-hook 런타임 미타깃). 활성 조건: Dynamis가 Claude Code
#   외 런타임을 실제로 타깃하기로 결정할 때(현재 아님).
#
# ponytail 사상(판별 키는 (추정) — 실타깃 결정 시 해당 호스트 최신 문서로
# 재검증 필요, Search Before Building):
#   PLUGIN_DATA 존재          -> Codex        (추정)
#   COPILOT_PLUGIN_DATA 존재  -> GitHub Copilot (추정)
#   그 외(기본값)              -> Claude Code
#
# 규약(US8 AC2): 호스트 판별·출력분기 로직은 이 모듈 하나에만 존재해야 한다.
# 다른 스크립트(uninstall.sh, check-*.sh 등)에서 동일 로직을 복제하지 않는다
# (복제 시 CF-B5 AC2 위반).
#
# 사용법:
#   source dist/lib/host-detect.sh
#   host="$(bathos_detect_host)"
#   bathos_write_hook_output "$host" '{"decision":"allow"}'
# =============================================================================

# bathos_detect_host — 현재 실행 호스트를 판별해 소문자 문자열로 반환.
#   반환값: "claude" | "codex" | "copilot"
# (추정) 환경변수 키는 미검증 — 실타깃 결정 시 해당 호스트 공식 문서 확인 필요.
bathos_detect_host() {
  if [[ -n "${BATHOS_FORCE_HOST:-}" ]]; then
    # 테스트/디버깅 전용 강제 오버라이드(스모크 테스트에서 사용).
    printf '%s\n' "$BATHOS_FORCE_HOST"
    return 0
  fi
  if [[ -n "${PLUGIN_DATA:-}" ]]; then
    printf 'codex\n'
    return 0
  fi
  if [[ -n "${COPILOT_PLUGIN_DATA:-}" ]]; then
    printf 'copilot\n'
    return 0
  fi
  printf 'claude\n'
  return 0
}

# bathos_write_hook_output <host> <json_payload>
#   호스트별 훅 출력 형태로 분기해 stdout에 쓴다.
#   Claude Code 경로만 실동작(그대로 통과). 그 외 호스트는 미구현을
#   명시적으로 알리고 실패 종료(2) — 조용히 잘못된 형태를 출력하지 않는다
#   (날조 금지: 지원하지 않는 걸 지원하는 척하지 않는다).
bathos_write_hook_output() {
  local host="$1"
  local payload="$2"
  case "$host" in
    claude)
      printf '%s\n' "$payload"
      return 0
      ;;
    codex|copilot)
      printf '[bathos host-detect] ! %s 출력 분기는 미구현(스텁, Could 범위 밖). ' "$host" >&2
      printf '참조: docs/agent-portability-kr.md §2\n' >&2
      return 2
      ;;
    *)
      printf '[bathos host-detect] ✗ 알 수 없는 host: %s\n' "$host" >&2
      return 2
      ;;
  esac
}

# 직접 실행 시(source가 아니라 실행) 자가진단만 수행 — 부작용 없음.
if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
  detected="$(bathos_detect_host)"
  printf '[bathos host-detect] 감지된 host: %s (스텁 — 실제 분기는 claude만 동작)\n' "$detected"
fi
