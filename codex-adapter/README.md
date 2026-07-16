# codex-adapter — BATHOS × OpenAI Codex CLI 게이트/저장 어댑터

> 성격: Claude Code의 `TaskCompleted`(W3 게이트) · `SessionEnd`(종료 시 저장)를
> Codex CLI의 확장점(`PreToolUse` · `Stop`)으로 재설계한 실구현. 설계 원본:
> `.agent-team/04-architecture/w2-runtime-p4-design-kr.md` §B (James, 2026-07-16).
> 왜 `.claude/hooks/`가 아니라 이 디렉터리인가: freeze-guard 경로 충돌을 피하고,
> Codex 전용 런타임 코드를 Claude Code 훅과 분리하기 위해서다.

---

## 무엇이 들어있나

```
codex-adapter/
├── hooks/
│   ├── pretooluse-gate.sh       # W3 Implementation 게이트: FAIL이면 구현 진입 exit 2 차단
│   ├── stop-save.sh             # SessionEnd 근사: 턴마다 증분 저장(state show 덤프)
│   └── _test-codex-hooks.sh     # 시뮬레이션 테스트(실제 Codex 설치 불요, 14케이스)
└── config.toml.example          # ~/.codex/config.toml 등록 예시
```

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

- **트리거될 때만** `bathos gate show`를 호출한다(모든 도구 호출마다가 아님):
  - **T1 소스 쓰기**: `apply_patch`/`write_file`/`edit`/`create_file` 도구가
    `src/`·`core/crates/`·`core/src/`·`codex-adapter/hooks/` 경로를 언급.
    `.agent-team/` 전용 언급은 제외(문서 쓰기는 게이트 대상 아님).
  - **T2 웨이브 진입 명령**: `Bash` 계열 도구의 command가
    `bathos wave advance|start|enter`, `wave5`, `wave4-implement`,
    `W5 진입/시작`, `implementation start` 등에 매치.
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
- **Codex `tool_name` 실측값 미확인(⚠️ 최대 리스크)** — 공식 문서 예시는
  `"Bash"`·`"apply_patch"`뿐이라, 실제 설치에서 다른 값(`"shell"` 등)이 오면
  T1/T2 트리거가 발화하지 않아 **게이트가 무력화(fail-open)** 될 수 있다.
  설치 후 반드시 실측하고 필요시 `BATHOS_GATE_SRC_ERE`/matcher를 조정할 것.
- **Windows 네이티브 미지원** — macOS/Linux/WSL만 지원(bash 3.2+ 호환 작성).
  `.ps1` 포트는 P4 범위 밖. Windows 사용자는 WSL에서 Codex 구동을 권장
  (`scripts/wsl-setup.sh`).

## 테스트

```bash
bash codex-adapter/hooks/_test-codex-hooks.sh
```

실제 Codex 설치나 실제 `bathos` 빌드 없이도 동작한다(스텁 `bathos`를
`mktemp` 디렉터리에 생성해 `gate show`/`state show` 출력을 재생). 14개
설계 테스트 케이스(B-1~B-14)를 세분화한 assertion으로 검증하며, 전부 통과 시
`전체 통과` + exit 0으로 끝난다.

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
- Codex hooks 공식(이벤트·stdin 스키마·exit 2·config 문법 — 2026-07-16 확인):
  https://developers.openai.com/codex/hooks , https://learn.chatgpt.com/docs/hooks
