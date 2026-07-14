<!--
출처: BATHOS 신규 제작 (BMAD 원천 없음 — BATHOS W4 Research Pack 고유 자산)
작성: Nathanael(BATHOS Research Writer, #6) · 2026-06-30
모듈: modules/research-pack · wave: W4 (플러그·비본류)
사용: abstract-introduction.md 워크플로우 5단계에서 이 템플릿을 기반으로 생성.
출력: .agent-team/06-research/introduction-kr.md
-->

# Introduction 템플릿 — BATHOS Research Pack

> **사용 지침:**
> - 이 파일은 **골격 템플릿**이다. `{{...}}`를 실제 내용으로 채운다.
> - `<!-- [Source: ...] -->` 주석은 evidence_trace용. 제출 전 제거.
> - `[CITE: ...]` 는 3단계 참고문헌 검색 결과로 채운다. 미확인 인용은 `[TODO: verify]`.
> - **핵심 규칙:** Introduction은 반드시 **기여 목록(§4)으로 종료**한다.
> - 완성된 Introduction은 `.agent-team/06-research/introduction-kr.md`에 저장.

---

## 채워야 할 메타 정보

```yaml
title: "{{논문 제목}}"
system_name: "{{시스템/방법 이름}}"
related_work_count: {{3~5}}
contributions_count: {{3~5}}
evidence_trace_verified: false  # 6단계 검증 후 true로 변경
```

---

## §1. Motivation (동기)

<!-- [Source: .agent-team/03-service-planning/*.md] -->
<!-- [Source: .agent-team/04-architecture/architecture-overview.md#배경] -->

{{이 문제가 왜 중요한가? 독자가 공감할 실제 고통 포인트(pain point)를 먼저 제시한다.}}

*작성 가이드:*
- 도메인의 성장·중요성을 구체적 추세로 시작
- "The challenge is..." 또는 "Despite advances in X, Y remains unsolved..." 패턴
- 학술적·실용적 양쪽 동기를 1~2 문단으로 서술
- 과도한 배경 설명 회피 (2~3 문단 이내)

---

## §2. Problem Statement (문제 정의)

<!-- [Source: .agent-team/04-architecture/design-patterns.md] -->
<!-- [Source: .agent-team/07-design/*.md] -->

{{해결하는 구체적 문제를 정의한다. 측정 가능하고 경계가 명확해야 한다.}}

**핵심 문제:**
{{문제를 한 문장으로 요약한다. 이 문장이 논문 전체를 관통하는 research question이 된다.}}

*작성 가이드:*
- "The core challenge is how to..." 또는 "We address the problem of..."
- 범위를 명확히: 이 논문이 다루는 것 + 다루지 않는 것
- 문제의 어려움(hardness)을 정당화: 왜 단순한 해결책이 통하지 않는가?

---

## §3. Limitations of Existing Work (기존 연구 한계)

> **주의:** 이 섹션의 모든 인용은 3단계(WebSearch)에서 검증된 것만 사용.
> 확인되지 않은 논문은 인용하지 않거나 `[TODO: verify — 저자, 연도, 제목]`로 명기.

{{관련 연구를 3~5개 범주로 분류하고, 각 접근법의 한계를 구체적으로 지적한다.}}

### 기존 접근법 A: {{범주 이름}}

[CITE: {{저자 등, 연도}}] proposes {{방법 요약}}. However, this approach
{{한계 1}} and {{한계 2}}.

<!-- [Source: 검색 확인된 논문 URL 또는 .agent-team/01-reverse/...] -->

### 기존 접근법 B: {{범주 이름}}

[CITE: {{저자 등, 연도}}] addresses {{무엇을}} by {{어떻게}}. While effective for
{{적용 범위}}, it fails to {{우리가 해결하는 한계}}.

### 기존 접근법 C: {{범주 이름}}

[CITE: {{저자 등, 연도}}] ...

### Gap Summary (갭 요약)

위 접근법들이 공통적으로 다루지 못하는 핵심 갭:
1. {{갭 1 — 우리가 기여 1로 해결}}
2. {{갭 2 — 우리가 기여 2로 해결}}
3. {{갭 3 — 우리가 기여 3으로 해결}}

*작성 가이드:*
- 각 관련 연구를 공정하게 평가 (과도한 비판 회피)
- "While [work X] is an important step, it does not address..."
- Gap Summary가 §4 기여 목록과 1:1 대응되어야 함

---

## §4. Our Approach and Contributions (접근법·기여) ⭐ 종료 섹션

<!-- [Source: .agent-team/04-architecture/design-patterns.md] -->
<!-- [Source: .agent-team/06-research/contributions-kr.md] -->

### 접근법 개요

{{우리 시스템/방법의 핵심 아이디어를 1~2 문단으로 서술한다.
기존 한계를 어떻게 극복하는지 직접 연결한다.}}

We present **{{system_name}}**, {{한 문장 설명}}. Unlike prior work that
{{기존 한계 요약}}, {{system_name}} {{핵심 차별점}}.

<!-- [Source: .agent-team/04-architecture/architecture-overview.md#핵심-설계] -->

### 기여 목록 (Contributions)

> ⭐ **이 목록이 Introduction의 마지막 내용이어야 한다.**

This paper makes the following contributions:

1. **{{기여 1 제목}}:**
   {{기여 1 설명 — 구체적이고 검증 가능한 진술. 설계 근거 추적.}}
   <!-- [Source: .agent-team/04-architecture/design-patterns.md#{{섹션}}] -->

2. **{{기여 2 제목}}:**
   {{기여 2 설명}}
   <!-- [Source: .agent-team/04-architecture/{{파일}}#{{섹션}}] -->

3. **{{기여 3 제목}}:**
   {{기여 3 설명}}
   <!-- [Source: .agent-team/07-design/{{파일}}#{{섹션}}] -->

<!-- 기여 4, 5가 있으면 동일 형식으로 추가 (최대 5개) -->

*작성 가이드:*
- 각 기여는 "We present / We propose / We demonstrate / We show..." 로 시작
- 기술적·방법론적·평가적 기여를 균형 있게 포함
- 각 기여가 §3의 갭 중 하나를 해결함을 암묵적으로 연결
- 과대 주장 회피: "We believe", "Our design targets" 등 적절한 hedging

---

## §5. Paper Organization (논문 구성) — 선택

> §5는 짧게 유지 (2~4문장). 논문 구성이 자명하면 생략 가능.

The remainder of this paper is organized as follows.
Section 2 reviews related work in detail.
Section 3 describes the {{system_name}} architecture.
Section 4 presents {{평가/구현 설명}}.
Section 5 discusses {{토론/한계}}.
Section 6 concludes.

---

## 완성 예시 — 기여 목록 부분 (BATHOS 기준)

```
This paper makes the following contributions:

1. **Policy-as-Code Hook Architecture:** We present a deterministic quality
   enforcement mechanism that encodes workflow policies as OS-level hooks,
   eliminating reliance on prompt-based guardrails. Our design targets
   zero-bypass gate enforcement.
   [Source: .agent-team/04-architecture/design-patterns.md#5]

2. **Context Compiler with Zero-Context-Loss:** We propose a story-compilation
   engine that aggregates upstream artifacts (architecture, design, prior story
   intelligence, web research) into a single self-contained developer story
   file, reducing context loss across agent handoffs.
   [Source: .agent-team/04-architecture/design-patterns.md#3]

3. **Adversarial Verification Loop:** We introduce a structured generation-
   verification separation pattern where independent reviewer agents (Thomas,
   Matthias) validate artifacts from fresh context, detecting errors invisible
   to the generating agent.
   [Source: .agent-team/04-architecture/design-patterns.md#4]
```

---

*BATHOS Research Pack Template · Nathanael(#6) · 2026-06-30*
