# BATHOS 용어집 (한글 현지화 일관성 기준)

> 출처: `.agent-team/01-reverse/bmad-localized-kr/_glossary-kr.md`
> 원본 작성: John(BATHOS Reverse Specialist) · 2026-06-29
> 패키지 배치: Timothy(BATHOS Doc Specialist) · 2026-06-29
> 목적: BATHOS 패키지 전반의 용어 일관성.
> 표기 원칙: 한국어 우선, 첫 등장 시 (영문) 병기, 코드/파일명/식별자는 원문 유지.

---

## 핵심 용어

| 영문 원어 | 한글 표준 역어 | 비고/맥락 |
|---|---|---|
| Skill / SKILL.md | 스킬 / `SKILL.md` | BMAD의 실행 단위. BATHOS에선 역할 프롬프트/워크플로우로 흡수. |
| Agent / Persona | 에이전트 / 페르소나 | 고정 정체성 + 조정 페르소나. BATHOS 17역할에 대응. |
| Workflow | 워크플로우 | 다단계 절차. BATHOS '웨이브 내 작업'에 대응. |
| Phase (Analysis/Planning/Solutioning/Implementation) | 단계 (분석/기획/솔루셔닝/구현) | "Solutioning"은 **솔루셔닝**으로 통일. |
| Story File | 스토리 파일 | "자족적 단일 컨텍스트 파일". W3 핵심 산출물. |
| zero context loss | 무손실 컨텍스트 (제로 컨텍스트 손실) | 설계→구현 컨텍스트 유실 0. BATHOS W3 목표. |
| project-context.md | 프로젝트 헌법 | 모든 구현 워크플로우가 자동 로드하는 규칙집. |
| Epic / Story | 에픽 / 스토리 | 에픽=기능 묶음, 스토리=구현 단위. |
| Acceptance Criteria (AC) | 인수 기준 (AC) | BDD 형식(Given/When/Then). |
| Implementation Readiness Gate | 구현 준비도 게이트 | PRD↔에픽↔UX↔아키 정렬 검증 게이트. |
| **PASS / CONCERNS / FAIL** | **통과 / 유의 / 실패** | **BATHOS 통일 용어**. BMAD의 READY/NEEDS WORK/NOT READY를 통일. |
| Scale-Adaptive / Level 0~4 | 스케일 적응 / 레벨 0~4 | 복잡도별 깊이 조절. BATHOS 명시적 라우터로 승격. |
| Quick Flow / Full Flow | 퀵 플로우 / 풀 플로우 | 소규모 직행 vs 전 단계. |
| Customize (base/team/user) | 커스터마이즈 (기본/팀/개인) | 3계층 오버라이드 병합. |
| persistent_facts | 지속 사실 | 세션 내내 유지하는 정적 컨텍스트(헌법 등). |
| Sprint Status | 스프린트 현황 | 스토리 상태머신 (backlog/ready-for-dev/in-progress/in-review/done). |
| SELECTIVE_LOAD | 선택 로딩 | 패턴 매칭으로 필요분만 로드. |
| Module / Expansion Pack | 모듈 / 확장팩 | 코어 위 플러그인. BATHOS: IP팩/연구팩. |
| Adversarial Review | 적대적 리뷰 | "반드시 이슈를 찾아라". 신선한 컨텍스트로 적대적 검증. |
| Headless | 헤드리스 | 비대화 자동 실행. 게이트 생략 불가. |
| on_complete (hook) | 완료 훅 | 종료 시 다음 행동 안내. |
| Architecture Spine | 아키텍처 스파인 | 독립 구현 단위 간 일관성을 지키는 불변식 집합. |
| AD (Architecture Decision) | 아키텍처 결정(AD) | ADR 내 결정 단위. |
| memlog | 멤로그 (진행 로그) | 워크플로우 이벤트 append-only 누적 기록. |
| Working Backwards / PRFAQ | 워킹 백워즈 / PRFAQ | 아마존식 보도자료+FAQ 선작성으로 컨셉 검증. |
| Forge (forge-idea) | 단조 (아이디어 단조) | 아이디어를 압박해 더 강하게 만드는 W0 기법. |
| JTBD (Jobs To Be Done) | 할 일(JTBD) | 사용자가 달성하려는 과업. |
| FR / NFR / SM | 기능요구/비기능요구/성공지표 | 전역 안정 ID로 교차참조(FR-1, SM-1). |
| Counter-metric | 카운터 지표 | 최적화하면 안 되는 지표. |
| Blind Hunter / Edge Case Hunter | 블라인드 헌터 / 엣지케이스 헌터 | 적대적 리뷰 레이어. |
| source_hash | 소스 해시 | 상류 파일 변경 감지용 해시. D3 신선도 방어. |
| bathos | 바토스 | βάθος(그리스어 '깊이·심연'). 제품명. |

---

## BATHOS 전용 신규 용어

| 용어 | 정의 |
|------|------|
| Zero-Context-Loss (ZCL) | D1~D4 4중 방어로 설계→구현 컨텍스트 유실 0 달성 목표. |
| Wave (W0~W6) | BATHOS 7 웨이브. 각 웨이브는 독립 팀원 셋 + 게이트로 구성. |
| Lv0~Lv4 | Scale-Adaptive 라우팅 레벨. `_state/manifest.json`에 현재 Lv 기록. |
| _base / _preamble | `bathos/.claude/agents/` 하위 구조. base=역할 고정 정체성, preamble=공통 ETHOS 주입. |
| Implementation Readiness Gate (이중) | W3의 게이트 구조: (1) 정렬 검증 6단계 + (2) Thomas·Matthias 독립 리뷰 종합. |
| bathos (바이너리) | Rust 단일 정적 바이너리. `core/crates/bathos-cli`가 빌드. 훅·커맨드가 호출. |
| M1~M12 | 모듈 번호 (bathos-state=M1, bathos-router=M2, … bathos-plug=M12). |

---

## 고유명사 (원어 유지)

- **BATHOS**: 본 패키지 제품명. βάθος(깊이·심연).
- **BMAD-METHOD**: 원천 참고 레포(MIT, bmad-code-org). 상표 사용 금지.
- "BMad", "BMad Method", "BMad Builder", "TEA", "CIS", "GDS", "WDS" — 원저작사 상표, 본 패키지 미사용.
- 루트 역할명(John~Martin, Story Engineer): BATHOS 자체 정의. BMAD 원본 페르소나명(John=PM, Winston=Architect 등)과 **다름** (우연히 일부 겹치나 별개 정체성).
