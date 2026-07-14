# BATHOS — 역할 커스터마이즈 (3계층 오버라이드)

> 17개 역할을 **base 정의를 건드리지 않고** 프로젝트에 맞춰(언어·소유 경로·플랫폼·톤) 조정한다. 바꾸는 것은 역할이 크래프트를 *어떻게* 적용하는지이지, 역할이 *누구인지*가 아니다.
>
> **함께 보기:** [모듈 작성(MODULE-GUIDE-kr)](MODULE-GUIDE-kr.md) · [사용(USAGE-kr)](USAGE-kr.md) · English: [`ROLE-GUIDE-en.md`](ROLE-GUIDE-en.md) · Español: [`ROLE-GUIDE-es.md`](ROLE-GUIDE-es.md)

---

## 0. 3계층

역할의 유효 정의는 세 계층을 순서대로 겹쳐 합친 결과다:

| 계층 | 위치 | 소유 |
|------|------|------|
| **base** | `.claude/agents/_base/NN-*.md` | 고정 정체성 — 이름·배경·모델 + 탑티어 크래프트 표준/철학/DoD |
| **team** | 프로젝트 오버라이드 | 소유 경로·플랫폼(웹/iOS…)·도메인 강조·기술 스택·NFR 임계·톤&보이스 |
| **user** | 개인 오버라이드 | 언어·상세도·facilitation 수위 |

**병합 규칙:** 스칼라는 덮어쓰기(user > team > base), 배열은 append. 그래서 사용자는 언어를, 팀은 소유 경로를 정하고, 둘 다 고정 base 정체성 위에 합성된다.

## 1. 오버라이드 불가 항목
base 계층은 **이름·배경·모델**과 크래프트 표준을 고정한다. 일부러 못 바꾸게 막아 둔 부분인데, 역할을 "탑티어"답고 한결같게 유지하는 게 바로 이 고정이다. *누구인지*가 아니라 *어떻게 크래프트를 프로젝트에 적용하는지*를 오버라이드하라.

## 2. base 역할의 구조
모든 base 파일(`.claude/agents/_base/NN-name.md`)은 YAML frontmatter + 본문:

```yaml
---
role_number: 4
name: james
slug: james-architect          # ← 스폰에 쓰는 agent-type 슬러그
model: opus                     # opus = Opus 4.8 · sonnet = Sonnet 5
wave: W2 (Joshua 완료 후)
spawnable: true                 # Paul(#0)은 false — 리드는 메인 세션
tools: [Read, Grep, Glob, Bash, Write, WebFetch, WebSearch]
---
```
본문 섹션(상향 후 표준): 정체성 · §0 철학 · 미션&산출물 · 크래프트 표준(타협 불가) · 안티패턴 · 프로세스 · DoD · 3계층 노트.

## 3. team 오버라이드 추가 (예)
base 역할 위에 합성되는 team 계층 노트를 만든다. 흔한 team 오버라이드:
- **소유 경로** — 예: Andrew(#9)=`web/`, Phillip(#8)=`api/`(겹침 0 → 병렬 안전).
- **플랫폼/스택** — "웹=Next.js", "백엔드=Go + Postgres".
- **NFR 임계** — "p95 < 150ms", "번들 < 200KB".
- **도메인 강조/톤** — 브랜드 보이스, 규제 포커스.

team 오버라이드는 *프로젝트 특정 사실*에 한정; 크래프트 표준은 base에 맡긴다.

## 4. user 오버라이드 추가
프로젝트를 가로지르는 개인 설정:
- **언어** — 한국어/영어 출력.
- **상세도** — 간결 vs 상세.
- **facilitation** — 역할이 얼마나 적극적으로 제안하는지(User Sovereignty는 항상 준수).

## 5. 역할 스폰
리드(Paul)가 `slug`(agent-type)로 팀원을 스폰하며 입력 경로·소유 경로·DoD를 전달한다. 예: "`james-architect` 스폰, `03-service-planning/` 읽고 `04-architecture/` 소유, ETHOS.md 준수." 동시 ≤3; 웨이브 끝에 shutdown.

## 6. 좋은 습관
아래 습관들을 지키면 커스터마이즈가 깔끔하게 남고, 병렬에서도 안 부딪히며, base에 충실한 상태를 유지한다:

- 프로젝트 사실을 바꾸려고 **base를 포크하지 말 것** — team/user 계층 사용.
- **소유 경로 분리** — 병렬 역할끼리 겹치지 않는 경로를 배정한다(freeze-guard가 강제).
- **상향된 바 유지** — 새 역할 추가 시 6섹션 탑티어 구조(철학·크래프트 표준·안티패턴·프로세스·DoD)를 따를 것.
- *역할*이 아니라 *도메인 팩*이 필요하면 플러그로([`MODULE-GUIDE-kr.md`](MODULE-GUIDE-kr.md)).

---

<div align="center"><a href="ROLE-GUIDE-en.md">ROLE-GUIDE-en</a> · <a href="ROLE-GUIDE-es.md">ROLE-GUIDE-es</a></div>
