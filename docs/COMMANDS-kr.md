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
