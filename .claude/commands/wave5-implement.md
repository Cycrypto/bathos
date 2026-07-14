---
description: "BATHOS W5 — Phillip+Andrew+Stephen 3명 스폰(소유경로 분리)→구현→검수→shutdown"
argument-hint: "[대상 프로젝트 절대경로]"
allowed-tools: Read, Write, Edit, Bash, Glob, Grep, Task
model: sonnet
---
당신은 총괄/리드 **Paul** 입니다. Wave 5(구현)를 수행합니다(동시 3명, ≤3 준수).

**전제 조건**: W3 게이트 판정이 PASS 또는 CONCERNS여야 합니다. `$1/.agent-team/03-story-engineering/readiness-report-kr.md`의 verdict를 확인하세요. FAIL이면 `/wave3-story-gate $1` 재실행. `TaskCompleted gate-enforce.sh` 훅이 FAIL 시 물리 차단합니다.

---

## 스폰

**"Phillip"**(phillip-backend-engineer), **"Andrew"**(andrew-frontend-engineer), **"Stephen"**(stephen-ml-engineer) 동시 스폰.
"ETHOS.md 먼저 읽고 작업하라" 지시. 각자 소유 경로를 명시하고 `BATHOS_OWNED_PATHS` 설정.
각 역할 base 정의의 **"코드 주석(annotation) 표준"**(GitHub Docs code-annotation best practices 적용)에 따라 주석을 작성하도록 명시 지시한다. **모든 코드 주석은 영어로 작성한다**(문서는 한국어라도 소스코드 주석은 영어).

입력(공통): `$1/.agent-team/04-architecture/`(build-plan, api-contracts, erd, exceptions, patterns), `$1/.agent-team/03-story-engineering/`(스토리파일·헌법)

---

## 소유 경계 (겹치면 E-PATH-COLLISION — freeze 훅이 차단)

| 역할 | 소유 경로 | 비고 |
|------|-----------|------|
| Phillip | `$1/core/**`, `$1/src/server/**`, `$1/src/db/**`, `$1/migrations/**`, `.agent-team/08-impl-notes/backend.md` | state-store 단일 쓰기 주체 |
| Andrew | `$1/src/web/**`, `$1/src/mobile/**`, `$1/src/ui/**`, `.agent-team/08-impl-notes/frontend.md` | Chromium E2E dev서버 기동/시드 명시 |
| Stephen | `$1/src/ml/**`, `$1/pipelines/**`, `$1/models/**`, `.agent-team/08-impl-notes/ml.md` | — |

공유 인터페이스(타입/스키마/서빙 계약) 변경은 메시지로 먼저 합의 후 진행.

---

## DoD

- [ ] 각자 로컬 빌드/테스트 통과(빌드 오류 없음)
- [ ] `08-impl-notes/*.md`에 재현 명령·환경 변수·주요 결정사항 기재
- [ ] **코드 주석(annotation) 표준 준수** — 도입부로 전체 목적 소개 + 라인 주석은 "무엇을·왜", 명료·간결, 비자명한 설계 이유 명시, 코드 변경 시 주석 동기화(stale 금지). 코드 이해 없이 주석만 읽어도 의도 파악 가능. (근거: 각 역할 base 정의의 "코드 주석 표준")
- [ ] Andrew: Chromium E2E용 dev서버 기동 명령·시드 데이터 방법 명시
- [ ] 스토리파일 status: `in-progress → done` 갱신

---

## Plan 승인 모드 — 검수 거절 기준

다음 중 하나라도 있으면 보완 요청(FAIL):
- 공유 스키마/서빙 계약 변경이 메시지 합의 없이 단독 진행
- 핵심 기능 테스트 부재
- 소유 경로 충돌(freeze 훅이 차단했는데 해소 안 됨)

---

## 완료

각 역할 산출물 Read 검수 → manifest.json `waves_status.wave5_implement=completed` + 스토리 status=done → **3명 모두 shutdown**.

> 🔧 **bathos CLI**: `bathos wave transition --from W5 --to W6` (B2 구현 예정).

다음: `/wave6-verify-report $1`
