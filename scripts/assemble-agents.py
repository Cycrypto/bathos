#!/usr/bin/env python3
"""BATHOS final assembly — compile spawnable Claude Code agents from canonical `_base`.

Reads the canonical role definitions in `.claude/agents/_base/NN-slug.md` (the single
source of truth, 3-layer model §5) and emits flat, Claude-Code-loadable agent files at
`.claude/agents/<slug>.md` for every role marked `spawnable: true`.

Why this exists (LD-5): Claude Code loads teammate agents from the top-level
`.claude/agents/*.md`; the `_base/` templates are NOT auto-loaded. Before distributing
the package (install.sh copies the whole `.claude/`), the flat registry must be
materialized so a downstream clone can spawn the wave teammates. Paul (role 0,
spawnable:false) is intentionally NOT emitted — he is the main/lead session, never a
spawned teammate.

Idempotent: re-running overwrites the generated flat files from `_base`. Deterministic
output (roles emitted in role_number order).
"""
from __future__ import annotations
import re
import sys
from pathlib import Path

PKG_ROOT = Path(__file__).resolve().parents[1]
BASE_DIR = PKG_ROOT / ".claude" / "agents" / "_base"
OUT_DIR = PKG_ROOT / ".claude" / "agents"


def parse_base(path: Path) -> tuple[dict, str]:
    """Split a `_base` file into (frontmatter dict, body). Skips `#` comment lines and
    strips inline ` # ...` comments from values. `tools` is parsed from a `[a, b]` list."""
    text = path.read_text(encoding="utf-8")
    parts = text.split("---\n")
    if len(parts) < 3:
        raise ValueError(f"{path.name}: expected YAML frontmatter delimited by ---")
    fm_raw, body = parts[1], "---\n".join(parts[2:])
    fm: dict[str, str] = {}
    for line in fm_raw.splitlines():
        s = line.strip()
        if not s or s.startswith("#"):  # comment / blank line inside frontmatter
            continue
        if ":" not in s:
            continue
        key, _, val = s.partition(":")
        key = key.strip()
        val = re.split(r"\s+#", val, 1)[0].strip()  # drop trailing inline comment
        fm[key] = val
    return fm, body.lstrip("\n")


def tools_csv(raw: str) -> str:
    """`[Read, Grep, Glob]` -> `Read, Grep, Glob`."""
    inner = raw.strip().lstrip("[").rstrip("]")
    items = [t.strip() for t in inner.split(",") if t.strip()]
    return ", ".join(items)


def tagline_of(body: str) -> str:
    """First blockquote line, de-marked (`> **…**` -> `…`)."""
    for line in body.splitlines():
        s = line.strip()
        if s.startswith(">"):
            t = s.lstrip(">").strip()
            if t.startswith("**") and t.endswith("**") and len(t) > 4:
                t = t[2:-2].strip()
            return t
    return ""


def clean_body(body: str) -> str:
    """Remove the internal `[base]` marker and the decorative ⭐ from the runtime agent."""
    return body.replace(" [base]", "").replace(" ⭐", "")


def main() -> int:
    if not BASE_DIR.is_dir():
        print(f"[assemble-agents] ✗ base dir not found: {BASE_DIR}", file=sys.stderr)
        return 1

    emitted, skipped = [], []
    for path in sorted(BASE_DIR.glob("*.md")):
        fm, body = parse_base(path)
        slug = fm.get("slug")
        if not slug:
            print(f"[assemble-agents] ✗ {path.name}: missing slug", file=sys.stderr)
            return 1
        if fm.get("spawnable", "false").lower() != "true":
            skipped.append(f"{slug} (spawnable=false)")
            continue

        role_no = fm.get("role_number", "?")
        name = fm.get("name", slug).capitalize()
        model = fm.get("model", "").strip()
        wave = fm.get("wave", "").strip()
        tools = tools_csv(fm.get("tools", "[]"))
        tagline = tagline_of(body)

        desc = f"Role {role_no} · {name} — {tagline} (wave: {wave})"
        out = (
            "---\n"
            f"name: {slug}\n"
            "description: |\n"
            f"  {desc}\n"
            f"tools: {tools}\n"
            f"model: {model}\n"
            "---\n\n"
            f"{clean_body(body)}"
        )
        if not out.endswith("\n"):
            out += "\n"
        (OUT_DIR / f"{slug}.md").write_text(out, encoding="utf-8")
        emitted.append(slug)

    print(f"[assemble-agents] ✓ emitted {len(emitted)} spawnable agents:")
    for s in emitted:
        print(f"    - {s}")
    if skipped:
        print(f"[assemble-agents] · skipped {len(skipped)}: {', '.join(skipped)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
