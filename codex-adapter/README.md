# codex-adapter — BATHOS × OpenAI Codex CLI 게이트/저장 어댑터

> 성격: Claude Code의 `TaskCompleted`(W3 게이트) · `SessionEnd`(종료 시 저장)를
> Codex CLI의 확장점(`PreToolUse` · `Stop`)으로 재설계한 실구현. 설계 원본:
> `.agent-team/04-architecture/w2-runtime-p4-design-kr.md` §B (James, 2026-07-16).
> 왜 `.claude/hooks/`가 아니라 이 디렉터리인가: freeze-guard 경로 충돌을 피하고,
> Codex 전용 런타임 코드를 Claude Code 훅과 분리하기 위해서다.
>
> **P5 하드닝(2026-07-16, Phillip):** Codex CLI **v0.144.5**(macos-x86_64)를 실제
> 설치해 바이너리 임베드 스키마를 실측했다. 문서 예시 기준 가정(`tool_name` =
> `Bash`/`apply_patch`)이 실측과 달라(실제 값 = `shell`/`exec_command`/
> `apply_patch`, **`Bash`는 존재하지 않음**) T1/T2 트리거가 발화하지 않고 W3
> 게이트가 **fail-open**되는 버그가 있었다. 아래 문서·훅·config 예시는 모두
> 이 실측값 기준으로 갱신됐다(버전 고정 v0.144.5 — 업그레이드 시 재확인 필요).

---

## 무엇이 들어있나

```
codex-adapter/
├── hooks/
│   ├── pretooluse-gate.sh       # W3 Implementation 게이트: FAIL이면 구현 진입 exit 2 차단
│   ├── stop-save.sh             # SessionEnd 근사: 턴마다 증분 저장(state show 덤프)
│   └── _test-codex-hooks.sh     # 시뮬레이션 테스트(실제 Codex 설치 불요, 18케이스: 게이트 B-1~B-10·B-15~B-18 + 저장 B-11~B-14, 37 assertion)
├── run-role.sh                  # runtime=codex 역할 위임 러너(ADR-D-0006, §A3.3) — Claude Code 팀원이 아닌 별도 프로세스로 역할 실행
├── _test-run-role.sh            # run-role.sh 스모크 테스트(T8, 10케이스·24 assertion, 스텁 codex로 실제 설치 불요)
└── config.toml.example          # ~/.codex/config.toml 등록 예시(실측 스키마)
```

### `run-role.sh` — Codex 런타임 위임 러너

`_state/model-plan.json`에서 `runtime=codex`로 배정된 역할은 Claude Code 팀원으로
스폰되지 않는다(GLM과 달리 별도 프로세스라 in-process 팀원과 병행 무충돌 —
ADR-D-0006). 웨이브 커맨드가 대신 이 스크립트로 위임한다:

```bash
codex-adapter/run-role.sh <role-slug> <task-file.md> [--project <절대경로>] [--dry-run]
```

- **exit 0**=완료 / **1**=실행 실패(`.codex/agents/<slug>.toml` 부재 포함) /
  **3**=`E-CODEX-ABSENT`(codex CLI 미설치) / **4**=`E-CODEX-AUTH`(미인증 — 선제
  `codex login status` 검사 또는 `codex exec` 실행 결과 양쪽에서 판정 가능).
- 프롬프트 = 역할 base md 본문(`.claude/agents/_base/<n>-<slug>.md`, `slug:`
  프론트매터로 탐색) + `ETHOS.md` 전문 + 웨이브 커맨드가 작성한 task-file. 조립된
  프롬프트를 `codex exec "<프롬프트>"`로 비대화 실행한다(⚠️[추정] — 라이브 인증
  세션 미실측, §정직한 한계 참고). `bathos model resolve <slug> --json`이
  `model`/`reasoning_effort` 오버라이드를 찾으면 `-c` 플래그로 전달한다.
- `.codex/agents/<slug>.toml`이 프로젝트 로컬(`<project>/.codex/agents/`) 또는
  `~/.codex/agents/`에 없으면 **자동 생성하지 않고** `scripts/to-codex.sh --write`
  안내 후 exit 1로 끝난다(홈 디렉터리 쓰기는 사용자 승인 사안).
- 완료/실패는 항상 `_state/panes/inbox/codex-<slug>-<ts>-<pid>.txt`에
  `<DONE|FAIL|DRY-RUN><TAB><요약 1줄>`로 기록된다(bathos-tui `inbox.rs`와 동일
  ts-pid·atomic tmp→mv 관례) — Codex는 Agent Teams 메시징/훅 밖이므로 이 파일이
  Paul/패널이 진행을 감지하는 유일한 경로다(경계 명시, §A3.3).
- 테스트: `bash codex-adapter/_test-run-role.sh` — 스텁 `codex` 바이너리로
  10가지 분기(부재/TOML부재/인증실패 선제·사후/dry-run/정상실행/일반오류/
  잘못된 slug/task-file 부재)를 24개 assertion으로 검증한다.

