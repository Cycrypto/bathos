# BATHOS — Token & Quota Management

> A multi-agent pipeline burns tokens faster than a single conversation. This guide explains **what drives cost, how to control it, and how to recover when you hit a limit**. It reflects field lessons learned the expensive way: **the most common reason a teammate looks "stuck" is an account usage limit, not a bug.**
>
> **See also:** [Usage (USAGE-en)](USAGE-en.md) · [Use case (USECASE-en)](USECASE-en.md) · 한국어: [`QUOTA-kr.md`](QUOTA-kr.md) · Español: [`QUOTA-es.md`](QUOTA-es.md)

---

## 0. One-line summary

- **Pick the right level.** Scale-Adaptive Lv0–4 is the biggest cost lever — don't run W0–W6 for a bug fix.
- **Concurrent teammates ≤ 3.** Token cost scales roughly linearly with active roles; the engine hard-caps at 3 (`E-CONCURRENCY`).
- **Spawn only the roles you need.** No ML → skip Stephen. Greenfield → skip John.
- **Sequence waves; never run everything in parallel.** Each wave shuts down its teammates before the next.
- **`/save` before you stop.** Even if you hit a limit mid-wave, disk artifacts allow lossless resumption — after reset, just re-run the wave.
- **A teammate that goes silent with no output is usually quota, not a crash.** Wait for reset and respawn.

---

## 1. What drives cost

BATHOS consumes tokens through these factors:

| Driver | Why it matters | Lever |
|--------|----------------|-------|
| **Number of active teammates** | Each spawned role is an independent agent burning tokens; cost is ~linear in active roles | Concurrency ≤ 3; spawn only what's needed |
| **Model tier per role** | Opus roles cost more than Sonnet roles | When tight, run Sonnet-heavy waves first and keep Opus roles focused |
| **Number of waves run** | More waves → more total agent work | Scale-Adaptive level (only the waves you need) |
| **Re-gate cycles** | FAIL → fix → re-gate loops re-run work | Invest in W3 story quality so W5 doesn't churn |

**Role–model map** (who's expensive):
- **Opus 4.8:** Paul (lead), John, Caleb, Joshua, James, Jonnathan, Mark, Matthew (#17)
- **Sonnet 5:** Nathanael, Phillip, Andrew, Stephen, Timothy, Thomas, Michael, Hananiah, Matthias, Martin

In short, W2 (Joshua→James, Jonnathan) and W3 (Matthew) are Opus-dense (design-centric — the depth is worth it), while W5 (Phillip/Andrew/Stephen) and W6 (Thomas/Timothy/Matthias→Martin) are Sonnet-dense.

---

## 2. Lever #1 — Scale-Adaptive levels

Running fewer waves is the biggest saving. Rough scale:

| Lv | Waves run | Opus-dense waves | Relative cost |
|----|-----------|------------------|:-------------:|
| **0** | W5 (+ultra-light W6) | none | $ |
| **1** | light W2 + W3 (abridged) + W5 + light W6 | W2/W3 (abridged) | $$ |
| **2** | W1 + W2 + W3 + W5 + W6 | W2/W3 | $$$ |
| **3** | W0–W6 (W4 optional) | W0/W2/W3 | $$$$ |
| **4** | full W0–W6 + W4 | W0/W2/W3/W4 | $$$$$ |

**Scale honestly.** `/route` recommends a level from the stakes, but *you* confirm it (User Sovereignty). If a "new product" is actually a focused MVP, Lv2 is often better than Lv3 — it skips W0 market analysis and the W4 plug. You can always route upward later.

---

## 3. Lever #2 — Concurrency & role selection

- **Hard cap of 3.** `MAX_CONCURRENT_ROLES = 3`; a 4th spawn within a wave is rejected with `E-CONCURRENCY`. Don't fight it — respect it.
- **Spawn only roles with work to do.** W5 spawns Phillip/Andrew/Stephen by default, but:
  - No AI/ML in the product → **don't spawn Stephen**.
  - Greenfield app (no existing codebase to reverse) → **skip John** in W1, or give just one reference repo.
  - Backend-only change → Phillip alone.
- **Sequence — never everything in parallel.** Waves already run in order and shut teammates down in between. Respect intra-wave order too (e.g. in W2, James/Jonnathan start only after Joshua's planning gate).

---

## 4. Lever #3 — Defer the optional parts

- **W4 (IP · Research) is off the mainline.** If tokens are tight, run the mainline first (W0→W1→W2→W3→W5→W6) and W4 later — or skip it entirely this iteration.
- **Plan-review gates are optional.** `/plan-ceo-review`, `/plan-eng-review`, etc. add Opus review passes. Use them for high-stakes planning; skip for small work.

---

## 5. Hitting the usage limit — recognize & recover

**Recognize it.** If a spawned teammate looks **"stuck" with no output**, the most likely cause is the **account usage (session) limit**, not a code/hook bug. At the limit, a spawned teammate can't reason — the process stays alive but goes quiet, which looks like a hang. (The teammate's transcript will show a "you've hit your session limit / resets at …" message.)

**Recover — losslessly.** BATHOS is built for this because **every handoff lives on disk** (`.agent-team/`) and teammates never depend on in-memory conversation history:

1. **Don't kill anything rashly.** A quiet, limit-hit teammate isn't broken — it's waiting.
2. **`/save`** (if the lead can still act) to snapshot the session.
3. **Wait for the quota reset.**
4. **Respawn:** re-run the relevant `/waveN-…` command. Upstream artifacts are on disk, so the respawned teammate picks up exactly where things left off — zero work lost.

> This is why `/save`+`/resume` and disk-only handoffs matter: a usage limit becomes a pause, not a loss.

---

## 6. A practical low-quota playbook

When building lean on tokens:

1. `/route` → confirm the **lowest** level that fits (usually Lv1–Lv2).
2. Run waves **one at a time** and check `/team-status` in between.
3. In each wave, spawn **only the roles needed** (skip Stephen/John when irrelevant).
4. **Defer W4 and heavy plan-reviews.**
5. **`/save`** at the end of each session (and before long waves).
6. If a teammate goes quiet: assume quota, `/save`, wait for reset, `/resume` + re-run the wave.
7. When quota is plentiful (right after reset), prioritize the Opus-dense waves (W2/W3).

---

## 7. Related docs

- Levels & routing: [`USAGE-en.md`](USAGE-en.md) §4 · concurrency & waves: §2, §3
- Save/resume: [`USAGE-en.md`](USAGE-en.md) §12, [`FEATURES-en.md`](FEATURES-en.md) §2.11
- The "hang = quota" lesson and other field notes: [`USAGE-en.md`](USAGE-en.md) §13

---

<div align="center">

**BATHOS** · βάθος — depth, not surface
한국어: [`QUOTA-kr.md`](QUOTA-kr.md) · Español: [`QUOTA-es.md`](QUOTA-es.md)

</div>
