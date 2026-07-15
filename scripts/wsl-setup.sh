#!/usr/bin/env bash
# =============================================================================
# BATHOS — wsl-setup.sh   (WSL 프리플라이트 · 복구)
# WSL(Windows Subsystem for Linux)에서 BATHOS를 쓰기 전에 한 번 실행한다.
#
#   ./scripts/wsl-setup.sh            점검 + 복구(줄바꿈 LF 정규화·실행비트)
#   ./scripts/wsl-setup.sh --check    점검만(수정하지 않음)
#
# 배경: WSL은 리눅스 환경이므로 BATHOS의 **bash 버전(.sh 훅 + 리눅스 `bathos`
# 바이너리)** 을 그대로 쓴다(Windows PowerShell 포트 .ps1은 WSL에서 불필요).
# 유일한 함정은 저장소를 **Windows에서 CRLF로 클론**한 경우 — `.sh`의 shebang이
# `#!/usr/bin/env bash^M`이 되어 `bad interpreter` 오류가 난다. `.gitattributes`가
# 앞으로는 이를 막지만, 이미 CRLF로 받은 트리를 이 스크립트가 LF로 복구한다.
#
# 안전: 아무것도 삭제하지 않는다. .sh 파일의 CR 제거 + 실행비트 부여만 한다(멱등).
# 이식성: bash 3.2+(macOS 기본) 호환 — mapfile/연관배열 미사용.
# =============================================================================
set -uo pipefail

PKG_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CHECK_ONLY=0
[ "${1:-}" = "--check" ] && CHECK_ONLY=1

say()  { printf '\033[0;36m[bathos-wsl]\033[0m %s\n' "$*"; }
warn() { printf '\033[0;33m[bathos-wsl] ⚠ %s\033[0m\n' "$*" >&2; }
ok()   { printf '\033[0;32m[bathos-wsl] ✓ %s\033[0m\n' "$*"; }

# 제품 트리의 .sh 파일을 나열(core/target·.git·node_modules 제외).
list_sh() {
  find "$PKG_ROOT" -type f -name '*.sh' \
    -not -path '*/core/target/*' \
    -not -path '*/.git/*' \
    -not -path '*/node_modules/*' 2>/dev/null | sort
}

# --- 1. WSL 여부 안내 (강제는 아님 — 리눅스/macOS에서도 무해하게 동작) -----------
if grep -qiE '(microsoft|wsl)' /proc/version 2>/dev/null; then
  say "WSL 환경 감지됨 — BATHOS는 여기서 bash(.sh) 버전으로 동작합니다."
else
  say "WSL이 아닌 것 같습니다(순수 Linux/macOS?). 이 스크립트는 그래도 안전하게 동작합니다."
fi

TOTAL_SH="$(list_sh | wc -l | tr -d ' ')"
say "검사 대상 .sh: ${TOTAL_SH}개"

# --- 2. CRLF 탐지 + (복구 모드면) LF 정규화 --------------------------------------
crlf_count=0
fixed_count=0
while IFS= read -r f; do
  [ -n "$f" ] || continue
  if LC_ALL=C grep -lq $'\r' "$f" 2>/dev/null; then
    crlf_count=$((crlf_count + 1))
    rel="${f#$PKG_ROOT/}"
    if [ "$CHECK_ONLY" -eq 1 ]; then
      warn "CRLF: $rel"
    else
      tmp="$f.wsl-eol.$$"
      if tr -d '\r' < "$f" > "$tmp" 2>/dev/null && mv "$tmp" "$f"; then
        fixed_count=$((fixed_count + 1))
        ok "LF 정규화: $rel"
      else
        rm -f "$tmp" 2>/dev/null || true
        warn "정규화 실패(권한?): $rel"
      fi
    fi
  fi
done < <(list_sh)

if [ "$crlf_count" -eq 0 ]; then
  ok "모든 .sh가 이미 LF입니다(shebang 안전)."
elif [ "$CHECK_ONLY" -eq 1 ]; then
  warn "$crlf_count개 파일이 CRLF입니다 — './scripts/wsl-setup.sh'(--check 없이)로 복구하세요."
else
  say "$fixed_count개 파일을 LF로 정규화했습니다."
fi

# --- 3. 실행비트 부여 (복구 모드) ------------------------------------------------
if [ "$CHECK_ONLY" -eq 0 ]; then
  while IFS= read -r f; do
    [ -n "$f" ] || continue
    [ -x "$f" ] || chmod +x "$f" 2>/dev/null || true
  done < <(list_sh)
  ok ".sh 실행비트 확인/부여 완료."
fi

# --- 4. 의존성 점검 (WSL=리눅스 → apt 안내) --------------------------------------
say "의존성 점검:"
if command -v cargo >/dev/null 2>&1; then ok "cargo 있음 ($(cargo --version 2>/dev/null | awk '{print $2}'))"; else warn "Rust 미설치 — https://rustup.rs (WSL 안에서 설치). 엔진 빌드에 필요."; fi
if command -v jq >/dev/null 2>&1; then ok "jq 있음"; else warn "jq 미설치 — 'sudo apt-get update && sudo apt-get install -y jq' (bash 훅의 JSON 파싱에 필요)."; fi
if command -v claude >/dev/null 2>&1; then ok "claude CLI 있음"; else warn "Claude Code CLI 미발견 — WSL 안에 설치된 Claude Code로 실행하세요(v2.1.32+)."; fi

# --- 5. 엔진 바이너리 상태 -------------------------------------------------------
# WSL은 리눅스이므로 네이티브 바이너리는 `bathos`(ELF)다. 실수의 핵심은 Windows에서
# 빌드한 `bathos.exe`(PE)를 WSL로 들고 오는 것 — OS 무관하게 그 경우만 경고한다.
LINUX_BIN="$PKG_ROOT/core/target/release/bathos"
if [ -x "$LINUX_BIN" ]; then
  btype="$(file "$LINUX_BIN" 2>/dev/null || true)"
  if printf '%s' "$btype" | grep -qiE 'PE32|MS Windows|MS-DOS'; then
    warn "core/target/release/bathos 가 Windows 실행파일(PE)로 보입니다 — WSL/리눅스에서는 'cd core && cargo build --release'로 네이티브(ELF) 바이너리를 다시 빌드하세요."
  else
    ok "엔진 빌드됨: core/target/release/bathos ($(printf '%s' "$btype" | sed 's/.*: //' | cut -c1-24))"
  fi
else
  say "엔진 미빌드 — WSL 안에서: cd core && cargo build --release  (→ core/target/release/bathos)"
fi

# --- 6. 다음 단계 ----------------------------------------------------------------
printf '\n'
say "다음 단계(WSL):"
printf '  1) 엔진 빌드:   cd core && cargo build --release && cd ..\n'
printf '  2) 경로 지정:   export BATHOS_BIN="$PWD/core/target/release/bathos"\n'
printf '  3) 설치(선택):  ./install.sh --into /abs/path/to/project\n'
printf '  4) WSL 안에서 Claude Code를 열고 /team-kickoff 부터 진행\n'
printf '  가이드: docs/wsl-install-kr.md · docs/macos-linux-install-kr.md\n'

exit 0
