# BATHOS — Writing Custom Plug Modules

> Want to extend BATHOS with your own domain features (e.g. a security-audit pack, a game-design pack) **without touching the core**? That's exactly what plug modules are for. This guide walks you end-to-end through writing a module → installing it → enabling it → running it.
>
> **See also:** [Concepts (FEATURES-en)](FEATURES-en.md) · [Usage (USAGE-en)](USAGE-en.md) · 한국어: [`MODULE-GUIDE-kr.md`](MODULE-GUIDE-kr.md) · Español: [`MODULE-GUIDE-es.md`](MODULE-GUIDE-es.md)

---

## 0. What a module is (and the one rule)

The BATHOS **core stays slim**. Domain-specific features ship as **opt-in plug modules** under `modules/`. Two are bundled by default: `ip-pack` (patent application specifications) · `research-pack` (academic Abstract/Intro).

> **The one rule (invariant A9): the core never depends on modules.** Modules are discovered and toggled at runtime; the engine has zero compile-time knowledge of any particular module. This is what keeps the core small and lets domains be added freely.

A module contributes workflows, templates, and an output directory to **Wave 4 (IP · Research)** — the optional, off-mainline plug wave. Enable it with `bathos plug enable <id>`; state is persisted in `manifest.modules[]`.

---

## 1. Anatomy of a module

A module is, in the end, just one directory under `modules/<your-id>/` — this is the structure to aim for:

```
modules/security-pack/
├── module.yaml            # required — the module's self-declaration (the contract)
├── README.md              # recommended — what it is, how to use it
├── workflows/             # procedures the module provides
│   └── threat-model.md
├── templates/             # output templates the workflows fill in
│   └── threat-model-report.md
└── checklists/            # (optional) quality / adversarial checklists
    └── threat-model-quality.md
```

Strictly speaking, the only thing the engine needs to **discover** a module is `module.yaml`; everything else exists to help the role that runs the module *use it well*.

---

## 2. The `module.yaml` contract

`module.yaml` is where the module introduces itself to the engine. Below is the exact schema the engine (`bathos-plug`) actually parses, with every field annotated:

```yaml
module_id: security               # required — unique id (ip | research | game | security ...)
name: Security Pack               # required — human-readable name
wave: W4                          # required — the wave it plugs into (W4)
trigger: "Lv>=4 OR domain=security"  # required — auto-trigger condition (DSL below)
enabled_default: false            # optional (default false)
provides:                         # optional
  workflows: [threat-model]       #   file basenames under workflows/
  templates: [threat-model-report]#   file basenames under templates/
outputs: ".agent-team/13-security/"  # optional — where this module writes its outputs
evidence_trace: true              # optional (default false) — require [Source:] evidence tracing
```

| Field | Required? | Meaning |
|-------|:---------:|---------|
| `module_id` | ✓ | Unique id used by `plug enable/disable <id>` and triggers (`domain=<id>`) |
| `name` | ✓ | Name displayed by `plug list` |
| `wave` | ✓ | The wave the module runs in (currently `W4`) |
| `trigger` | ✓ | Auto-trigger condition (§3) |
| `enabled_default` | — | Enabled by default? (default `false`) |
| `provides.workflows` | — | Basenames of provided workflows |
| `provides.templates` | — | Basenames of provided templates |
| `outputs` | — | Output directory (pick a new `.agent-team/NN-<name>/`) |
| `evidence_trace` | — | If `true`, claims must carry a `[Source:]` marker (recommended) |

> For `outputs`, pick a new number that doesn't collide with the existing role directories (`00`–`12` are taken) — e.g. `13-security/`.

---

## 3. The trigger DSL

`trigger` tells the router *when* to auto-activate this module. Syntax (parsed by `bathos-plug`):

- **Level:** `Lv>=N` · `Lv>N` · `Lv<=N` · `Lv<N` · `Lv=N` (N = 0–4)
- **Domain:** `domain=X` (or `domain:X`) — true when the project domain matches `X` (usually the `module_id`)
- **Combination:** join terms with ` OR ` — triggers if **any** term is true.
- **Safety:** unparseable tokens evaluate to `false` (conservative).

Examples:
- `"Lv>=3 OR domain=ip"` — on for large/enterprise work, or when the domain is explicitly IP.
- `"Lv>=4 OR domain=security"` — on only for enterprise/regulated builds or an explicit security domain.
- `"domain=game"` — on only when the project domain is `game` (level alone never auto-enables it).

