---
name: bathos-evals
description: 로컬 evals 스켈레톤(골든셋 SHA 고정 + 파일럿 + judge 페어와이즈 + 4섹션 리포트). LangSmith 비의존, 자체 태스크셋(웨이브 완료시간/게이트 오탐률/컨텍스트 무손실). LD-3 스켈레톤+1~2 레퍼런스 케이스만. "evals 실행", "벤치마크 리포트" 요청 시 사용.
---

# bathos-evals — 로컬 evals 스켈레톤 + 벤치마크 스텁 (CF-C3·C4 · US13·US14 · Could/LD-3)

> 근거: `_recon/open-swe-analysis.md` §3-7(evals 하니스), `service-layer-design-kr.md §3~4`,
> `surface-formats-kr.md §c`, `story-c3-c4-evals-skeleton-kr.md`.
> **범위 상한(LD-3, 엄수): 스켈레톤 + 레퍼런스 1~2 케이스.** 그 이상 골든셋 확장 금지
> (Matthias W6 후속). **LangSmith·외부 클라우드 벤치 카피 금지** — 전부 로컬 파일.

## 파이프라인

```
골든(SHA 고정) → 파일럿(bathos inspect류, 부재 시 정직한 스텁) → judge(stub|agent) → JSON 결과 → 4섹션 리포트
```

## 절차 (에이전트가 수행)

1. **드리프트만 점검**(빠른 헬스체크):
   ```bash
   python3 .claude/skills/bathos-evals/scripts/harness.py verify
   ```
2. **전체 골든셋 실행**:
   ```bash
   python3 .claude/skills/bathos-evals/scripts/harness.py run-all \
     --golden-dir .agent-team/11-qa/evals/golden \
     --results-dir .agent-team/11-qa/evals/results
   ```
   기본 judge는 `stub-heuristic-v1`(결정론적 로컬 함수, 재현 가능). **프로덕션에서
   더 정교한 판정이 필요하면**, `judge.format_judge_prompt(golden, candidate)`로
   프롬프트를 만들어 **이 스킬을 호출 중인 에이전트 자신이 judge 역할**을 겸해
   verdict를 판단하고, 그 결과를 결과 JSON에 반영한다(별도 API 키 불필요). 단,
   이 경로는 **seed/온도를 제어할 수 없어 재현성이 (추정) 미보장**임을 리포트에
   반드시 명시한다(R-C2).
3. **리포트 생성**:
   ```bash
   python3 .claude/skills/bathos-evals/scripts/report.py \
     --results-dir .agent-team/11-qa/evals/results \
     --out .agent-team/11-qa/evals/report-latest-kr.md
   ```
4. 결과를 사용자에게 보고할 때 **"(추정)" 배지·베이스라인 미실측·N값을 그대로** 전달한다
   (대외 수치 주장 금지, in-repo만 — project-context-kr.md §7).

## 레퍼런스 골든 케이스 (LD-3 상한 — 2개, 그 이상 추가 금지)

| eval_id | task | 내용 |
|---------|------|------|
| `E-0001` | `ctx-loss` | 스토리 프론트매터(story-c1)만으로 5개 필드를 재복원할 수 있는가 — "스토리 컨텍스트 무손실" 결정론적 프록시 |
| `E-0002` | `gate-fp` | `bathos inspect doctor` 파일럿 출력에서 기대 판정 문자열 확인 — 바이너리 부재 시 정직한 스텁 강등 |

두 태스크 모두 **자체 태스크셋**이다(BATHOS 산출용, 웹 스니펫 벤치 카피 아님).

## 골든셋 관리

- `case.json` + `case.json.sha256`(sha256sum 형식) 쌍으로 고정. 내용이 바뀌면 SHA를
  재고정해야 하며, 재고정하지 않은 변경은 `harness.py verify`/`run`이 **드리프트
  경고**(exit 2)로 잡는다.
- 신규 케이스 생성: `harness.write_golden_case(case_dir, data)` 헬퍼 사용.
- **케이스 3개 이상 추가 금지**(AC5, LD-3 상한). 확장 필요 시 Matthias(W6)에게 인계.

## 정직성 규약 (surface-formats §c-4, report.py가 코드로 강제)

- N<3 → `(추정) — 표본 부족` 배지 강제.
- golden/results 0건 → "데이터 없음(스켈레톤)" 배너(빈 표를 0으로 채우지 않음).
- 실행 실패 섞이면 → `부분 리포트(k건 실패)` 헤더 + §3 개별 분리 표기.
- 베이스라인(무파이프라인) 미실측 → 델타를 산출하지 않고 `—`만 표시.
- §4에 정정 이력 절 상시 유지(ponytail 자기수정 문화 계승).

## Boundaries (스코프 밖)

- LangSmith SDK·store·프로젝트 라우팅 없음(전부 로컬 JSON 파일).
- `bathos-inspect` 크레이트 수정 없음(재사용만, 수정 필요 시 Phillip과 합의).
- 실측 벤치마크 "완성"이 아니다 — 구조 증명 + 레퍼런스 2케이스까지(LD-3).
- 대외(README/마케팅) 수치 주장 없음(in-repo 리포트만).
- HTML 렌더링 없음(마크다운/JSON까지, HTML은 Martin W6 범위).

## 테스트

```bash
bash .claude/skills/bathos-evals/tests/run_tests.sh
```
11개 테스트: 골든 SHA 무결성/드리프트(1바이트 변조), ctx-loss e2e 스모크(완전
복원=win, 부분 손실=tie/lose), gate-fp 바이너리 부재 스텁 강등, judge 순수성/
재현성, 리포트 빈 상태·표본부족 배지·부분실패 상태·베이스라인 미실측 표시.
전부 오프라인(네트워크·API 키 불요).
