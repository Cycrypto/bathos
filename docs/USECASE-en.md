# BATHOS Use Case — Building a New Service from Scratch, Step by Step

> **Goal of this document:** show, as simply as possible, how to build *an actual new service from scratch* with BATHOS. Installation uses the **"adopt into my project"** style (B):
>
> ```bash
> # B) Build the engine + copy the method package into my project
> ./install.sh --into /abs/path/to/your-project
> ```
>
> We follow one concrete example to the end: **"ReadShelf"** — a small web app for logging books you've read and sharing short reviews with friends. By the end you'll know exactly *what you type, what BATHOS does at each step, and which files appear on disk*.
>
> **See also:** Curious about the principles? Get the concepts from [`FEATURES-en.md`](FEATURES-en.md) → then check the detailed CLI · hooks in the [`USAGE-en.md`](USAGE-en.md) reference. · 한국어: [`USECASE-kr.md`](USECASE-kr.md) · Español: [`USECASE-es.md`](USECASE-es.md)

---

## 0. The 30-second mental model

BATHOS is **not an app you run** — it's a **method package that runs on Claude Code**. You:

1. **Install it into your project** (copy `.claude/` + `assets/` + `modules/`, set an env var pointing at the engine).
2. **Open Claude Code in your project** and type **slash commands** (`/team-kickoff`, `/route`, `/wave1-discovery`, …).
3. BATHOS spins up specialist "teammates" wave by wave, writes its work artifacts to `.agent-team/`, and **your actual product code accumulates in the usual place — `src/`**.

That's it. The rest of this document is just walking those three things slowly through a real example.

---

## 1. The example service: "ReadShelf"

| | |
|---|---|
| **What** | Web app: log books you've read → write short reviews → follow friends → see friends' reading feed. |
| **Who** | A solo developer (you). |
| **Stack (planned)** | Next.js (frontend) · Node/Express API · PostgreSQL. No AI/ML in the MVP. |
| **Ambition** | Not a giant platform — a **focused MVP** first. |

Keep this in mind: ReadShelf is a *new but focused* product. That choice matters in step 4, when picking a BATHOS "level."

---

## 2. One-time prerequisites

- **Claude Code v2.1.32+** + the experimental **Agent Teams** feature.
- **Rust toolchain** (cargo 1.92+) — needed only to build the engine once.
- **`jq`** — used by the safety hooks.
- A macOS/Linux shell.

The BATHOS package itself needs to live somewhere on disk. Here we assume it's cloned to `~/tools/bathos`.

---

## 3. Step 1 — Install BATHOS into my project (style "B")

Create an empty project folder, then **from the BATHOS repo** run the install script with `--into` pointing at your project:

```bash
# Where the new project lives (can be empty, or an existing repo)
mkdir -p ~/projects/readshelf

# From the BATHOS package:
cd ~/tools/bathos
./install.sh --into ~/projects/readshelf
```

What this command does (and **does not** do):

- Builds the engine once → `~/tools/bathos/core/target/release/bathos` (~5.6MB).
- Copies **3 folders** into `~/projects/readshelf/`:
  - `.claude/` — slash commands, 17 role definitions, 6 safety hooks, `settings.json`
  - `assets/` — templates/workflows/checklists used by Wave 3 and the plugs
  - `modules/` — optional plug modules (`ip-pack`, `research-pack`)
- **Never deletes anything**, and won't overwrite an existing `.claude/` without `--force`.
- Does **not** copy the compiled binary into your project. The engine stays in the BATHOS repo; your project merely points at it (next step).

Now wire up two environment variables (the install script prints the guidance):

```bash
# So the hooks can find the engine:
export BATHOS_BIN="$HOME/tools/bathos/core/target/release/bathos"

# Enable Agent Teams (already set in the copied settings.json, but explicit):
export CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS=1
```

> Put those two `export` lines in your shell profile so they apply to every session.

The project now looks like this:

```
~/projects/readshelf/
├── .claude/        ← (just installed) commands · roles · hooks · settings
├── assets/         ← (just installed) templates · workflows · checklists
└── modules/        ← (just installed) ip-pack · research-pack
```

Nothing else yet — no `.agent-team/`, no `src/`. They appear in the next steps.

