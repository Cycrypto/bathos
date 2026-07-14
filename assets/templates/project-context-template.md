<!--
출처: .agent-team/01-reverse/bmad-localized-kr/templates/project-context-template.md
원본: src/bmm-skills/3-solutioning/bmad-generate-project-context/project-context-template.md
레포: bmad-code-org/BMAD-METHOD @ main (MIT © 2025 BMad Code, LLC)
현지화: John(BATHOS Reverse Specialist) · 2026-06-29
패키지 배치: Timothy(BATHOS Doc Specialist) · 2026-06-29
-->
---
project_name: '{{project_name}}'
user_name: '{{user_name}}'
date: '{{date}}'
sections_completed: ['technology_stack']
existing_patterns_found: {{number_of_patterns_discovered}}
---

# AI 에이전트를 위한 프로젝트 컨텍스트 (프로젝트 헌법)

_이 파일은 본 프로젝트에서 코드를 구현할 때 AI 에이전트가 반드시 따라야 할 핵심 규칙과 패턴을 담는다.
에이전트가 놓치기 쉬운 **비자명한(unobvious) 세부**에 집중하라._

_W5 모든 구현자(Phillip/Andrew/Stephen)와 W3 Story Engineer가 persistent_fact로 자동 로드한다._

---

## 기술 스택 & 버전 (Technology Stack & Versions)

_발견·확정 단계 후 문서화. 예시:_

```
- 언어: [예: Rust 1.78, TypeScript 5.3]
- 런타임: [예: Claude Code 단일 런타임]
- 코어 엔진: bathos 바이너리 (Rust, ADR-0006)
- 상태 저장소: [예: 파일 기반 JSON]
- 훅: bash (POSIX)
- 설정/스키마: JSON/YAML (serde)
```

---

## 핵심 구현 규칙 (Critical Implementation Rules)

_에이전트가 놓칠 수 있는 패턴·관례를 문서화._

### 코드 구조 & 명명 규칙

- [예: 모듈 경계, 파일명 규칙, 네임스페이스]

### 금지 사항 (Anti-Patterns)

- 상태 직접 편집 금지 → 반드시 `bathos-state` 크레이트 경유
- 역방향 의존 금지 (L4→L3→L2→L1 단방향만)
- [기타 프로젝트별 금지 사항]

### 보안·인증 규칙

- [예: 비밀 파일 경로 접근 제한, API 키 환경변수 필수]

### 성능·NFR 기준

- [예: gate-engine PASS/FAIL 판정 < 500ms]

### 테스트 표준

- [예: 각 크레이트 `#[cfg(test)]` 단위테스트 필수]
- [예: `cargo test` 그린 없이 PR 불가]

---

## 디렉터리 구조 & 파일 소유 경계

_팀원별 소유 경계 — 충돌 방지._

```
bathos/
├── .claude/agents/_base/   → Timothy 소유
├── assets/                 → Timothy 소유
├── core/                   → Phillip 소유 (Rust 워크스페이스)
├── .claude/hooks/          → Andrew 소유 (bash 훅)
├── .claude/commands/       → Andrew 소유 (슬래시 커맨드)
└── _state/                 → bathos-state 크레이트 경유 (직접 편집 금지)
```

---

## 알려진 패턴 & 관례

_발견된 기존 패턴 목록 ({{number_of_patterns_discovered}}개)._

1. [패턴명]: [설명] — [파일:라인 참조]
2. ...

---

<!--
BATHOS 보강:
- 이 헌법은 W2(아키텍처) 말미에 Story Engineer(#15)가 초안 작성 → Timothy가 확정.
- 루트의 `bathos/CLAUDE.md`(팀 운영규칙)·`bathos/ETHOS.md`(원칙)와 역할이 겹칩니다.
  → 본 'project-context-kr.md'는 **프로젝트별 기술 규칙**에 한정.
  팀 운영/원칙은 CLAUDE.md/ETHOS.md에 위임(중복 회피).
- 모든 기술 세부에 출처 [Source: path#section] 명기 필수.
-->
