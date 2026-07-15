# BATHOS macOS / Linux 설치 가이드 (bash)

> **BATHOS** — βάθος('깊이·심연'). 이 문서는 **macOS/Linux에서** BATHOS를 빌드·설치·구동하는 방법만 다룬다.
> macOS/Linux판은 **bash 훅(`.sh`)** 으로 동작한다. Windows(PowerShell)는 [`windows-install-kr.md`](windows-install-kr.md) 참조.
> 설치 이후의 파이프라인 사용법(웨이브·게이트·CLI)은 플랫폼 공통이므로 [`USAGE-kr.md`](USAGE-kr.md)를 그대로 따르면 된다.
> 제거는 [`uninstall-kr.md`](uninstall-kr.md) 참조(스크립트는 `scripts/uninstall.sh`).

---

## 0. macOS/Linux판 한눈에

| 항목 | macOS / Linux |
|------|---------------|
| 엔진 바이너리 | `bathos` (단일 정적 바이너리, Rust) |
| 훅 스크립트 | `.claude/hooks/*.sh` (bash) |
| JSON 파싱 | `jq` 필요 |
| 훅 배선(settings.json) | 커밋된 `.claude/settings.json`(bash 배선)을 그대로 사용 |

> **핵심:** 커밋된 리포지토리의 `.claude/settings.json`이 **macOS/Linux용(bash 배선)** 정본이라 별도 교체가 필요 없다. (Windows에서는 `install.ps1`이 설치 시점에 PowerShell 배선으로 교체한다 — [Windows 가이드](windows-install-kr.md) §4.)

---

## 1. 전제 (Prerequisites)

