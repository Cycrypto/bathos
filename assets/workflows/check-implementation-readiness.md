<!--
출처: .agent-team/01-reverse/bmad-localized-kr/workflows/check-implementation-readiness.md
원본: src/bmm-skills/3-solutioning/bmad-check-implementation-readiness/{SKILL.md, steps/step-01..06}
레포: bmad-code-org/BMAD-METHOD @ main (MIT © 2025 BMad Code, LLC)
현지화: John(BATHOS Reverse Specialist) · 2026-06-29
패키지 배치: Timothy(BATHOS Doc Specialist) · 2026-06-29
-->

# 워크플로우: 구현 준비도 게이트 (W3 Implementation Readiness Gate) — BATHOS

**목표:** 구현(W5) 진입 전, PRD·UX·아키텍처·에픽/스토리 목록이 **서로 정렬**되어 있는지 검증하고 명확한 판정을 내린다.

> **판정 용어(BATHOS 통일):** **PASS / CONCERNS / FAIL**
> *(BMAD 원본의 READY/NEEDS WORK/NOT READY를 이에 통일.)*

---

## 핵심 규칙

- 🛑 **사용자 입력 없이 내용 생성 금지.** 당신은 **FACILITATOR**이지 generator가 아니다.
- 📖 각 단계를 **끝까지 읽고** 행동.
- 🚫 **메시지를 누그러뜨리지 말 것**(Don't soften). 문제는 구체적 예시와 함께 직설적으로.
- ✅ 근거 없는 자동 PASS 금지. 의심되면 CONCERNS 또는 FAIL.

---

## 이중 게이트 구조

### 게이트 1: 정렬 검증 6단계 (Story Engineer 수행)

| 단계 | 목표 |
|------|------|
| 01 문서 발견 | 검증 대상 문서(PRD/UX/아키텍처/에픽) 식별·로드 |
| 02 PRD 분석 | PRD의 요구사항·성공기준 분석, 누락/모순 식별 |
| 03 에픽 커버리지 검증 | PRD의 모든 기능요구(FR)가 에픽/스토리로 커버되는지 |
| 04 UX 정렬 | UX 설계와 에픽/요구사항 정렬 |
| 05 에픽 품질 리뷰 | 에픽 품질 위반(크기·의존성·AC 누락 등) |
| 06 최종 판정 | 전 단계 종합 → 전체 준비도 결정 |

#### 단계 01 — 문서 발견

검색 경로:
- PRD: `.agent-team/03-service-planning/*prd*.md`
- UX: `.agent-team/07-design/*.md`
- Architecture: `.agent-team/04-architecture/*.md`
- Epics: `.agent-team/03-service-planning/*epic*.md`

누락 문서가 있으면 즉시 FAIL.

#### 단계 02 — PRD 분석

- 기능요구(FR) 전수 목록화
- 성공지표(SM) 정의 여부 확인
- 비기능요구(NFR) 명시 여부 확인
- 모순·누락 식별 → Critical 또는 Non-critical 분류

#### 단계 03 — 에픽 커버리지 검증

- PRD의 모든 FR이 에픽/스토리로 커버되는지 매핑
- 미커버 FR = Critical 이슈

#### 단계 04 — UX 정렬

- UX Flow Map의 모든 화면이 에픽/스토리에 반영되는지
- 상태 처리(로딩/빈/에러/권한) 명세 여부
- 불일치 = Non-critical (치명이면 Critical)

#### 단계 05 — 에픽 품질 리뷰

- 에픽 크기: 스프린트 내 완료 가능한가?
- AC(인수 기준): BDD 형식·측정 가능한가?
- 의존성: 순환 의존 없는가?
- 기술 요구사항: 구체적인가?

#### 단계 06 — 최종 판정 (Story Engineer)

```
critical > 0  → FAIL
non-critical > 0  → CONCERNS
else  → PASS
```

---

### 게이트 2: 독립 리뷰 (Thomas · Matthias 수행)

**Thomas (코드리뷰 관점):**
- 스토리파일의 아키텍처 준수성
- API 계약 명확성
- 보안·성능 제약 명시 여부

**Matthias (QA 관점):**
- AC 테스트 가능성
- 엣지 케이스 누락
- NFR 검증 기준 명시 여부

**독립 리뷰 결과 수렴:**
- 각 리뷰어: `reviews/review-{reviewer}.md`에 풀리뷰 작성, 요약만 반환.
- Story Engineer가 두 리뷰를 종합해 최종 판정 갱신.

---

## 최종 판정 섹션 (readiness-report-kr.md에 작성)

```markdown
## 요약 및 권고

### 전체 준비도 상태
**[PASS / CONCERNS / FAIL]**

### 즉시 조치가 필요한 치명적 이슈
[Critical 이슈 목록 — 구체적, 누그러뜨리지 않음]

### 권고 다음 단계
1. [액션 1]
2. [액션 2]

### 최종 메모
이 평가는 [Y]개 범주에서 [X]개 이슈를 식별했다. 구현 진입 전 치명 이슈를 해결하라.
```

---

## BATHOS W3 게이트 결과 처리

| 판정 | 처리 |
|------|------|
| **PASS** | W5(Implementation) 진입 허용. `gate-enforce.sh` 통과. |
| **CONCERNS** | 조건부 통과: 리스크를 `_state/`에 로그, W5 진입 허용. 후속 해소 추적. |
| **FAIL** | W5 진입 물리 차단(`gate-enforce.sh` exit 2). W2로 반려, 보완 후 재게이트. |

> 재게이트 상한 초과 (예: 3회 이상 FAIL) 시 → 리드(Paul)에 보고 후 사용자 결정(User Sovereignty).
