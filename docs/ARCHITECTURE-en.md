# BATHOS — Architecture (for Contributors)

> A map for contributors of how the engine is put together and what it guarantees. Read this before touching `core/` or the hooks — this document is precisely what keeps a change from silently breaking a guarantee.
>
> **See also:** [Concepts (FEATURES-en)](FEATURES-en.md) · [Usage (USAGE-en)](USAGE-en.md) · 한국어: [`ARCHITECTURE-kr.md`](ARCHITECTURE-kr.md) · Español: [`ARCHITECTURE-es.md`](ARCHITECTURE-es.md)

---

## 1. The two execution planes

BATHOS splits cleanly into two execution planes. Internalizing this distinction is the very first thing to do before reading the code.

| Plane | What | Where |
|-------|------|-------|
| Orchestration | Slash commands · role definitions · hooks (markdown/bash) | `.claude/` |
| Engine | Single static Rust binary `bathos` | `core/` |

**Anything that must be trusted and reproducible** lives in the engine (unit-tested); anything humans edit lives on the orchestration plane. Hooks and commands call `bathos <subcommand>` under the hood.

## 2. Rust workspace (`core/`, 7 crates)

The engine is a single Cargo workspace of 7 crates — one crate per module responsibility.

| Crate | Module | Responsibility |
|-------|--------|----------------|
| `bathos-state` | M1 | SSOT: `manifest.json` (schema-validated, atomic writes) + tamper-evident audit hash chain |
| `bathos-router` | M2 | Scale-Adaptive Lv0–4 routing (recommend/confirm) |
| `bathos-wave-engine` | M3 | 7-wave state transitions; concurrency cap |
| `bathos-gate-engine` | M4 | PASS/CONCERNS/FAIL verdicts; critical→FAIL |
| `bathos-story-engine` | M5 | Story compilation (D1/D2/D3) — zero-context-loss |
| `bathos-plug` | M12 | Plug module manager (module.yaml · trigger DSL) |
| `bathos-cli` | bin | The `bathos` binary — subcommand dispatch |

Dependency direction: `bathos-cli` → engine crates → `bathos-state`. **No engine crate depends on `bathos-plug`** (invariant A9).

## 3. Invariants (do not break)

These are the guarantees holding up the structure, enforced by code, not convention. If you modify code related to any of them, keep the enforcement point that upholds that invariant working honestly.

| ID | Invariant | Enforced by |
|----|-----------|-------------|
| **A9** | The core never depends on plug modules | Crate dependency graph (verified via `cargo tree`) |
| Gate FACILITATOR | A verdict's `facilitator` can never be empty; no unsubstantiated auto-PASS | `bathos-gate-engine` |
| `critical > 0 → FAIL` | A single critical issue means FAIL | `bathos-gate-engine` |
| Concurrency ≤ 3 | `MAX_CONCURRENT_ROLES = 3`; the 4th spawn is rejected | `bathos-wave-engine` (`E-CONCURRENCY`) |
| Audit chain | `hash_prev[n] == hash_self[n-1]`, genesis anchor, monotonic `seq`, single writer | `bathos-state::audit` (`bathos audit verify`) |
| Atomic writes | The manifest is never left half-written | `bathos-state::store` |
| Story completeness (D1) | 6 required sections + non-empty `developer_context` | `bathos-story-engine` (`E-CTX-LOSS`) |
| W3 FAIL blocks W5 | A FAIL verdict physically blocks entry into implementation | `gate-enforce.sh` (exit 2) |

## 4. Exit codes & error taxonomy

Errors surface in two ways — process exit codes that hooks use for branching, and symbolic E-codes that give failures a name.

- **Exit codes:** `0` success · `1` error · **`2` gate FAIL** (used by hooks to block).
- **E-codes:** `E-LEVEL-DRIFT`, `E-CONCURRENCY`, `E-CTX-LOSS`, `E-STALE`, `E-STATE-CORRUPT`, `E-AUDIT-TAMPER`, `E-PLUG-NOTFOUND`.

## 5. Router scoring (deterministic)

The router turns a stakes profile into a level using pure arithmetic only — no heuristics, so the same input always yields the same recommendation.

`scope` (0–3: bug=0, feature=1, large/module=2, product/platform=3, unclear=1) + `novelty` (+1) + `regulation_ip` (+2) + `team_size` (0–2: solo=0, medium=1, large=2). Total → level: 0→Lv0, 1→Lv1, 2–3→Lv2, 4–5→Lv3, 6+→Lv4. Recommendation and confirmation are separated (User Sovereignty).

## 6. Hooks (`.claude/hooks/`)

Six fail-safe hooks bound to Claude Code events by `settings.json`: `careful-guard` · `freeze-guard` · `audit-log` · `artifact-verify` · `gate-enforce` · `next-action`. They call the engine via `$BATHOS_BIN`. Self-verification: `bash .claude/hooks/_test-hooks.sh` (46 determinism tests). **Only valid event names belong in the `hooks` block** — a stray comment key sends subagent startup into an infinite wait (`bathos doctor` detects this).

## 7. Key ADRs

- **ADR-0006:** Core engine = Rust (single static binary); hooks = bash; commands/roles/assets = markdown; config = JSON/YAML.
- The full set of ADRs lives in the design record (`.agent-team/04-architecture/adr/`).

## 8. Engine contribution rules

1. Keep the core slim — new domain features go into `modules/`, never into the core (A9).
2. `cd core && cargo test && cargo clippy --all-targets -- -D warnings` must be green, and add invariant tests.
3. Run `cargo fmt --all` before committing.
4. When changing hooks, run `bash .claude/hooks/_test-hooks.sh` and preserve determinism and fail-safety.
5. CI (`.github/workflows/ci.yml`): fmt · clippy(-D) · test · release · jq · hook harness.

---

<div align="center">한국어: <a href="ARCHITECTURE-kr.md">ARCHITECTURE-kr</a> · Español: <a href="ARCHITECTURE-es.md">ARCHITECTURE-es</a></div>
