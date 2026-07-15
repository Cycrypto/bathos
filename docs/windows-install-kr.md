# BATHOS Windows 설치 가이드 (PowerShell)

> **BATHOS** — βάθος('깊이·심연'). 이 문서는 **Windows에서** BATHOS를 빌드·설치·구동하는 방법만 다룬다.
> Windows판은 macOS/Linux판과 **동일한 피쳐·게이트**를 목표로, bash 훅(`.sh`)과 나란히 **PowerShell 훅(`.ps1`)** 을 제공한다.
> 설치 이후의 파이프라인 사용법(웨이브·게이트·CLI)은 플랫폼 공통이므로 [`USAGE-kr.md`](USAGE-kr.md)를 그대로 따르면 된다.
> 제거는 [`uninstall-kr.md`](uninstall-kr.md) 참조(스크립트는 `scripts/uninstall.ps1`).

---

## 0. Windows판은 무엇이 다른가 (한눈에)

| 항목 | macOS/Linux | **Windows** |
|------|-------------|-------------|
| 엔진 바이너리 | `bathos` | **`bathos.exe`** (동일 Rust 코드, 크로스컴파일) |
| 훅 스크립트 | `.claude/hooks/*.sh` (bash) | **`.claude/hooks/*.ps1`** (PowerShell) |
| JSON 파싱 | `jq` 필요 | **불필요** — PowerShell 네이티브 `ConvertFrom-Json` |
| 훅 배선(settings.json) | `.sh` + 기본 셸 | `.ps1` + `"shell": "powershell"` |
| 배선 방식 | 커밋된 `settings.json` 그대로 | **설치 시 자동 교체**(`install.ps1`) |

> **핵심:** 커밋된 리포지토리의 `.claude/settings.json`은 **macOS/Linux용(bash 배선)** 이 정본이다. Windows에서는 `install.ps1`이 **대상 프로젝트의 `settings.json`만** PowerShell 배선본(`settings.windows.json`)으로 교체한다. 즉 하나의 리포지토리로 양쪽 OS를 모두 지원한다(**설치 시점 OS 디스패치**).

---

## 1. 전제 (Prerequisites)

