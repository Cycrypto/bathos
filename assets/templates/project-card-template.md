# PROJECT CARD — {{codename}}

> BATHOS 전역 메모리 카드 · 위치: `~/.bathos/registry/{{slug}}.md`
> 목적: **다른 프로젝트/세션에서** 이 프로젝트의 컨텍스트를 깊이 파악해 cold start를 용이하게.
> 갱신: {{timestamp}} · 원천 프로젝트 경로: `{{project_path}}`

## 식별
- **project_id:** {{project_id}} · **codename:** {{codename}}
- **경로:** `{{project_path}}` (`.agent-team/`이 이 아래)
- **현재 레벨(Lv):** {{current_level}} · **상태:** {{status}} (active|paused|done)
- **도메인/스택:** {{domain}} / {{stack}}

## 한 줄 요약
{{one_line}}

## USP & 제품 정의 (재사용 가능한 프레이밍)
{{usp_and_features}}

## 아키텍처 결정 (재사용 가능한 결정·ADR 하이라이트)
- 핵심 결정: {{key_decisions}}
- 데이터/API 패턴: {{data_api_patterns}}
- 관련 ADR: {{adr_pointers}}

## 디자인 (재사용 가능한 시스템 하이라이트)
- 디자인 토큰/시스템: {{design_tokens}}
- UX 패턴/매직모먼트: {{ux_patterns}}

## 재사용 패턴 (다른 프로젝트에 적용 가능한 것)
{{reusable_patterns}}

## 교훈 & 인시던트 (반복하지 말 것)
{{lessons_and_incidents}}

## 현재 상태 & 다음 단계
{{status_and_next}}

## 포인터 (깊이 파고들 위치)
- 기획: `{{project_path}}/.agent-team/03-service-planning/`
- 아키텍처: `{{project_path}}/.agent-team/04-architecture/`
- 디자인: `{{project_path}}/.agent-team/07-design/`
- 상태/사인오프: `{{project_path}}/.agent-team/_state/{manifest.json, signoff.md, SESSION-SNAPSHOT.md}`
