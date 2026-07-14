# BATHOS Dynamis — 에이전트 이식성(Portability) 표

> 작성: Andrew(#9, 배포/클라이언트) · 2026-07-08 · W4 구현(축 B)
> 소유: 본 문서는 CF-B2(Must)의 정본 산출물. 값은 실측/설계확정본 우선, 미확정은 `(추정)` 표기(날조 금지).
> 전제: `.agent-team/03-story-engineering/project-context-kr.md` §5-1·2, `04-architecture/distribution-architecture-kr.md`, `07-design/surface-formats-kr.md` §b.
> 검증: `scripts/check-rule-copies.sh --dup-scan`(무중복) · `dist/tests/test-manifests.sh`(스키마/포인터) 가 본 표의 주장을 자동 회귀로 뒷받침한다.

---

## 0. 이 문서가 규정하는 것

**"BATHOS Dynamis를 런타임 X로 배포한다"가 무슨 뜻인지**를 코드 착수 전에 규정하는 지도다. 이 표는 구현이 아니라 **약속**이다 — 구현은 Claude Code(정본) + MCP(레퍼런스 1채널)만 실제로 존재하며, 나머지 행은 "이 호스트를 지원한다면 이런 모양이 될 것"이라는 명시적 스텁/미구현 표시다.

**핵심 원칙(위반 시 게이트 FAIL 사유, CF-B1):**

> 하나의 behavior 소스 + 얇은 per-surface 어댑터. 로직/산문을 절대 두 곳 이상에 포크하지 않는다.

- 역할(role) 정본: `.claude/agents/_base/*.md` (+ `.claude/agents/_preamble/*.md` — 전 역할 공통 주입분)
- 스킬(skill) 정본: `.claude/skills/*/SKILL.md`
- 커맨드(command) 정본: `.claude/commands/*.md`
- 훅(hook) 정본: `.claude/hooks/*.sh` (full-hook 계층에서만 유효 — 아래 §1 참조)

어떤 매니페스트·MCP 서버·문서도 위 정본의 **산문을 복제하지 않는다**. 참조는 항상 **경로 포인터**로만 한다.

---

## 1. 능력 계층(capability tier) — 3단 (색 무관, 흑백 기호)

| 계층 | 기호 | 의미 | BATHOS 핵심(웨이브/게이트/훅) 지원 |
|------|:---:|------|--------------------------------------|
| **full-hook** | `●` | 라이프사이클 훅이 매 턴 규칙 주입 + 게이트 물리 강제 + intensity 토글 | 전부 동작 |
| **instruction-only** | `◐` | 호스트가 자동 로드하는 정적 규칙 파일만(always-on·비대화) | 역할 규칙 텍스트만 — **훅·게이트 강제 없음** |
| **mcp-fallback** | `○` | 훅·파일로딩이 없는 호스트에 MCP prompt+tool(read-only)로 노출 | 조회만 — **최저공통** |

> 기호는 채움 정도로 능력 정도를 나타낸다(색 의존 없음, WCAG 1.4.1). 항상 계층 명칭을 텍스트로 병기한다(기호 단독 표기 금지).

---

## 2. host → files → capability 어댑터 표 (정본)

| Host | Capability | Manifest 경로(고정) | Instruction 파일 | 훅 계약 | Dynamis 범위 |
|------|:---------:|--------------------|-------------------|---------|--------------|
| **Claude Code** | `●` full-hook | `dist/claude/.claude-plugin/plugin.json` (+`marketplace.json`) | `CLAUDE.md` · `.claude/agents/**` · `.claude/skills/**` · `.claude/commands/**` | `PreToolUse`·`PostToolUse`·`TaskCompleted`·`TeammateIdle` (`.claude/settings.json`) | **Must — 구현(정본)** |
| **MCP-only 호스트** | `○` mcp-fallback | `dist/mcp/manifest.json` (호스트 측 MCP 서버 등록은 호스트 설정에서 수행 — 표준 등록 절차는 호스트마다 다름, `(추정)`) | 없음(파일 자동로드 아님) — `bathos_instructions` tool 호출로 대체 | 없음(read-only) | **Should(LD-2) — 레퍼런스 구현** |
| **Codex** | `●`/`◐` `(추정)` | `dist/manifests/codex/plugin.json` | `AGENTS.md`(기존 `bathos/AGENTS.md`와 별개 경로 규약 — 호스트 확인 전 `(추정)`) | 호스트별 JSON 출력 분기(형태 미검증) | **Could — 매니페스트 스켈레톤만(미타깃)** |
| **Cursor / Windsurf / Cline** | `◐` instruction-only | (호스트 자체 매니페스트 없음) | `.cursor/rules/*.mdc` · `.windsurf/rules/*.md` · `.clinerules/*.md` `(추정 — 각 호스트 최신 문서로 재검증 필요)` | 없음 | **Could — 표에만 명시(미구현)** |
| **Gemini / Antigravity** | `◐` instruction-only | `gemini-extension.json`(`contextFileName: "AGENTS.md"` 지정 방식, `(추정)`) | `AGENTS.md` | 없음 | **Could — 표에만 명시(미구현)** |
| **GitHub Copilot** | `◐` instruction-only | (없음) | `.github/copilot-instructions.md` | 없음 | **Could — 표에만 명시(미구현)** |

**표의 필수 6열(US6 AC2 충족):** Host · Capability(기호+명칭) · Manifest 경로 · Instruction 파일 · 훅 계약 · Dynamis 범위. 위 표가 이 6열을 모두 갖춘다.

---

## 3. 정직성 주석 3종 (필수 — 표에서 분리하지 말 것)

1. **열화 계층 고지:** `◐`(instruction-only)·`○`(mcp-fallback) 행은 **always-on 주입과 동등하지 않다** — 훅·게이트 물리 강제가 없다. 이 계층에서 BATHOS의 "정책을 사실로" 원칙(결정론적 차단)은 적용되지 않으며, 규칙은 advisory(권고)로만 전달된다.
2. **단일 소스 고지:** 모든 매니페스트(위 §2의 "Manifest 경로" 열)는 canonical 소스(§0)를 **경로 포인터로만** 참조한다. behavior 텍스트를 매니페스트에 임베드하지 않는다(각 매니페스트 목표 <20줄).
3. **범위 고지:** 16런타임 폭 자체를 목표로 삼지 않는다(charter §3 카피 금지). 이번 구현은 **Claude Code(정본) + MCP(레퍼런스 1채널)** 만 실제로 동작하며, 그 외 행은 "지원한다면 이런 형태" 수준의 설계 지도다. 실제 npm/마켓플레이스 퍼블리시는 범위 밖(Won't, LD-2).

---

## 4. 정본 언어(canonical language) 결정 — R-B2 해소

**결정(design-handoff-kr.md §4-1 인용 근거로 확정, Andrew):**

- BATHOS 운영 규칙 산문(역할 정의·훅 메시지·스킬 본문·커맨드 정의)의 **정본 언어 = 한국어(kr)**. 이는 ponytail의 "en 정본" 관행을 그대로 따르지 않은 것이다 — 실측 결과 `bathos/CLAUDE.md`·`bathos/.claude/agents/_base/*.md`·`bathos/ETHOS.md`가 이미 한국어로 작성된 정본이기 때문이다(사실 — 본 파일들 직접 확인).
- 대외 공개 문서(`README.md`)만 기존 정책대로 **영어 정본**을 유지한다(사실 — `bathos/README.md`가 영어로 확인됨).
- `docs/{ARCHITECTURE,FAQ,FEATURES,MODULE-GUIDE,QUOTA,ROLE-GUIDE,USAGE,USECASE}-{en,es}.md`는 kr 정본의 **커뮤니티/보조 번역**으로 취급하며, 완전 병렬 번역을 강제하지 않는다(design-handoff §4-1 "무언 노후화 허용 계층"). 이 계층은 drift-guard(B3) 검사에서 **명시적으로 제외**한다(`scripts/drift-exclusions.json` 참조 — 암묵 제외 금지 원칙).
- **User Sovereignty 고지:** 이 결정은 설계 문서의 "en 정본" 권고안을 뒤집는 것이다. 근거는 위 실측 사실이며, 사용자가 다른 정본 언어를 원하면 언제든 재지정 가능(놓쳤을 맥락: 향후 실제 다국어 배포 확장 시 en 정본이 더 유리할 수 있음 — 현재는 기존 자산 실태를 따름).

---

## 5. 런타임 언어 결정 — R-B1 해소

배포 레이어(축 B) 스크립트·서버의 런타임 언어를 다음과 같이 확정한다(Andrew, 착수 시점 결정, 상세 근거는 `.agent-team/08-impl-notes/frontend.md` §2 참조):

| 산출물 | 언어 | 근거(요약) |
|--------|------|-----------|
| `scripts/check-rule-copies.sh` · `check-versions.sh` · `uninstall.sh` | **Bash** | 기존 `.claude/hooks/*.sh` 전부 Bash(사실) — 동일 관례(SCRIPT_DIR 해석·jq 폴백) 계승, 신규 런타임 의존성 0 |
| `dist/lib/host-detect.sh` | **Bash** | 위와 동일 — 현재 실제 분기 대상이 없어(Could 스텁) 경량 Bash로 충분 |
| `dist/mcp/server.py` | **Python** | 공식 MCP SDK 보유 언어 중 하나(Anthropic 공식 Python SDK, `_recon` 인용 사실). 저장소가 이미 Python 사용 중(`bathos/site/build-docs.py`, 사실) → Node 툴체인(`package.json`/`node_modules`) 신규 도입 없이 재사용 |

**미해결(정직 표기):** Codex/Copilot 등 다른 호스트를 실제로 타깃할 경우, 해당 호스트의 SDK/관례에 따라 언어가 달라질 수 있다(현재 미타깃이므로 결정 불필요).
