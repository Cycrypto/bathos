<!--
출처: BATHOS 신규 제작 (BMAD 원천 없음 — BATHOS W4 Research Pack 고유 자산)
작성: Nathanael(BATHOS Research Writer, #6) · 2026-06-30
모듈: modules/research-pack · wave: W4 (플러그·비본류)
사용: abstract-introduction.md 워크플로우 4단계에서 이 템플릿을 기반으로 생성.
출력: .agent-team/06-research/abstract-kr.md
-->

# Abstract 템플릿 — BATHOS Research Pack

> **사용 지침:**
> - 이 파일은 **골격 템플릿**이다. `{{...}}`를 실제 내용으로 채운다.
> - `<!-- [Source: ...] -->` 주석은 evidence_trace용. 제출 전 제거.
> - 완성된 Abstract는 `.agent-team/06-research/abstract-kr.md`에 저장.
> - 단어 수 목표: **150~250단어** (자족 — 다른 섹션 없이 독립 이해 가능).

---

## 채워야 할 메타 정보

```yaml
title: "{{논문 제목}}"
system_name: "{{시스템/방법 이름}}"
domain: "{{도메인 — 예: AI Workflow Orchestration, Multi-Agent Systems}}"
contributions_count: {{3~5}}
word_count: {{목표: 150~250}}
evidence_trace_verified: false  # 6단계 검증 후 true로 변경
```

---

## Abstract 본문 골격

<!-- 아래를 완성된 Abstract 산문으로 교체한다. 4단락 또는 연속 산문 모두 가능. -->

### [1] Problem — 문제 진술 (2~3문장)

{{다루는 문제를 서술한다. 이 문제가 중요한 이유를 1~2문장으로 정당화한다.}}

<!-- [Source: .agent-team/04-architecture/architecture-overview.md#배경] -->

*작성 가이드:*
- "X is increasingly important because Y" 패턴으로 시작
- 현재 시스템/방법의 구체적 한계 1가지 언급
- 기술 용어는 이 섹션에서 처음 소개

---

### [2] Approach — 접근법 (2~3문장)

{{우리가 제안하는 시스템/방법의 핵심 아이디어를 서술한다.
시스템 이름을 처음으로 소개한다: "We present {{system_name}}, a {{설명}}."}}

<!-- [Source: .agent-team/04-architecture/design-patterns.md#핵심-패턴] -->

*작성 가이드:*
- 핵심 메커니즘/방법론을 1~2문장으로 요약
- 기술적 세부보다 "무엇을 어떻게 다르게 했는가"에 집중
- 약어 첫 등장 시 정의 (예: "Large Language Model (LLM)")

---

### [3] Key Results / Contributions — 핵심 결과·기여 (2~4문장)

{{핵심 기여를 구체적으로 서술한다. 설계 기반 기대 효과는 수식어를 사용한다.}}

<!-- [Source: .agent-team/06-research/contributions-kr.md] -->

*작성 가이드:*
- 기여 3~5개 중 가장 중요한 2~3개를 응축
- 측정된 실험 결과가 있으면 구체적 수치 제시 가능
- 실험 미수행 시: "our design targets...", "the architecture is designed to...", "we expect..." 등
- **날조 수치 절대 금지** (미검증 수치를 사실로 제시하면 안 됨)

---

### [4] Implications — 함의·적용 (1~2문장)

{{이 연구의 더 넓은 의의와 향후 연구 방향을 서술한다.}}

<!-- [Source: .agent-team/03-service-planning/*.md] -->

*작성 가이드:*
- 학술적·실용적 시사점 1가지씩
- "We believe...", "This work opens the door to..." 등의 표현 사용 가능
- 과도한 과장 회피

---

## 완성 예시 (BATHOS 프로젝트 기준)

> 아래는 BATHOS 시스템 자체를 논문화할 경우의 예시 구조다. 실제 프로젝트에 맞게 대체한다.

```
[Problem] Modern AI-assisted software development increasingly relies on
multi-agent orchestration, yet existing frameworks suffer from context loss
across agent handoffs and lack deterministic quality gates.
[Approach] We present BATHOS (βάθος), a file-based stateful workflow engine
for multi-agent development teams, featuring a 7-wave pipeline, explicit
level routing (Lv0–4), and a context-compilation story engine that encodes
all upstream artifacts into self-contained developer stories.
[Contributions] BATHOS introduces (1) a Policy-as-Code hook architecture
that enforces quality gates deterministically at the OS level, (2) a
Context Compiler that achieves zero-context-loss through four defense
mechanisms, and (3) an Adversarial Verification pattern separating
generation and review roles. Our architecture is designed to eliminate the
seven classes of LLM implementation failures identified in prior work.
[Implications] BATHOS demonstrates that workflow correctness can be
guaranteed through file-state determinism rather than prompt engineering,
suggesting a path toward auditable, reproducible AI-native development.
```

---

*BATHOS Research Pack Template · Nathanael(#6) · 2026-06-30*
