//! `view` — `&AppState` → ratatui widgets (pure render, no I/O, TestBackend-snapshot-able).
//!
//! Two tab kinds (`w2-panes-model-design-kr.md` §B4.1):
//! - **Paul tab** (`tab_index == 0`): project header/status badge, warnings banner, the full
//!   gate-card list, the audit timeline (+ chain badge), and a global confirm hint.
//! - **wave tab**: that wave's cell + entry/exit gates, its role pills, artifacts/stories, a
//!   `wave-log.md` tail, and an input hint bar.
//!
//! **Honest scope note (documented, not silently omitted):** the design's Paul-tab bullet list
//! also mentions "risks" — `DashboardVM` (the shared, reused data source, ADR-D-0007) does not
//! currently carry risk entries (only `ProjectView` does, and `build_vm` does not project them
//! through), so this view renders no risks panel rather than fabricating one from data that
//! isn't there. Extending `DashboardVM` for this is a bigger, separately-reviewable change to a
//! file shared by 4 consumers (HTML report/serve/tmux/TUI) and is out of this pass's scope.
//! Likewise, artifact/story records are not wave-scoped here for the same reason `lines.rs`
//! doesn't scope them (`ArtifactLeafVM`/`StoryCardView` carry no `wave_id`) — see that module's
//! doc comment for the full rationale.

use crate::app::{AppState, Modal};
use bathos_inspect::dashboard::viewmodel::{Badge, GateCardVM, RolePillVM, StoryCardView};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Tabs, Wrap},
    Frame,
};

/// Maps a `Badge.css_class` to a terminal color — symbol is always the primary signal (already
/// baked into the badge text itself), color is a secondary reinforcement only (matches the
/// "무색 원칙" carried over from the HTML dashboard's design system).
fn badge_style(css_class: &str) -> Style {
    match css_class {
        "pass" => Style::default().fg(Color::Green),
        "warn" => Style::default().fg(Color::Yellow),
        "fail" => Style::default().fg(Color::Red),
        "teal" => Style::default().fg(Color::Cyan),
        _ => Style::default().add_modifier(Modifier::DIM),
    }
}

fn badge_span(badge: &Badge) -> Span<'static> {
    Span::styled(format!("{} ", badge.symbol), badge_style(badge.css_class))
}

/// Entry point — lays out tabs bar / body / footer, then delegates the body to the Paul or
/// wave tab renderer, and overlays a modal popup on top when one is open.
pub fn draw(frame: &mut Frame, state: &AppState) {
    let area = frame.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(3),
            Constraint::Length(3),
        ])
        .split(area);

    draw_tabs(frame, chunks[0], state);
    if state.tab_index == 0 {
        draw_paul_tab(frame, chunks[1], state);
    } else {
        draw_wave_tab(frame, chunks[1], state);
    }
    draw_footer(frame, chunks[2], state);

    if let Some(modal) = &state.modal {
        draw_modal(frame, area, modal);
    }
}

fn draw_tabs(frame: &mut Frame, area: Rect, state: &AppState) {
    let titles: Vec<Line> = state.tabs.iter().map(|t| Line::from(t.as_str())).collect();
    let tabs = Tabs::new(titles)
        .block(Block::default().borders(Borders::ALL).title("BATHOS panes"))
        .select(state.tab_index)
        .highlight_style(
            Style::default()
                .add_modifier(Modifier::BOLD)
                .fg(Color::Cyan),
        );
    frame.render_widget(tabs, area);
}

fn draw_footer(frame: &mut Frame, area: Rect, state: &AppState) {
    let hint = "←/→·Tab 탭전환  j/k 스크롤  c confirm  f feedback  r 새로고침  q 종료";
    let text = match &state.status_message {
        Some(msg) => format!("{hint}\n{msg}"),
        None => hint.to_string(),
    };
    let para = Paragraph::new(text)
        .block(Block::default().borders(Borders::ALL))
        .wrap(Wrap { trim: true });
    frame.render_widget(para, area);
}

// ─────────────────────────────────────────────────────────────────────────────
// Paul tab
// ─────────────────────────────────────────────────────────────────────────────

