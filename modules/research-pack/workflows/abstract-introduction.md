<!--
출처: BATHOS 신규 제작 (BMAD 원천 없음 — BATHOS W4 Research Pack 고유 자산)
작성: Nathanael(BATHOS Research Writer, #6) · 2026-06-30
모듈: modules/research-pack · wave: W4 (플러그·비본류)
소유 경로: bathos/modules/research-pack/**
-->

# 워크플로우: Abstract & Introduction 생성 (W4 Research Pack) — BATHOS

**목표:** W2 산출물(아키텍처·디자인·서비스기획)과 W3 스토리 산출물에서 학문적 기여를 도출하여
최고 수준 학술 **Abstract**와 **Introduction**을 생성한다.
핵심 원칙: **모든 주장은 산출물 근거를 추적(evidence_trace)**하며, 미검증 인용·수치 날조는 절대 금지.

**역할 정의:** 당신은 **Nathanael(#6) — 20년 이상 경력의 연구 저술 전문가**다.
복잡한 시스템을 명료한 학술 서사로 변환하고, 기여를 설득력 있게 프레이밍한다.

---

## 사전 조건

| 조건 | 체크 |
|------|------|
| `module.yaml`의 `trigger` 조건(`Lv>=3 OR domain=research`) 충족 | 필수 |
| W2 산출물(`04-architecture/`, `07-design/`, `03-service-planning/`) 존재 | 필수 |
| W3 스토리 산출물(`03-story-engineering/`) 존재 (없으면 W2만으로 진행) | 권장 |
| `05-ip/invention-disclosure.md` 존재 (IP팩 병행 시) | 선택 |

---

## 입력 아티팩트 (SELECTIVE_LOAD)

| 입력 | 경로 패턴 | 우선도 |
|------|-----------|--------|
| 아키텍처 개요 | `.agent-team/04-architecture/architecture-overview.md` | **P0** |
| 디자인 패턴 | `.agent-team/04-architecture/design-patterns.md` | **P0** |
| 서비스 기획 | `.agent-team/03-service-planning/*.md` | P1 |
| UX 플로우 | `.agent-team/07-design/*.md` | P1 |
| 발명 공개 | `.agent-team/05-ip/invention-disclosure.md` | P1(있을 때) |
| 스토리 파일 | `.agent-team/03-story-engineering/project-context-kr.md` | P2 |
| API 계약 | `.agent-team/04-architecture/api-contracts.md` | P2 |
| 빌드 계획 | `.agent-team/04-architecture/build-plan.md` | P2 |

---

## 출력 아티팩트

| 파일 | 경로 | 설명 |
|------|------|------|
| `abstract-kr.md` | `.agent-team/06-research/abstract-kr.md` | 자족 Abstract (한국어, 150~250단어) |
| `introduction-kr.md` | `.agent-team/06-research/introduction-kr.md` | Introduction (한국어, 기여 목록으로 종료) |
| `contributions-kr.md` | `.agent-team/06-research/contributions-kr.md` | 기여 목록 (novelty 3~5개 + 근거 추적) |
| `references.bib` | `.agent-team/06-research/references.bib` | BibTeX 참고문헌 (미검증 인용은 TODO 명기) |

> **언어 정책:** 기본 출력 한국어(`-kr.md`). 리드(Paul)가 영문 병행을 요청하면 동일 구조의 `-en.md`를 추가 생성.

---

## 절차 (6단계)

### 1단계 — 입력 아티팩트 발견 및 로드

1. 위 입력 테이블 경로를 **우선도(P0→P1→P2)** 순으로 로드.
2. **P0가 하나라도 없으면 즉시 중단** → 리드(Paul)에 보고(사용 가능 아티팩트 목록 포함).
3. 각 아티팩트의 핵심 섹션을 추출해 **내부 작업 메모**에 정리:
   - 시스템 목적·범위
   - 핵심 구성요소 및 설계 결정(ADR)
   - 제약·품질 속성(성능·보안·확장성)
   - 차별화된 패턴·메커니즘

> **추적 메모:** 이 단계에서 로드한 모든 파일과 섹션을 기록. 이후 `[Source:<path>#Section]` 인용에 사용.

---

### 2단계 — 핵심 기여 추출 (Novelty 도출)

**3~5개의 기여**를 다음 기준으로 식별한다:

| 기여 유형 | 설명 | 예시 질문 |
|-----------|------|-----------|
| **기술적 신규성** | 기존과 다른 설계·알고리즘·아키텍처 | "이 패턴은 왜 새로운가?" |
| **문제 해결** | 기존 한계를 극복하는 구체적 방법 | "어떤 고통을 어떻게 없앴나?" |
| **시스템 통합** | 여러 구성요소의 조합이 만드는 시너지 | "부품의 합 이상의 가치가 있는가?" |
| **검증/평가** | 정량·정성 평가 방법론 | "어떻게 효과를 측정하나?" |
| **일반화** | 다른 도메인 적용 가능성 | "이 접근법은 얼마나 범용적인가?" |

**각 기여 항목 형식:**
```markdown
## 기여 N: {{기여 제목}}
- **주장:** {{핵심 주장 한 문장}}
- **근거:** [Source: {{아티팩트 경로}}#{{섹션}}]
- **기존 한계 대비:** {{어떤 한계를 극복하는가}}
- **예상 효과:** {{설계상 기대 효과 — 측정된 수치가 아님을 명시}}
```

> **🚫 수치 날조 금지:** 실험으로 검증되지 않은 수치를 사실로 제시하지 않는다.
> 설계상 기대 효과는 반드시 "예상", "설계 목표", "이론적으로" 등의 수식어와 함께 제시.

---

### 3단계 — 관련 연구 위치 선정 (WebSearch)

ETHOS §2(Search Before Building)에 따라 **반드시 WebSearch를 수행한다**.

**검색 전략:**
1. **기여별 핵심 키워드** 추출 (예: "agentic workflow orchestration", "context compilation LLM")
2. **최신 논문 검색** — Google Scholar / arXiv / ACM DL / IEEE Xplore (2022~2026)
3. **관련 연구 3~5편** 식별 및 분류:
   - 유사 접근법 → 비교 기준 도출
   - 우리가 해결하는 한계를 지적한 선행 연구
   - 우리 작업의 기반이 되는 핵심 선행 연구

**참고문헌 처리 규칙:**
```
- 직접 검색·확인된 인용: references.bib에 완전 기재
- 제목·저자 확인했으나 DOI 미확인: # TODO: verify DOI 주석
- 내용 유추·불확실: 인용 거부 또는 "~와 유사한 접근" 서술로 대체
```

> **🚫 허위 인용 절대 금지:** 존재하지 않거나 확인되지 않은 논문을 인용하지 않는다.
> 불확실한 경우 `[TODO: 참고문헌 검증 필요]`로 명기한다.

---

### 4단계 — Abstract 생성

`bathos/modules/research-pack/templates/abstract.md` 기반으로 **자족 Abstract**를 작성한다.

**Abstract 구조 (150~250단어, 4단락 또는 연속 산문):**

```
[1 Problem] — 이 연구가 다루는 문제와 그것이 중요한 이유 (2~3문장)
[2 Approach] — 우리가 취한 핵심 접근법과 방법론 (2~3문장)
[3 Contributions] — 핵심 결과·기여 (2~4문장, 구체적이되 날조 없이)
[4 Implications] — 이 연구의 의의, 적용 가능성, 미래 방향 (1~2문장)
```

**자족성 체크:** Abstract는 다른 섹션 없이 독립적으로 읽혀야 한다.
- 약어는 첫 등장 시 정의
- 시스템 이름·역할을 처음 소개하듯 서술
- 결과는 2단계 기여에서 직접 도출 (새로운 주장 추가 금지)

**evidence_trace:** Abstract의 각 주장 옆에 출처 주석 추가:
```markdown
<!-- [Source: .agent-team/04-architecture/design-patterns.md#7] -->
```
(최종 논문 제출 시 주석 제거. 검토 단계에서는 유지.)

---

### 5단계 — Introduction 생성

`bathos/modules/research-pack/templates/introduction.md` 기반으로 **Introduction 섹션**을 작성한다.

**Introduction 구조 (5개 하위섹션):**

#### §1 Motivation (동기)
- 이 문제가 왜 중요한가? (실용적·학문적 관점)
- 현재 상황: 어떤 도구·시스템이 이미 존재하는가?
- 독자가 공감할 "고통 포인트" 제시

#### §2 Problem Statement (문제 정의)
- 구체적·측정 가능한 문제 진술
- 해결하지 못했을 때의 결과
- 우리가 집중하는 범위와 그 이유

#### §3 Limitations of Existing Work (기존 연구 한계)
- 관련 연구 3~5편 인용 (3단계 검색 결과)
- 각 연구의 접근법과 **한계**를 구체적으로 서술
- 우리가 메우는 갭(gap) 도출

#### §4 Our Approach and Contributions (접근법·기여)
- 우리 시스템/방법의 핵심 아이디어 (1~2 문단)
- **기여 목록** (번호 매긴 열거):
  ```
  This paper makes the following contributions:
  1. {{기여 1 — 2단계에서 도출}}
  2. {{기여 2}}
  ...
  ```
  > ⭐ Introduction은 반드시 이 기여 목록으로 종료한다.

#### §5 Paper Organization (논문 구성)
- 나머지 섹션 안내 (2~3문장)
- 예: "Section 2 reviews related work. Section 3 describes..."

**evidence_trace:** §1~4의 모든 구체적 주장에 출처 주석 추가.

---

### 6단계 — Evidence Trace 검증 + 산출물 완료

**검증 체크리스트:**

| 항목 | 기준 | 결과 |
|------|------|------|
| Abstract 단어 수 | 150~250단어 | ✅/❌ |
| Abstract 자족성 | 약어 정의·시스템명 소개 포함 | ✅/❌ |
| 기여 항목 수 | 3~5개 | ✅/❌ |
| Introduction 종료 | 기여 목록으로 종료 | ✅/❌ |
| 모든 기여 → 설계 근거 | `[Source:...]` 추적 가능 | ✅/❌ |
| 참고문헌 검증 | TODO 없이 완전 기재하거나 명시적 TODO | ✅/❌ |
| 수치/결과 명시 | 날조 없음, 기대 효과 수식어 | ✅/❌ |
| 허위 인용 없음 | 모든 인용 검색으로 확인 | ✅/❌ |

**산출물 생성:**
1. `.agent-team/06-research/contributions-kr.md` — 기여 목록 + 근거
2. `.agent-team/06-research/abstract-kr.md` — Abstract (evidence_trace 주석 포함)
3. `.agent-team/06-research/introduction-kr.md` — Introduction (evidence_trace 주석 포함)
4. `.agent-team/06-research/references.bib` — BibTeX (TODO 항목 명기)

---

## 완료 기준 (DoD)

- [ ] 모든 출력 파일 생성됨 (`abstract-kr.md`, `introduction-kr.md`, `contributions-kr.md`, `references.bib`)
- [ ] Abstract: 150~250단어, 자족, 4-part 구조
- [ ] Introduction: 5개 하위섹션, **기여 목록으로 종료**
- [ ] 기여 3~5개, 각각 `[Source:...]` 근거 추적
- [ ] 날조 수치 없음 (기대 효과는 수식어 명시)
- [ ] 허위 인용 없음 (미검증은 TODO 명기)
- [ ] 리드(Paul)에 핵심 기여 3가지 보고

---

## BATHOS W4 매핑

- **Nathanael(#6)** 이 이 워크플로우를 단독 수행. W4는 비본류 플러그로, W2 산출 완료 후 언제든 실행 가능.
- **코어 비의존:** 이 모듈은 `core/**`를 참조하지 않음. `bathos-plug`(M12)가 `module.yaml`을 로드해 활성화.
- **evidence_trace:** 설계 산출물과 논문 주장 간 1:1 추적 체인. 구현 완료 전 "설계 기반 기대 효과"로 서술.
- **병행 가능:** Mark(#5, IP팩)와 동시 실행 가능(소유 경로 비충돌 — `05-ip/**` vs `06-research/**`).

---

*BATHOS Research Pack · Nathanael(#6) 저작 · 2026-06-30*
*이 파일은 `bathos/modules/research-pack/` 소유 경로 내 신규 자산입니다.*
