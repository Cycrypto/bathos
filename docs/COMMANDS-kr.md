# BATHOS 슬래시 커맨드 모음 (`/` 명령어 레퍼런스)

> **BATHOS** — βάθος(그리스어 '깊이·심연'). 17역할 · 7웨이브 AI Workflow Agent.
> 이 문서는 `.claude/commands/`의 모든 슬래시 커맨드를 카테고리별로 정리한 단일 인덱스입니다.
> 커맨드 정의(정본)는 각 `.claude/commands/<name>.md`. 운영 규칙은 `CLAUDE.md`, 역할·흐름은 `AGENTS.md`.
> 총 **35개** 커맨드(웨이브 9 · 세션/메모리 8 · plan 리뷰 5 · 안전 2 · gstack 운영 5 · 원격/기타 2 · 라우팅/현황 3 · 리포트 1).

---

## 1. 웨이브 파이프라인 (BATHOS 본류)

> 본류 의존성: **W0 → W1 → W2 → W3 → W5 → W6** (W4는 선택·비본류 플러그). 게이트 용어: **PASS / CONCERNS / FAIL**.

| 커맨드 | 웨이브 | 설명 | 인자 |
|--------|--------|------|------|
| `/team-kickoff` | (사전) | 리드 단독 — `.agent-team` 골격 + charter + manifest 초기화 | `[프로젝트경로] [컨셉 한 줄]` |
| `/wave0-analysis` | W0 | Analysis(선택·전단) — Caleb이 brainstorm→forge→brief → Brief Readiness 게이트 | `[프로젝트경로]` |
| `/wave1-discovery` | W1 | John(리버스)+Caleb(시장분석) 동시 스폰 → USP Readiness 게이트 | `[프로젝트경로] [레포경로/URL]` |
| `/wave2-design` | W2 | Joshua(기획) 게이트 → James(아키)+Jonnathan(디자인) 병렬 → Plan Readiness 게이트 | `[프로젝트경로]` |
| `/wave3-story-gate` | W3 ★ | Story Engineering & Readiness Gate(심장) — Matthew(#17) 자족 스토리파일 + Thomas·Matthias 독립리뷰 → 이중 게이트 | `[프로젝트경로]` |
| `/wave4-ip-research` | W4 | (플러그) Mark(특허)+Nathanael(논문) — W2 이후 언제든 실행 | `[프로젝트경로]` |
| `/wave5-implement` | W5 | Phillip+Andrew+Stephen 3명 스폰(소유경로 분리)→구현→검수 | `[프로젝트경로]` |
| `/wave6-verify-report` | W6 | Thomas+Timothy+Matthias 검증·문서화 → Michael(보안) → Hananiah(리팩토링) → Martin(리포트) → Release Readiness | `[프로젝트경로]` |
| `/team-confirm` | (사후) | 리드 최종 confirm + signoff + 팀 cleanup | `[프로젝트경로]` |

## 2. 라우팅 · 현황 · 정리

| 커맨드 | 설명 | 인자 |
|--------|------|------|
| `/route` | Scale-Adaptive 라우팅 — 작업 stakes 입력 → Lv0~4 추천 + wave-set + User 확정 | `[프로젝트경로] [--level=0..4]` |
| `/team-status` | 진행 현황 점검 — manifest/wave-log/산출물 인벤토리·다음 커맨드 안내 | `[프로젝트경로]` |
| `/team-cleanup` | 긴급·수동 정리 — 잔여 팀원 shutdown + 팀 cleanup | — |

## 3. 세션 저장 / 복원 (무손실 인계)

> 정본 2개(`/save-session`·`/cold-start`) + 짧은 별칭 + gstack 레거시 별칭. 모두 동일 `_state`·동일 SSOT.

| 커맨드 | 설명 | 인자 |
|--------|------|------|
| `/save-session` | **(정본)** 세션 전체 저장 — 서술 스냅샷 + 머신 SSOT 덤프 완전 보존 | `[프로젝트경로]` |
| `/save` | 별칭 — `/save-session`과 동일 | `[프로젝트경로]` |
| `/cold-start` | **(정본)** 콜드스타트 — 문맥 0에서 이전 세션까지 100% 복원·브리핑(읽기 전용) | `[프로젝트경로]` |
| `/resume` | 별칭 — `/cold-start`와 동일 | `[프로젝트경로]` |
| `/context-save` | [gstack 레거시] 작업 컨텍스트 저장(git 상태/결정/남은 작업) | `[프로젝트경로]` |
| `/context-restore` | [gstack 레거시] 저장된 컨텍스트에서 재개 | `[프로젝트경로]` |

**자연어 트리거:** "저장/세이브/체크포인트" → `/save-session` · "이어서/재개/콜드스타트/continue" → `/cold-start`.

## 4. 크로스-프로젝트 메모리 (`~/.bathos/registry`)

| 커맨드 | 설명 | 인자 |
|--------|------|------|
| `/project-handoff` | 현 프로젝트를 전역 레지스트리에 카드로 증류·upsert(다른 프로젝트가 `/recall`로 참조). `/save-session`이 자동 수행 | `[프로젝트경로]` |
| `/recall` | 전역 레지스트리에서 관련 이전 프로젝트 컨텍스트를 끌어와 warm-start(읽기 전용) | `[관심 주제/기술/도메인]` |

## 5. 작업 리포트

| 커맨드 | 설명 | 인자 |
|--------|------|------|
| `/taskreport` | 즉시 작업 리포트 생성 — `YYYY-MM-DD-taskreport-N.html`를 `.agent-team/12-report/`에 저장(온디맨드). `/exit` 시엔 SessionEnd 훅이 자동 생성 | `[프로젝트경로]` |

> 참고: 세션별 정식 리포트(`result_report/task_report_<날짜>_<시각>_session_no<NN>.html`, session_no 자동 증가, 필수 6항목)는 `bash result_report/generate-task-report.sh`로 생성. 과거 세션 소급 백필은 `result_report/backfill-from-archive.sh`.

## 6. plan-mode 리뷰 게이트 (gstack · Wave 2 보강)

| 커맨드 | 설명 | 인자 |
|--------|------|------|
| `/autoplan` | CEO→디자인→엔지니어링→DX 플랜 리뷰를 순차 실행 | `[프로젝트경로]` |
| `/plan-ceo-review` | CEO/창업자 모드 — '10점짜리 제품'을 찾고 전제를 도전 | `[프로젝트경로]` |
| `/plan-design-review` | 디자이너의 눈 — Jonnathan이 디자인 품질 루브릭 11차원 0~10 적대적 자기검증 | `[프로젝트경로]` |
| `/plan-eng-review` | 엔지니어링 매니저 — James가 아키텍처/엣지케이스/테스트/성능 락인 | `[프로젝트경로]` |
| `/plan-devex-review` | DX/UX — 페르소나·매직 모먼트·마찰점·TTHW 점검 | `[프로젝트경로]` |

## 7. 안전 · 스코핑 (gstack)

| 커맨드 | 설명 | 인자 |
|--------|------|------|
| `/guard` | careful(파괴적 명령 차단) + freeze(편집 범위 잠금) 활성 | `[허용 편집 경로…]` |
| `/unfreeze` | freeze 해제 — 사용자 명시 요청 시만 | — |

> careful/freeze는 `PreToolUse` 훅(`careful-guard.sh`·`freeze-guard.sh`)이 하드 강제. 위험경로 변경은 `bathos fingerprint approve`로 정식 승인.

## 8. 구현 · 리뷰 · 운영 (gstack)

| 커맨드 | 설명 | 인자 |
|--------|------|------|
| `/review` | 랜딩 전 PR 리뷰 — Thomas가 'CI 통과·prod에서 깨지는' 버그를 찾는다 | `[프로젝트경로]` |
| `/investigate` | 체계적 근본원인 디버깅 — '조사 없이는 수정 없다' | `[프로젝트경로] [증상/버그]` |
| `/cso` | 보안 감사 — OWASP Top 10 + STRIDE 위협 모델링 | `[프로젝트경로]` |
| `/retro` | 회고 — 이번 사이클 산출물/지표/배운 점/다음 액션 | `[프로젝트경로]` |
| `/health` | 코드 품질 대시보드 — 타입체커/린터/테스트/데드코드 요약 | `[프로젝트경로]` |

## 9. 원격 · 교육 · 기타

| 커맨드 | 설명 | 인자 |
|--------|------|------|
| `/remote-dev` | **(신규)** 원격 개발(Remote Control) 셋업·가이드 — 내 머신 세션을 폰/웹에서 조종. BATHOS 상태·안전훅 유지하는 원격 근무 정본 | `[프로젝트경로]` |
| `/lecture` | David(Engineering Tutor) 스폰 — 가이드·코드·웹 소개 기반 주니어용 강의안(MD+HTML) 생성 | `[프로젝트경로]` |

---

## 10. `bathos` CLI 서브커맨드 — 역할별 모델(A) + Wave 패널(B)

> 위 1~9는 **슬래시 커맨드**(`.claude/commands/*.md`, Paul이 대화로 실행)이고 여기부터는
> **`bathos` 엔진 바이너리**(`core/target/release/bathos`)를 직접 호출하는 CLI 서브커맨드다
> (총 35개 슬래시 커맨드 집계에는 포함되지 않음). 설계 원본:
> `.agent-team/04-architecture/w2-panes-model-design-kr.md`(James) — 구현: Phillip.
> 각 웨이브 커맨드(`/wave0-analysis` ~ `/wave6-verify-report`)는 스폰 직전에 "## 모델 선택"
> 절차로 `model detect/show/set/validate`를 호출하고, 스폰 완료 직후 `bathos panes` 관측을
> 안내한다(§A2.3·§B4.3 정본 텍스트, 7개 웨이브 커맨드 모두 삽입 완료).

### 10.1 `bathos model` — 역할별 모델/런타임 선택 (ADR-D-0005/0006)

`_state/model-plan.json`(schema `bathos/model-plan@1`)을 SSOT로 삼아 역할별 `runtime`
(`claude`/`glm`/`codex`)·`model`을 배정한다. **plan 파일이 없거나 그 역할이 미등재면 항상
agent frontmatter `model:`로 폴백** — 이 기능을 한 번도 쓰지 않은 프로젝트는 이전과 100%
동일하게 동작한다(하위호환이 최우선 제약).

```
bathos model show    [--wave W5] [--json]                 # 역할별 유효값 표 (source: plan|default|frontmatter|runtime-default)
bathos model set     <slug> --runtime <r> [--model <m>] [--reasoning-effort <e>]   # plan에 기록(변경분만)
bathos model unset   <slug>                                # plan에서 제거 → frontmatter 폴백
bathos model detect                                        # session_backend 판별·기록(env ANTHROPIC_BASE_URL 검사)
bathos model validate [--wave W5]                          # GLM/Codex 혼합 배치 규칙 검증. PASS=exit 0 / 위반=exit 2 + 해소 선택지
bathos model resolve  <slug> [--json]                      # 한 역할의 유효 runtime/model/적용채널 1줄 출력(스크립트 소비용)
```

- **claude**: 팀원 스폰 파라미터로 역할별 상이 모델 지정 가능(fable5/sonnet5/haiku, 제약 없음).
- **glm**: `ANTHROPIC_BASE_URL`이 프로세스 전역이라 **역할 단위 지정 불가** — 웨이브·세션 단위만.
  같은 배치에 `claude`+`glm`이 섞이면 `validate`가 `E-MODEL-MIX`(exit 2)로 차단하고 해소
  선택지 3종(배치 전체 GLM/역할 이동/순차 분할)을 제시한다. 상세·제약의 물리적 근거는
  `docs/glm-backend-kr.md` §"모델/런타임 혼합 제약(ADR-D-0005)" 참고.
- **codex**: 별도 CLI 프로세스라 Claude 팀원과 병행 무충돌(GLM과의 비대칭) — 팀원으로
  스폰하지 않고 `codex-adapter/run-role.sh <slug> <task-file>`로 위임한다.
- 오류 코드: `E-MODEL-ROLE-UNKNOWN`·`E-MODEL-RUNTIME-INVALID`·`E-MODEL-MIX`·
  `E-MODEL-BACKEND-MISMATCH`·`E-MODEL-PLAN-CORRUPT`(파손 시 `.bak` 백업 후 재생성 안내).

### 10.2 `bathos inspect vm` — Wave 패널 공유 데이터원 (ADR-D-0007)

4개 프론트엔드(HTML report·live serve·tmux 패널·bathos TUI)가 공유하는 단일 데이터원
`DashboardVM`을 노출한다. 새 상태 계층을 발명하지 않고 기존 `build_dashboard_vm_json`을
재사용한다(Search Before Building).

```
bathos inspect vm [--path <.agent-team>] [--wave W5] [--format json|lines]
```

- `--format json`(기본) = `bathos inspect report --json`과 바이트 동일(기존 하위호환 유지).
- `--format lines` = bash 3.2/jq-금지 환경용 TSV 라인 프로토콜(`proto`/`meta`/`wave`/`gate`/
  `role`/`story`/`artifact`/`audit`/`chain` 레코드, 필드는 항상 뒤에만 추가). `scripts/bathos-panes.sh`가
  이 출력을 `grep`/`cut`/`read`로만 소비한다(jq 불요).
- `--wave W5`로 해당 웨이브 관련 레코드(웨이브 셀+게이트+역할/산출물/스토리)만 필터.

### 10.3 `bathos panes` — Wave 패널 프론트엔드 선택 (ADR-D-0008/0009)

```
bathos panes [--mode tui|tmux|dump] [--path <.agent-team>] [--wave W5] [--interval 2]
```

- `--mode tui`(TTY 기본) = 내장 `bathos-tui` 크레이트(ratatui+crossterm) 대화식 패널.
  탭 구성 = `Paul | W0…W6`(라우팅된 웨이브만), 키맵 `←/→·Tab`=탭 전환·`j/k`=스크롤·
  `c`=confirm 모달·`f`=feedback 모달·`r`=새로고침·`q`=종료.
- `--mode tmux` = `scripts/bathos-panes.sh up`으로 위임 — tmux 3.7 분할 패널(제어/상태/입력).
  Windows 네이티브는 tmux 부재로 미지원(WSL 또는 `--mode tui` 권장).
- `--mode dump` = 비대화 1프레임 렌더 스냅샷(테스트/CI/파이프 소비용).
- 모드 미지정 + TTY: 대화형으로 tmux/TUI 중 선택받고 `_state/panes/prefs`에 기억(다음 기본값).
- **패널의 confirm/feedback은 "제안 채널"**이다 — `_state/panes/inbox/`에 파일로 쌓이고
  웨이브 흐름이 게이트 체크포인트에서 소비한다. 게이트 판정 정본 기록은 여전히
  `bathos gate`(FACILITATOR 원칙, 근거 없는 자동 PASS 금지)이며 패널이 대신 기록하지 않는다.
- 자동 기동 안 함 — Paul(Claude Code) 자신은 이미 자기 TTY를 점유 중이므로, 패널은
  **사람이 여는 두 번째 터미널**의 관측/입력 도구다(각 웨이브 커맨드 말미의 힌트 1줄 참고).

---

## 권장 흐름

```
/team-kickoff → /route(레벨 확정) → /wave0-analysis(선택) → /wave1-discovery
→ /wave2-design [→ /autoplan 로 락인] → /wave3-story-gate(PASS/CONCERNS면 진입)
→ /wave4-ip-research(선택) → /wave5-implement → /wave6-verify-report → /team-confirm
```
- 위험 작업 전 `/guard`. 규모에 따라 scale-adaptive로 일부 웨이브 생략(Lv0~4).
- 원격 근무: `/remote-dev`로 셋업 → 자리 뜰 때 `/save`(저장) → 복귀·타 기기에서 `/resume`(이어서).
- 커맨드 정합 메모: `/wave4-ip-research`·`/wave5-implement`·`/wave6-verify-report`는 각각 웨이브 W4·W5·W6에 매핑(파일명은 구 번호 흔적 유지).

## 원격 실행 옵션 요약 (BATHOS 관점)

| 옵션 | 실행 위치 | `_state`·바이너리·안전훅 | BATHOS 파이프라인 |
|------|-----------|--------------------------|-------------------|
| 로컬 | 내 머신 | ✅ 유지 | ✅ 권장 |
| **Remote Control** (`/remote-dev`) | 내 머신(폰/웹은 조종창) | ✅ 유지 | ✅ **원격 정본** |
| SSH + tmux + `/save`·`/resume` | 내 머신 | ✅ 유지 | ✅ 권장 |
| 클라우드(Web/Routines/`/schedule`) | Anthropic 클라우드(fresh clone) | ✗ 부재 | ⚠️ BATHOS 무관 단발 작업만 |

> ⚠️ `.agent-team/`·`_state/`는 gitignore, 안전훅은 `bathos` 바이너리 의존 → **클라우드형은 BATHOS 상태·감사·게이트가 없음**. 파이프라인 작업은 로컬/Remote Control로.
