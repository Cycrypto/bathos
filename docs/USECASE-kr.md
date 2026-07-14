# BATHOS 활용 사례 — 새 서비스를 처음부터 만들기, 단계별로

> **이 문서의 목표:** BATHOS로 *실제 새 서비스를 처음부터* 만드는 과정을 가능한 한 쉽게 보여준다. 설치는 **"내 프로젝트에 채택하는"** 방식(B)을 사용한다:
>
> ```bash
> # B) 엔진 빌드 + 메서드 패키지를 내 프로젝트로 복사
> ./install.sh --into /abs/path/to/your-project
> ```
>
> 하나의 구체적 예시를 끝까지 따라간다: **"ReadShelf"** — 읽은 책을 기록하고 짧은 리뷰를 친구와 공유하는 작은 웹앱. 다 읽고 나면 *무엇을 입력하고, BATHOS가 각 단계에서 무엇을 하며, 디스크에 어떤 파일이 생기는지* 정확히 알게 된다.
>
> **함께 보기:** 원리가 궁금하면 [`FEATURES-kr.md`](FEATURES-kr.md)로 개념을 잡고 → 상세 CLI·훅은 [`USAGE-kr.md`](USAGE-kr.md) 레퍼런스에서 확인하면 된다. · English: [`USECASE-en.md`](USECASE-en.md) · Español: [`USECASE-es.md`](USECASE-es.md)

---

## 0. 30초 멘탈 모델

BATHOS는 **실행하는 앱이 아니다** — **Claude Code 위에서 도는 메서드 패키지**다. 당신은:

1. **내 프로젝트에 설치한다** (`.claude/` + `assets/` + `modules/` 복사, 엔진 경로 환경변수 지정).
2. **내 프로젝트에서 Claude Code를 열고** **슬래시 커맨드**를 입력한다(`/team-kickoff`, `/route`, `/wave1-discovery`, …).
3. BATHOS가 웨이브 단위로 전문 "팀원"을 띄우고, 작업 산출물은 `.agent-team/`에 쓰며, **실제 제품 코드는 평소 위치 — `src/`** 에 쌓인다.

이게 전부다. 이 문서의 나머지는 이 세 가지를 실제 예시로 천천히 해보는 것뿐이다.

---

## 1. 예시 서비스: "ReadShelf"

| | |
|---|---|
| **무엇** | 웹앱: 읽은 책 기록 → 짧은 리뷰 작성 → 친구 팔로우 → 친구들의 독서 피드 보기. |
| **누가** | 1인 개발자(당신). |
| **스택(계획)** | Next.js(프론트) · Node/Express API · PostgreSQL. MVP에는 AI/ML 없음. |
| **포부** | 거대 플랫폼이 아니라 우선 **집중된 MVP**부터. |

기억해 두자: ReadShelf는 *새롭지만 집중된* 제품이다. 이 선택이 4단계에서 BATHOS "레벨"을 고를 때 중요해진다.

---

## 2. 사전 준비 (1회)

- **Claude Code v2.1.32+** + 실험 기능 **Agent Teams**.
- **Rust 툴체인**(cargo 1.92+) — 엔진을 한 번 빌드할 때만 필요하다.
- **`jq`** — 안전 훅이 사용한다.
- macOS/Linux 셸.

BATHOS 패키지 자체도 디스크 어딘가에 있어야 한다. 여기서는 `~/tools/bathos`에 클론했다고 가정한다.

---

## 3. 1단계 — 내 프로젝트에 BATHOS 설치 ("B" 방식)

빈 프로젝트 폴더를 만들고, **BATHOS 레포에서** `--into`가 내 프로젝트를 가리키도록 설치 스크립트를 실행한다:

```bash
# 새 프로젝트 위치(비어 있어도 되고, 기존 레포여도 됨)
mkdir -p ~/projects/readshelf

# BATHOS 패키지에서:
cd ~/tools/bathos
./install.sh --into ~/projects/readshelf
```

이 명령이 하는 일(그리고 **하지 않는** 일):

- 엔진을 1회 빌드 → `~/tools/bathos/core/target/release/bathos` (~5.6MB).
- **3개 폴더**를 `~/projects/readshelf/`로 복사:
  - `.claude/` — 슬래시 커맨드, 역할 17개 정의, 안전 훅 6종, `settings.json`
  - `assets/` — Wave 3·플러그가 쓰는 템플릿/워크플로우/체크리스트
  - `modules/` — 선택 플러그 모듈(`ip-pack`, `research-pack`)