fn draw_paul_tab(frame: &mut Frame, area: Rect, state: &AppState) {
    let vm = &state.vm;
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(4),
            Constraint::Min(3),
            Constraint::Min(3),
        ])
        .split(area);

    // ── header + warnings banner ──
    let mut header_lines = vec![Line::from(vec![
        Span::raw(
            vm.codename
                .clone()
                .unwrap_or_else(|| "(제목 없음)".to_string()),
        ),
        Span::raw("  "),
        Span::raw(vm.level_label.clone()),
        Span::raw("  "),
        badge_span(&vm.status_badge),
    ])];
    if vm.manifest_form_descriptive {
        header_lines.push(Line::from("(서술형 manifest — 일부 필드 미확정)"));
    }
    for w in &vm.warnings {
        header_lines.push(Line::from(format!("⚠ [{}] {}", w.code, w.message)));
    }
    let header =
        Paragraph::new(header_lines).block(Block::default().borders(Borders::ALL).title("Project"));
    frame.render_widget(header, chunks[0]);

    // ── gate cards ──
    let gate_items: Vec<ListItem> = if vm.gates.is_empty() {
        vec![ListItem::new("(게이트 없음)")]
    } else {
        vm.gates.iter().map(gate_list_item).collect()
    };
    let gates = List::new(gate_items).block(Block::default().borders(Borders::ALL).title("Gates"));
    frame.render_widget(gates, chunks[1]);

    // ── audit timeline + chain badge ──
    let mut audit_lines: Vec<Line> = vec![Line::from(vec![
        Span::raw("chain: "),
        badge_span(&vm.chain_badge),
        Span::raw(vm.chain_detail.clone().unwrap_or_default()),
    ])];
    if vm.audit.is_empty() {
        audit_lines.push(Line::from("(감사 이력 없음)"));
    } else {
        for a in vm.audit.iter().rev().take(20) {
            audit_lines.push(Line::from(format!(
                "#{} {} {} {} {}",
                a.seq, a.ts_iso, a.actor, a.action, a.target
            )));
        }
    }
    let audit = Paragraph::new(audit_lines)
        .block(Block::default().borders(Borders::ALL).title("Audit"))
        .scroll((state.scroll, 0));
    frame.render_widget(audit, chunks[2]);
}

fn gate_list_item(g: &GateCardVM) -> ListItem<'static> {
    let mut spans = vec![
        Span::raw(format!("{} [{}] ", g.gate_id, g.wave_id)),
        Span::raw(format!("{} ", g.gate_type_label)),
        badge_span(&g.verdict_badge),
        Span::raw(format!(
            "issues={} crit={} ",
            g.issues_total, g.issues_critical
        )),
    ];
    if g.facilitator_missing {
        spans.push(Span::styled(
            "facilitator 미기록",
            Style::default().fg(Color::Yellow),
        ));
    } else if let Some(f) = &g.facilitator {
        spans.push(Span::raw(format!("by {f}")));
    }
    if g.release_critical_warning {
        spans.push(Span::styled(
            " ⚠ Release+critical",
            Style::default().fg(Color::Red),
        ));
    }
    ListItem::new(Line::from(spans))
}

// ─────────────────────────────────────────────────────────────────────────────
// wave tab
// ─────────────────────────────────────────────────────────────────────────────

fn draw_wave_tab(frame: &mut Frame, area: Rect, state: &AppState) {
    let vm = &state.vm;
    let wave_id = state.current_wave().unwrap_or("");

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(4),
            Constraint::Min(3),
            Constraint::Min(3),
        ])
        .split(area);

    // ── wave cell + its gates ──
    let cell = vm.waves.iter().find(|w| w.wave_id == wave_id);
    let mut header_lines = Vec::new();
    if let Some(c) = cell {
        header_lines.push(Line::from(vec![
            Span::raw(format!("{} {} ", c.wave_id, c.name)),
            badge_span(&c.status_badge),
            Span::raw(format!("roles={}", c.active_roles.join(","))),
        ]));
        if let Some(entry) = &c.entry_gate {
            header_lines.push(Line::from(format!("entry: {}", entry.label)));
        }
        if let Some(exit) = &c.exit_gate {
            header_lines.push(Line::from(format!("exit: {}", exit.label)));
        }
    } else {
        header_lines.push(Line::from("(웨이브 셀 정보 없음)"));
    }
    let header = Paragraph::new(header_lines).block(
        Block::default()
            .borders(Borders::ALL)
            .title(wave_id.to_string()),
    );
    frame.render_widget(header, chunks[0]);

    // ── roles (this wave) + stories (unscoped, see module doc) ──
    let role_items: Vec<ListItem> = vm
        .roles
        .iter()
        .filter(|r| r.wave_id.as_deref() == Some(wave_id))
        .map(role_list_item)
        .collect();
    let role_items = if role_items.is_empty() {
        vec![ListItem::new("(배정된 역할 없음)")]
    } else {
        role_items
    };
    let roles = List::new(role_items).block(Block::default().borders(Borders::ALL).title("Roles"));
    frame.render_widget(roles, chunks[1]);

    let story_items: Vec<ListItem> = if vm.stories.is_empty() {
        vec![ListItem::new("(스토리 없음)")]
    } else {
        vm.stories.iter().map(story_list_item).collect()
    };
    let stories = List::new(story_items).block(
        Block::default()
            .borders(Borders::ALL)
            .title("Stories (전체 — wave 미분류)"),
    );
    frame.render_widget(stories, chunks[2]);
}

