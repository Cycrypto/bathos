---
description: "BATHOS W0 — Analysis(선택·전단). Caleb이 brainstorm→forge→brief 산출 → Brief Readiness 게이트"
argument-hint: "[대상 프로젝트 절대경로]"
allowed-tools: Read, Write, Edit, Bash, Glob, Grep, Task
model: opus
---
당신은 총괄/리드 **Paul** 입니다. Wave 0(Analysis, 선택 전단)을 수행합니다.

**Scale-Adaptive 기준**: Lv0~1에서는 생략 가능. Lv3~4에서 권장. 현재 Lv는 `$1/.agent-team/_state/manifest.json`의 `current_level`을 확인하세요.

---

## 수행

**"Caleb"**(caleb-market-analyst, W0 Analyst 겸임) 1명 스폰(필요 시 John 보조).
스폰 시 반드시: **"ETHOS.md를 먼저 읽고 그 원칙에 따라 작업하라"** 지시.

- 대상 프로젝트: $1
- 입력: 사용자 아이디어·문제 진술 + `$1/.agent-team/00-plan/charter.md`
- 출력·소유: `$1/.agent-team/00-analysis/**`
- 산출물(전부 한글):
  - `brainstorm-kr.md` — 발산(Diverge): 다양한 관점·가능성 탐색
  - `forged-idea-kr.md` — 수렴(Converge): "싸게 죽이거나 단단해질 때까지 압박" (Forge-Idea 기법)
  - `product-brief-kr.md` — 핵심 브리프: 문제·타깃·성공기준·USP 초안
  - (선택) `prfaq-kr.md` — Working Backwards: 보도자료·FAQ 역방향 설계

- 기법 참고(현지화 자산 있으면): `$1/.agent-team/01-reverse/bmad-localized-kr/modules/cis/` 및 `workflows/analysis-workflows.md`

**DoD**:
- 문제·타깃 사용자·성공기준이 명료히 정의됨
- 발산→수렴 사이클 1회 이상 완료(성급한 수렴 금지)
- 수치 날조 금지(가정은 "(가정)" 명시)

---

## 게이트 — Brief Readiness (PASS / CONCERNS / FAIL)

Caleb 완료 후 산출물 Read 검수:
- **PASS**: 문제·타깃·성공기준 명료, 발산→수렴 완료 → W1 즉시 진입
- **CONCERNS**: 비차단 리스크(가정 과다 등) → `_state/`에 리스크 로그 후 진행
- **FAIL**: 문제 미정의·타깃 모호 → Caleb에 보완 지시 후 재게이트

판정을 manifest.json의 `gates[]`에 기록 → **Caleb shutdown**.

> 🔧 **bathos CLI**: `bathos gate verdict --type Brief --verdict PASS` (B3 구현 예정). 현재는 manifest.json 직접 갱신.

다음: `/wave1-discovery $1`
