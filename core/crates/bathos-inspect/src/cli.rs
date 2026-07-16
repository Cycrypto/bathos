//! `bathos inspect <sub>` clap contract — common flags (`CommonArgs`) + subcommands (`InspectCommand`).
//!
//! `bathos-cli` embeds this module's types into its own `Commands::Inspect`
//! variant via `#[command(flatten)]`/`#[command(subcommand)]` (delegation pattern, minimally invasive).
//! [Source: story-1-1-crate-scaffold-cli-kr.md AC, api-contracts-kr.md §A]

use std::path::PathBuf;

use clap::{Args, Subcommand, ValueEnum};

/// Flags common to all subcommands (CR-7). `global = true` allows them before or after the subcommand.
///
/// - `--path` : target `.agent-team` directory (auto-discovered if unset). [Source: api-contracts-kr.md §A-0]
/// - `--json` : machine-readable output.
/// - `-v/--verbose` : verbose output.
/// - `--strict` : escalate to exit 2 when a FAIL is present, e.g. in doctor (§A-4). Passed through as `InspectCtx.strict`.
#[derive(Debug, Clone, Default, Args)]
pub struct CommonArgs {
    /// Target `.agent-team` directory. If unset, auto-discovered by walking up from cwd.
    #[arg(long, global = true)]
    pub path: Option<PathBuf>,

    /// Machine-readable JSON output (instead of human text).
    #[arg(long, global = true)]
    pub json: bool,

    /// Verbose output.
    #[arg(short = 'v', long = "verbose", global = true)]
    pub verbose: bool,

    /// Escalate to exit 2 when a FAIL is present (doctor `--strict`). [Source: api-contracts-kr.md §A-4]
    #[arg(long, global = true)]
    pub strict: bool,
}

/// Dashboard display language. [Source: api-contracts-kr.md §A-1/§A-2, project-context-kr.md CR-6]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, ValueEnum)]
pub enum LangArg {
    En,
    Kr,
    /// Toggle-embedded (includes en↔kr switch UI) — the default.
    #[default]
    Both,
}

/// `bathos inspect vm --format` — the shared `DashboardVM` data source (ADR-D-0007), rendered
/// either as the existing pretty JSON dump (`json`, byte-identical to `report --json`) or as
/// the bash-3.2/jq-free TSV "lines protocol" (`lines`, consumed by `scripts/bathos-panes.sh`
/// and the `bathos-tui` crate). [Source: w2-panes-model-design-kr.md §B1]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, ValueEnum)]
pub enum VmFormat {
    #[default]
    Json,
    Lines,
}

/// The four `bathos inspect <sub>` subcommands.
///
/// This story (1-1) defines only the contract surface (flags); the handlers are stubs.
/// The actual logic is filled in by later stories (2-1 report, 2-2 serve, 3-1/3-2 story, 4-1/4-2 doctor).
/// [Source: story-1-1-crate-scaffold-cli-kr.md AC, api-contracts-kr.md §A-1~§A-4]
#[derive(Debug, Clone, Subcommand)]
pub enum InspectCommand {
    /// Generate a single static dashboard HTML file (offline, zero external CDN). [Source: §A-1]
    Report {
        /// Output path.
        #[arg(short, long = "out", default_value = "bathos-dashboard.html")]
        out: PathBuf,

        /// Initial display language (`both`=toggle-embedded).
        #[arg(long, value_enum, default_value_t = LangArg::Both)]
        lang: LangArg,
    },

    /// Live dashboard server (P1, requires the `serve` feature). [Source: §A-2]
    Serve {
        /// Local bind port (127.0.0.1).
        #[arg(long, default_value_t = 8787)]
        port: u16,

        /// Initial display language.
        #[arg(long, value_enum, default_value_t = LangArg::Both)]
        lang: LangArg,

        /// Auto-open the browser.
        #[arg(long)]
        open: bool,
    },

    /// Story-file list/viewer/linter. If `<KEY>` is omitted, lists all. [Source: §A-3]
    ///
    /// Note: this never shares an alias with bare `bathos doctor` (install/wiring
    /// preflight, ADR-P-0002) — no clap alias is attached to this subcommand.
    Story {
        /// Story key (e.g. `1-2-payment-auth`). If omitted, lists all.
        key: Option<String>,

        /// Run the linter (exit 2 on violation).
        #[arg(long)]
        lint: bool,

        /// (P2) Cross-check source_hash against upstream artifact sha256 + stale_story_keys.
        #[arg(long)]
        stale: bool,
    },

    /// Diagnose consistency of artifacts, gates and state. [Source: §A-4]
    ///
    /// **Naming note (ADR-P-0002):** bare `bathos doctor` is preempted by the
    /// existing install/wiring preflight. This subcommand is exposed only as
    /// `bathos inspect doctor` and carries no alias.
    Doctor,

    /// Shared `DashboardVM` data source for the wave panels (tmux/TUI, ADR-D-0007) —
    /// `--format json` is byte-identical to `report --json` (same `build_dashboard_vm_json`
    /// call); `--format lines` emits the bash-3.2/jq-free TSV protocol.
    /// [Source: w2-panes-model-design-kr.md §B1]
    Vm {
        /// Scope wave/gate/role records to this wave (e.g. `W5`). Omit for all waves.
        #[arg(long)]
        wave: Option<String>,

        #[arg(long, value_enum, default_value_t = VmFormat::Json)]
        format: VmFormat,
    },
}