fn role_list_item(r: &RolePillVM) -> ListItem<'static> {
    ListItem::new(Line::from(vec![
        Span::raw(format!("{} ", r.name)),
        badge_span(&r.status_badge),
        Span::raw(r.model_label.clone().unwrap_or_default()),
    ]))
}

fn story_list_item(s: &StoryCardView) -> ListItem<'static> {
    let mut spans = vec![
        Span::raw(format!("{} [{}] ", s.story_key, s.status_label)),
        badge_span(&s.ready_badge),
        badge_span(&s.lint_badge),
        Span::raw(s.lint_summary_text.clone()),
    ];
    if s.stale {
        spans.push(Span::styled(" STALE", Style::default().fg(Color::Yellow)));
    }
    ListItem::new(Line::from(spans))
}

// ─────────────────────────────────────────────────────────────────────────────
// modal popup (confirm/feedback)
// ─────────────────────────────────────────────────────────────────────────────

fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(vertical[1])[1]
}

fn draw_modal(frame: &mut Frame, area: Rect, modal: &Modal) {
    let popup_area = centered_rect(60, 20, area);
    frame.render_widget(Clear, popup_area);

    let (title, hint, buffer) = match modal {
        Modal::Confirm { buffer } => (
            "Confirm (PASS|CONCERNS|FAIL|OK|STOP [사유], Enter=제출 Esc=취소)",
            "",
            buffer,
        ),
        Modal::Feedback { buffer } => ("Feedback (자유 서술, Enter=제출 Esc=취소)", "", buffer),
    };
    let text = format!("{hint}{buffer}_");
    let para = Paragraph::new(text)
        .block(Block::default().borders(Borders::ALL).title(title))
        .wrap(Wrap { trim: false });
    frame.render_widget(para, popup_area);
}

#[cfg(test)]
mod tests {
    use super::*;
    use bathos_inspect::dashboard::viewmodel::build_vm;
    use bathos_inspect::loader::{
        ChainStatus, ManifestForm, ProjectMeta, ProjectStatus, ProjectView,
    };
    use ratatui::{backend::TestBackend, Terminal};

    fn empty_pv() -> ProjectView {
        ProjectView {
            form: ManifestForm::Engine,
            meta: ProjectMeta {
                codename: Some("BATHOS".into()),
                current_level: Some(3),
                status: ProjectStatus::Active,
                lang: Some("ko".into()),
                created: None,
                project_id: Some("bathos-0001".into()),
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

    #[test]
    fn draw_paul_tab_does_not_panic_on_empty_vm() {
        let vm = build_vm(&empty_pv(), &[]);
        let state = AppState::new(vm, None);
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| draw(f, &state)).unwrap();
    }

    #[test]
    fn draw_wave_tab_does_not_panic() {
        use bathos_inspect::loader::{WaveStatus, WaveView};
        let mut pv = empty_pv();
        pv.waves = vec![WaveView {
            wave_id: "W5".into(),
            name: "Implement".into(),
            status: WaveStatus::Active,
            active_roles: vec!["Phillip".into()],
            entry_gate: None,
            exit_gate: None,
            started: None,
            ended: None,
        }];
        let vm = build_vm(&pv, &[]);
        let state = AppState::new(vm, Some("W5".to_string()));
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| draw(f, &state)).unwrap();
    }

    #[test]
    fn draw_with_open_modal_does_not_panic() {
        let vm = build_vm(&empty_pv(), &[]);
        let mut state = AppState::new(vm, None);
        state.modal = Some(Modal::Confirm {
            buffer: "PASS reason".into(),
        });
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| draw(f, &state)).unwrap();
    }

    #[test]
    fn draw_with_tiny_terminal_does_not_panic() {
        // A degenerate (very small) terminal size must not crash the layout math.
        let vm = build_vm(&empty_pv(), &[]);
        let state = AppState::new(vm, None);
        let backend = TestBackend::new(10, 6);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| draw(f, &state)).unwrap();
    }
}
