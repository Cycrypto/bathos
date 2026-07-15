# BATHOS WSL 설치 가이드 (Windows Subsystem for Linux)

> **BATHOS** — βάθος('깊이·심연'). 이 문서는 **WSL(Windows Subsystem for Linux)에서** BATHOS를 빌드·설치·구동하는 방법을 다룬다.
> **핵심 한 줄:** WSL은 리눅스 환경이므로 BATHOS는 여기서 **bash 버전**(`.sh` 훅 + 리눅스 `bathos` 바이너리)으로 동작한다 — Windows 네이티브용 PowerShell 포트(`.ps1`)는 WSL에서 **쓰지 않는다**.
> 순수 macOS/Linux는 [`macos-linux-install-kr.md`](macos-linux-install-kr.md), Windows 네이티브(PowerShell)는 [`windows-install-kr.md`](windows-install-kr.md).
> 설치 이후 파이프라인 사용법은 플랫폼 공통 — [`USAGE-kr.md`](USAGE-kr.md).

---

## 0. WSL에서는 무엇을 쓰나 (한눈에)

| 항목 | WSL (= 리눅스) |
|------|----------------|
| 엔진 바이너리 | `bathos` (**ELF**, WSL 안에서 `cargo build`) — `bathos.exe` 아님 |
| 훅 스크립트 | `.claude/hooks/*.sh` (bash) — `.ps1` 아님 |
| 설치기 | `install.sh` (bash) — `install.ps1` 아님 |
| JSON 파싱 | `jq` (apt로 설치) |
| settings.json | 커밋된 bash 배선본 그대로(교체 불필요) |

> **왜 PowerShell 포트를 안 쓰나:** WSL 안의 Claude Code는 리눅스 실행환경에서 훅을 bash로 실행한다. `.ps1`/`settings.windows.json`/`install.ps1`은 **Windows 네이티브(PowerShell)** 전용이다. WSL에서는 macOS/Linux와 동일한 경로를 따른다.

---

## 1. 전제 (Prerequisites)

WSL **안에서**(WSL 배포판 셸, 예: Ubuntu) 다음을 준비한다:

