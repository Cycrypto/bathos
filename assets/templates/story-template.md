<!--
출처: .agent-team/01-reverse/bmad-localized-kr/templates/story-template.md
원본: src/bmm-skills/4-implementation/bmad-create-story/template.md
레포: bmad-code-org/BMAD-METHOD @ main (MIT © 2025 BMad Code, LLC)
현지화: John(BATHOS Reverse Specialist) · 2026-06-29
canonical 정렬: Paul(BATHOS Lead) · 2026-07-13 — `bathos story compile`(D1 완전성·D2 출처추적) 통과 포맷.
규약: frontmatter(`---`, story_key/status/source_hash) + 필수 6 snake_case 섹션 + 조건부 4 섹션.
     헤더는 snake_case 키 뒤에 한글 gloss 허용: `## story_requirements (스토리 요구사항)`.
     기술 주장에는 [Source: <경로>#<섹션>] 태그를 단다. developer_context는 절대 비우지 않는다.
-->
---
story_key: "{{epic}}-{{story}}-{{slug}}"
status: "ready-for-dev"
source_hash: "{{source_hash}}"
# 상태값: backlog → ready-for-dev → in-progress → in-review → done
---

# 스토리 {{epic}}.{{story}}: {{story_title}}

## story_requirements (스토리 요구사항)

As a {{role}}, I want {{action}}, so that {{benefit}}.

**인수조건(AC):**
- AC1: {{검증 가능한 수용 기준}} [Source: {{ref}}]
- AC2: …

## developer_context (개발자 컨텍스트)

<!-- 가장 중요. 구현자가 이 파일만 보고 착수하도록 배경·설계 요약을 자족적으로 채운다(비우면 E-CTX-LOSS). -->
{{설계 요약·핵심 결정·데이터 흐름·해서는 안 되는 것}} [Source: {{arch_ref}}]

## architecture_compliance (아키텍처 준수)

{{준수할 아키텍처 패턴·API 계약·데이터 스키마; 없으면 "해당 제약 없음" 명시}} [Source: {{arch_ref}}]

## library_framework_requirements (라이브러리·프레임워크 요구사항)

{{사용 라이브러리·런타임·버전(breaking/보안/deprecated); 최소면 "기존 스택/표준 라이브러리만"}} [Source: {{ref}}]

## file_structure_requirements (파일 구조 요구사항)

| 파일 | 작업(신규/수정/삭제) |
|------|----------------------|
| `{{path}}` | 신규 |

## testing_requirements (테스트 요구사항)

- 단위: {{대상 함수/모듈}}
- 통합/E2E: {{대상 플로우}} [Source: {{ref}}]

## project_context_reference (프로젝트 컨텍스트 참조)

USP→CF→US→SS 트레이스: {{trace}}. [Source: project-context-kr.md#0-1]

## previous_story_intelligence (선행 스토리 지능)

{{직전 스토리에서 확립된 패턴·피해야 할 함정; story=1이면 "선행 없음"}} [Source: {{ref}}]

## git_intelligence (git/저장소 지능)

{{최근 커밋 패턴·의존성 변화·제품 트리 규약(신규 vs 확장)}} [Source: project-context-kr.md#3]

## latest_tech_information (최신 기술 정보)

{{웹리서치 결과(breaking·보안·deprecated); 무관하면 그 취지 명시(Search Before Building)}} [Source: {{ref}}]

## dev_notes (개발 노트·리스크·우선순위)

- 리스크: 비차단 리스크는 `CONCERNS:` 앵커로 표기(→ `/bathos-debt` 수집).
- 우선순위·의존: {{Must/Should/Could · 선행 의존}}
- 구현 기록(Dev Agent Record)·File List는 구현 중 dev가 append.
