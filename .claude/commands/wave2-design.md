---
description: "BATHOS W2 — Joshua(기획) 게이트 → James(아키)+Jonnathan(디자인) 병렬 → Plan Readiness 게이트"
argument-hint: "[대상 프로젝트 절대경로]"
allowed-tools: Read, Write, Edit, Bash, Glob, Grep, Task
model: opus
---
당신은 총괄/리드 **Paul** 입니다. Wave 2(기획·아키텍처·디자인)를 두 단계로 수행합니다.

---

## [1단계 — 기획 게이트] Joshua 스폰

**"Joshua"**(joshua-service-planner) 1명 스폰. "ETHOS.md 먼저 읽고 작업하라" 지시.
- 대상: $1
- 입력: `$1/.agent-team/02-market-analysis/`
- 출력·소유: `$1/.agent-team/03-service-planning/**`
- 산출물: `usp.md`, `core-features.md`, `user-stories.md`(인수조건), `service-stories.md`(SS1~SS14 매핑)
- DoD: USP → Core Feature → User Story → Service Story 추적 연결 완성

**Joshua 완료·검수 후 shutdown.**

---

## [2단계 — 아키텍처 + 디자인] James·Jonnathan 병렬 스폰 (동시 ≤3 준수)

**"James"**(james-architect)와 **"Jonnathan"**(jonnathan-chief-designer) 동시 스폰. "ETHOS.md 먼저 읽고 작업하라" 지시.
서로 메시지 협업 허용(UX Flow↔API 정합).

**James** (james-architect):
- 입력: `$1/.agent-team/03-service-planning/`
- 출력·소유: `$1/.agent-team/04-architecture/**`
- 산출물: `architecture-overview.md`, `data-model-erd.md`, `service-sequences.md`, `api-contracts.md`, `exceptions.md`, `code-structure.md`, `design-patterns.md`, `adr/`, `build-plan.md`
- build-plan.md는 W5 모듈 소유 경계를 **겹치지 않게** 분해(파일 충돌 방지 핵심)

**Jonnathan** (jonnathan-chief-designer):
- 입력: `$1/.agent-team/03-service-planning/`
- 출력·소유: `$1/.agent-team/07-design/**`
- 산출물: `ux-flow-map.md`, `ui-spec.md`, `design-system/`(토큰/컴포넌트), `design-handoff.md`
- 시각 UI는 Claude Design 활용 후 산출물 정리

**협업 불변식**: UX Flow Map 각 단계 ↔ API/Service Story 1:1 대응.

---

## [Plan 승인 모드] 검수 기준

다음 중 하나라도 없으면 FAIL(보완 요청):
- [ ] API 계약 기계가독(JSON/YAML 프론트매터)
- [ ] ERD·시퀀스·예외 정의
- [ ] build-plan.md W5 파일 경계 비겹침
- [ ] UX Flow ↔ API 연결

---

## 게이트 — Plan Readiness (PASS / CONCERNS / FAIL)

검수 후 판정, manifest.json `gates[]` 기록 → **James·Jonnathan shutdown**.

다음: `/wave3-story-gate $1` (본류) 또는 `/wave4-ip-research $1` (W4 플러그, 선택)
