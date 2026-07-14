# BATHOS — Role Customization (3-Layer Overrides)

> Tune the 17 roles to your project (language, owned paths, platform, tone) **without touching the base definitions**. What you change is *how* a role applies its craft — not *who* the role is.
>
> **See also:** [Writing modules (MODULE-GUIDE-en)](MODULE-GUIDE-en.md) · [Usage (USAGE-en)](USAGE-en.md) · 한국어: [`ROLE-GUIDE-kr.md`](ROLE-GUIDE-kr.md) · Español: [`ROLE-GUIDE-es.md`](ROLE-GUIDE-es.md)

---

## 0. The three layers

A role's effective definition is the ordered merge of three layers:

| Layer | Location | Owns |
|-------|----------|------|
| **base** | `.claude/agents/_base/NN-*.md` | Fixed identity — name · background · model + top-tier craft standards / philosophy / DoD |
| **team** | Project overrides | Owned paths · platform (web/iOS…) · domain emphasis · tech stack · NFR thresholds · tone & voice |
| **user** | Personal overrides | Language · verbosity · facilitation intensity |

**Merge rule:** scalars are overwritten (user > team > base), arrays are appended. So the user sets the language, the team sets the owned paths, and both compose on top of the fixed base identity.

## 1. What cannot be overridden
The base layer pins **name · background · model** plus the craft standards. This is deliberately immutable — it's what keeps roles "top-tier" and consistent. Override *how the craft is applied to your project*, not *who they are*.

## 2. Anatomy of a base role
Every base file (`.claude/agents/_base/NN-name.md`) is YAML frontmatter + body:

```yaml
---
role_number: 4
name: james
slug: james-architect          # ← the agent-type slug used to spawn
model: opus                     # opus = Opus 4.8 · sonnet = Sonnet 5
wave: W2 (after Joshua completes)
spawnable: true                 # Paul (#0) is false — the lead is the main session
tools: [Read, Grep, Glob, Bash, Write, WebFetch, WebSearch]
---
```
Body sections (standardized after the uplift): Identity · §0 Philosophy · Mission & Deliverables · Craft Standards (non-negotiable) · Anti-patterns · Process · DoD · 3-layer note.

## 3. Adding a team override (examples)
Create a team-layer note that composes on top of the base role. Common team overrides:
- **Owned paths** — e.g. Andrew (#9) = `web/`, Phillip (#8) = `api/` (zero overlap → parallel-safe).
- **Platform/stack** — "web = Next.js", "backend = Go + Postgres".
- **NFR thresholds** — "p95 < 150ms", "bundle < 200KB".
- **Domain emphasis/tone** — brand voice, regulatory focus.

Team overrides should be limited to *project-specific facts*; leave craft standards to the base.

## 4. Adding a user override
Personal settings that span projects:
- **Language** — Korean/English output.
- **Verbosity** — terse vs. detailed.
- **Facilitation** — how proactively roles make suggestions (User Sovereignty always applies).

## 5. Spawning roles
The lead (Paul) spawns teammates by `slug` (agent-type), passing input paths, owned paths, and the DoD. Example: "Spawn `james-architect`, read `03-service-planning/`, own `04-architecture/`, follow ETHOS.md." Concurrency ≤ 3; shut down at the end of the wave.

## 6. Good habits
A few habits keep customization clean, parallel-safe, and faithful to the base:

- **Don't fork the base** to change project facts — use the team/user layers.
- **Keep owned paths disjoint** — assign non-overlapping paths to parallel roles (freeze-guard enforces this).
- **Maintain the uplifted bar** — when adding a new role, follow the 6-section top-tier structure (philosophy · craft standards · anti-patterns · process · DoD).
- If what you need is a *domain pack* rather than a *role*, use a plug ([`MODULE-GUIDE-en.md`](MODULE-GUIDE-en.md)).

---

<div align="center">한국어: <a href="ROLE-GUIDE-kr.md">ROLE-GUIDE-kr</a> · Español: <a href="ROLE-GUIDE-es.md">ROLE-GUIDE-es</a></div>
