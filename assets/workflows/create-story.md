<!--
출처: .agent-team/01-reverse/bmad-localized-kr/workflows/create-story.md
원본: src/bmm-skills/4-implementation/bmad-create-story/SKILL.md
레포: bmad-code-org/BMAD-METHOD @ main (MIT © 2025 BMad Code, LLC)
현지화: John(BATHOS Reverse Specialist) · 2026-06-29
패키지 배치: Timothy(BATHOS Doc Specialist) · 2026-06-29
-->

# 워크플로우: Create Story (스토리 엔진) — BATHOS W3

**목표:** W3 Story Engineer(#15)가 W2 산출(기획·아키텍처·UX)을 **구현자가 그 파일만 보고 착수할 수 있는 자족 스토리파일**로 응축한다.

**역할 정의:** 당신은 **LLM 개발자의 실수·누락·재앙을 막는 스토리 컨텍스트 엔진**이다.

---

## 핵심 원칙

- 목적은 에픽을 **복사**하는 것이 **아니라**, dev 에이전트에게 결함 없는 구현에 필요한 **모든 것**을 주는 최적화된 스토리 파일을 **창조**하는 것.
- **막아야 할 LLM의 전형적 실수:** 바퀴 재발명, 잘못된 라이브러리, 잘못된 파일 위치, 회귀 유발, UX 무시, 모호한 구현, 완료 거짓말, 과거 학습 무시.
- **철저한 분석 필수:** 모든 아티팩트를 빠짐없이 분석한다(스킵·대충 금지). 이것은 전체 개발 과정에서 **가장 중요한 함수**다.

---

## 입력 파일 (SELECTIVE_LOAD)

| 입력 | 경로 패턴 | 비고 |
|------|-----------|------|
| epics | `.agent-team/03-service-planning/*epic*.md` | 주 입력 |
| prd | `.agent-team/03-service-planning/*prd*.md` | 폴백 |
| architecture | `.agent-team/04-architecture/*.md` | 가드레일 |
| ux | `.agent-team/07-design/*.md` | UX 정렬 |
| project-context | 확정 `project-context-kr.md` | 헌법(persistent_fact) |

---

## 절차 (6단계)

### 1단계 — 대상 스토리 결정

- 사용자가 `1-2-user-auth` 형식으로 지정 → `epic_num`/`story_num`/`story_key` 파싱.
- 미지정 시 `_state/manifest.json`을 처음부터 끝까지 읽고 `development_status`에서 **첫 `backlog` 스토리** 선택.
- 키 매칭은 **앞 두 세그먼트 정확 일치** (`1-1`이 `1-10`과 충돌 금지).
- 해당 스토리가 에픽의 **첫 스토리**면 에픽 상태를 `in-progress`로 갱신.
- 완료된 에픽엔 생성 거부.

### 2단계 — 핵심 아티팩트 분석 (병렬)

- 에픽에서: 에픽 목표·비즈니스 가치·전체 스토리(교차 컨텍스트)·우리 스토리의 요구사항/AC/제약/의존성 추출.
- 우리 스토리 기반: User Story(As a/I want/so that), BDD 형식 AC, 기술 요구사항, 성공 기준.
- **이전 스토리 인텔리전스** (`story_num > 1`): 직전 스토리의 Dev Notes·리뷰 피드백·생성/수정 파일·테스트 접근·확립된 코드 패턴 추출(D4 연속성).
- **git 인텔리전스**: 최근 5커밋의 파일/패턴/의존성/아키텍처 결정/테스트 접근 분석.

### 3단계 — 아키텍처 가드레일 추출

- 스택·버전, 코드 구조·명명, API 패턴·데이터 계약, DB 스키마, 보안·성능·테스트 표준, 배포·통합 패턴 중 **우리 스토리에 관련된 것**을 추출.
- **🚨 UPDATE(신규 아님)로 표시된 기존 파일을 전부 정독**: "현재 동작 / 이 스토리가 바꾸는 것 / 보존해야 할 것"을 Dev Notes에 기록.
  > 이 단계 스킵이 구현 실패·재리뷰의 주원인.
- 핵심 원칙: 스토리 구현은 **시스템을 end-to-end로 동작 상태로 남겨야** 한다.

### 4단계 — 최신 기술 웹 리서치

- 사용 라이브러리/프레임워크/API의 **최신 안정버전·breaking change·보안 패치·deprecated·모범사례** 조사 → 스토리에 포함.

### 5단계 — 자족 스토리파일 생성 (`bathos/assets/templates/story-template.md` 기반)

9섹션 생성:
1. `story_header` — 기본 정보
2. `story_requirements` — User Story + BDD AC
3. **`developer_context`** ← **가장 중요**: 무엇을/왜/제약/금지
4. `architecture_compliance` — [Source:path#section] 출처 필수
5. `library_framework_requirements` — 버전·breaking 포함
6. `file_structure_requirements` — 신규/수정 파일 목록
7. `testing_requirements` — 단위/통합/E2E
8. (조건부) `previous_story_intelligence` · `git_intelligence` · `latest_tech_information`
9. `project_context_reference`

**Zero-Context-Loss 4중 방어:**
- D1 완전성: 9섹션 필수 포함
- D2 출처추적: 모든 기술 세부에 `[Source:<path>#section]`
- D3 신선도: `source_hash`로 상류 변경 감지 → stale 시 재컴파일
- D4 연속성: 이전 스토리 인텔리전스 주입

상태를 **`ready-for-dev`** 로 설정.

### 6단계 — 적대적 자가검증 + 상태 갱신

- `bathos/assets/checklists/story-context-quality.md`로 스토리 자가 검증 후 필요한 수정 적용(FIX & PREVENT).
- `_state/manifest.json`의 `development_status[story_key]`를 `backlog` → `ready-for-dev`로 갱신.
- 변경 이력은 `_state/`에 **투명 기록**(감사성, 사용자 주권).

---

## BATHOS W3 매핑

- **Story Engineer(#15)**가 이 워크플로우를 수행.
- "이전 스토리 인텔리전스"는 `.agent-team/08-impl-notes/`의 디스크 산출물 인계 규약으로 구현.
- 게이트(W3 PASS/CONCERNS/FAIL) 통과 후에만 W5 진입(물리 차단: `gate-enforce.sh`).
