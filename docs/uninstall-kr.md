# BATHOS Dynamis — 제거(Uninstall) 가이드

> 작성: Andrew(#9, 배포) · 2026-07-08 · CF-B4 / SS11 (Should)
> 스크립트: `scripts/uninstall.sh` (Bash · dry-run 기본)

## 1. 실행순서 계약 (반드시 지킬 것)

> **호스트(Claude Code 등)의 플러그인 제거 명령보다 먼저 이 스크립트를 실행하세요.**

이유: `scripts/uninstall.sh`는 플러그인 폴더 **안**에 있는 파일입니다. 호스트의
"플러그인 제거"는 플러그인 폴더 전체를 지우므로, 먼저 지워버리면 이 스크립트
자체가 함께 사라져 실행할 수 없게 됩니다(ponytail 실전 확인 사실). 순서를
반드시 지키세요:

```
1) scripts/uninstall.sh --apply        # 외부 상태(설정/상태줄/플래그) 정리
2) (호스트) 플러그인 제거 명령 실행       # 플러그인 폴더 자체 삭제
```

## 2. 무엇을 정리하는가

플러그인 폴더 **밖**의 외부 상태만 정리합니다(플러그인 폴더 자체는 호스트가 지웁니다):

| 대상 | 위치 | 비고 |
|------|------|------|
| 세션 플래그(plan/intensity) | `_state/session-flags.json` | Phillip A4/A5 소유 파일 형식 |
| 상태줄 등록 | `~/.claude/settings.json`의 `statusLine` 키 | **bathos 관련 엔트리만** 정밀 제거(`.bak` 백업 생성), 무관한 설정은 건드리지 않음 |
| 전역 설정 잔여물 | `~/.config/bathos/*` (존재 시) | |

**절대 건드리지 않는 것**(SSOT 보호): `_state/manifest.json`(프로젝트 SSOT), `_state/audit-log.jsonl`(append-only 감사 이력).

## 3. 사용법

```bash
scripts/uninstall.sh                # 기본값 = dry-run: 계획만 출력, 아무것도 지우지 않음
scripts/uninstall.sh --apply         # 실제 삭제(대화형 y/N 확인, 기본 N)
scripts/uninstall.sh --apply --yes   # 확인 없이 즉시 삭제(비대화형/CI용)
```

출력 예:

```
[bathos uninstall] ! 되돌릴 수 없음 — 호스트의 플러그인 제거 명령보다 먼저 이 스크립트를 실행하세요.
[bathos uninstall] 삭제 계획 (2건):
   - 세션 플래그(plan/intensity): _state/session-flags.json
   - 전역 settings.json의 statusLine 엔트리(bathos 관련분만): ~/.claude/settings.json
[bathos uninstall] (dry-run) 아무것도 삭제하지 않았습니다. 실제 삭제: --apply [--yes]
```

빈 상태(정리할 것 없음):

```
[bathos uninstall] ✓ 외부 상태 없음 — 호스트 제거만 하면 됩니다.
[bathos uninstall] 다음 행동: 호스트의 플러그인 제거 명령을 실행하세요.
```

부분 실패 시(개별 실패로 전체를 죽이지 않음):

```
[bathos uninstall] ✓ [removed] _state/session-flags.json
[bathos uninstall] ✗ [fail: 백업 실패] ~/.claude/settings.json
[bathos uninstall] 결과: 성공 1건 / 실패 1건
[bathos uninstall] ! 일부 항목 정리 실패 — 다음 행동: 위 [fail] 항목을 수동으로 확인/제거한 뒤, 그래도 호스트 제거는 진행 가능합니다.
```

## 4. 스크립트가 이미 사라진 경우 — 수동 정리 경로

호스트 제거를 먼저 실행해 `scripts/uninstall.sh`가 이미 삭제됐다면, 아래를 수동으로 확인하세요:

1. `~/_state/session-flags.json`(또는 프로젝트의 `_state/session-flags.json`) — 존재하면 삭제.
2. `~/.claude/settings.json`을 열어 `"statusLine"` 키의 `command`가 `bathos`를 참조하면 해당 키만 삭제(파일 전체를 지우지 마세요 — 다른 도구 설정이 함께 있을 수 있습니다).
3. `~/.config/bathos/` 디렉터리가 존재하면 삭제.

이 정리는 필수는 아닙니다(고아 상태 파일이 남아도 다른 도구 동작에 영향 없음) — 다만 흔적을 남기지 않으려면 수행하세요.

## 5. 테스트

`dist/tests/test-uninstall.sh`가 4개 픽스처(① dry-run 전체 나열 ② `--yes` 실제 삭제 ③ 빈 상태 ④ 부분 실패 분리 표기)로 회귀 검증합니다. 실제 `$HOME`을 건드리지 않고 `BATHOS_ROOT`/`BATHOS_STATE_DIR`/`BATHOS_HOME`/`BATHOS_CONFIG_DIR` 환경변수로 격리된 임시 디렉터리에서 실행됩니다.
