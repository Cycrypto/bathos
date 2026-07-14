<!--
출처: BATHOS 오리지널 자산 (BMAD 파생 아님)
원천 포맷 참조: KIPO 특허출원 명세서 표준 서식 / USPTO MPEP §608 / PCT Rule 5 (공개 공공 가이드)
작성: Mark(BATHOS IP Specialist, 25년차) · 2026-06-30
모듈: modules/ip-pack/templates (W4 플러그)
라이선스: MIT (BATHOS 패키지)
사용: workflows/patent-spec-draft.md 5단계가 본 템플릿을 채워 .agent-team/05-ip/patent-spec-draft-kr.md 생성.
-->

# 출원명세서 템플릿 (특허청 가이드 포맷)

> ⚠️ **법적 한계 고지:** 본 문서는 **변리사/변호사의 법적 자문이 아니라 출원 보조용 초안**입니다. 정식 선행기술 검색·등록 가능성 판단·청구항 확정은 **전문가(변리사) 검토가 필요**합니다.
> `{{...}}`는 patent-spec-draft 워크플로우가 채우는 플레이스홀더. 모든 기술 서술에 `[Source: <path>#section]` 부착(evidence_trace).

---

## 【발명의 명칭】

{{invention_title}}  <!-- 간결·기술적. 광고성 표현 금지 -->

## 【기술분야】

본 발명은 {{technical_field}}에 관한 것으로, 보다 상세하게는 {{detailed_field}}에 관한 것이다.
<!-- [Source: ...] -->

## 【배경기술】

{{background}}
<!-- 종래기술의 구성과 그 문제점. 선행기술 노트(prior-art-notes-kr.md) 활용. [Source: ...] -->

## 【발명의 요약】

### 【발명이 해결하려는 과제】

{{problem}}
<!-- 종래기술이 해결하지 못한 기술적 과제 -->

### 【과제의 해결 수단】

{{solution}}
<!-- 청구항의 핵심 구성을 요약 서술. 도면 부호 인용. [Source: ...] -->

### 【발명의 효과】

{{effects}}
<!-- 해결수단으로 달성되는 기술적 효과(진보성 논거와 정합) -->

## 【도면의 간단한 설명】

- 도 1은 {{fig1_desc}}이다.
- 도 2는 {{fig2_desc}}이다.
- 도 N은 {{figN_desc}}이다.
<!-- 도면 부호 매핑은 claims-map-kr.md 참조 -->

## 【발명을 실시하기 위한 구체적인 내용】

<!-- ★ enablement(실시가능성) 핵심 섹션. 통상의 기술자가 재현 가능하도록 구성·동작·데이터흐름을 구체 기술. 도면 부호 인용 필수. -->

### 실시예 1

{{embodiment_1}}
<!-- 시스템 구성(도면 부호 100, 110, 120 …) → 각 구성요소 동작 → 상호작용/데이터흐름. [Source: architecture-overview.md#..., data-model-erd.md#...] -->

### 실시예 2 (변형 실시형태)

{{embodiment_2}}
<!-- 대안적 구현·확장(종속항 뒷받침). [Source: ...] -->

### 동작 흐름 (방법 실시예)

{{method_flow}}
<!-- service-sequences 기반 단계별 동작. 방법 독립항 뒷받침. [Source: service-sequences.md#...] -->

## 【청구범위】

<!-- 독립항 ≥2(시스템/방법 [+매체]). 종속항 다수 계층화. 용어 일관(antecedent basis). 청구항↔근거는 claims-map-kr.md. -->

### 【청구항 1】 (독립 — 시스템/장치)

{{claim_1}}
... 를 포함하는 것을 특징으로 하는 {{system_name}}.
<!-- [Source: ...] -->

### 【청구항 2】 (종속, 청구항 1 인용)

제1항에 있어서,
{{claim_2_limitation}}
... 을 특징으로 하는 {{system_name}}.

### 【청구항 3】 (종속, 청구항 1 또는 2 인용)

{{claim_3}}

### 【청구항 N】 (독립 — 방법)

{{claim_method}}
... 하는 단계를 포함하는 것을 특징으로 하는 {{method_name}}.
<!-- [Source: service-sequences.md#...] -->

### 【청구항 N+1】 (독립 — 컴퓨터 판독가능 매체, 권장)

{{claim_crm}}
... 하도록 하는 프로그램을 기록한 컴퓨터로 판독 가능한 기록매체.

### 【청구항 N+2 …】 (방법/매체 종속항)

{{claim_dependents}}

## 【요약서】

### 【요약】

{{abstract}}
<!-- 발명의 핵심을 1문단으로. 권리범위 한정 아님. -->

### 【대표도】

도 {{representative_fig}}

---

## 【도면】

<!-- Mermaid 또는 텍스트 도면. 각 구성요소에 도면 부호 부여. -->

### 도 1 — 시스템 구성도

```mermaid
%% {{fig1_diagram}} — 구성요소에 도면 부호(100, 110 …) 표기
```

### 도 2 — 동작 흐름도

```mermaid
%% {{fig2_diagram}}
```

---

## 부속 산출물 링크 (evidence_trace)

- 발명 신고서: `.agent-team/05-ip/invention-disclosure-kr.md`
- 선행기술 노트(한계 명시): `.agent-team/05-ip/prior-art-notes-kr.md`
- 청구항 추적표: `.agent-team/05-ip/claims-map-kr.md` (청구항↔설계요소↔도면부호↔Source)
- 품질 점검: `modules/ip-pack/checklists/patent-quality.md`