## 설치

1. `bathos` 바이너리를 빌드해 둔다(`cargo build --release -p bathos-cli` 또는
   저장소 루트에서 `cargo build --release`). 훅은 다음 순서로 바이너리를 찾는다:
   `$BATHOS_BIN` > `<project>/core/target/release/bathos` >
   `<project>/core/target/debug/bathos` > `PATH`의 `bathos`.
2. `codex-adapter/config.toml.example`을 열어 `<BATHOS_ROOT>`를 실제 절대경로로
   치환한 뒤, 그 내용을 `~/.codex/config.toml`에 병합한다(기존 `[hooks.*]`가
   있으면 배열 항목을 추가). `hooks.json`을 이미 쓰고 있다면 Codex가 둘을
   병합하며 경고를 낼 수 있다([문서]) — 한쪽으로 통일 권장.
3. 두 훅 스크립트가 실행 가능한지 확인: `chmod +x codex-adapter/hooks/*.sh`
   (이미 실행 권한이 설정돼 있음).
4. Codex를 재시작하고, 아무 도구 호출이나 실행해 훅이 조용히 통과하는지 확인
   (기본값 = 무해).

## 동작 요약

### `pretooluse-gate.sh` — W3 게이트

- **트리거될 때만** `bathos gate show`를 호출한다(모든 도구 호출마다가 아님).
  tool_name은 **Codex v0.144.5 실측값만** 매칭한다 — `shell` / `exec_command`
  (셸 실행) / `apply_patch`(파일 편집). ⚠️ `Bash`는 존재하지 않는 tool_name이며
  (P4가 문서 예시만 보고 가정했던 값 — 실측으로 폐기), 케이스문에서 완전히
  제거했다:
  - **T1 소스 쓰기**: `apply_patch`가 `src/`·`core/crates/`·`core/src/`·
    `codex-adapter/hooks/` 경로를 언급하거나, `shell`/`exec_command`가
    (`sed -i`/`tee`/`cp`/`mv` 등으로) 같은 경로에 직접 쓸 때. `.agent-team/`
    전용 언급은 제외(문서 쓰기는 게이트 대상 아님).
  - **T2 웨이브 진입 명령**: `shell`/`exec_command`의 명령 문자열이
    `bathos wave advance|start|enter`, `wave5`, `wave4-implement`,
    `W5 진입/시작`, `implementation start` 등에 매치. (`apply_patch`는 명령
    실행이 아니므로 T2 대상이 아니다.)
- verdict가 **FAIL**일 때만 `exit 2` + stderr 사유로 차단한다. PASS/CONCERNS/
  verdict 확인 불가(bathos·state·report 모두 없음)는 전부 `exit 0`(fail-safe).
- verdict 조회 우선순위: `bathos gate show` > `manifest.json` 보수적 파싱 >
  `readiness-report-kr.md` 프론트매터(`.claude/hooks/gate-enforce.sh`와 동일
  3단 우선순위·동일 판정 의미론 — ADR-P4-2).

### `stop-save.sh` — SessionEnd 근사

- Codex에는 `SessionEnd`가 없다. 이 훅은 **매 턴 종료(Stop)마다** 다음을
  증분 수행해 근사한다:
  1. `bathos state show` 덤프를 `_state/session-state.json`에 원자적으로
     (`tmp` → `mv`) 교체 저장.
  2. `_state/codex-stop-save.log`에 1줄 append(500줄 롤링, 1행은 항상 마지막
     heavy-save epoch을 자기 기록).
  3. `_state/SESSION-SNAPSHOT.md`가 있으면 그날짜 아카이브로 복사(원본이
     없으면 아무 것도 만들지 않는다 — 훅은 서술을 생성하지 않는다).
  4. **디바운스**(기본 10초, `BATHOS_STOP_SAVE_DEBOUNCE`로 조정, 0=끔): 짧은
     간격의 연속 턴에서는 무거운 저장(1·3)을 건너뛰고 로그만 남긴다.
- **항상 exit 0.** `stop_hook_active=true`면 즉시 통과(루프 가드).

### 정직한 한계 (읽어야 함)

- **LLM 서술 스냅샷 생성 불가** — `SESSION-SNAPSHOT.md`의 "★ 현재 상태" 같은
  서술은 훅이 아니라 모델 턴에서만 만들 수 있다. Codex에서도 세션 중
  `/save-session` 상당(프롬프트) 수동 실행을 병행해야 한다.
- **HTML 태스크리포트 생성은 범위 밖** — `session-report.sh` 이식은 P4
  스코프가 아니다.
- **저장 손실 창** — 최대 "1턴 + 디바운스(기본 10초)". SessionEnd 방식보다
  좁지만(세션 단위가 아니라 턴 단위) 0은 아니다.
