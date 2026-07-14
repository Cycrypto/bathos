# BATHOS — 커스텀 플러그 모듈 작성 가이드

> **코어를 건드리지 않고** 나만의 도메인 기능(예: 보안 감사 팩, 게임 기획 팩)으로 BATHOS를 확장하고 싶은가. 바로 그럴 때 쓰는 것이 플러그 모듈이다. 이 가이드는 모듈을 작성하고 → 설치하고 → 활성화하고 → 실행하는 과정을 처음부터 끝까지 따라간다.
>
> **함께 보기:** [개념(FEATURES-kr)](FEATURES-kr.md) · [사용(USAGE-kr)](USAGE-kr.md) · English: [`MODULE-GUIDE-en.md`](MODULE-GUIDE-en.md) · Español: [`MODULE-GUIDE-es.md`](MODULE-GUIDE-es.md)

---

## 0. 모듈이란 (그리고 단 하나의 규칙)

BATHOS **코어는 슬림하게** 유지된다. 도메인 특화 기능은 `modules/` 아래 **opt-in 플러그 모듈**로 제공한다. 기본 동봉 2종: `ip-pack`(특허 출원명세) · `research-pack`(학술 Abstract/Intro).

> **단 하나의 규칙(불변식 A9): 코어는 모듈을 절대 의존하지 않는다.** 모듈은 런타임에 탐색·토글되며, 엔진은 특정 모듈에 대한 컴파일타임 지식이 0이다. 이것이 코어를 작게 유지하고 도메인을 자유롭게 추가하게 한다.

모듈은 **Wave 4(IP·연구)** — 선택·비본류 플러그 웨이브 — 에 워크플로우·템플릿·산출 디렉터리를 선언해 기여한다. `bathos plug enable <id>`로 켜며, 상태는 `manifest.modules[]`에 영속된다.

---

## 1. 모듈의 구조

모듈은 결국 `modules/<your-id>/` 아래의 디렉터리 하나일 뿐이다 — 목표로 삼을 구조는 이렇다:

```
modules/security-pack/
├── module.yaml            # 필수 — 모듈 자기 선언(계약)
├── README.md              # 권장 — 무엇인지, 어떻게 쓰는지
├── workflows/             # 모듈이 제공하는 절차
│   └── threat-model.md
├── templates/             # 워크플로우가 채우는 산출 템플릿
│   └── threat-model-report.md
└── checklists/            # (선택) 품질·적대적 체크리스트
    └── threat-model-quality.md
```

엔진이 모듈을 **탐색**하는 데 엄밀히 필요한 것은 `module.yaml` 하나뿐이고, 나머지는 그 모듈을 실행하는 역할이 *유용하게 쓰도록* 돕는 요소다.

---

## 2. `module.yaml` 계약

`module.yaml`은 모듈이 엔진에게 자신을 소개하는 자리다. 아래는 엔진(`bathos-plug`)이 실제로 파싱하는 정확한 스키마이며, 모든 필드에 주석을 달아 두었다:

```yaml
module_id: security               # 필수 — 고유 id (ip | research | game | security ...)
name: Security Pack               # 필수 — 사람이 읽는 이름
wave: W4                          # 필수 — 플러그되는 웨이브 (W4)
trigger: "Lv>=4 OR domain=security"  # 필수 — 자동 트리거 조건 (아래 DSL)
enabled_default: false            # 선택 (기본 false)
provides:                         # 선택
  workflows: [threat-model]       #   workflows/ 아래 파일 basename
  templates: [threat-model-report]#   templates/ 아래 파일 basename
outputs: ".agent-team/13-security/"  # 선택 — 이 모듈이 산출물을 쓰는 위치
evidence_trace: true              # 선택 (기본 false) — [Source:] 근거 추적 요구
```

