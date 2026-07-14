//! `render` — `DashboardVM` → a single HTML string (CF-1.6 report). No template engine
//! is used (architecture-overview §4 "no template engine needed") — pure string build +
//! `include_str!` asset inlining. All dynamic text is escaped via [`esc`] (even trusted
//! data may contain `<`/`&` — prevents breaking HTML structure).
//!
//! [Source: story-2-1-dashboard-report-kr.md file_structure_requirements,
//!          design-system-kr.md, ui-spec-kr.md §0]

use super::i18n;
use super::viewmodel::{Badge, DashboardVM, GateRefVM, TreeNode};
use super::Lang;

/// No-CDN inline assets (story 2-1 file_structure `assets/`). Baked into the binary at
/// build time, so there is no runtime file I/O or network request at all.
const STYLE_CSS: &str = include_str!("assets/style.css");
const SCRIPT_JS: &str = include_str!("assets/script.js");

// ─────────────────────────────────────────────────────────────────────────────
// common helpers
// ─────────────────────────────────────────────────────────────────────────────

/// HTML text/attribute escaping. Even trusted data (local read-only state) can break
/// structure if `<`/`&` is mixed in, so this is always applied.
fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

/// Renders `i18n::t(key)` as a `<span class="en">…</span><span class="kr">…</span>` pair
/// — the `data-lang` toggle hides one of this pair via CSS (CR-6).
fn bi(key: &str) -> String {
    let l = i18n::t(key);
    format!(r#"<span class="en">{}</span><span class="kr">{}</span>"#, esc(l.en), esc(l.kr))
}

/// Colorless status badge — symbol (aria-hidden) + bilingual label (visible text =
/// accessible name, no separate aria-label needed). [Source: design-system-kr.md §5.1]
fn badge_html(b: &Badge) -> String {
    format!(
        r#"<span class="badge {cls}"><span aria-hidden="true">{sym}</span> {lbl}</span>"#,
        cls = b.css_class,
        sym = b.symbol,
        lbl = bi(b.label_key)
    )
}

/// Wraps an RFC3339 string in `<time>`. Local-time conversion is done by browser JS
/// reading `data-iso` (the ISO original remains as a fallback even offline).
fn time_el(iso: &str) -> String {
    let e = esc(iso);
    format!(r#"<time datetime="{e}" data-iso="{e}">{e}</time>"#)
}

/// `Lang` → initial `data-lang` value. `Both` always embeds the toggle but defaults to en.
/// [Source: api-contracts-kr.md §A-1 "both = embed toggle"]
fn initial_lang_attr(lang: Lang) -> &'static str {
    match lang {
        Lang::Kr => "kr",
        Lang::En | Lang::Both => "en",
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// ① header + language toggle
// ─────────────────────────────────────────────────────────────────────────────

fn render_header(vm: &DashboardVM, initial_lang: Lang) -> String {
    let codename_html = match &vm.codename {
        Some(c) => format!(r#"<span class="codename">{}</span>"#, esc(c)),
        // B-1: a missing codename is not an error — "(untitled)" + (the banner shows the warning separately).
        None => format!(r#"<span class="codename missing">{}</span>"#, bi("nofollow.untitled")),
    };
    format!(
        r#"<header class="appbar">
  <div class="title">
    <span class="mono">{app_title}</span>
    {codename}
    <span class="chip">{level}</span>
    {status}
  </div>
  {lang_toggle}
</header>"#,
        app_title = bi("app.title"),
        codename = codename_html,
        level = esc(&vm.level_label),
        status = badge_html(&vm.status_badge),
        lang_toggle = render_lang_toggle(initial_lang),
    )
}

/// Language toggle (LangToggle) — flag emoji are allowed as an exception only in this component.
/// [Source: design-system-kr.md §5.6, ui-spec-kr.md §0]
fn render_lang_toggle(initial: Lang) -> String {
    let (en_pressed, kr_pressed) =
        if matches!(initial, Lang::Kr) { ("false", "true") } else { ("true", "false") };
    let group_label = i18n::t("lang.toggle_group");
    format!(
        r#"<div class="langtog" role="group" aria-label="{aria}">
    <button type="button" data-setlang="en" aria-pressed="{enp}"><span class="flag" aria-hidden="true">&#127482;&#127480;</span>EN</button>
    <button type="button" data-setlang="kr" aria-pressed="{krp}"><span class="flag" aria-hidden="true">&#127472;&#127479;</span>KR</button>
  </div>"#,
        aria = esc(&format!("{} / {}", group_label.en, group_label.kr)),
        enp = en_pressed,
        krp = kr_pressed,
    )
}

// ─────────────────────────────────────────────────────────────────────────────
// global warning banners (descriptive manifest / N parse warnings / N audit skips)
// ─────────────────────────────────────────────────────────────────────────────

fn render_banners(vm: &DashboardVM) -> String {
    let mut out = String::new();

    if vm.manifest_form_descriptive {
        out += &render_banner_row(bi("banner.descriptive"));
    }
    if vm.audit_skipped > 0 {
        out += &render_banner_row(format!(
            r#"<span class="num">{n}</span> {label}"#,
            n = vm.audit_skipped,
            label = bi("audit.skipped_suffix")
        ));
    }
    if !vm.warnings.is_empty() {
        let mut items = String::new();
        for w in &vm.warnings {
            items += &format!(
                r#"<li><code>{code}</code> — {msg} <span class="mono">({loc})</span></li>"#,
                code = esc(&w.code),
                msg = esc(&w.message),
                loc = esc(&w.location)
            );
        }
        let body = format!(
            r#"<details><summary>{label} ({n})</summary><ul>{items}</ul></details>"#,
            label = bi("banner.warnings"),
            n = vm.warnings.len(),
            items = items
        );
        out += &render_banner_row(body);
    }

    if out.is_empty() {
        return String::new();
    }
    format!(r#"<div class="banners">{out}</div>"#)
}

fn render_banner_row(body: impl Into<String>) -> String {
    format!(
        r#"<div class="banner"><div>{body}</div><button type="button" class="x" aria-label="{dismiss}">&times;</button></div>"#,
        body = body.into(),
        dismiss = esc(&format!(
            "{} / {}",
            i18n::t("banner.dismiss").en,
            i18n::t("banner.dismiss").kr
        )),
    )
}

// ─────────────────────────────────────────────────────────────────────────────
// ② wave rail
// ─────────────────────────────────────────────────────────────────────────────

fn render_wave_rail(vm: &DashboardVM) -> String {
    if vm.waves.is_empty() {
        // empty state — common in descriptive manifests (not an error). [Source: ui-spec §2 states]
        return format!(r#"<p class="empty-note">{}</p>"#, bi("empty.waves"));
    }
    let mut items = String::new();
    for w in &vm.waves {
        let active_class = if w.is_active { " active" } else { "" };
        let now_badge = if w.is_active {
            format!(r#" <span class="badge teal">{}</span>"#, bi("wave.now"))
        } else {
            String::new()
        };
        let gates = format!(
            "{}{}",
            gate_ref_html("gate.entry_label", &w.entry_gate),
            gate_ref_html("gate.exit_label", &w.exit_gate),
        );
        let roles = if w.active_roles.is_empty() {
            String::new()
        } else {
            let chips: Vec<String> =
                w.active_roles.iter().map(|r| format!("<span>·{}</span>", esc(r))).collect();
            chips.join(" ")
        };
        items += &format!(
            r#"<li><div class="wave-cell{active_class}">
  <div class="id"><span class="mono">{wid}</span>{status}{now}</div>
  <div class="nm">{name}</div>
  <div class="gates">{gates}</div>
  <div class="roles">{roles}</div>
</div></li>"#,
            active_class = active_class,
            wid = esc(&w.wave_id),
            status = badge_html(&w.status_badge),
            now = now_badge,
            name = esc(&w.name),
            gates = gates,
            roles = roles,
        );
    }
    format!(r#"<ol class="wave-rail">{items}</ol>"#)
}

/// entry/exit gate badge. **When the key is absent, the badge itself is omitted** (not an error).
/// [Source: design-handoff-kr.md §1.2 "omit badge when key absent"]
fn gate_ref_html(label_key: &str, g: &Option<GateRefVM>) -> String {
    match g {
        None => String::new(),
        Some(gr) => {
            let verdict = gr.verdict_badge.as_ref().map(badge_html).unwrap_or_default();
            format!(
                r#"<span class="chip">{kind}: {label} {verdict}</span>"#,
                kind = bi(label_key),
                label = esc(&gr.label),
                verdict = verdict,
            )
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// ③ gate badge panel
// ─────────────────────────────────────────────────────────────────────────────

fn render_gate_panel(vm: &DashboardVM) -> String {
    if vm.gates.is_empty() {
        return format!(r#"<p class="empty-note">{}</p>"#, bi("empty.gates"));
    }
    let mut cards = String::new();
    for g in &vm.gates {
        let facilitator_html = if g.facilitator_missing {
            format!(r#"<span class="badge warn"><span aria-hidden="true">★</span> {}</span>"#, bi("gate.facilitator_missing"))
        } else {
            esc(g.facilitator.as_deref().unwrap_or(""))
        };
        let release_warning = if g.release_critical_warning {
            format!(
                r#"<div class="release-warn"><span class="badge fail"><span aria-hidden="true">✗</span> {}</span></div>"#,
                bi("gate.release_warning")
            )
        } else {
            String::new()
        };
        let story_row = g
            .story_key
            .as_ref()
            .map(|k| format!(r#"<div class="row mono">{}</div>"#, esc(k)))
            .unwrap_or_default();
        let report_row = g
            .report_path
            .as_ref()
            .map(|p| {
                format!(
                    r#"<div class="row"><a href="{href}">{label}</a></div>"#,
                    href = esc(p),
                    label = bi("gate.report_link")
                )
            })
            .unwrap_or_default();
        let decided_row = match &g.decided_iso {
            Some(iso) => format!(
                r#"<div class="row">{label}: {t}</div>"#,
                label = bi("gate.decided_label"),
                t = time_el(iso)
            ),
            None => String::new(),
        };
        cards += &format!(
            r#"<div class="gate-card {cls}">
  <div class="hd">{verdict}<span class="gt mono">{gt}</span></div>
  <div class="row">{fac_label}: {fac}</div>
  <div class="row"><span class="num">{total}</span> {total_label} &middot; <span class="num">{crit}</span> {crit_label}</div>
  {release_warning}{story_row}{report_row}{decided_row}
</div>"#,
            cls = g.verdict_badge.css_class,
            verdict = badge_html(&g.verdict_badge),
            gt = esc(&g.gate_type_label),
            fac_label = bi("gate.facilitator_label"),
            fac = facilitator_html,
            total = g.issues_total,
            total_label = bi("gate.issues_total"),
            crit = g.issues_critical,
            crit_label = bi("gate.issues_critical"),
            release_warning = release_warning,
            story_row = story_row,
            report_row = report_row,
            decided_row = decided_row,
        );
    }
    format!(r#"<div class="gate-grid">{cards}</div>"#)
}

// ─────────────────────────────────────────────────────────────────────────────
// ④ audit timeline + integrity
// ─────────────────────────────────────────────────────────────────────────────

fn render_audit_section(vm: &DashboardVM) -> String {
    let detail = vm
        .chain_detail
        .as_ref()
        .map(|d| format!(r#" <span class="mono">{}</span>"#, esc(d)))
        .unwrap_or_default();
    let mut out =
        format!(r#"<div class="chain-badge-row">{badge}{detail}</div>"#, badge = badge_html(&vm.chain_badge));

    if vm.audit_skipped > 0 {
        out += &format!(
            r#"<p class="skip-note">{n} {label}</p>"#,
            n = vm.audit_skipped,
            label = bi("audit.skipped_suffix")
        );
    }

    if vm.audit.is_empty() {
        out += &format!(r#"<p class="empty-note">{}</p>"#, bi("empty.audit"));
        return out;
    }

    let mut items = String::new();
    for a in &vm.audit {
        items += &format!(
            r#"<li>
  <div class="ts">{time}<span class="actor mono">{actor}</span></div>
  <div class="action">{action}</div>
  <div class="target mono">&rarr; {target}</div>
</li>"#,
            time = time_el(&a.ts_iso),
            actor = esc(&a.actor),
            action = esc(&a.action),
            target = esc(&a.target),
        );
    }
    out += &format!(r#"<ol class="timeline">{items}</ol>"#);
    out
}

// ─────────────────────────────────────────────────────────────────────────────
// ⑤ side panel: artifact tree / team activity / stale / story cards
// ─────────────────────────────────────────────────────────────────────────────

fn render_artifact_tree(nodes: &[TreeNode]) -> String {
    if nodes.is_empty() {
        return String::new();
    }
    let mut out = String::from(r#"<ul class="tree">"#);
    for n in nodes {
        out += &render_tree_node(n);
    }
    out += "</ul>";
    out
}

fn render_tree_node(n: &TreeNode) -> String {
    let label = if n.leaf.is_some() {
        format!(r#"<span class="fname">{}</span>"#, esc(&n.name))
    } else {
        format!(r#"<span class="dirname">{}/</span>"#, esc(&n.name))
    };
    let (meta, title_attr) = match &n.leaf {
        Some(leaf) => {
            let owner = leaf
                .owner_role
                .as_ref()
                .map(|o| format!(r#"<span class="chip">{}</span>"#, esc(o)))
                .unwrap_or_else(|| r#"<span class="chip">&mdash;</span>"#.to_string());
            let updated =
                leaf.updated_iso.as_ref().map(|iso| time_el(iso)).unwrap_or_else(|| "&mdash;".to_string());
            let title = leaf
                .sha256
                .as_ref()
                .map(|s| format!(r#" title="sha256:{}""#, esc(s)))
                .unwrap_or_default();
            (format!(r#"{owner} <span class="mono">{updated}</span>"#), title)
        }
        None => (String::new(), String::new()),
    };
    let children = render_artifact_tree(&n.children);
    format!(r#"<li><div class="leaf"{title_attr}>{label} {meta}</div>{children}</li>"#)
}

fn render_roles(vm: &DashboardVM) -> String {
    if vm.roles.is_empty() {
        return format!(r#"<p class="empty-note">{}</p>"#, bi("empty.roles"));
    }
    let mut out = String::new();
    for r in &vm.roles {
        let rn = r.role_no.map(|n| format!("#{n}")).unwrap_or_default();
        let model = r
            .model_label
            .as_ref()
            .map(|m| format!(r#"<span class="model">{}</span>"#, esc(m)))
            .unwrap_or_default();
        let wave = r
            .wave_id
            .as_ref()
            .map(|w| format!(r#"<span class="chip">{}</span>"#, esc(w)))
            .unwrap_or_default();
        let paths = if r.owned_paths.is_empty() {
            String::new()
        } else {
            format!(r#"<div class="paths">{}</div>"#, esc(&r.owned_paths.join(" &middot; ")))
        };
        out += &format!(
            r#"<div class="role-pill">{badge}<span class="mono">{name}</span><span class="rn">{rn}</span>{model}{wave}{paths}</div>"#,
            badge = badge_html(&r.status_badge),
            name = esc(&r.name),
            rn = esc(&rn),
            model = model,
            wave = wave,
            paths = paths,
        );
    }
    out
}

fn render_stale(vm: &DashboardVM) -> String {
    if vm.stale_story_keys.is_empty() {
        // empty = fresh — no badge (neither error nor warning). [Source: ui-spec §6.3]
        return format!(r#"<p class="empty-note">{}</p>"#, bi("empty.stale"));
    }
    let stale_badge = Badge { symbol: "★", css_class: "warn", label_key: "stale.badge" };
    let mut out = String::from(r#"<div class="stale-list">"#);
    for k in &vm.stale_story_keys {
        out += &format!(
            r#"<div class="item">{badge}<span class="mono">{key}</span></div>"#,
            badge = badge_html(&stale_badge),
            key = esc(k)
        );
    }
    out += "</div>";
    out
}

/// Story cards — renders `vm.stories`, which `dashboard/mod.rs::load_story_cards` fills by
/// integrating `story::list_stories`+`lint_story` (stories 3-1/3-2) ([item 3],
/// design-vs-impl-gaps-kr.md §2 [R] resolved — it used to be a stub so it was always an
/// empty slice and this section was omitted entirely).
///
/// Like the artifacts/roles/stale panels (8-state principle), even with 0 stories the
/// section itself is always shown, guided by a "no stories" placeholder — hiding the whole
/// section would prevent the user from distinguishing "no folder", "folder exists but 0",
/// and "not integrated yet".
/// [Source: viewmodel.rs StoryCardView, ui-spec-kr.md §3.1/§5.3, §6.3 (sibling-panel convention)]
fn render_stories_section(vm: &DashboardVM) -> String {
    let body = if vm.stories.is_empty() {
        format!(r#"<p class="empty-note">{}</p>"#, bi("empty.stories"))
    } else {
        let mut items = String::new();
        for s in &vm.stories {
            let stale_mark = if s.stale {
                format!(
                    r#" <span class="badge warn"><span aria-hidden="true">★</span> {}</span>"#,
                    bi("stale.badge")
                )
            } else {
                String::new()
            };
            items += &format!(
                r#"<div class="item"><span class="mono">{key}</span> {ready}<span class="chip">{status}</span> {lint}<span class="mono">{summary}</span>{stale}</div>"#,
                key = esc(&s.story_key),
                ready = badge_html(&s.ready_badge),
                status = esc(&s.status_label),
                lint = badge_html(&s.lint_badge),
                summary = esc(&s.lint_summary_text),
                stale = stale_mark,
            );
        }
        format!(r#"<div class="side-body">{items}</div>"#)
    };
    format!(
        r#"<details class="side-section" open>
  <summary>{title}</summary>
  {body}
</details>"#,
        title = bi("nav.stories"),
        body = body,
    )
}

// ─────────────────────────────────────────────────────────────────────────────
// footer
// ─────────────────────────────────────────────────────────────────────────────

fn render_footer(vm: &DashboardVM) -> String {
    let created = match &vm.created_iso {
        Some(iso) => time_el(iso),
        None => "&mdash;".to_string(),
    };
    let form_label =
        if vm.manifest_form_descriptive { bi("footer.manifest.descriptive") } else { bi("footer.manifest.engine") };
    format!(
        r#"<footer>
  <span>{gen_label}: {gen_time}</span>
  <span>{created_label}: {created}</span>
  <span class="chip">{form_label}</span>
  <span>{readonly}</span>
</footer>"#,
        gen_label = bi("footer.generated"),
        gen_time = time_el(&vm.generated_at_iso),
        created_label = bi("footer.created"),
        created = created,
        form_label = form_label,
        readonly = bi("app.readonly"),
    )
}

// ─────────────────────────────────────────────────────────────────────────────
// final assembly
// ─────────────────────────────────────────────────────────────────────────────

/// `DashboardVM` → a complete single HTML document string. Zero external CDN requests
/// (CSS/JS inlined). [Source: api-contracts-kr.md §A-1, §C `build_report`]
pub fn render_html(vm: &DashboardVM, initial_lang: Lang) -> String {
    let initial = initial_lang_attr(initial_lang);
    let html_lang_attr = if initial == "kr" { "ko" } else { "en" };
    let title_text = vm.codename.clone().unwrap_or_else(|| i18n::t("nofollow.untitled").en.to_string());

    let artifacts_body = if vm.artifacts_tree.is_empty() {
        format!(r#"<p class="empty-note">{}</p>"#, bi("empty.artifacts"))
    } else {
        render_artifact_tree(&vm.artifacts_tree)
    };

    format!(
        r#"<!doctype html>
<html lang="{html_lang}">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<meta name="robots" content="noindex,nofollow">
<meta name="generator" content="bathos-inspect">
<title>BATHOS inspect &mdash; {title}</title>
<style>{css}</style>
</head>
<body>
<div class="wrap" id="bathos-inspect-wrap" data-lang="{lang}">
{header}
{banners}
<div class="shell">
  <div class="layout">
    <main>
      <section class="block" aria-labelledby="h-waves">
        <h2 id="h-waves">{waves_title}</h2>
        {waves}
      </section>
      <section class="block" aria-labelledby="h-gates">
        <h2 id="h-gates">{gates_title}</h2>
        {gates}
      </section>
      <section class="block" aria-labelledby="h-audit">
        <h2 id="h-audit">{audit_title}</h2>
        {audit}
      </section>
    </main>
    <aside>
      <details class="side-section" open>
        <summary>{artifacts_title}</summary>
        <div class="side-body">{artifacts}</div>
      </details>
      <details class="side-section" open>
        <summary>{roles_title}</summary>
        <div class="side-body">{roles}</div>
      </details>
      <details class="side-section" open>
        <summary>{stale_title}</summary>
        <div class="side-body">{stale}</div>
      </details>
      {stories}
    </aside>
  </div>
</div>
{footer}
</div>
<script>{js}</script>
</body>
</html>
"#,
        html_lang = html_lang_attr,
        title = esc(&title_text),
        css = STYLE_CSS,
        lang = initial,
        header = render_header(vm, initial_lang),
        banners = render_banners(vm),
        waves_title = bi("nav.waves"),
        waves = render_wave_rail(vm),
        gates_title = bi("nav.gates"),
        gates = render_gate_panel(vm),
        audit_title = bi("nav.audit"),
        audit = render_audit_section(vm),
        artifacts_title = bi("nav.artifacts"),
        artifacts = artifacts_body,
        roles_title = bi("nav.roles"),
        roles = render_roles(vm),
        stale_title = bi("nav.stale"),
        stale = render_stale(vm),
        stories = render_stories_section(vm),
        footer = render_footer(vm),
        js = SCRIPT_JS,
    )
}

// ─────────────────────────────────────────────────────────────────────────────
// tests — no-CDN / bilingual / accessibility / non-USP / 8-state (testing_requirements)
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dashboard::viewmodel::build_vm;
    use crate::loader::{ChainStatus, ManifestForm, ProjectMeta, ProjectStatus, ProjectView};

    fn empty_pv() -> ProjectView {
        ProjectView {
            form: ManifestForm::Descriptive,
            meta: ProjectMeta {
                codename: None,
                current_level: Some(2),
                status: ProjectStatus::Unknown(String::new()),
                lang: None,
                created: None,
                project_id: None,
            },
            waves: vec![],
            gates: vec![],
            roles: vec![],
            tasks: vec![],
            risks: vec![],
            artifacts: vec![],
            routing: vec![],
            modules: vec![],
            stale_story_keys: vec![],
            audit: vec![],
            chain_status: ChainStatus::Absent,
            audit_skipped: 0,
            warnings: vec![],
        }
    }

    /// **No-CDN core check**: the rendered document must have zero external protocol references.
    /// [Source: testing_requirements "no-CDN check (core)"]
    #[test]
    fn rendered_html_has_zero_external_protocol_references() {
        let vm = build_vm(&empty_pv(), &[]);
        let html = render_html(&vm, Lang::Both);
        assert!(!html.contains("http://"), "외부 http 참조 발견 — 무CDN 위반");
        assert!(!html.contains("https://"), "외부 https 참조 발견 — 무CDN 위반");
    }

    /// Bilingual: both en/kr spans exist and en≠kr (at least for core labels).
    #[test]
    fn rendered_html_embeds_both_languages() {
        let vm = build_vm(&empty_pv(), &[]);
        let html = render_html(&vm, Lang::Both);
        assert!(html.contains(r#"class="en""#));
        assert!(html.contains(r#"class="kr""#));
        assert!(html.contains("data-lang=\"en\""), "both 기본 초기값은 en");
    }

    /// `--lang kr` → initial data-lang=kr.
    #[test]
    fn lang_kr_sets_initial_data_lang_kr() {
        let vm = build_vm(&empty_pv(), &[]);
        let html = render_html(&vm, Lang::Kr);
        assert!(html.contains("data-lang=\"kr\""));
    }

    /// No color emoji — only the language toggle's flags (the allowed exception) and no other emoji.
    /// [Source: project-context-kr.md CR-5, ui-spec §5]
    #[test]
    fn no_color_emoji_outside_lang_toggle_flags() {
        let vm = build_vm(&empty_pv(), &[]);
        let html = render_html(&vm, Lang::Both);
        for banned in ['✅', '❌', '⚠', '🔥', '💯', '🚀', '🎉'] {
            assert!(!html.contains(banned), "금지된 컬러 이모지 '{banned}' 발견");
        }
        // spot-check that only allowed symbols are used (not a full charset check).
        assert!(html.contains('·') || html.contains('—'));
    }

    /// B-1: codename=None → "(untitled)" banner/header, neither a crash nor a global error.
    #[test]
    fn missing_codename_renders_untitled_placeholder_not_error() {
        let vm = build_vm(&empty_pv(), &[]); // empty_pv().meta.codename == None
        let html = render_html(&vm, Lang::Both);
        assert!(html.contains("codename missing"));
        assert!(html.contains("(untitled)") || html.contains("(제목 없음)"));
        // The CSS reserves a `.error-card` class definition (in anticipation of serve reuse),
        // but a missing codename alone must not render an actual error-card *element* (B-1).
        assert!(
            !html.contains(r#"class="error-card""#),
            "codename 부재는 전역 에러가 아니다(B-1)"
        );
    }

    /// Descriptive manifest → footer tag + warning banner (not an error card).
    #[test]
    fn descriptive_manifest_shows_banner_and_footer_tag_not_error() {
        let vm = build_vm(&empty_pv(), &[]); // form = Descriptive
        let html = render_html(&vm, Lang::Both);
        assert!(html.contains("banners"));
        assert!(html.contains("footer.manifest.descriptive") || html.contains("서술형 manifest"));
    }

    /// Empty arrays (waves/gates/artifacts) → render the "none" placeholder.
    #[test]
    fn empty_collections_render_placeholder_text() {
        let vm = build_vm(&empty_pv(), &[]);
        let html = render_html(&vm, Lang::Both);
        assert!(html.contains("empty-note"));
    }

    /// Accessibility landmarks (header/main/aside/footer) present.
    #[test]
    fn accessibility_landmarks_present() {
        let vm = build_vm(&empty_pv(), &[]);
        let html = render_html(&vm, Lang::Both);
        assert!(html.contains("<header"));
        assert!(html.contains("<main>"));
        assert!(html.contains("<aside>"));
        assert!(html.contains("<footer>"));
        assert!(html.contains("<ol class=\"wave-rail\">") || html.contains("empty-note"));
    }

    /// Inline CSS/JS is actually embedded in the document (no external `<link>`/`<script src>`).
    #[test]
    fn css_and_js_are_inlined_no_external_link_or_src() {
        let vm = build_vm(&empty_pv(), &[]);
        let html = render_html(&vm, Lang::Both);
        assert!(html.contains("<style>"));
        assert!(html.contains("<script>"));
        assert!(!html.contains("<link "), "외부 스타일시트 링크 금지");
        assert!(!html.contains("<script src="), "외부 스크립트 금지");
        // whether reduced-motion / focus-visible rules were actually inserted (design-system §6).
        assert!(html.contains("prefers-reduced-motion"));
        assert!(html.contains(":focus-visible"));
    }

    /// Responsive breakpoint (640/1024) CSS present. [Source: design-handoff §6]
    #[test]
    fn responsive_breakpoints_present_in_inlined_css() {
        let vm = build_vm(&empty_pv(), &[]);
        let html = render_html(&vm, Lang::Both);
        assert!(html.contains("639px") || html.contains("640px"));
        assert!(html.contains("1023px") || html.contains("1024px"));
    }

    /// Non-USP guard: the entire render output has no token/cost-related wording.
    #[test]
    fn rendered_html_never_shows_token_or_cost_figures() {
        let vm = build_vm(&empty_pv(), &[]);
        let html = render_html(&vm, Lang::Both).to_lowercase();
        for forbidden in ["token", " cost", "usd", "$/"] {
            assert!(!html.contains(forbidden), "비-USP 위반 문구 '{forbidden}' 발견");
        }
    }

    /// A Release + critical>0 gate actually emits the visual warning markup (integration check).
    #[test]
    fn release_gate_critical_warning_renders_in_html() {
        use crate::loader::{GateType, GateView, Verdict};
        let mut pv = empty_pv();
        pv.gates = vec![GateView {
            gate_id: "g1".into(),
            wave_id: "W6".into(),
            story_key: None,
            gate_type: GateType::Release,
            verdict: Verdict::Pass,
            issues_total: 2,
            issues_critical: 2,
            report_path: None,
            facilitator: Some("Thomas".into()),
            decided: None,
        }];
        let vm = build_vm(&pv, &[]);
        let html = render_html(&vm, Lang::Both);
        assert!(html.contains("release-warn"));
    }
}