- **절대 아무것도 삭제하지 않으며**, 기존 `.claude/`가 있으면 `--force` 없이는 덮어쓰지 않는다.
- 컴파일된 바이너리는 프로젝트로 **복사하지 않는다.** 엔진은 BATHOS 레포에 그대로 두고, 내 프로젝트는 그것을 가리키기만 한다(다음 단계).

이제 환경변수 두 개를 연결한다(설치 스크립트가 안내를 출력해 준다):

```bash
# 훅이 엔진을 찾도록:
export BATHOS_BIN="$HOME/tools/bathos/core/target/release/bathos"

# Agent Teams 활성화(복사된 settings.json에도 이미 설정돼 있지만 명시):
export CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS=1
```

> 위 두 `export` 줄을 셸 프로필에 넣어두면 매 세션마다 적용된다.

이제 프로젝트는 이렇게 보인다:

```
~/projects/readshelf/
├── .claude/        ← (방금 설치) 커맨드 · 역할 · 훅 · settings
├── assets/         ← (방금 설치) 템플릿 · 워크플로우 · 체크리스트
└── modules/        ← (방금 설치) ip-pack · research-pack
```

아직 그 외엔 아무것도 없다 — `.agent-team/`도 `src/`도 없다. 다음 단계에서 생긴다.

---

## 4. 2단계 — Claude Code 열고 킥오프

내 프로젝트 디렉터리(`~/projects/readshelf`)에서 Claude Code 세션을 연다. 이 세션이 **리드 "Paul"** 이다. 그리고:

```text
/team-kickoff
```

**무슨 일이 일어나나:** BATHOS가 작업 영역과 단일 진실 원천을 만든다.

**디스크에 생기는 것:**

```
~/projects/readshelf/
├── .claude/  assets/  modules/        (설치본)
└── .agent-team/                       ← 신규
    ├── 00-plan/        (charter, task graph)
    ├── 01-reverse/ … 12-report/       (역할별 산출 폴더)
    └── _state/
        └── manifest.json              ← SSOT: 전 프로젝트 상태, 스키마 검증
```

> **두 디렉터리, 두 목적.** `.agent-team/`은 BATHOS의 *작업 노트*(기획·설계·리뷰·리포트·상태). 당신의 *실제 서비스 코드*는 `src/`에 쌓인다. 둘은 섞이지 않는다.

---

## 5. 3단계 — `/route`로 레벨 정하기 (당신이 결정)

BATHOS는 모든 작업에 모든 웨이브를 돌리지 않는다. 노력을 작업 규모에 맞춘다. ReadShelf의 "stakes"를 BATHOS에 알려 준다:

```text
/route /Users/you/projects/readshelf
```

내부적으로 엔진이 네 축에 점수를 매긴다:

| 축 | ReadShelf 답 | 이유 |
|----|-------------|------|
| `scope` | `feature` | 집중된 MVP, 거대 플랫폼 아님 → 점수 1 |
| `novelty` | `true` | 새로운 제품 아이디어 → +1 |
| `regulation_ip` | `false` | 규제·특허 핵심 도메인 아님 → +0 |
| `team_size` | `solo` | 혼자 → +0 |

합계 = **2 → 추천 Level 2.** 엔진은 이런 결과를 돌려준다:

```json
{ "recommended_level": 2, "wave_set": ["W1","W2","W3","W5","W6"],
  "requires_confirmation": true }
```

> **이것은 당신의 결정이다(User Sovereignty).** BATHOS는 *추천만* 한다. 당신이 확정해야 하고, 확정만 기록된다:
>
> ```bash
> bathos -s .agent-team/_state route decide \
>   --stakes-json '{"scope":"feature","novelty":true,"regulation_ip":false,"team_size":"solo"}' \
>   --confirm 2
> ```
>
> 이제 `manifest.json → current_level = 2`.

**Level 2가 ReadShelf에 의미하는 것** — 다음 웨이브가 순서대로 돈다:

```
W1 디스커버리 → W2 설계 → W3 스토리 게이트 ★ → W5 구현 → W6 검증
```