| 필드 | 필수? | 의미 |
|------|:-----:|------|
| `module_id` | ✓ | `plug enable/disable <id>`·트리거(`domain=<id>`)에 쓰는 고유 id |
| `name` | ✓ | `plug list`에 표시되는 이름 |
| `wave` | ✓ | 모듈이 도는 웨이브 (현재 `W4`) |
| `trigger` | ✓ | 자동 트리거 조건 (§3) |
| `enabled_default` | — | 기본 활성 여부 (기본 `false`) |
| `provides.workflows` | — | 제공 워크플로우 basename |
| `provides.templates` | — | 제공 템플릿 basename |
| `outputs` | — | 산출 디렉터리 (새 `.agent-team/NN-<name>/` 선택) |
| `evidence_trace` | — | `true`면 주장에 `[Source:]` 표기 요구(권장) |

> `outputs`는 기존 역할 디렉터리(`00`–`12`는 사용 중)와 겹치지 않는 새 번호(예: `13-security/`)를 고를 것.

---

## 3. 트리거 DSL

`trigger`는 라우터에게 이 모듈을 *언제* 자동 활성화할지 알려준다. 문법(`bathos-plug` 파싱):

- **레벨:** `Lv>=N` · `Lv>N` · `Lv<=N` · `Lv<N` · `Lv=N` (N = 0–4)
- **도메인:** `domain=X` (또는 `domain:X`) — 프로젝트 도메인이 `X`와 일치하면 참 (보통 `module_id`)
- **결합:** ` OR `로 항을 연결 — **하나라도** 참이면 트리거.
- **안전:** 해석 불가 토큰은 `false`(보수적).

예:
- `"Lv>=3 OR domain=ip"` — 대형/엔터프라이즈 또는 도메인이 명시적으로 IP일 때 on.
- `"Lv>=4 OR domain=security"` — 엔터프라이즈/규제 빌드 또는 명시적 security 도메인에만 on.
- `"domain=game"` — 프로젝트 도메인이 `game`일 때만 on(레벨만으로는 자동 on 안 됨).

> 트리거는 *자동 활성화 힌트*다. 사용자는 항상 통제권을 갖는다(User Sovereignty): 무관하게 `plug enable`/`disable` 가능하며, 실제로 무엇이 도는지 확인한다.

---

## 4. 단계별 — "security-pack" 만들기

이론은 이쯤 하고, 직접 하나 만들어 보자. `security-pack`을 맨바닥부터 파일 하나씩 쌓아 가며 만든다.

### 4.1 디렉터리 스캐폴드
```bash
cd your-project    # (또는 bathos 레포)
mkdir -p modules/security-pack/{workflows,templates,checklists}
```

### 4.2 `module.yaml` 작성
```yaml
module_id: security
name: Security Pack
wave: W4
trigger: "Lv>=4 OR domain=security"
enabled_default: false
provides:
  workflows: [threat-model]
  templates: [threat-model-report]
outputs: ".agent-team/13-security/"
evidence_trace: true
```

### 4.3 워크플로우 작성 (`workflows/threat-model.md`)
워크플로우는 역할이 따르는 마크다운 절차다. 구체적·단계 기반으로 — 보안 팩이면 예컨대 OWASP Top 10 + STRIDE 패스로 템플릿을 채운다. 읽을 입력과 산출 경로를 명시하고, (`evidence_trace: true`이므로) 모든 발견에 `[Source:]` 표기가 필요함을 밝힌다.

### 4.4 템플릿 작성 (`templates/threat-model-report.md`)
워크플로우가 채우는 구조화된 산출물로, 범위·자산·위협(STRIDE별)·심각도·완화책·잔여 리스크 헤딩을 갖춘다. 이것이 `outputs` 아래에 쌓인다.

### 4.5 (선택) 품질 체크리스트 (`checklists/threat-model-quality.md`)
역할이 워크플로우 완료 선언 전에 돌리는 적대적 자체 검토 체크리스트(`ip-pack/checklists/patent-quality.md` 참고).

