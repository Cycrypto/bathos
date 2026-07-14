# 워크플로우 — Design Excellence (탑티어 UI/UX 설계 절차)

> 소유: #7 Jonnathan(수석 디자이너) · 웨이브: W2 · 산출: `.agent-team/07-design/`
> 목적: "예쁜 화면"이 아니라 **제품을 성공시키는 디자인 체계**를 재현 가능한 절차로 만든다.
> 원칙: 문제 고정 → 트리트먼트 보정 → 제약이 곧 우아함 → 생성≠검증(자기검증) → User Sovereignty.

---

## 단계 0 — 입력 흡수 & 문제 고정 (Frame)
1. 읽기: `03-service-planning/`(USP·Core Feature·User/Service Story), `04-architecture/`(데이터·API·제약), `02-market-analysis/`(경쟁 UX).
2. **한 문장으로 못 박기:** "*[누구]*가 *[어떤 맥락]*에서 *[어떤 Job]*을 하도록 돕는다." 흐리면 여기서 멈추고 Joshua/리드에 질의.
3. **트리트먼트 판정:** 유틸리티(절제된 폴리시) vs 편집/랜딩(대담) vs 데이터 UI(정보설계 우선). 화면군마다 다르게.
4. Search Before Building: 낯선 도메인·패턴은 WebSearch로 경쟁 UX·플랫폼 컨벤션·최신 HIG 파악.
→ 산출: `ux-strategy.md`(페르소나 1~3 · JTBD · 매직 모먼트 · TTHW · 마찰점 지도 · 성공지표).

## 단계 1 — 정보구조 (IA)
- 화면 인벤토리·네비게이션 모델(탭/사이드/계층)·콘텐츠 위계·URL/라우트 구조.
- 카드소팅 관점: 사용자 멘탈 모델과 시스템 모델의 간극을 메운다.
→ 산출: `information-architecture.md`.

## 단계 2 — 플로우 (Flows)
- 핵심 유저 플로우를 Mermaid로. **행복 경로만 그리지 말 것** — 분기·엣지·복귀·중단/재개·권한 분기 포함.
- 각 플로우에 "매직 모먼트"와 "마찰점"을 표시.
→ 산출: `ux-flow-map.md`.

## 단계 3 — 저충실도 구조 (Wireframe / 레이아웃 로직)
- 화면별 레이아웃 골격을 그리드·영역으로. 요소별 마진이 아니라 **flex/grid + gap**으로 간격 논리를 세운다.
- 이 단계에서 **모든 상태**(로딩/빈/부분/에러/권한없음/성공)를 미리 열거.

## 단계 4 — 비주얼 시스템 (Design System) ★ 핵심
- `assets/templates/design-system-template.md`로 **토큰 먼저**: 색(뉴트럴은 hue 편향 선택)·타입 스케일·간격(8pt)·라운드·엘리베이션·모션.
- 컴포넌트 계약: 각 컴포넌트의 변이(variant)·상태·크기·토큰 매핑·a11y 계약.
- 크래프트 표준(Jonnathan §2) 전면 적용. AI 클리셰(§3) 회피.
→ 산출: `design-system/`(tokens.md + components.md + patterns.md).

## 단계 5 — 고충실도 & Claude Design 캔버스
- **Claude Design(Pencil MCP)에서 실제 설계·검증:**
  1. `get_guidelines` + `get_editor_state(include_schema:true)`
  2. `set_variables`로 토큰 반영 → `batch_design`으로 화면·컴포넌트 생성
  3. `snapshot_layout` + `get_screenshot`로 **눈으로 검증**(overflow·대비·정렬·상태 누락 색출)
  4. `export_nodes`로 인계 산출물 내보내기
- 마이크로카피 확정(사람 언어·능동태·명확한 에러).

## 단계 6 — 모션 & 마이크로인터랙션
- 목적 있는 전환만(로드 시퀀스·상태 전환·피드백). 타이밍/이징 명세, `prefers-reduced-motion` 대응.
→ 산출: `motion-spec.md`.

## 단계 7 — 접근성 감사 (WCAG 2.2 AA)
- 대비 수치·키보드 전 경로·포커스 가시성·스크린리더 레이블·타깃 44px+·모션 민감·색 비의존 검증.
→ 산출: `accessibility.md`.

## 단계 8 — 자기검증 (생성≠검증)
- `assets/checklists/design-quality.md`로 전 차원 0~10 자가 채점. **10 미만 차원마다 "10점의 모습"과 격차·조치**를 기록하고 끌어올린다.

## 단계 9 — 개발 핸드오프
- Andrew가 **추가 질문 0**으로 착수 가능하도록: 토큰값(실제 hex/px/ms)·컴포넌트 상태·엣지·에셋·인터랙션 타이밍·반응형 브레이크포인트·빈/에러 카피까지 전부.
→ 산출: `design-handoff.md`.

---

### 완료 기준 (DoD)
Jonnathan base §7과 동일. 요약: 모든 기능 플로우·IA·6상태 완비 · 토큰+컴포넌트 계약 · WCAG 2.2 AA 근거 · 모션+reduced-motion · 루브릭 전 차원 8+ · 핸드오프 질문 0.