- **POSIX 셸 환경**(macOS 또는 Linux) — 훅은 bash로 작성됨.
- **[Claude Code](https://claude.com/claude-code) v2.1.32 이상** + Agent Teams 실험 기능(아래 §5).
- **Rust 툴체인** — 엔진(`bathos`) 빌드용. [rustup.rs](https://rustup.rs)에서 설치. `cargo --version`으로 확인(1.92+ 검증).
- **`jq`** — bash 안전 훅의 JSON 파싱에 필요. macOS `brew install jq` / Debian·Ubuntu `apt-get install -y jq`.

---

## 2. 소스 받기 & 엔진 빌드

```bash
git clone <your-fork-url> bathos
cd bathos

# 엔진 빌드 (단일 정적 바이너리 bathos 생성, ~5.9MB)
cd core
cargo build --release        # → core/target/release/bathos
cargo test --all             # (선택) 전체 검증
cd ..
```

빌드가 끝나면 엔진은 `core/target/release/bathos`에 생긴다.

---

## 3. `install.sh` — 엔진 빌드 + (선택) 타겟 프로젝트 주입

`install.sh`는 두 가지를 한다. 그리고 **아무것도 삭제하지 않는다**(대상에 이미 `.claude/`가 있으면 `--force` 없이는 덮어쓰기를 거부).

```bash
# A) 엔진만 빌드 + 다음 단계 안내 출력
./install.sh

# B) 엔진 빌드 + 메서드 패키지를 타겟 프로젝트로 복사
./install.sh --into /abs/path/to/your-project
#   → your-project/ 안으로 .claude/  assets/  modules/  를 cp -R

# 기존 .claude/ 를 덮어써야 하면:
./install.sh --into /abs/path/to/your-project --force

# 도움말:
./install.sh --help
```

### 플래그

| 플래그 | 의미 |
|--------|------|
| `--into <DIR>` | 엔진 빌드에 더해 `.claude/`·`assets/`·`modules/`를 `<DIR>`로 복사 |
| `--force` | 대상에 이미 `.claude/`가 있어도 덮어씀 |
| `--help` | 사용법 출력 |

---

## 4. 타겟 프로젝트에 생기는 두 디렉터리

```
your-project/
├── .claude/          ← (연동 자산) 커맨드·역할·훅·settings   ※install로 주입
└── .agent-team/      ← (산출물) 웨이브 진행 중 팀이 쌓는 결과물 · 상태(_state)
```

실제 제품 소스 코드는 통상 경로(`src/` 등)에 둔다. `.agent-team/`은 팀 작업·산출 메타다.

---

## 5. 엔진 경로 · Agent Teams 플래그 설정

훅과 커맨드가 엔진을 찾도록 `BATHOS_BIN`을 지정한다(또는 `core/target/release`를 PATH에 추가):

```bash
# 현재 셸에만:
export BATHOS_BIN="$(pwd)/core/target/release/bathos"

# 영구 설정(예: ~/.zshrc / ~/.bashrc 에 추가):
echo 'export BATHOS_BIN="/abs/path/to/bathos/core/target/release/bathos"' >> ~/.zshrc
```

Agent Teams 실험 기능은 배선된 `settings.json`(env 블록)에 이미 켜져 있다. 필요하면 셸에서 직접:

```bash
export CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS=1
```

> `BATHOS_BIN`을 지정하지 않으면 훅의 기본 탐색 경로는 `core/target/debug/bathos`다(release를 권장).

---

## 6. 설치 검증 (Preflight)

```bash
# 엔진 동작 확인
"$BATHOS_BIN" --version

# 엔진 자체 프리플라이트(설치·배선·jq·훅 exec 비트·settings 점검)
"$BATHOS_BIN" doctor

# 안전 훅 결정성 하네스 실행
bash .claude/hooks/_test-hooks.sh
```

훅 하나를 손으로 시험하려면(파괴 명령이 차단되는지):

```bash
echo '{"tool_name":"Bash","tool_input":{"command":"rm -rf /"}}' \
  | bash .claude/hooks/careful-guard.sh
echo $?   # → 2 (차단)이면 정상
```

---

## 7. 사용 시작

여기부터는 플랫폼 공통이다 — Claude Code를 프로젝트에서 열고 파이프라인을 구동한다:

```text
/team-kickoff
/route        /abs/path/to/project
/wave1-discovery   /abs/path
/wave2-design      /abs/path
/wave3-story-gate  /abs/path
/wave5-implement   /abs/path
/wave6-verify-report /abs/path
/team-confirm
```

상세는 [`USAGE-kr.md`](USAGE-kr.md) 참조.

---

## 8. 문제 해결 (Troubleshooting)

| 증상 | 원인 · 해결 |
|------|-------------|
| 훅이 안 걸림 / `permission denied` | 훅 실행 비트 확인 → `chmod +x .claude/hooks/*.sh`. `"$BATHOS_BIN" doctor`로 배선 점검. |
| `jq: command not found` | `brew install jq`(macOS) / `apt-get install -y jq`(Debian·Ubuntu). 안전 훅의 JSON 파싱에 필요. |
| `build did not produce ...bathos` | Rust 툴체인 미설치/구버전. `rustup update` 후 `cargo build --release` 재시도. |
| 서브에이전트 시작 시 검증 프롬프트가 뜨며 무한 대기 | `settings.json`의 `hooks` 블록에 주석 키(`_note` 등)가 있는지 확인 — 유효 훅 이벤트명만 두어야 한다(설명은 top-level 키로). |
| 감사(audit) 관련 오류 | `BATHOS_BIN`이 실제 `bathos` 바이너리를 가리키는지 확인. 감사 키는 첫 실행 시 OS CSPRNG(`/dev/urandom`·`getentropy`)로 자동 생성된다. |

---

## 9. Windows와의 관계 (참고)

동일 리포지토리가 양쪽 OS를 지원한다. macOS/Linux는 커밋된 bash 배선(`.sh` 훅 + `settings.json`)을 그대로 쓰고, Windows는 `install.ps1`이 설치 시점에 PowerShell 배선(`.ps1` 훅 + `settings.windows.json`)으로 교체한다. 두 훅 트리(`*.sh`·`*.ps1`)가 모두 `.claude/hooks/`에 들어있다. 자세한 Windows 절차는 [`windows-install-kr.md`](windows-install-kr.md).
