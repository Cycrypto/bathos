# Quickstart — your first BATHOS run

A 10-minute walkthrough from clone to your first wave. 🇰🇷 한국어 주석을 함께 둡니다.

## 0. Build & set up (once)

```bash
# from the repo root
./install.sh
export BATHOS_BIN="$(pwd)/core/target/release/bathos"
export CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS=1
```

> 🇰🇷 `install.sh`가 엔진을 빌드합니다. `BATHOS_BIN`은 훅이 엔진을 찾는 경로, 환경변수는 Agent Teams 활성화용입니다.

## 1. Drive the engine directly (no Claude Code needed)

The `bathos` binary is the deterministic core. Try it on a throwaway state dir:

```bash
mkdir -p /tmp/bathos-demo/_state
cat > /tmp/bathos-demo/_state/manifest.json <<'JSON'
{"project_id":"bathos-demo","codename":"BATHOS","current_level":0,"status":"active",
 "lang":"en","created":"2026-06-30T00:00:00Z",
 "routing":[],"waves":[],"gates":[],"risks":[],"modules":[],"artifacts":[],"stale_story_keys":[]}
JSON

# validate the single source of truth
"$BATHOS_BIN" --state-dir /tmp/bathos-demo/_state state validate

# recommend a Scale-Adaptive level from "stakes" (recommend only)
cat stakes.example.json | "$BATHOS_BIN" --state-dir /tmp/bathos-demo/_state route decide

# confirm a level (records it to the manifest — User Sovereignty: explicit step)
cat stakes.example.json | "$BATHOS_BIN" --state-dir /tmp/bathos-demo/_state route decide --confirm 2

# inspect what changed
"$BATHOS_BIN" --state-dir /tmp/bathos-demo/_state route show
```

Expected: `route decide` prints a recommended level with its `wave_set` / `role_set`;
`--confirm 2` records a `LevelDecision` and sets `current_level` to 2.

> 🇰🇷 `route decide`는 추천만, `--confirm`을 줄 때만 manifest에 기록합니다(사용자 결정 분리).

## 2. Drive the full pipeline (in Claude Code)

Open Claude Code in your project (or use this repo) and run the slash commands in order.
A typical **Lv2** (standard feature) run:

```text
/team-kickoff                          # scaffold .agent-team/ + charter + manifest
/route        /abs/path/to/project     # confirm Lv2
/wave1-discovery   /abs/path           # discovery + market
/wave2-design      /abs/path           # planning · architecture · design
/wave3-story-gate  /abs/path           # ⭐ condense to story files + readiness gate
/wave5-implement   /abs/path           # implementation
/wave6-verify-report /abs/path         # verify · docs · report
/team-confirm                          # final sign-off
```

What to expect at each gate:
- **PASS** → proceed automatically.
- **CONCERNS** → a risk is logged in `_state/`; you proceed.
- **FAIL** → entry to the next wave is blocked until you remediate and re-gate.
  (At W3, the `gate-enforce` hook physically blocks W5 on a FAIL verdict.)

> 🇰🇷 게이트가 FAIL이면 다음 웨이브 진입이 막힙니다. 특히 W3 FAIL은 훅이 W5 진입을 물리적으로 차단합니다.

## 3. Where things land

```
your-project/
├── src/ …                      # your actual product code
└── .agent-team/                # BATHOS run artifacts
    ├── 00-plan/ 01-reverse/ 02-market-analysis/ 03-service-planning/
    ├── 03-story-engineering/   # W3 self-contained story files
    ├── 04-architecture/ 07-design/ 08-impl-notes/
    ├── 10-review/ 11-qa/ 12-report/
    └── _state/                 # manifest.json (SSOT) + audit-log.jsonl
```

## Next

- Full reference: [`../../docs/USAGE-kr.md`](../../docs/USAGE-kr.md)
- Enable a plugin: `"$BATHOS_BIN" --modules-dir ../../modules plug list`