- **Windows PowerShell 5.1 이상** — 모든 Windows에 기본 내장(추가 설치 0). PowerShell 7+(`pwsh`)도 그대로 동작한다.
- **[Claude Code](https://claude.com/claude-code) v2.1.32 이상** + Agent Teams 실험 기능(아래 §5).
- **Rust 툴체인** — 엔진(`bathos.exe`) 빌드용. [rustup.rs](https://rustup.rs)에서 설치. `cargo --version`으로 확인.
- **`jq` 불필요** — Windows PowerShell 훅은 네이티브 JSON을 쓴다. (bash 훅을 Git Bash/WSL로 돌릴 때만 `jq`가 필요하다.)
- (선택) **Git for Windows** — 소스 클론용. `winget install Git.Git`.

> PowerShell 버전 확인: `$PSVersionTable.PSVersion` (5.1 이상이면 OK).

---

## 2. 소스 받기 & 엔진 빌드

PowerShell(관리자 권한 불필요)에서:

```powershell
git clone <your-fork-url> bathos
cd bathos

# 엔진 빌드 (단일 정적 바이너리 bathos.exe 생성)
cd core
cargo build --release        # → core\target\release\bathos.exe
cargo test --all             # (선택) 전체 검증
cd ..
```

빌드가 끝나면 엔진은 `core\target\release\bathos.exe`에 생긴다.

---

## 3. `install.ps1` — 엔진 빌드 + (선택) 타겟 프로젝트 주입

`install.ps1`은 두 가지를 한다. 그리고 **아무것도 삭제하지 않는다**(대상에 이미 `.claude\`가 있으면 `-Force` 없이는 덮어쓰기를 거부).

```powershell
# A) 엔진만 빌드 + 다음 단계 안내 출력
.\install.ps1

# B) 엔진 빌드 + 메서드 패키지를 타겟 프로젝트로 복사 (+ Windows 훅 자동 배선)
.\install.ps1 -Into C:\path\to\your-project
#   → your-project\ 안으로 .claude\  assets\  modules\  를 복사하고,
#     Windows에서는 대상의 .claude\settings.json 을 PowerShell 배선본으로 교체한다.

# 기존 .claude\ 를 덮어써야 하면:
.\install.ps1 -Into C:\path\to\your-project -Force

# 도움말:
.\install.ps1 -Help
```

### 플래그

| 플래그 | 의미 |
|--------|------|
| `-Into <DIR>` | 엔진 빌드에 더해 `.claude\`·`assets\`·`modules\`를 `<DIR>`로 복사 |
| `-Force` | 대상에 이미 `.claude\`가 있어도 덮어씀 |
| `-NoWindowsHooks` | Windows여도 `settings.json`을 **bash 배선 그대로** 둠(대상에서 Git Bash/WSL로 훅을 돌릴 때) |
| `-Help` | 사용법 출력 |

> **더블대시(`--into`)는 지원하지 않는다** — PowerShell 관례상 단일대시(`-Into`)만 받는다.

---

## 4. 설치 시점 OS 디스패치 (동작 원리)

Claude Code의 훅 `settings.json`에는 런타임 OS 조건 필드가 없다. 대신 BATHOS는 **설치 시점**에 OS에 맞는 훅을 배선한다:

1. 리포지토리에는 두 벌의 훅이 모두 들어있다 — `.claude\hooks\*.sh`(bash)와 `.claude\hooks\*.ps1`(PowerShell).
2. 커밋된 `.claude\settings.json`은 **bash 배선**(각 훅 `command`가 `*.sh`)이다 → macOS/Linux에서 그대로 동작.
3. `.claude\settings.windows.json`은 같은 구조에서 **모든 훅 `command`를 `*.ps1`로 바꾸고 각 훅에 `"shell": "powershell"`을 추가**한 참조본이다.
4. Windows에서 `install.ps1 -Into <dir>`을 실행하면(`$env:OS -eq 'Windows_NT'` 판별), `settings.windows.json`을 **대상의** `settings.json`으로 덮어쓴다. 커밋된 리포지토리 원본은 건드리지 않는다.

> Claude Code는 `"shell": "powershell"` 훅을 **프로세스 스코프에서 `-ExecutionPolicy Bypass`로 스폰**하므로, 시스템 실행 정책을 바꾸지 않아도 서명 없는 로컬 `.ps1` 훅이 실행된다.

**리포지토리 자체를 작업 디렉터리로 쓰는 경우**(대상 주입 없이) Windows에서 PowerShell 훅을 쓰려면, 리포지토리의 `settings.windows.json`을 `settings.json`으로 복사한다:

```powershell
Copy-Item .claude\settings.windows.json .claude\settings.json -Force
```
(이 파일은 위험 경로라 freeze-guard가 감시한다 — 되돌리려면 `git checkout .claude/settings.json`.)

---

## 5. 엔진 경로 · Agent Teams 플래그 설정

훅과 커맨드가 엔진을 찾도록 `BATHOS_BIN`을 지정한다(또는 `core\target\release`를 PATH에 추가):

```powershell
# 현재 세션에만:
$env:BATHOS_BIN = "$PWD\core\target\release\bathos.exe"

# 영구 설정(사용자 환경변수):
[Environment]::SetEnvironmentVariable('BATHOS_BIN', "$PWD\core\target\release\bathos.exe", 'User')
```

Agent Teams 실험 기능은 배선된 `settings.json`(env 블록)에 이미 켜져 있다. 필요하면 세션에서 직접:

```powershell
$env:CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS = "1"
```

> `BATHOS_BIN`을 지정하지 않으면 훅은 `core\target\release\bathos.exe` → `core\target\debug\bathos.exe` 순으로 탐색한다(release 권장).

---

## 6. 설치 검증 (Preflight)

```powershell
# 엔진 동작 확인
& "$env:BATHOS_BIN" --version

# 엔진 자체 프리플라이트(설치·배선 점검)
& "$env:BATHOS_BIN" doctor

# 모든 PowerShell 훅이 구문상 유효한지(파싱) 확인
Get-ChildItem .claude\hooks\*.ps1 | ForEach-Object {
  $e = $null
  [System.Management.Automation.Language.Parser]::ParseFile($_.FullName, [ref]$null, [ref]$e) | Out-Null
  if ($e) { Write-Host "PARSE ERROR: $($_.Name)" -ForegroundColor Red } else { Write-Host "ok  $($_.Name)" }
}
```

훅 하나를 손으로 시험하려면(파괴 명령이 차단되는지):

```powershell
'{"tool_name":"Bash","tool_input":{"command":"rm -rf /"}}' | & powershell -NoProfile -ExecutionPolicy Bypass -File .claude\hooks\careful-guard.ps1
$LASTEXITCODE   # → 2 (차단)이면 정상
```

전체 결정성 하네스(49 케이스):

```powershell
.\.claude\hooks\_test-hooks.ps1
```

---

## 7. 사용 시작

여기부터는 플랫폼 공통이다 — Claude Code를 프로젝트에서 열고 파이프라인을 구동한다:

```text
/team-kickoff
/route        C:\abs\path\to\project
/wave1-discovery   C:\abs\path
/wave2-design      C:\abs\path
/wave3-story-gate  C:\abs\path
/wave5-implement   C:\abs\path
/wave6-verify-report C:\abs\path
/team-confirm
```

상세는 [`USAGE-kr.md`](USAGE-kr.md) 참조.

---

## 8. 문제 해결 (Troubleshooting)

| 증상 | 원인 · 해결 |
|------|-------------|
| `.ps1 ... cannot be loaded because running scripts is disabled` | Claude Code 훅은 `-ExecutionPolicy Bypass`로 스폰되므로 훅 실행엔 영향 없다. 손으로 실행할 때만 발생 → `powershell -ExecutionPolicy Bypass -File <hook>.ps1`로 실행하거나 `Set-ExecutionPolicy -Scope CurrentUser RemoteSigned`. |
| 훅이 전혀 안 걸림 | 대상의 `.claude\settings.json`이 `.ps1`+`"shell":"powershell"`로 배선됐는지 확인(§4). 아니라면 `Copy-Item .claude\settings.windows.json .claude\settings.json -Force`. |
| `build did not produce ...bathos.exe` | Rust 툴체인 미설치/구버전. `rustup update` 후 `cargo build --release` 재시도. |
| 감사(audit) 관련 오류 | 엔진 경로(`BATHOS_BIN`)가 `bathos.exe`를 가리키는지 확인. 감사 키는 첫 실행 시 OS CSPRNG(`BCryptGenRandom`)로 자동 생성된다. |
| 한글이 깨져 보임 | 콘솔 인코딩 문제. `chcp 65001`(UTF-8) 후 재시도. 훅/리포트 파일 자체는 UTF-8로 기록된다. |

---

## 9. bash판과의 동작 차이 (정직 고지)

- **`jq` 제거:** PowerShell 훅은 네이티브 JSON을 쓴다. 기능 동일, 의존성 하나 감소.
- **`plan-toggle`·`intensity-tracker`:** bash판의 "jq 부재 시 최소 스키마로 덮어쓰기(다른 필드 유실)" 폴백 대신, PowerShell판은 **항상 기존 필드를 보존하며 병합**한다(더 안전, 동일 계약).
- **`scope-inject` 세션 캐시:** 부모 PID를 `Get-CimInstance Win32_Process`로 조회한다(bash `$PPID` 대응, Windows 한정). 조회 실패 시 재주입만 발생하는 advisory 훅이라 안전에 영향 없다.
- **게이트 계약은 1:1 보존:** `plan-gate`는 stdout `permissionDecision:"deny"` + exit 0, `careful-guard`·`freeze-guard`·`gate-enforce`는 exit 2로 차단한다(bash와 동일).

> **검증 경계:** Windows 실제 실행 검증은 CI의 `windows-latest` 잡(엔진 빌드·테스트·`.ps1` 파싱 린트·훅 하네스)이 담당한다.