- **WSL2** + 리눅스 배포판(Ubuntu 권장). Windows PowerShell에서 `wsl --install` 후 배포판 셸 진입.
- **Claude Code v2.1.32+** — **WSL 안에** 설치된 것을 사용(Windows 네이티브 Claude Code 아님).
- **Rust 툴체인** — WSL 안에서 [rustup.rs](https://rustup.rs)로 설치(`curl … | sh`). `cargo --version`으로 확인.
- **`jq`** — `sudo apt-get update && sudo apt-get install -y jq` (bash 안전 훅의 JSON 파싱에 필요).
- **Git** — 보통 기본 포함. 없으면 `sudo apt-get install -y git`.

---

## 2. 소스 받기 (⚠️ 줄바꿈 주의)

**WSL 파일시스템 안(`~/…`)에 클론하는 것을 권장**한다(`/mnt/c/…`의 Windows 경로는 I/O가 느리고 권한/줄바꿈 문제가 잦다).

```bash
cd ~
git clone <your-fork-url> bathos
cd bathos
```

> **최대 함정 — CRLF 줄바꿈:** 저장소를 **Windows 쪽 git으로 클론**했거나 `core.autocrlf=true`이면 `.sh` 파일이 CRLF가 되어 WSL에서 `bad interpreter: /usr/bin/env bash^M` 오류가 난다. 이 저장소는 **`.gitattributes`로 `.sh`를 LF로 강제**하므로 WSL 안에서 클론하면 문제가 없다. 이미 CRLF로 받았다면 §3의 `wsl-setup.sh`가 복구한다.

---

## 3. WSL 프리플라이트 — `scripts/wsl-setup.sh`

WSL 사용 전 한 번 실행한다. `.sh` 줄바꿈을 LF로 정규화하고 실행비트를 부여하며, 의존성·엔진 상태를 점검한다(아무것도 삭제하지 않음, 멱등):

```bash
./scripts/wsl-setup.sh            # 점검 + 복구
./scripts/wsl-setup.sh --check    # 점검만(수정 안 함)
```

출력 예: WSL 감지 여부 · CRLF 파일 정규화 결과 · `cargo`/`jq`/`claude` 유무 · 엔진 바이너리 종류(Windows PE면 경고).

---

## 4. 엔진 빌드 (WSL 안에서 = 리눅스 ELF)

```bash
cd core
cargo build --release        # → core/target/release/bathos  (ELF 64-bit)
cargo test --all             # (선택) 전체 검증
cd ..
```

> **주의:** Windows에서 빌드한 `bathos.exe`(PE)를 WSL로 복사해 쓰지 말 것 — WSL에서는 **WSL 안에서 빌드한 ELF `bathos`** 를 쓴다. (`wsl-setup.sh`가 PE를 감지하면 경고한다.)

---

## 5. 엔진 경로 · Agent Teams 플래그

```bash
export BATHOS_BIN="$PWD/core/target/release/bathos"
# 영구 설정:  ~/.bashrc 에 위 export 줄 추가
```

Agent Teams 실험 기능은 배선된 `settings.json`(env)에 이미 켜져 있다. 필요 시 `export CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS=1`.

---

## 6. (선택) 타겟 프로젝트 주입 — `install.sh`

WSL에서는 **bash 설치기**를 쓴다(`install.ps1` 아님):

```bash
./install.sh                                   # 엔진 빌드 + 안내
./install.sh --into /abs/path/to/your-project  # + .claude/ assets/ modules/ 복사
#   --force 로 기존 .claude/ 덮어쓰기
```

커밋된 `.claude/settings.json`(bash 배선)이 그대로 쓰이므로 Windows판 같은 배선 교체는 없다.

---

## 7. 설치 검증

```bash
"$BATHOS_BIN" --version
"$BATHOS_BIN" doctor
bash .claude/hooks/_test-hooks.sh

# 파괴 명령 차단 스모크(2가 나오면 정상):
echo '{"tool_name":"Bash","tool_input":{"command":"rm -rf /"}}' | bash .claude/hooks/careful-guard.sh; echo $?
```

이후 WSL 안에서 Claude Code를 열고 파이프라인을 구동한다(`/team-kickoff` → `/route` → 웨이브 …). 상세는 [`USAGE-kr.md`](USAGE-kr.md).

---

## 8. 문제 해결 (WSL 특화)

| 증상 | 원인 · 해결 |
|------|-------------|
| `bad interpreter: /usr/bin/env bash^M` | `.sh`가 CRLF. `./scripts/wsl-setup.sh`로 LF 복구. 근본적으로는 WSL 안에서 다시 클론(`.gitattributes`가 LF 보장). |
| `permission denied` (훅/스크립트) | 실행비트 없음. `./scripts/wsl-setup.sh`가 `chmod +x` 부여, 또는 `chmod +x .claude/hooks/*.sh scripts/*.sh install.sh`. |
| `bathos: cannot execute binary file` / `Exec format error` | Windows용 `bathos.exe`(PE)를 WSL에서 실행 시도. WSL 안에서 `cargo build --release`로 ELF 재빌드. |
| `jq: command not found` | `sudo apt-get install -y jq`. |
| 빌드/파일 I/O가 느림 | 프로젝트가 `/mnt/c/…`(Windows 경로)에 있음. WSL 홈(`~/…`)으로 옮기면 빨라진다. |
| Windows 에디터로 편집 후 훅이 깨짐 | 에디터가 CRLF로 저장. 에디터를 LF로 설정하거나 편집 후 `./scripts/wsl-setup.sh` 실행. |

---

## 9. 세 가지 실행 경로 정리 (참고)

| 환경 | 엔진 | 훅 | 설치기 | 가이드 |
|------|------|-----|--------|--------|
| macOS / Linux | `bathos` (ELF/Mach-O) | `.sh` | `install.sh` | [macos-linux-install-kr.md](macos-linux-install-kr.md) |
| **WSL** | **`bathos` (ELF, WSL 빌드)** | **`.sh`** | **`install.sh`** | **(이 문서)** |
| Windows 네이티브 | `bathos.exe` | `.ps1` | `install.ps1` | [windows-install-kr.md](windows-install-kr.md) |

세 경로 모두 동일 리포지토리에서 나온다. 두 훅 트리(`*.sh`·`*.ps1`)가 `.claude/hooks/`에 함께 들어있고, `.gitattributes`가 `.sh`를 어느 OS에서 클론하든 LF로 유지한다.
