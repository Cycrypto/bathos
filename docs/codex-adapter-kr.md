# BATHOS × Codex CLI — 어댑터 설계 (포팅 가이드)

> **성격: 런타임 포팅.** Claude Code 접착층(Agent Teams·훅·커맨드)을 OpenAI Codex CLI의 확장점으로 옮긴다. `bathos` 엔진은 그대로 CLI로 호출.
> 상위 판단·단계 계획: `docs/PORTABILITY-kr.md`. 이 문서는 Codex 매핑 상세 + 재설계 지점 + 변환 스캐폴드(`scripts/to-codex.sh`) 사용법.
> ⚠️ Codex 훅은 실험적·Windows 미지원 가능성. 커스텀 프롬프트는 deprecated(→skills). 실사용 전 출처 재확인.

---

## 1. 매핑 매트릭스

| BATHOS(Claude Code) | Codex CLI 대응 | 변환 |
|---------------------|----------------|------|
| `.claude/agents/<r>.md` (frontmatter `model`) | `~/.codex/agents/<r>.toml` (`name`·`description`·`developer_instructions`·`model`·`model_reasoning_effort`) | `to-codex.sh`가 기계 변환 |
| `.claude/commands/<c>.md` | `~/.codex/prompts/<c>.md` (`/prompts:<c>`, `$1..$9`·`$ARGUMENTS`·front matter) | `to-codex.sh`가 기계 변환 |
| `AGENTS.md` | 네이티브(Codex가 읽음) | 그대로 |
| MCP 서버 | `~/.codex/config.toml` `[mcp_servers.x]` | 수동/후속 |
| 훅(settings.json) | `~/.codex/config.toml` `[hooks]`(실험적) | §3 재설계 |
| 모델 ID `claude-*` | Codex `model` + `[model_providers.x]` | §4 |

**중첩 규칙 부합:** Codex `agents.max_depth` 기본 1 = "루트만 서브에이전트 스폰, 팀원은 중첩 팀 금지" → BATHOS 규칙과 일치(추가 조치 불필요).

---

## 2. 커맨드·에이전트 변환 (기계적 80%)

```bash
bash scripts/to-codex.sh            # 미리보기(dry-run): 무엇이 생성될지 출력만
bash scripts/to-codex.sh --write    # 실제 생성: ~/.codex/prompts/ 와 프로젝트 .codex/agents/ 에 기록
bash scripts/to-codex.sh --write --dest /경로   # 출력 위치 지정
```
- 커맨드: `.claude/commands/*.md` → prompts. front matter(`description`/`argument-hint`)는 Codex도 인식하므로 대체로 그대로. `$ARGUMENTS`/`$1..$9` 규약 동일.
- 에이전트: `.claude/agents/_base/*.md`의 frontmatter(`name`/`description`/`model`)를 파싱 → `.toml`로 방출. 본문(역할 지침)은 `developer_instructions`에 삽입.
- ⚠️ **자동 변환이 못 하는 것**(수동): 훅 기반 게이트(§3), SessionEnd 종료 루틴(§3), MCP 등록(§1), 모델 프로바이더(§4), 커맨드가 Task/Agent 툴로 팀을 스폰하는 로직(Codex 서브에이전트 호출로 재작성).

---

## 3. 재설계 지점 — Codex 이식의 진짜 비용

### (a) `TaskCompleted` 게이트 → `PreToolUse` exit-2 게이트
BATHOS는 W3 등 핵심 게이트를 "태스크 완료 차단"(TaskCompleted 훅)으로 하드 강제했다. Codex엔 등가 이벤트가 없다.
- **대안:** 다음 단계 진입 트리거(예: 구현 파일 쓰기, `wave5` 커맨드 실행)를 `PreToolUse` 훅으로 가로채 `bathos gate show`가 PASS/CONCERNS 아니면 **exit 2 + stderr 사유**로 차단. Codex에서 exit 2 = `"decision":"block"`.

### (b) `SessionEnd` 없음 → `Stop` 근사
CLAUDE.md §9(종료 시 저장→리포트→종료 강제)는 Codex에 SessionEnd가 없어 물리적으로 동일 구현 불가.
- **대안:** `Stop`(턴 종료) 훅에서 스냅샷/리포트를 **증분 저장**(세션 종료 ≠ 턴 종료이므로 매 턴 갱신 방식). 완전한 "종료 시 1회"는 포기하고 "자주 저장"으로 대체. `/save-session` 수동 저장 병행 권장.

### (c) 훅 실험적·Windows 공백
Codex 훅은 실험적(첫 도입 2026-03)·Windows 미지원 가능성 ⚠️. BATHOS의 Windows/WSL 지원과 충돌 시, Windows에서는 훅 없는 축소 모드(수동 게이트) 문서화 필요.

---

## 4. 모델 프로바이더 — responses-only 함정

- Codex는 `~/.codex/config.toml` `[model_providers.x]`로 커스텀 백엔드를 붙이되 **`wire_api="responses"`만 지원**(2026-02 chat 제거).
- **Codex + GLM**: GLM은 Chat Completions 계열 → **직접 연결 시 깨짐**(issue #9612). Responses↔Chat **번역 프록시**(codex-relay, codex-glm-proxy 등) 경유 필요. 즉 "Codex 위에서 GLM"은 어댑터 2겹.
- `model_provider(s)`는 **유저 레벨 `~/.codex/config.toml`에서만** 유효(프로젝트 로컬 무시).

---

## 5. 남은 단계 (후속)
- **P4: 완료(2026-07-16, Phillip)** — (a)(b) 훅 재설계가 `codex-adapter/`에
  실구현되었다: `hooks/pretooluse-gate.sh`(W3 게이트 exit-2) ·
  `hooks/stop-save.sh`(Stop 증분 저장) · `hooks/_test-codex-hooks.sh`(14케이스
  시뮬레이션, 실 Codex 불요 — 전부 통과) · `config.toml.example`. 상세는
  `codex-adapter/README.md`와 `.agent-team/08-impl-notes/backend.md` 참고.
  **미해소:** 실 Codex 미설치 상태라 `tool_name` 실측값(§3-a 트리거 매칭의
  전제)은 여전히 문서 예시(`Bash`·`apply_patch`) 기준 ⚠️[추정]이다 — 설치
  후 최우선 재확인 필요(fail-open 리스크).
- P5: Codex+GLM 프록시 연동, 커맨드→skills 마이그레이션(공식 방향).
- P6: `bathos runtime` 추상화(런타임 감지→어댑터 자동 선택).

## 출처
- prompts/slash: developers.openai.com/codex/custom-prompts · /guides/slash-commands
- hooks(이벤트·exit2·실험적·Windows): learn.chatgpt.com/docs/hooks · codex.danielvaughan.com(2026-04)
- subagents(TOML·max_depth): learn.chatgpt.com/docs/agent-configuration/subagents
- config/MCP/AGENTS.md: developers.openai.com/codex/config-reference
- responses-only·프로바이더: learn.chatgpt.com/docs/config-file/config-reference · github.com/openai/codex/discussions/7782
- Codex+GLM 프록시: github.com/openai/codex/issues/9612 · KevinSHH/codex-glm-proxy · MetaFARS/codex-relay