(Level 0이면 버그 수정만, Level 3–4면 W0 시장분석과 W4 IP/연구가 추가된다 — 집중 MVP에는 과하다. ReadShelf가 나중에 큰 멀티모듈 플랫폼으로 커지면 Lv3로 다시 라우팅하면 된다.)

---

## 6. 4단계 — Wave 1: 디스커버리 & 시장 (무엇을 만들까?)

```text
/wave1-discovery /Users/you/projects/readshelf
```

**누가 일하나:** 여기선 Caleb(시장 분석가)이 주역. John(리버스 전문가)은 *참조* 코드베이스를 학습용으로 줄 때만 의미가 있다 — 그린필드 앱이면 생략하거나, 경쟁 서비스의 오픈소스 레포를 줘도 된다.

**무엇을 하나:** Caleb이 유사 독서/소셜 앱을 조사하고, 빈틈을 찾고, **USP**(ReadShelf를 쓸 이유)를 제안한다.

**무엇을 얻나:** `.agent-team/02-market-analysis/` — 경쟁 지형, USP 추천.

**게이트:** *USP Readiness.* 다음으로 넘어가기 전에 USP가 말이 되는지 당신(리드)이 확인한다.

> 각 웨이브 끝에서 작업 팀원은 다음 웨이브 전에 shutdown된다. 동시 활성을 ≤3으로 유지하면 토큰이 절약된다.

---

## 7. 5단계 — Wave 2: 기획·아키텍처·디자인

```text
/wave2-design /Users/you/projects/readshelf
```

이 웨이브는 의도된 순서로 돈다:

1. **Joshua(서비스 기획)** 가 USP를 구체적 **Core Feature + User Story + Service Story**로 바꾼다. *그의 산출물이 곧 게이트다* — 기획이 단단해질 때까지 나머지는 시작하지 않는다.
2. 그 다음 병렬로:
   - **James(아키텍트)** 가 데이터 모델(ERD)·API 계약·서비스 시퀀스·예외 처리를 설계한다 — 예: `users`, `books`, `reviews`, `follows` 테이블; `POST /reviews`, `GET /feed` 등.
   - **Jonnathan(디자이너)** 이 UX 흐름과 UI를 설계한다 — 회원가입, 책장(shelf), 리뷰 작성기, 친구 피드.

**무엇을 얻나:** `.agent-team/03-service-planning/`, `.agent-team/04-architecture/`, `.agent-team/07-design/`.

**게이트:** *Plan Readiness.*

> 선택 강화: 기획을 락인하기 전에 `/plan-ceo-review`(이게 10점짜리 제품인가?)나 `/plan-eng-review`(아키텍처/엣지케이스/테스트) 같은 리뷰 게이트를 돌릴 수 있다. 한 번에 한 리뷰어씩.

---

## 8. 6단계 — Wave 3: 스토리 게이트 ★ (BATHOS의 심장)

```text
/wave3-story-gate /Users/you/projects/readshelf
```

BATHOS를 다르게 만드는 단계다. 대부분의 "AI가 내 앱을 만들어줘" 시도가 무너지는 지점이 바로 **설계 → 구현**이다: 만드는 쪽이 설계의 절반을 잊거나 애초에 보지 못한다. Wave 3가 이 틈을 닫는다.

