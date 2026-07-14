# 디자인 시스템 — {{product}} (템플릿)

> 소유: #7 Jonnathan · 산출 위치: `.agent-team/07-design/design-system/`
> 규칙: **토큰이 진실 원천.** 컴포넌트·화면은 토큰을 참조하고, 하드코딩 값을 쓰지 않는다.
> 모든 값은 실제 값(hex/px/ms)으로 채운다 — 플레이스홀더로 핸드오프하지 않는다.

---

## 1. 브랜드 & 트리트먼트
- 한 줄 정체성 / 톤&보이스: {{...}}
- 트리트먼트: 유틸리티 · 편집(editorial) · 데이터 UI 중 { } — 이유: {{...}}
- 아이덴티티의 "한 가지 대담함"을 어디에 쓸지: {{...}} (나머지는 조용히)

## 2. 색 토큰 (Color)
> 뉴트럴은 순수 회색이 아니라 악센트 쪽으로 미세 hue 편향을 **선택**. 시맨틱은 브랜드 악센트와 분리. 모든 조합 WCAG 2.2 AA.

| 토큰 | 값(hex) | 용도 | 대비 근거 |
|------|---------|------|-----------|
| `--bg` | | 페이지 바탕 | |
| `--surface` / `--raised` | | 패널/카드 | |
| `--text` / `--muted` | | 본문/보조 | text 4.5:1↑ |
| `--line` | | 구분선/보더 | |
| `--accent` (+hover/active) | | 브랜드 악센트(한 곳 집중) | UI 3:1↑ |
| `--good` / `--warn` / `--critical` | | 시맨틱(악센트와 분리) | |
- 다크/라이트 모드: { 지원/미지원 } — 지원 시 각 토큰의 모드별 값.

## 3. 타이포그래피 (Type)
| 역할 | 폰트(스택) | 크기/라인/웨이트 | 비고 |
|------|-----------|-----------------|------|
| Display | | | text-wrap:balance |
| Heading | | | |
| Body | | measure ~65ch | |
| Label/Caption | | letter-spacing | 대문자 라벨 |
| Mono/Data | | tabular-nums | 숫자 정렬 |
- 타입 스케일 비율: { 1.200 / 1.250 / 1.333 } · 웹 구현: @font-face **data URI 인라인**(CDN 금지). 한글: **Pretendard 우선**.

## 4. 간격·라운드·엘리베이션 (Space / Radius / Elevation)
- 간격 스케일(8pt 기반): 4 · 8 · 12 · 16 · 24 · 32 · 48 · 64
- 라운드: `--r-sm` / `--r-md` / `--r-lg` (값). "어디나 rounded-lg" 금지 — 위계에 맞게.
- 엘리베이션(그림자): 레벨별 값 + 언제 쓰는지.

## 5. 모션 (Motion)
| 상황 | duration | easing | 비고 |
|------|----------|--------|------|
| 마이크로(hover/press) | 120–160ms | ease-out | |
| 전환/리빌 | 200–320ms | (cubic-bezier) | |
| 로드 시퀀스 | | | 오케스트레이션된 한 순간 |
- `prefers-reduced-motion: reduce` 대응 규칙(필수).

## 6. 컴포넌트 계약 (Components)
각 컴포넌트마다:
- **변이(variant):** { primary/secondary/ghost … }
- **크기:** { sm/md/lg } + 토큰 매핑(패딩/폰트/라운드)
- **상태:** default · hover · active · focus-visible · disabled · loading · error
- **a11y 계약:** 역할(role)·레이블·키보드 조작·포커스 순서·타깃 44px+
- (예: Button / Input / Select / Modal / Toast / Table / Card / Nav / EmptyState / ErrorState)

## 7. 패턴 (Patterns)
- 폼 검증 & 에러 표시 · 빈 상태(첫 사용 온보딩) · 로딩(스켈레톤 vs 스피너) · 파괴적 액션 확인 · 알림/피드백 · 반응형 브레이크포인트({sm/md/lg/xl} + 각 규칙).

## 8. 접근성 기준선 (WCAG 2.2 AA)
- 대비 본문 4.5:1 / 큰 텍스트·UI 3:1 · 키보드 전 경로 · 포커스 가시 · 색 비의존 · 타깃 24px(min)~44px · 모션 민감 대응.