---

## 4. Step 2 — Open Claude Code and kick off

Open a Claude Code session in your project directory (`~/projects/readshelf`). That session is **the lead, "Paul."** Then:

```text
/team-kickoff
```

**What happens:** BATHOS creates the workspace and the single source of truth.

**What appears on disk:**

```
~/projects/readshelf/
├── .claude/  assets/  modules/        (the install)
└── .agent-team/                       ← new
    ├── 00-plan/        (charter, task graph)
    ├── 01-reverse/ … 12-report/       (per-role output folders)
    └── _state/
        └── manifest.json              ← SSOT: all project state, schema-validated
```

> **Two directories, two purposes.** `.agent-team/` is BATHOS's *work notes* (planning · design · reviews · reports · state). Your *actual service code* accumulates in `src/`. The two never mix.

---

## 5. Step 3 — Pick a level with `/route` (you decide)

BATHOS doesn't run every wave for every job. It matches effort to the size of the work. Tell BATHOS ReadShelf's "stakes":

```text
/route /Users/you/projects/readshelf
```

Under the hood, the engine scores four axes:

| Axis | ReadShelf answer | Why |
|------|------------------|-----|
| `scope` | `feature` | A focused MVP, not a giant platform → score 1 |
| `novelty` | `true` | A new product idea → +1 |
| `regulation_ip` | `false` | Not a regulation/patent-critical domain → +0 |
| `team_size` | `solo` | Just you → +0 |

Total = **2 → recommended Level 2.** The engine returns something like:

```json
{ "recommended_level": 2, "wave_set": ["W1","W2","W3","W5","W6"],
  "requires_confirmation": true }
```

> **This is your decision (User Sovereignty).** BATHOS only *recommends*. You must confirm, and only the confirmation is recorded:
>
> ```bash
> bathos -s .agent-team/_state route decide \
>   --stakes-json '{"scope":"feature","novelty":true,"regulation_ip":false,"team_size":"solo"}' \
>   --confirm 2
> ```
>
> Now `manifest.json → current_level = 2`.

**What Level 2 means for ReadShelf** — these waves run, in order:

```
W1 discovery → W2 design → W3 story gate ★ → W5 implementation → W6 verification
```

(Level 0 would be for a bug fix only; Levels 3–4 add W0 market analysis and W4 IP/research — overkill for a focused MVP. If ReadShelf later grows into a big multi-module platform, just re-route to Lv3.)

---

## 6. Step 4 — Wave 1: discovery & market (what should we build?)

```text
/wave1-discovery /Users/you/projects/readshelf
```

**Who works:** Caleb (market analyst) is the star here. John (reverse specialist) only makes sense when you give him a *reference* codebase to learn from — for a greenfield app, skip him, or hand him an open-source repo of a competing service.

**What happens:** Caleb researches similar reading/social apps, finds the gaps, and proposes a **USP** (the reason to use ReadShelf).

**What you get:** `.agent-team/02-market-analysis/` — the competitive landscape, USP recommendation.

**Gate:** *USP Readiness.* Before moving on, you (the lead) confirm the USP makes sense.

> At the end of each wave, the working teammates are shut down before the next wave. Keeping concurrent actives ≤ 3 saves tokens.

---

## 7. Step 5 — Wave 2: planning · architecture · design

```text
/wave2-design /Users/you/projects/readshelf
```

This wave runs in a deliberate order:

1. **Joshua (service planning)** turns the USP into concrete **Core Features + User Stories + a Service Story**. *His output is the gate* — nothing else starts until the planning is solid.
2. Then, in parallel:
   - **James (architect)** designs the data model (ERD), API contracts, service sequences, and exception handling — e.g. `users`, `books`, `reviews`, `follows` tables; `POST /reviews`, `GET /feed`, etc.
   - **Jonnathan (designer)** designs the UX flows and UI — sign-up, the shelf, the review composer, the friends feed.

**What you get:** `.agent-team/03-service-planning/`, `.agent-team/04-architecture/`, `.agent-team/07-design/`.

**Gate:** *Plan Readiness.*

