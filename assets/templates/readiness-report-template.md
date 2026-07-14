<!--
출처: .agent-team/01-reverse/bmad-localized-kr/templates/readiness-report-template.md
원본: src/bmm-skills/3-solutioning/bmad-check-implementation-readiness/templates/readiness-report-template.md
       (+ step-06-final-assessment.md의 요약 섹션)
레포: bmad-code-org/BMAD-METHOD @ main (MIT © 2025 BMad Code, LLC)
현지화: John(BATHOS Reverse Specialist) · 2026-06-29 · 게이트 용어 PASS/CONCERNS/FAIL로 통일
패키지 배치: Timothy(BATHOS Doc Specialist) · 2026-06-29
-->

# 구현 준비도 평가 리포트 (W3 게이트)

**일자:** {{date}}
**프로젝트:** {{project_name}}
**평가자:** {{assessor}} (Story Engineer #15)
**독립 리뷰어:** Thomas(코드리뷰) · Matthias(QA)

---

## 1. 문서 발견 결과

- 검증 대상: PRD / UX(DESIGN·EXPERIENCE) / ARCHITECTURE / 에픽·스토리 목록
- 발견된 문서:

| 문서 | 경로 | 상태 |
|------|------|------|
| PRD | [경로] | 발견/미발견 |
| UX | [경로] | 발견/미발견 |
| Architecture | [경로] | 발견/미발견 |
| Epics | [경로] | 발견/미발견 |

---

## 2. PRD 분석 Findings

- [요구사항·성공기준 누락/모순 목록]
- 치명 이슈: [Critical 건수]건 / 비치명: [Non-critical 건수]건

---

## 3. 에픽 커버리지 검증

PRD의 각 기능요구(FR)가 에픽/스토리로 커버되는가:

| FR ID | 커버 여부 | 매핑 에픽/스토리 |
|-------|----------|----------------|
| FR-1 | ✅/❌ | [에픽명] |

- 미커버 FR: [목록]

---

## 4. UX 정렬 Findings

- [UX ↔ 에픽/요구사항 불일치 목록]

---

## 5. 에픽 품질 리뷰

- [크기·의존성·AC 누락 등 위반 목록]

---

## 6. 독립 리뷰 결과

### Thomas (코드리뷰 관점)

- Critical 이슈: [건수 + 목록]
- Non-critical 이슈: [건수 + 목록]

### Matthias (QA 관점)

- Critical 이슈: [건수 + 목록]
- Non-critical 이슈: [건수 + 목록]

---

## 요약 및 권고

### 전체 준비도 상태

**[PASS / CONCERNS / FAIL]**

> - **PASS**: 구현(W5) 진입 허용
> - **CONCERNS**: 조건부 통과 — 리스크를 `_state/`에 로그 후 진행
> - **FAIL**: W2로 반려, 보완 후 재게이트
>
> 판정 기준: `critical 이슈 > 0 → FAIL`, `non-critical 이슈 > 0 → CONCERNS`, `else → PASS`

### 즉시 조치가 필요한 치명적 이슈

- [반드시 해결해야 할 이슈 — 구체적 예시 포함, 누그러뜨리지 않음]

### 권고 다음 단계

1. [구체적 액션 1]
2. [구체적 액션 2]
3. [구체적 액션 3]

### 최종 메모

이 평가는 [Y]개 범주에서 [X]개 이슈를 식별했다 (Critical [C]건, Non-critical [N]건).
구현(W5) 진입 전 치명 이슈를 해결하라.

**평가자:** {{assessor}} · **일자:** {{date}}
