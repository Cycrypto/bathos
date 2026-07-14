# BATHOS — 아키텍처 (기여자용)

> 엔진이 어떻게 짜였고 무엇을 보장하는지 보여 주는 기여자용 지도다. `core/`나 훅을 건드리기 전에 읽어 두면, 네 변경이 어떤 보장을 소리 없이 깨뜨리는 일을 막을 수 있다.
>
> **함께 보기:** [개념(FEATURES-kr)](FEATURES-kr.md) · [사용(USAGE-kr)](USAGE-kr.md) · English: [`ARCHITECTURE-en.md`](ARCHITECTURE-en.md) · Español: [`ARCHITECTURE-es.md`](ARCHITECTURE-es.md)

---

## 1. 두 실행면

BATHOS는 두 실행면으로 깔끔하게 갈린다. 코드를 읽기 전에 이 구분부터 몸에 익혀 두는 게 좋다.

| 면 | 무엇 | 위치 |
|----|------|------|
| 오케스트레이션 | 슬래시 커맨드·역할 정의·훅 (마크다운/bash) | `.claude/` |
| 엔진 | 단일 정적 Rust 바이너리 `bathos` | `core/` |

**신뢰·재현 가능해야 하는 것**은 엔진(단위 테스트)에, 사람이 편집하는 것은 오케스트레이션 면에 둔다. 훅·커맨드는 내부에서 `bathos <subcommand>`를 호출한다.

## 2. Rust 워크스페이스 (`core/`, 7 크레이트)

엔진은 7개 크레이트로 이뤄진 단일 Cargo 워크스페이스다 — 모듈 책임 하나마다 크레이트 하나씩이다.

| 크레이트 | 모듈 | 책임 |
|----------|------|------|
| `bathos-state` | M1 | SSOT: `manifest.json`(스키마 검증·원자적 쓰기) + tamper-evident 감사 해시체인 |
| `bathos-router` | M2 | Scale-Adaptive Lv0–4 라우팅(추천/확정) |
| `bathos-wave-engine` | M3 | 7웨이브 상태 전이; 동시성 캡 |
| `bathos-gate-engine` | M4 | PASS/CONCERNS/FAIL 판정; critical→FAIL |
| `bathos-story-engine` | M5 | 스토리 컴파일(D1/D2/D3) — zero-context-loss |
| `bathos-plug` | M12 | 플러그 모듈 매니저(module.yaml·트리거 DSL) |
| `bathos-cli` | bin | `bathos` 바이너리 — 서브커맨드 디스패치 |

의존 방향: `bathos-cli` → 엔진 크레이트 → `bathos-state`. **어떤 엔진 크레이트도 `bathos-plug`를 의존하지 않음**(불변식 A9).

## 3. 불변식 (깨지 말 것)

이들은 구조를 떠받치는 보장으로, 관습이 아니라 코드로 강제된다. 이 중 하나와 관련된 코드를 수정한다면, 그 불변식을 강제하는 지점이 계속 정직하게 동작하도록 유지하라.

| ID | 불변식 | 강제 주체 |
|----|--------|-----------|
| **A9** | 코어는 플러그 모듈을 의존하지 않는다 | 크레이트 의존 그래프(`cargo tree`로 검증) |
| 게이트 FACILITATOR | 판정의 `facilitator`는 비어 있을 수 없음; 근거 없는 자동 PASS 금지 | `bathos-gate-engine` |
| `critical > 0 → FAIL` | critical 이슈 하나라도 있으면 FAIL | `bathos-gate-engine` |
| 동시성 ≤ 3 | `MAX_CONCURRENT_ROLES = 3`; 4번째 스폰 거부 | `bathos-wave-engine` (`E-CONCURRENCY`) |
| 감사 체인 | `hash_prev[n] == hash_self[n-1]`, genesis 앵커, 단조 `seq`, 단일 writer | `bathos-state::audit` (`bathos audit verify`) |
| 원자적 쓰기 | manifest가 반쯤 쓰인 채 남지 않음 | `bathos-state::store` |
| 스토리 완전성(D1) | 필수 6섹션 + 비어있지 않은 `developer_context` | `bathos-story-engine` (`E-CTX-LOSS`) |
| W3 FAIL이 W5 차단 | FAIL 판정이 구현 진입을 물리 차단 | `gate-enforce.sh` (exit 2) |

## 4. 종료 코드 & 오류 분류

오류는 두 방식으로 드러난다 — 훅이 분기 판단에 쓰는 프로세스 종료 코드, 그리고 실패에 이름을 붙이는 상징적 E-code다.

- **종료 코드:** `0` 성공 · `1` 오류 · **`2` 게이트 FAIL**(훅이 차단에 사용).
- **E-codes:** `E-LEVEL-DRIFT`, `E-CONCURRENCY`, `E-CTX-LOSS`, `E-STALE`, `E-STATE-CORRUPT`, `E-AUDIT-TAMPER`, `E-PLUG-NOTFOUND`.

## 5. 라우터 점수 (결정적)

라우터는 stakes 프로필을 순수 산술만으로 레벨로 환산한다. 휴리스틱이 없으니 같은 입력이면 추천도 늘 같다.

`scope`(0–3: bug=0, feature=1, large/module=2, product/platform=3, 불명=1) + `novelty`(+1) + `regulation_ip`(+2) + `team_size`(0–2: solo=0, medium=1, large=2). 합계 → 레벨: 0→Lv0, 1→Lv1, 2–3→Lv2, 4–5→Lv3, 6+→Lv4. 추천과 확정 분리(User Sovereignty).

## 6. 훅 (`.claude/hooks/`)

`settings.json`이 Claude Code 이벤트에 바인딩한 6종 fail-safe 훅: `careful-guard`·`freeze-guard`·`audit-log`·`artifact-verify`·`gate-enforce`·`next-action`. `$BATHOS_BIN`으로 엔진 호출. 자체 검증: `bash .claude/hooks/_test-hooks.sh`(46 결정성 테스트). **`hooks` 블록엔 유효 이벤트명만** — 주석키가 섞이면 서브에이전트 시작이 무한 대기에 빠진다(`bathos doctor`가 탐지).

## 7. 핵심 ADR

- **ADR-0006:** 코어 엔진=Rust(단일 정적 바이너리); 훅=bash; 커맨드/역할/자산=마크다운; 설정=JSON/YAML.
- 전체 ADR은 설계 기록(`.agent-team/04-architecture/adr/`)에 있다.

## 8. 엔진 기여 규칙

1. 코어는 슬림하게 유지한다 — 새 도메인 기능은 `modules/`에 두고, 코어에 넣지 않는다(A9).
2. `cd core && cargo test && cargo clippy --all-targets -- -D warnings`가 그린이어야 하며, 불변식 테스트를 추가한다.
3. 커밋 전 `cargo fmt --all`을 실행한다.
4. 훅 변경 시 `bash .claude/hooks/_test-hooks.sh`를 실행하고, 결정성과 fail-safe를 유지한다.
5. CI(`.github/workflows/ci.yml`): fmt · clippy(-D) · test · release · jq · 훅 하네스.

---

<div align="center"><a href="ARCHITECTURE-en.md">ARCHITECTURE-en</a> · Español: <a href="ARCHITECTURE-es.md">ARCHITECTURE-es</a></div>