**누가 일하나:** **Matthew(#17, 스토리 엔지니어)** + *독립* 검증자 **Thomas**, **Matthias**.

**Matthew가 하는 일:** W2 설계 전체를 `.agent-team/03-story-engineering/` 아래 **자족 dev 스토리파일**로 응축한다. 각 스토리(예: `story-1-2-write-review-kr.md`)는 구현자가 *스토리만 보고도* 착수할 수 있게 쓰인다. 엔진이 강제하는 것:

- **6개 필수 섹션**이 있어야 하고 `developer_context`는 비어 있으면 안 된다(`story_requirements`, `developer_context`, `architecture_compliance`, `library_framework_requirements`, `file_structure_requirements`, `testing_requirements`). 하나라도 누락 → **`E-CTX-LOSS`** 로 컴파일 실패.
- 모든 기술 주장에 **`[Source: …]`** 표기 → 아키텍처/디자인 문서로 추적.

**게이트(여기가 중요):** Thomas와 Matthias가 스토리를 독립 리뷰하고 게이트는 **PASS / CONCERNS / FAIL**을 돌려준다:

- **PASS** → 구현으로 진행.
- **CONCERNS** → 진행하되 리스크 기록.
- **FAIL** → 훅(`gate-enforce`)이 **Wave 5 시작을 물리적으로 차단**한다(엔진이 exit code 2). 실패한 readiness 게이트 위에서는 말 그대로 구현을 시작할 수 없다. 스토리를 고치고 재게이트하면 된다.

> "생성 ≠ 검증"의 물리적 구현이다: 같은 모델이 자기 작업을 슬쩍 통과시킬 수 없다.

---

## 9. 7단계 — Wave 5: 구현 (실제 코드 작성)

```text
/wave5-implement /Users/you/projects/readshelf
```

**누가 일하나 (ReadShelf에 필요한 역할만):**

- **Phillip(백엔드)** — Express API + PostgreSQL 스키마/마이그레이션, 인증, `/reviews`·`/feed` 엔드포인트.
- **Andrew(프론트/모바일)** — Jonnathan 디자인 기반 Next.js UI를 James의 API 계약에 연결.
- *(AI/ML 수석 Stephen은 **스폰하지 않음** — ReadShelf MVP에 ML 없음.)*

**코드가 가는 곳:** 실제 소스 트리 — `~/projects/readshelf/src/`(그리고 `api/`, `web/` 등 프로젝트 코드 위치). 각 팀원은 **자기 소유 경로만** 편집하므로 백엔드·프론트 작업이 충돌하지 않는다.

```
~/projects/readshelf/
├── src/  api/  web/ …      ← 실제 코드가 스토리 단위로 여기 생김
└── .agent-team/08-impl-notes/   ← 구현 노트(backend.md, frontend.md)
```

**게이트:** 스토리 단위 완료를 검증한 뒤 해당 스토리를 done으로 간주한다.

> 안전 훅은 내내 활성 상태다: `careful-guard`가 파괴적 셸 명령(`rm -rf`, `DROP TABLE` …)을 차단하고, `freeze-guard`가 각 팀원을 자기 소유 경로 안에 가둔다.

---

## 10. 8단계 — Wave 6: 검증·문서·리포트

```text
/wave6-verify-report /Users/you/projects/readshelf
```

**누가 일하나:** **Thomas**(코드 리뷰), **Timothy**(개발 문서), **Matthias**(QA / E2E 테스트) — 병렬 — 그 다음 **Martin**이 단일 **HTML 리포트**를 취합한다.

**무엇을 얻나:** `.agent-team/10-review/`, `.agent-team/09-docs/`, `.agent-team/11-qa/`, `.agent-team/12-report/report.html`.

**게이트:** *Release Readiness*(PASS / CONCERNS / FAIL). 독립 리뷰가 차단성 결함을 찾으면 보완하고 재게이트한다 — BATHOS 자신이 거쳐온 바로 그 루프다.

---

## 11. 9단계 — 최종 확정

```text
/team-confirm
```

리드가 사인오프하고 팀원을 정리한다. 이제 당신이 갖게 되는 것:

```
~/projects/readshelf/
├── src/ api/ web/ …                  ← 동작하는 ReadShelf 서비스
└── .agent-team/
    ├── 02-market-analysis/  (USP)
    ├── 03-service-planning/ (기능 + 스토리)
    ├── 04-architecture/     (ERD, API, 시퀀스)
    ├── 03-story-engineering/(dev 스토리파일)
    ├── 07-design/           (UX/UI)
    ├── 08-impl-notes/       (어떻게 만들었는가)
    ├── 10-review/ 11-qa/    (독립 검증)
    ├── 12-report/report.html
    └── _state/              (manifest.json, wave-log.md, signoff.md)
```

모든 것이 추적 가능하다: 시장 → USP → 기능 → 아키텍처 → 스토리 → 코드 → 검증.

---

## 12. 전체 흐름 한 페이지

```text
# 1회
cd ~/tools/bathos && ./install.sh --into ~/projects/readshelf
export BATHOS_BIN="$HOME/tools/bathos/core/target/release/bathos"
export CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS=1

# ~/projects/readshelf 에서 연 Claude Code 안에서:
/team-kickoff                                   # .agent-team + manifest 골격
/route          /Users/you/projects/readshelf   # → Lv2 추천, 당신이 --confirm 2
/wave1-discovery   /Users/you/projects/readshelf # Caleb: 시장 + USP
/wave2-design      /Users/you/projects/readshelf # Joshua → James + Jonnathan
/wave3-story-gate  /Users/you/projects/readshelf # ★ 스토리 + readiness 게이트 (FAIL이면 구현 차단)
/wave5-implement   /Users/you/projects/readshelf # Phillip + Andrew → src/ 에 코드
/wave6-verify-report /Users/you/projects/readshelf # 리뷰 + QA + 문서 → report.html
/team-confirm                                   # 사인오프 + 정리

# 언제든 진행 점검:
/team-status

# 멈추기 전 전부 저장 — 다음 세션에서 재개:
/save            # 또는 그냥 "저장" / "체크포인트"
/resume          # 다음 세션: 또는 그냥 "이어서" / "불러와"
```

---

## 13. 팁 & 흔한 함정

- **멈추기 전 저장, 돌아오면 재개.** ReadShelf 같은 실제 빌드는 여러 세션에 걸친다. 매 세션을 **`/save`** 로 끝내자(한 단어 — 엔진 상태·git·결정·진행 중 웨이브·다음 커맨드를 `_state/SESSION-SNAPSHOT.md`에 담음). 다음 세션은 **`/resume`** 으로 시작. 자연어로도 동작한다: "저장"/"save", "이어서"/"resume". 단, 재개 시 팀원은 되살아나지 않으니 해당 `/waveN-…` 커맨드를 다시 실행하면 된다(디스크 산출물로 무손실).
- **팀원은 동시 ≤ 3.** 토큰 비용은 활성 역할 수에 비례. BATHOS가 이미 웨이브를 시퀀싱하니 전부 한꺼번에 돌리려 하지 말 것.
- **필요한 역할만 스폰.** ML 없으면 Stephen 생략. 그린필드면 John(리버스) 생략하거나 참조 레포를 줄 것.
- **결정자는 당신이다.** 레벨·USP·기획·게이트 판정 — BATHOS는 추천하고, 당신이 확정한다. BATHOS가 방향 전환을 권할 때는 "추천 + 근거 + 놓친 맥락"을 제시해야 하며, 스스로 실행해서는 안 된다.
- **FAIL은 정말로 구현을 멈춘다.** `/wave5-implement`가 시작되지 않으면 Wave 3 판정을 확인한다: `bathos -s .agent-team/_state gate show`. 스토리 고치고 재게이트.
- **`settings.json`의 `hooks` 블록에 주석 키 금지** — 팀원 시작 시 무한 대기 유발.
- **인수인계는 디스크로만.** 팀원은 리드의 대화 히스토리를 공유하지 않으며, 모든 것은 `.agent-team/` 파일로 전달된다. 이것은 한계가 아니라 기능(zero context loss)이다.
- **팀원이 무출력으로 "멈춘 듯" 보이면** 대개 계정 사용량 한도 때문이지 버그 때문이 아니다 — 리셋 후 재스폰하면 되고, 디스크 산출물은 보존된다.

---

## 14. ReadShelf가 더 크다면?

| ReadShelf가… | stakes 변화 | 레벨 | 추가 웨이브 |
|--------------|-------------|------|-------------|
| 한 줄 버그 수정 | scope=bug | **Lv0** | W5만 (+경량 W6) |
| 집중 MVP *(이 문서)* | scope=feature, novelty | **Lv2** | W1+W2+W3+W5+W6 |
| 본격 신규 제품/플랫폼 | scope=product, team=large | **Lv3** | **W0**(분석) + 선택 **W4**(IP/연구) 추가 |
| 규제·특허 중심 제품 | + regulation_ip=true | **Lv4** | W0–W6 전체 + **W4 필수** |

플러그를 켜려면(예: Lv3+ 제품의 특허 초안):

```bash
bathos -s .agent-team/_state --modules-dir modules plug enable ip
```

---

## 15. 라이선스

BATHOS는 MIT 라이선스이며, [BMAD-METHOD](https://github.com/bmad-code-org/BMAD-METHOD)를 정밀 역분석한 뒤 제1원리에서 독립 재구현한 것이다 — 그 근간이 된 선행 작업에 경의를 표하며, 그 상표는 사용하지 않는다. 전문: [`../README.md`](../README.md).

---

<div align="center">

**BATHOS** · βάθος — 표층이 아닌 깊이
[`USECASE-en.md`](USECASE-en.md) · Español [`USECASE-es.md`](USECASE-es.md)

</div>