> Triggers are *auto-activation hints*. The user always retains control (User Sovereignty): you can `plug enable`/`disable` regardless, and you verify what actually runs.

---

## 4. Step by step — building a "security-pack"

Enough theory — let's build one. We'll construct `security-pack` from scratch, file by file.

### 4.1 Scaffold the directory
```bash
cd your-project    # (or the bathos repo)
mkdir -p modules/security-pack/{workflows,templates,checklists}
```

### 4.2 Write `module.yaml`
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

### 4.3 Write the workflow (`workflows/threat-model.md`)
A workflow is a markdown procedure a role follows. Make it concrete and step-based — for a security pack, e.g. an OWASP Top 10 + STRIDE pass that fills in the template. State which inputs to read and where outputs go, and (since `evidence_trace: true`) make clear that every finding needs a `[Source:]` marker.

### 4.4 Write the template (`templates/threat-model-report.md`)
The structured deliverable the workflow fills in, with headings for scope, assets, threats (per STRIDE), severity, mitigations, and residual risk. This is what accumulates under `outputs`.

### 4.5 (Optional) quality checklist (`checklists/threat-model-quality.md`)
An adversarial self-review checklist the role runs before declaring the workflow done (see `ip-pack/checklists/patent-quality.md` for reference).

### 4.6 Add a `README.md`
One screenful: what it produces, what triggers it, how to enable it.

---

## 5. Install · discover · enable

Once the files are in place, three commands take the module from "sitting on disk" to "alive in the manifest":

```bash
# 1) Discover — the engine reads modules/<id>/module.yaml
bathos --modules-dir modules plug list
#   → security-pack listed with its enabled state (JSON)

# 2) Enable — persisted into manifest.modules[]
bathos -s .agent-team/_state --modules-dir modules plug enable security

# 3) Disable when done
bathos -s .agent-team/_state --modules-dir modules plug disable security
#   nonexistent id → exit 1 (E-PLUG-NOTFOUND)
```

If you adopted BATHOS into your project via `install.sh --into`, put the module under that project's `modules/` (i.e. the directory `--modules-dir` points at).

---

## 6. Running it in a wave

Modules plug into **W4**. After enabling, run the W4 command and the responsible role (Mark/Nathanael for the bundled packs; for your own pack, assign the best-fit role or let the lead run it directly) executes the workflow and writes to the `outputs` directory:

```
/wave4-ip-research /abs/path/to/project
```

W4 sits outside the mainline (W0→W1→W2→W3→W5→W6), so it can run any time after W2 — if tokens are tight, feel free to defer it.

---

## 7. Validating a module

Before trusting it, run these four quick checks — each verifies one link in the chain from parsing to triggering:

- **Parse check:** `bathos --modules-dir modules plug list` — if the module shows up, `module.yaml` parsed. If not, check the 4 required fields (`module_id`, `name`, `wave`, `trigger`) and the YAML syntax.
- **Enable/persistence check:** after `plug enable <id>`, run `bathos -s _state state show` — confirm the id in `modules[]`.
- **Trigger check:** set the project level/domain and confirm the module is auto-recommended when `trigger` should fire.
- **Doctor:** `bathos doctor` still passes (the module doesn't touch the core — A9).

---

## 8. Good habits

A few habits keep modules clean, composable, and true to the BATHOS ethos:

- **Leave the core alone.** Everything lives under `modules/<id>/`. If you find yourself editing a `core/` crate for a module, stop — that's an A9 violation.
- **Pick non-overlapping `outputs` paths** (`13-…`, `14-…`).
- **Turn on `evidence_trace`** and require `[Source:]` on claims — it matches the "generation ≠ verification" ethos.
- **Write a checklist** so the role does an adversarial self-review before declaring done.
- **Model after the bundled packs** — `modules/ip-pack/` is the reference implementation (module.yaml + workflows + templates + checklists + README).

---

## 9. License

MIT. See [`../README.md`](../README.md).

---

<div align="center">

**BATHOS** · βάθος — depth, not surface
한국어: [`MODULE-GUIDE-kr.md`](MODULE-GUIDE-kr.md) · Español: [`MODULE-GUIDE-es.md`](MODULE-GUIDE-es.md)

</div>