> Optional reinforcement: before locking the plan in, you can run review gates like `/plan-ceo-review` (is this a 10/10 product?) or `/plan-eng-review` (architecture/edge cases/tests). One reviewer at a time.

---

## 8. Step 6 — Wave 3: the story gate ★ (the heart of BATHOS)

```text
/wave3-story-gate /Users/you/projects/readshelf
```

This is the step that makes BATHOS different. The point where most "AI, build my app" attempts collapse is precisely **design → implementation**: the builder forgets half the design or never saw it in the first place. Wave 3 closes that gap.

**Who works:** **Matthew (#17, story engineer)** + the *independent* verifiers **Thomas** and **Matthias**.

**What Matthew does:** condenses the entire W2 design into **self-contained dev story files** under `.agent-team/03-story-engineering/`. Each story (e.g. `story-1-2-write-review-en.md`) is written so an implementer can start *from the story alone*. The engine enforces:

- **6 required sections** must exist and `developer_context` must be non-empty (`story_requirements`, `developer_context`, `architecture_compliance`, `library_framework_requirements`, `file_structure_requirements`, `testing_requirements`). Any missing one → compilation fails with **`E-CTX-LOSS`**.
- Every technical claim carries a **`[Source: …]`** marker → traceable to the architecture/design documents.

**The gate (this is the important part):** Thomas and Matthias review the stories independently, and the gate returns **PASS / CONCERNS / FAIL**:

- **PASS** → proceed to implementation.
- **CONCERNS** → proceed, logging the risks.
- **FAIL** → the hook (`gate-enforce`) **physically blocks Wave 5 from starting** (the engine exits with code 2). You literally cannot start implementation on top of a failed readiness gate. Fix the stories and re-gate.

> This is the physical implementation of "generation ≠ verification": the same model cannot wave its own work through.

---

## 9. Step 7 — Wave 5: implementation (writing the actual code)

```text
/wave5-implement /Users/you/projects/readshelf
```

**Who works (only the roles ReadShelf needs):**

- **Phillip (backend)** — the Express API + PostgreSQL schema/migrations, auth, the `/reviews` · `/feed` endpoints.
- **Andrew (frontend/mobile)** — the Next.js UI based on Jonnathan's design, wired to James's API contracts.
- *(Stephen, the AI/ML principal, is **not spawned** — no ML in the ReadShelf MVP.)*

**Where the code goes:** the real source tree — `~/projects/readshelf/src/` (and `api/`, `web/`, wherever your project's code lives). Each teammate edits **only their owned paths**, so backend and frontend work never collide.

```
~/projects/readshelf/
├── src/  api/  web/ …      ← real code appears here, story by story
└── .agent-team/08-impl-notes/   ← implementation notes (backend.md, frontend.md)
```

**Gate:** per-story completion is verified, then the story is considered done.

> The safety hooks stay active throughout: `careful-guard` blocks destructive shell commands (`rm -rf`, `DROP TABLE` …), and `freeze-guard` confines each teammate to their owned paths.

---

## 10. Step 8 — Wave 6: verification · docs · report

```text
/wave6-verify-report /Users/you/projects/readshelf
```

**Who works:** **Thomas** (code review), **Timothy** (developer docs), **Matthias** (QA / E2E tests) — in parallel — then **Martin** aggregates a single **HTML report**.

**What you get:** `.agent-team/10-review/`, `.agent-team/09-docs/`, `.agent-team/11-qa/`, `.agent-team/12-report/report.html`.

**Gate:** *Release Readiness* (PASS / CONCERNS / FAIL). If the independent review finds a blocking defect, fix and re-gate — the exact loop BATHOS itself went through.

---

## 11. Step 9 — Final confirmation

```text
/team-confirm
```

The lead signs off and cleans up the teammates. What you now have:

```
~/projects/readshelf/
├── src/ api/ web/ …                  ← the working ReadShelf service
└── .agent-team/
    ├── 02-market-analysis/  (USP)
    ├── 03-service-planning/ (features + stories)
    ├── 04-architecture/     (ERD, API, sequences)
    ├── 03-story-engineering/(dev story files)
    ├── 07-design/           (UX/UI)
    ├── 08-impl-notes/       (how it was built)
    ├── 10-review/ 11-qa/    (independent verification)
    ├── 12-report/report.html
    └── _state/              (manifest.json, wave-log.md, signoff.md)
```

Everything is traceable: market → USP → features → architecture → stories → code → verification.

---

## 12. The whole flow on one page

```text
# once
cd ~/tools/bathos && ./install.sh --into ~/projects/readshelf
export BATHOS_BIN="$HOME/tools/bathos/core/target/release/bathos"
export CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS=1

# Inside Claude Code opened at ~/projects/readshelf:
/team-kickoff                                   # .agent-team + manifest skeleton
/route          /Users/you/projects/readshelf   # → Lv2 recommended; you --confirm 2
/wave1-discovery   /Users/you/projects/readshelf # Caleb: market + USP
/wave2-design      /Users/you/projects/readshelf # Joshua → James + Jonnathan
/wave3-story-gate  /Users/you/projects/readshelf # ★ stories + readiness gate (FAIL blocks implementation)
/wave5-implement   /Users/you/projects/readshelf # Phillip + Andrew → code in src/
/wave6-verify-report /Users/you/projects/readshelf # review + QA + docs → report.html
/team-confirm                                   # sign-off + cleanup

# Check progress any time:
/team-status

# Save everything before stopping — resume in the next session:
/save            # or just say "save" / "checkpoint"
/resume          # next session: or just "continue" / "load"
```

---

## 13. Tips & common pitfalls

- **Save before stopping; resume when you're back.** A real build like ReadShelf spans multiple sessions. End every session with **`/save`** (one word — it captures engine state · git · decisions · the in-progress wave · the next command into `_state/SESSION-SNAPSHOT.md`). Start the next with **`/resume`**. Natural language works too: "save", "resume". Note that resuming doesn't revive teammates — just re-run the relevant `/waveN-…` command (lossless thanks to disk artifacts).
- **Teammates: concurrent ≤ 3.** Token cost scales with active roles. BATHOS already sequences the waves — don't try to run everything at once.
- **Spawn only the roles you need.** No ML → skip Stephen. Greenfield → skip John (reverse), or give him a reference repo.
- **You are the decider.** Level · USP · plan · gate verdicts — BATHOS recommends, you confirm. When BATHOS suggests a change of direction, it must present "recommendation + rationale + missed context" and must not act on its own.
- **A FAIL really does stop implementation.** If `/wave5-implement` won't start, check the Wave 3 verdict: `bathos -s .agent-team/_state gate show`. Fix the stories and re-gate.
- **No comment keys in the `settings.json` `hooks` block** — they cause an infinite wait when teammates start.
- **Handoffs are disk-only.** Teammates don't share the lead's conversation history; everything travels via `.agent-team/` files. This is a feature (zero context loss), not a limitation.
- **If a teammate looks "stuck" with no output,** it's usually the account usage limit, not a bug — respawn after the reset, and the disk artifacts are preserved.

---

## 14. What if ReadShelf were bigger?

| If ReadShelf were… | Stakes change | Level | Added waves |
|--------------------|---------------|-------|-------------|
| A one-line bug fix | scope=bug | **Lv0** | W5 only (+light W6) |
| A focused MVP *(this document)* | scope=feature, novelty | **Lv2** | W1+W2+W3+W5+W6 |
| A full new product/platform | scope=product, team=large | **Lv3** | adds **W0** (analysis) + optional **W4** (IP/research) |
| A regulation/patent-centric product | + regulation_ip=true | **Lv4** | full W0–W6 + **W4 required** |

To enable a plug (e.g. a patent draft for a Lv3+ product):

```bash
bathos -s .agent-team/_state --modules-dir modules plug enable ip
```

---

## 15. License

BATHOS is MIT-licensed and was independently re-implemented from first principles after a careful reverse analysis of [BMAD-METHOD](https://github.com/bmad-code-org/BMAD-METHOD) — with respect for the foundational prior work, whose trademarks are not used. Full text: [`../README.md`](../README.md).

---

<div align="center">

**BATHOS** · βάθος — depth, not surface
한국어: [`USECASE-kr.md`](USECASE-kr.md) · Español [`USECASE-es.md`](USECASE-es.md)

</div>
