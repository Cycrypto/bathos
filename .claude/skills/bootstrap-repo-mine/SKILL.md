---
name: bootstrap-repo-mine
description: 낯선 코드베이스 첫 진입 시 git/gh 이력(병합 PR 코멘트, 봇 제외)에서 저장소 컨벤션·버그 taxonomy 초안을 채굴한다. 읽기 전용, John(#1 Reverse) 01-reverse/에는 절대 쓰지 않는다. "콜드스타트", "저장소 컨벤션 채굴", "bootstrap" 요청 시 사용.
---

# bootstrap-repo-mine — 콜드스타트 컨벤션 채굴 (CF-C2 · US12 · SS13)

> 근거: `_recon/open-swe-analysis.md` §3-3(bootstrap-repo-analysis), `service-layer-design-kr.md §2`,
> `story-c2-bootstrap-mine-kr.md`. **범위(risk-log A-5 확정): 초안 텍스트 생성까지.**
> `.agent-team/01-reverse/`는 John(#1 Reverse Specialist) 소유 — 이 스킬은 절대 쓰지 않는다.

## 언제 쓰는가

새 코드베이스에 처음 진입해 팀 컨벤션/버그 taxonomy를 빠르게 파악하고 싶을 때, 또는
John의 W1 리버스 분석 착수 전 콜드스타트 가속기로 온디맨드 호출한다.

## 절차 (에이전트가 수행)

1. 대상 저장소에서 실행:
   ```bash
   python3 .claude/skills/bootstrap-repo-mine/scripts/mine.py --repo <대상 경로>
   ```
   `gh` CLI가 설치·인증되어 있으면 병합 PR + 코멘트를 우선 사용한다. 없거나 실패하면
   자동으로 `git log --merges` 근사치로 폴백하고(부분 결과 명시), 그마저 커밋 이력이
   없으면 디렉터리/테스트/린트 구성 기반 구조 폴백으로 넘어간다(에러 없이 3단 강등).
2. **원자료를 검토해 서술을 다듬는다**: `--json` 옵션으로 원자료(taxonomy 빈도·근거
   인용·구조 신호)를 받아, 호출 에이전트(Claude)가 의미론적으로 정리된 최종 문장으로
   보강할 것을 권장한다. 스크립트의 빈도 집계는 결정론적 휴리스틱이지 의미 이해가
   아니다(과장 금지) — "초안의 초안"으로 취급한다.
3. **`.agent-team/01-reverse/`에 절대 쓰지 않는다.** 완성된 초안은 대화 응답으로
   사용자/John에게 전달하거나, 사용자가 명시적으로 지정한 다른 경로에만 저장한다.
   CLI는 `--out` 경로에 `01-reverse`가 포함되면 즉시 거부(exit 2)한다(하드 가드).
4. 최종 초안을 사용자에게 제시하며 "이것은 초안이며 01-reverse/ 반영은 John 소유"임을
   명시한다(Boundaries 정직 고지).

## 폴백 매트릭스 (전부 AC)

| 상황 | 동작 |
|------|------|
| `gh` 설치+인증, 병합 PR 8건 이상 | PR 코멘트 기반 taxonomy, 신뢰도 "충분" |
| `gh` 설치+인증, 병합 PR 8건 미만 | 동일 경로, 신뢰도 "낮음" 표기(수치 날조 금지) |
| `gh` 미설치/미인증/실패 | `git log --merges` 근사치로 폴백, "원격 PR 코멘트 없음(부분 결과)" 명시 |
| git 이력 자체 없음(커밋 0) | 디렉터리/테스트/린트 구조 폴백, "이력 부족" 명시 |

봇 계정 제외 패턴(기본, 확장 가능): `*[bot]` 접미, `dependabot*`, `renovate*`,
`github-actions*`, `snyk-bot*`(`--bot-pattern` 정규식으로 추가 가능).

## 출력 규약

- 접두 `[bathos bootstrap-mine]`, 흑백 기호만.
- 읽기 전용 — 대상 저장소를 절대 수정하지 않는다(AC5).
- 초안 분량 목표 400~1200단어(US12 AC2). 데이터가 빈약하면 일반 체크리스트를
  덧붙여 하한에 가깝게 하되, 이는 "일반 권고"임을 명시하고 없는 사실을 지어내지 않는다.

## Boundaries (스코프 밖)

- `.agent-team/01-reverse/` 쓰기 금지(John 소유, risk-log A-5) — CLI가 하드 가드.
- 실제 리버스 산출물 반영(John의 W1 파이프라인 결선)은 이번 범위 밖(후속 단계).
- 대상 저장소에 대한 쓰기/수정 없음(읽기 전용 채굴만).
- taxonomy는 빈도 기반 휴리스틱이지 LLM 의미 분류가 아니다 — 최종 다듬기는 호출
  에이전트 몫.
- 병합 PR 8건 임계는 open-swe 관례를 차용한 **추정치**이며 조정 가능.

## 테스트

```bash
bash .claude/skills/bootstrap-repo-mine/tests/run_tests.sh
```
합성 git 저장소 픽스처(임시 디렉터리에 실제 `git init`)로 7개 테스트: git-log 폴백
taxonomy·봇 제외, 얕은 저장소 구조 폴백, gh 강제 비활성, 표본 미달 신뢰도 표기,
봇 판별 단위 테스트, `01-reverse/` 출력 하드 거부, 초안 분량. **네트워크 호출 없음**
(`--no-gh`로 오프라인 결정성 보장).