### 4.6 `README.md` 추가
한 화면 분량으로 무엇을 산출하는지, 트리거는 무엇인지, 어떻게 활성화하는지를 적는다.

---

## 5. 설치·탐색·활성화

파일이 제자리에 놓였다면, 세 개의 커맨드가 모듈을 '디스크에 있는 상태'에서 '매니페스트에 살아 있는 상태'로 옮긴다:

```bash
# 1) 탐색 — 엔진이 modules/<id>/module.yaml을 읽음
bathos --modules-dir modules plug list
#   → security-pack이 활성 상태와 함께 나열됨(JSON)

# 2) 활성화 — manifest.modules[]에 영속
bathos -s .agent-team/_state --modules-dir modules plug enable security

# 3) 완료 후 비활성화
bathos -s .agent-team/_state --modules-dir modules plug disable security
#   없는 id → exit 1 (E-PLUG-NOTFOUND)
```

`install.sh --into`로 BATHOS를 내 프로젝트에 채택했다면, 모듈을 그 프로젝트의 `modules/`(=`--modules-dir`가 가리키는 디렉터리) 아래에 둔다.

---

## 6. 웨이브에서 실행

모듈은 **W4**에 플러그된다. 활성화 후 W4 커맨드를 실행하면, 담당 역할(동봉 팩은 Mark/Nathanael, 자체 팩은 가장 적합한 역할을 배정하거나 리드가 직접 실행한다)이 워크플로우를 수행하고 `outputs` 디렉터리에 쓴다:

```
/wave4-ip-research /abs/path/to/project
```

W4는 본류(W0→W1→W2→W3→W5→W6) 밖이므로 W2 이후 언제든 실행 가능 — 토큰이 빠듯하면 뒤로 미뤄도 된다.

---

## 7. 모듈 검증

믿고 쓰기 전에, 아래 네 가지 빠른 점검을 거친다 — 각 점검은 파싱부터 트리거까지 이어지는 사슬의 고리 하나씩을 확인한다:

- **파싱 확인:** `bathos --modules-dir modules plug list` — 모듈이 보이면 `module.yaml` 파싱 성공. 안 보이면 필수 4필드(`module_id`, `name`, `wave`, `trigger`)와 YAML 문법 확인.
- **활성화/영속 확인:** `plug enable <id>` 후 `bathos -s _state state show` — `modules[]`에 id 확인.
- **트리거 확인:** 프로젝트 레벨/도메인을 설정하고, `trigger`가 발동해야 할 때 모듈이 자동 추천되는지 확인.
- **Doctor:** `bathos doctor`가 여전히 통과(모듈이 코어를 안 건드림 — A9).

---

## 8. 좋은 습관

아래 습관들을 지키면 모듈이 깔끔하게 남고, 서로 조합하기 좋으며, BATHOS 에토스에도 충실해진다:

- **코어는 그대로.** 모든 것은 `modules/<id>/` 아래. 모듈을 위해 `core/` 크레이트를 편집하고 있다면 멈춰라 — A9 위반이다.
- **겹치지 않는 `outputs` 경로**(`13-…`, `14-…`)를 골라라.
- **`evidence_trace`를 켜고** 주장에 `[Source:]`를 요구하라 — "생성 ≠ 검증" 에토스에 부합한다.
- **체크리스트를 작성**해 역할이 완료 전 적대적 자체 검토를 하게 하라.
- **동봉 팩을 본떠라** — `modules/ip-pack/`이 레퍼런스 구현(module.yaml + workflows + templates + checklists + README).

---

## 9. 라이선스

MIT. [`../README.md`](../README.md) 참조.

---

<div align="center">

**BATHOS** · βάθος — 표층이 아닌 깊이
[`MODULE-GUIDE-en.md`](MODULE-GUIDE-en.md) · Español: [`MODULE-GUIDE-es.md`](MODULE-GUIDE-es.md)

</div>