- **Codex `tool_name` fail-open 리스크 — P5(2026-07-16)에서 해소됨** — P4는
  공식 문서 예시(`"Bash"`·`"apply_patch"`)만 보고 매칭했으나, 실제 설치에서
  `tool_name`이 다른 값(`shell`/`exec_command`)으로 와 T1/T2가 발화하지 않고
  게이트가 무력화되는 버그가 실측으로 확정됐다. `pretooluse-gate.sh`를
  실측 3종 tool_name(`shell`/`exec_command`/`apply_patch`)으로 갱신하고
  `_test-codex-hooks.sh`에 회귀 케이스(B-15~B-18)를 추가해 재발을 감시한다.
  **단, 이 해소는 v0.144.5(macos-x86_64) 기준이다 — 다른 버전/플랫폼으로
  업그레이드하면 `codex features list`·PreToolUse stdin을 재실측할 것.**
- **라이브 인증 세션 미실행(남은 미실증)** — 이번 실측은 로컬 API 키 없이
  진행되어 `codex` 바이너리의 스키마·계약(이벤트명·필드·tool_name·exit 코드
  의미론)만 확인했다. 실제 인증된 세션에서 훅이 배선대로 호출되는지(config
  등록 반영·타이밍 등)는 아직 라이브로 실행해 보지 못했다 — 다음 실사용
  시 최우선 확인 항목으로 남긴다.
- **Windows 네이티브 미지원** — macOS/Linux/WSL만 지원(bash 3.2+ 호환 작성).
  `.ps1` 포트는 이번 스코프 밖이나, `commandWindows` 필드가 실측 스키마에
  존재함을 확인했다(Windows 네이티브 경로가 실제로 있다는 뜻 — 포트 시
  참고). Windows 사용자는 현재로선 WSL에서 Codex 구동을 권장
  (`scripts/wsl-setup.sh`).
- **네이티브 마이그레이션 대안(미검토)** — Codex는 외부 config(AGENTS_MD/
  HOOKS/COMMANDS/SUBAGENTS/MCP)의 네이티브 import를 지원한다. `to-codex.sh`
  스캐폴드 변환 대신 이 경로를 쓰는 방안은 아직 검토하지 않았다(후속 과제).

## 테스트

```bash
bash codex-adapter/hooks/_test-codex-hooks.sh
```

실제 Codex 설치나 실제 `bathos` 빌드 없이도 동작한다(스텁 `bathos`를
`mktemp` 디렉터리에 생성해 `gate show`/`state show` 출력을 재생). 18개
설계 테스트 케이스(게이트 B-1~B-10·B-15~B-18, 저장 B-11~B-14)를 세분화한
37개 assertion으로 검증하며, 전부 통과 시 `전체 통과` + exit 0으로 끝난다.
B-15~B-18은 P5(2026-07-16) 실측 회귀 케이스로, (a) `exec_command`의 웨이브
진입 명령 차단, (b) `shell`의 소스 직접쓰기(`sed -i`) 차단, (c) 폐기된
`Bash` 가정이 여전히 무해함(비트리거·bathos 미호출), (d) 무관 tool_name
통과를 계약화한다.

추가로 실제 `bathos` 바이너리(`core/target/release/bathos`)를 빌드해 두면
두 훅을 실제 프로젝트 state 사본에 대해 수동으로 돌려 통합 확인도 가능하다
(위 §동작 요약의 verdict 우선순위·원자적 저장이 실물로 동작함을 확인 완료
— 구현 노트 참고).

## 환경변수

| 변수 | 기본값 | 설명 |
|------|--------|------|
| `BATHOS_BIN` | (자동 탐지) | `bathos` 바이너리 절대경로 강제 지정 |
| `BATHOS_STATE_DIR` | `<project>/.agent-team/_state` | state 디렉터리 강제 지정 |
| `BATHOS_PROJECT_DIR` | stdin의 `cwd` | 프로젝트 루트 강제 지정 |
| `BATHOS_GATE_SRC_ERE` | (§B2 기본 ERE) | pretooluse-gate.sh T1 소스 경로 패턴 오버라이드 |
| `BATHOS_STOP_SAVE_DEBOUNCE` | `10` | stop-save.sh 디바운스 초(0=끔) |

## 출처·근거

- 설계: `.agent-team/04-architecture/w2-runtime-p4-design-kr.md`
- 상위 포팅 판단: `docs/PORTABILITY-kr.md`, `docs/codex-adapter-kr.md`
- 스타일 정본: `.claude/hooks/{gate-enforce,careful-guard,session-start}.sh`
- Codex hooks 공식 문서(2026-07-16 확인): https://developers.openai.com/codex/hooks ,
  https://learn.chatgpt.com/docs/hooks
- **실측 SSOT(2026-07-16, P5, Phillip)**: Codex CLI **v0.144.5**
  (macos-x86_64) 실제 설치 바이너리에서 `codex features list` + PreToolUse
  훅 stdin/config 스키마를 직접 확인. 이 문서·훅·config 예시의 이벤트명·
  필드명·tool_name 값은 모두 이 실측에 근거한다(문서 예시만 보고 추정한
  것이 아님). 라이브 인증 세션은 미실행(auth 부재) — §정직한 한계 참고.
