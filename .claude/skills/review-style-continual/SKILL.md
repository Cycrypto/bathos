---
name: review-style-continual
description: audit-log.jsonl의 finding.resolved 이력(확정/기각)을 읽어 프로젝트별 리뷰 스타일 프롬프트(10-review/review-style/style.md)를 승격/억제로 편집한다. 온디맨드·로컬 전용, 외부 SDK 없음. "리뷰 학습", "review style", "continual learning" 요청 시 사용.
---

# review-style-continual — 리뷰 자기개선 스킬 (CF-C1 · US11 · SS12)

> 근거: `_recon/open-swe-analysis.md` §3-2(continual-learning), `service-layer-design-kr.md §1`,
> `story-c1-review-continual-kr.md`. **편집이지 재작성이 아니다**(adr/D-0003) — 스타일 파일의
> 마커 블록만 갱신하고 사람이 쓴 나머지 내용은 절대 건드리지 않는다.

## 언제 쓰는가

사용자가 "리뷰 스타일을 학습/갱신해줘", "지난 리뷰 이력으로 리뷰 기준 업데이트해줘" 등을
요청하거나, 웨이브 종료 시 리뷰 확정/기각 이력이 쌓였는지 점검할 때 온디맨드로 호출한다.
**크론 아님** — 상시 서비스로 등록하지 않는다(charter §3 카피금지, open-swe 야간 크론 배제).

## 절차 (에이전트가 수행)

1. **계획 실행(dry-run, 기본)**:
   ```bash
   python3 .claude/skills/review-style-continual/scripts/aggregate.py \
     --audit-log .agent-team/_state/audit-log.jsonl \
     --findings-index .agent-team/10-review/findings-index.jsonl \
     --style-path .agent-team/10-review/review-style/style.md \
     --history-dir .agent-team/10-review/review-style/history
   ```
   출력은 diff 미리보기다(파일을 쓰지 않는다).
2. **diff를 사용자에게 그대로 보여주고 승인을 받는다**(UX 규약 — AI가 리뷰 스타일을
   몰래 바꾸지 않는다, User Sovereignty). 승인 없이 3단계로 넘어가지 않는다.
3. **승인 시**: 위 명령에 `--apply`를 추가해 재실행한다. 완료 요약(`[bathos review-learn] ✓ ...`)을
   그대로 사용자에게 보고한다.
4. **거부/보류 시**: 아무것도 쓰지 않는다. "다음 실행 시 재검토"라고 안내한다.

## 입력 계약 (읽기 전용)

- `.agent-team/_state/audit-log.jsonl` — `action=="finding.resolved"`, `target="<finding_id>:<resolution>"`
  형식(schema-extensions-kr.md §2b, Phillip A1 스토리가 CLI로 확립). 손상된 라인은 관용적으로 스킵.
- `.agent-team/10-review/findings-index.jsonl`(선택) — `finding_id -> category` 조인 뷰.
  **이 인덱스의 실 데이터는 Thomas(W5 코드 리뷰)가 채워야 한다**(schema-extensions §2a
  `resolution` 필드 실기록, risk-log C-3). 부재 시 전부 `uncategorized`로 정직하게 표시하며
  크래시하지 않는다(날조 금지).

## 분류 규칙 (모두 "(추정, 구현 시 조정)" — 실 데이터로 튜닝 필요)

| 상태 | 조건 | 동작 |
|------|------|------|
| `insufficient`(관측 부족) | 결정적 표본(fixed+dismissed) < 5 | 편집 보류, 기본 스타일 유지 |
| `conflict`(판단 유보) | \|확정률-기각률\| < 0.2 | 세분류 필요 안내, 억지 승격 금지 |
| `promote`(승격) | 확정률 ≥ 0.7 | style.md에 강조 반영 |
| `suppress`(억제) | 기각률 ≥ 0.7 | style.md에 디강조/제외 권고 |
| `neutral` | 그 외 | 변경 없음 |

콜드스타트(`finding.resolved` 이벤트 0건)는 에러가 아니라 "관측 부족" 안내로 처리한다(AC4).

## 출력 규약

- 접두 `[bathos review-learn]`, 흑백 기호(`✓`/`·`)만 사용(design-handoff §7 계승).
- 완료: `[bathos review-learn] ✓ 스타일 v3 저장: +2 승격 / -1 억제`
- 콜드스타트: `[bathos review-learn] · 관측 부족(콜드스타트) — 편집 보류`
- 변경 없음: `[bathos review-learn] · 변경 없음(임계 미달 카테고리뿐)`

## Boundaries (스코프 밖)

- `audit-log.jsonl`에 직접 쓰지 않는다(단일 writer는 `bathos audit append` 경유, 이 스킬은
  소비자일 뿐).
- 외부 SDK/네트워크(LangSmith 등) 의존 없음 — 전부 로컬 파일 집계(AC5).
- `style.md`를 통째로 재작성하지 않는다 — 마커 블록만 편집(재작성 시도는 버그로 간주).
- `findings-index.jsonl`의 실데이터 채굴/기록은 이 스킬의 책임이 아니다(Thomas/Phillip 인계).
- 표본 임계(5)·승격/억제 임계(0.7)·상충 마진(0.2)은 전부 **추정치**다. 실 데이터 누적 후
  Matthias/Thomas와 함께 재조정할 것(구현 시 확정이 아니라 관측 기반 튜닝 대상).

## 테스트

```bash
bash .claude/skills/review-style-continual/tests/run_tests.sh
```
합성 audit-log 픽스처로 9개 결정성 테스트(승격/억제/보류/유보/콜드스타트/조인 실패/손상
라인/재실행 편집/이력 스냅샷)를 검증한다. 외부 의존 없음(python3 표준 라이브러리만).
