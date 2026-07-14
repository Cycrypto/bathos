//! `story` — story-file list/viewer/linter (SS-2.x).
//!
//! - [`lint`] (story 3-1, P0) — `StoryCompiler`-delegated lint. The rule-ID single source of
//!   truth is [`rules`]. Frontmatter access is [`frontmatter`], readiness linkage is [`readiness`].
//! - The list/detail viewer ([`list_stories`]/[`render_story`], story 3-2, P1) and the stale
//!   cross-check ([`stale`], story 3-2, P2) are filled in by this module reusing 3-1's `lint_story`
//!   (function-level ownership separation, no reimplementation).
//!
//! CR-1 note: this module depends only on `bathos-story-compiler` (a leaf, 0 deps). It does not
//! depend on `bathos-story-engine` (which transitively depends on gate/wave/router)
//! (ADR-P-0007). [Source: adr-kr.md ADR-P-0007, api-contracts-kr.md §C]

pub mod frontmatter;
pub mod lint;
pub mod readiness;
pub mod rules;
pub mod stale;

use std::path::{Path, PathBuf};

use serde::Serialize;
use thiserror::Error;

pub use lint::{lint_content, StoryLint, Summary};
pub use rules::{Finding, Severity};
pub use stale::StaleVerdict;

/// Story-file naming convention under `03-story-engineering/` (`story-*-kr.md`).
const STORY_FILE_PREFIX: &str = "story-";
const STORY_FILE_SUFFIX: &str = "-kr.md";

/// Load errors specific to the story module (Fatal layer, exit 1).
///
/// [Source: exceptions-kr.md §0 error model] A separate type from `loader::LoadError`
/// (manifest-only) — this crate's `loader/*` modules are outside this task's (story 3-1/3-2)
/// edit-ownership scope (read-only), so no story-specific variant can be added to that enum.
/// The semantics (fatal vs verification-failure vs warning vs normal-absence) follow the §0
/// 2-layer model as-is: this type expresses only **Fatal** cases such as "cannot read the
/// file / cannot find a specific key", while "the folder itself is missing" (Absent-OK) is not
/// an error but handled as `Ok(vec![])`/a guidance message (see [`list_stories`]).
#[derive(Debug, Error)]
pub enum StoryLoadError {
    #[error("[E-STORY-READ] 스토리파일을 읽을 수 없습니다: {path} — {reason}")]
    ReadFailed { path: PathBuf, reason: String },

    #[error("[E-STORY-NOT-FOUND] 스토리 키 '{key}'를 찾을 수 없습니다 (경로: {dir})")]
    KeyNotFound { key: String, dir: PathBuf },
}

/// A one-story summary used in the list (`bathos inspect story`, no KEY).
#[derive(Debug, Clone, Serialize)]
pub struct StoryView {
    pub story_key: Option<String>,
    pub file_name: String,
    pub status: Option<String>,
    pub source_hash: Option<String>,
    pub ready_for_dev: bool,
}

// ─────────────────────────────────────────────────────────────────────────────
// public API (api-contracts-kr.md §C)
// ─────────────────────────────────────────────────────────────────────────────

/// `StoryCompiler`-delegated lint (story 3-1). [Source: api-contracts-kr.md §C]
pub fn lint_story(file: &Path) -> Result<StoryLint, StoryLoadError> {
    lint::lint_file(file)
}

/// Enumerate all `story-*-kr.md` under `story_dir` by lightly skimming only the frontmatter
/// (no linting — the list is fast, detail/lint is `lint_story`'s job).
///
/// **Folder absence is not an error** (Absent-OK) — returns `Ok(Vec::new())`.
/// The caller ([`run_story`]) distinguishes "no stories" guidance from "folder exists but has 0
/// files". [Source: exceptions-kr.md §3 W-STORY-DIR-ABSENT]
pub fn list_stories(story_dir: &Path) -> Result<Vec<StoryView>, StoryLoadError> {
    if !story_dir.is_dir() {
        return Ok(Vec::new());
    }

    let mut paths: Vec<PathBuf> = std::fs::read_dir(story_dir)
        .map_err(|e| StoryLoadError::ReadFailed {
            path: story_dir.to_path_buf(),
            reason: e.to_string(),
        })?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|p| is_story_file(p))
        .collect();

    // Lexicographic sort by file name (a stable baseline — the natural sort below keeps the
    // relative order of parse-failed entries by this baseline, a stable sort).
    paths.sort();

    let mut views = Vec::with_capacity(paths.len());
    for path in paths {
        let content = std::fs::read_to_string(&path).map_err(|e| StoryLoadError::ReadFailed {
            path: path.clone(),
            reason: e.to_string(),
        })?;
        let meta = frontmatter::extract(&content);
        let file_name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        let ready_for_dev = meta.status.as_deref() == Some("ready-for-dev");

        views.push(StoryView {
            story_key: meta.story_key,
            file_name,
            status: meta.status,
            source_hash: meta.source_hash,
            ready_for_dev,
        });
    }

    // Backlog item 5 (natural sort, 2026-07-02 Phillip): lexicographic file-name order breaks
    // with two-digit story numbers (e.g. "story-1-10-…" < "story-1-2-…" because string
    // comparison puts '1'<'2', so 10 comes before 2). Parse the epic/story numbers from
    // `story_key` ("<epic>-<story>-<slug>", storyfile-format-kr.md §1.1) as numbers and natural-sort.
    // Parse failure (non-standard story_key/file name) falls back, without crashing, to a stable
    // lexicographic file-name sort (the stable sort preserves the baseline order above).
    views.sort_by_key(natural_sort_key);

    Ok(views)
}

/// (epic_num, story_num, file_name) sort key. Entries whose numbers fail to parse are pushed to
/// `u32::MAX` so they always sort last (no crash); the order among parse-failed entries is
/// decided by `file_name` (and the stable-sort baseline above).
fn natural_sort_key(view: &StoryView) -> (u32, u32, String) {
    let parsed = view
        .story_key
        .as_deref()
        .and_then(parse_epic_story_from_key)
        .or_else(|| parse_epic_story_from_filename(&view.file_name));

    match parsed {
        Some((epic, story)) => (epic, story, view.file_name.clone()),
        None => (u32::MAX, u32::MAX, view.file_name.clone()),
    }
}

/// Parse the first two numeric segments of `story_key` ("<epic>-<story>-<slug>",
/// storyfile-format-kr.md §1.1). `None` if the format differs (e.g. a non-numeric prefix).
fn parse_epic_story_from_key(key: &str) -> Option<(u32, u32)> {
    let mut parts = key.splitn(3, '-');
    let epic = parts.next()?.parse::<u32>().ok()?;
    let story = parts.next()?.parse::<u32>().ok()?;
    Some((epic, story))
}

/// Fallback when `story_key` is absent or fails to parse — re-parse the same info from the file
/// name `story-<epic>-<story>-...-kr.md`.
fn parse_epic_story_from_filename(file_name: &str) -> Option<(u32, u32)> {
    let stripped = file_name.strip_prefix(STORY_FILE_PREFIX)?;
    let stripped = stripped.strip_suffix(STORY_FILE_SUFFIX)?;
    parse_epic_story_from_key(stripped)
}

fn is_story_file(path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }
    path.file_name()
        .and_then(|n| n.to_str())
        .map(|n| n.starts_with(STORY_FILE_PREFIX) && n.ends_with(STORY_FILE_SUFFIX))
        .unwrap_or(false)
}

/// Render story detail (story 3-2). Since this pilot is a CLI text tool, "render" is not HTML
/// conversion but **returning the raw markdown as-is** (the sections are already markdown
/// headings, so "per-section render" is satisfied without extra conversion,
/// design-handoff-kr.md §3.2). The inline lint display is appended after this raw text by the
/// caller ([`run_story`]) from the [`lint_story`] result.
pub fn render_story(file: &Path) -> Result<String, StoryLoadError> {
    std::fs::read_to_string(file).map_err(|e| StoryLoadError::ReadFailed {
        path: file.to_path_buf(),
        reason: e.to_string(),
    })
}

/// Find the file in `story_dir` whose frontmatter `story_key` exactly matches `key`.
fn find_story_file(story_dir: &Path, key: &str) -> Result<PathBuf, StoryLoadError> {
    if !story_dir.is_dir() {
        return Err(StoryLoadError::KeyNotFound {
            key: key.to_string(),
            dir: story_dir.to_path_buf(),
        });
    }
    let entries = std::fs::read_dir(story_dir).map_err(|e| StoryLoadError::ReadFailed {
        path: story_dir.to_path_buf(),
        reason: e.to_string(),
    })?;

    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();
        if !is_story_file(&path) {
            continue;
        }
        if let Ok(content) = std::fs::read_to_string(&path) {
            if frontmatter::extract(&content).story_key.as_deref() == Some(key) {
                return Ok(path);
            }
        }
    }

    Err(StoryLoadError::KeyNotFound {
        key: key.to_string(),
        dir: story_dir.to_path_buf(),
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// CLI handler — `bathos inspect story [<KEY>] [--lint] [--stale]` (§A-3)
// ─────────────────────────────────────────────────────────────────────────────

/// The `story` subcommand handler that `lib.rs::run` delegates to.
///
/// [Source: api-contracts-kr.md §A-3, exceptions-kr.md §3]
pub fn run_story(
    ctx: &crate::InspectCtx,
    key: Option<&str>,
    lint_flag: bool,
    stale_flag: bool,
) -> i32 {
    let story_dir = ctx.agent_team_path.join("03-story-engineering");

    // Load the engine signal for stale cross-checking only when --stale (avoiding an unnecessary
    // manifest read). Even if the manifest load fails, it does not block story lookup itself (CR-3)
    // — the stale cross-check is supplementary, not a precondition of story lookup.
    let engine_stale_keys: Vec<String> = if stale_flag {
        match crate::loader::load_project(
            &ctx.agent_team_path,
            crate::loader::LoadOpts {
                verify_chain: false,
            },
        ) {
            Ok(pv) => pv.stale_story_keys,
            Err(e) => {
                if ctx.verbose {
                    eprintln!(
                        "[bathos inspect story] stale_story_keys 교차용 manifest 로드 실패(비차단): {e}"
                    );
                }
                Vec::new()
            }
        }
    } else {
        Vec::new()
    };

    match key {
        Some(k) => run_detail(
            ctx,
            &story_dir,
            k,
            lint_flag,
            stale_flag,
            &engine_stale_keys,
        ),
        None => run_list(ctx, &story_dir, stale_flag, &engine_stale_keys),
    }
}

fn run_list(
    ctx: &crate::InspectCtx,
    story_dir: &Path,
    stale_flag: bool,
    engine_stale_keys: &[String],
) -> i32 {
    if !story_dir.is_dir() {
        return report_dir_absent(ctx, story_dir);
    }

    let views = match list_stories(story_dir) {
        Ok(v) => v,
        Err(e) => return report_load_error(ctx, &e),
    };

    let readiness_verdict = readiness::read_verdict(story_dir);

    if ctx.json {
        let items: Vec<_> = views
            .iter()
            .map(|v| {
                let stale = stale_flag.then(|| {
                    stale::resolve(
                        v.story_key.as_deref(),
                        v.source_hash.as_deref(),
                        engine_stale_keys,
                        &ctx.agent_team_path,
                    )
                });
                serde_json::json!({
                    "story_key": v.story_key,
                    "file_name": v.file_name,
                    "status": v.status,
                    "ready_for_dev": v.ready_for_dev,
                    "readiness_verdict": readiness_verdict,
                    "stale": stale,
                })
            })
            .collect();
        println!(
            "{}",
            serde_json::json!({ "stories": items, "count": views.len() })
        );
        return 0;
    }

    if views.is_empty() {
        println!(
            "스토리 없음: {} 안에 story-*-kr.md 파일이 없습니다.",
            story_dir.display()
        );
        return 0;
    }

    println!("스토리 목록 ({}개) — {}", views.len(), story_dir.display());
    for v in &views {
        let ready_badge = if v.ready_for_dev { "✓" } else { "·" };
        let key_display = v.story_key.as_deref().unwrap_or("(story_key 없음)");
        let status_display = v.status.as_deref().unwrap_or("(status 없음)");

        let mut line = format!("  [{ready_badge}] {key_display}  ·  status={status_display}");

        if stale_flag {
            let verdict = stale::resolve(
                v.story_key.as_deref(),
                v.source_hash.as_deref(),
                engine_stale_keys,
                &ctx.agent_team_path,
            );
            line.push_str(&format!("  ·  stale={}", verdict.badge()));
        }

        if let Some(rv) = &readiness_verdict {
            line.push_str(&format!("  ·  readiness={rv}"));
        }

        println!("{line}");
    }

    0
}

fn run_detail(
    ctx: &crate::InspectCtx,
    story_dir: &Path,
    key: &str,
    lint_flag: bool,
    stale_flag: bool,
    engine_stale_keys: &[String],
) -> i32 {
    let file = match find_story_file(story_dir, key) {
        Ok(f) => f,
        Err(e) => return report_load_error(ctx, &e),
    };

    let lint = match lint_story(&file) {
        Ok(l) => l,
        Err(e) => return report_load_error(ctx, &e),
    };

    let stale_verdict = stale_flag.then(|| {
        stale::resolve(
            lint.story_key.as_deref(),
            lint.source_hash.as_deref(),
            engine_stale_keys,
            &ctx.agent_team_path,
        )
    });

    if ctx.json {
        let mut json = serde_json::to_value(&lint).unwrap_or_else(|_| serde_json::json!({}));
        if let (Some(obj), Some(sv)) = (json.as_object_mut(), &stale_verdict) {
            obj.insert(
                "stale".to_string(),
                serde_json::to_value(sv).unwrap_or(serde_json::Value::Null),
            );
        }
        println!("{json}");
    } else {
        // Left: raw markdown. Right/inline: lint result (design-handoff-kr.md §3.2).
        match render_story(&file) {
            Ok(body) => {
                println!("{body}");
                println!("{}", "-".repeat(60));
            }
            Err(e) => eprintln!("[bathos inspect story] 렌더 실패(린트는 계속 진행): {e}"),
        }

        let eligibility = if lint.ready_for_dev {
            "W5 진입 적격 ✓"
        } else {
            "W5 진입 부적격"
        };
        println!("린트 결과 — {} ({eligibility})", file.display());
        if lint.findings.is_empty() {
            println!(
                "  위반 없음(pass={}/{})",
                lint.summary.pass,
                rules::ALL_RULE_IDS.len()
            );
        }
        for f in &lint.findings {
            println!(
                "  [{}] {} @ {} — {}",
                f.severity.marker(),
                f.rule_id,
                f.location,
                f.message
            );
        }
        println!(
            "요약: pass={} warn={} fail={}",
            lint.summary.pass, lint.summary.warn, lint.summary.fail
        );

        if let Some(sv) = &stale_verdict {
            match sv {
                StaleVerdict::Uncomparable { reason } => {
                    println!("stale 대조: {} (대조 불가 — {reason})", sv.badge())
                }
                StaleVerdict::Fresh => println!("stale 대조: {} (신선)", sv.badge()),
                StaleVerdict::Stale { reason } => {
                    println!("stale 대조: {} (stale — {reason})", sv.badge())
                }
            }
        }
    }

    // FAIL is escalated to exit 2 only when the --lint flag is present (§A-3 "--lint & FAIL → 2, else 0").
    // A lookup that specifies only KEY without the flag always returns exit 0 (just information display, not a verification run).
    if lint_flag && lint.has_fail() {
        2
    } else {
        0
    }
}

fn report_dir_absent(ctx: &crate::InspectCtx, story_dir: &Path) -> i32 {
    if ctx.json {
        println!(
            "{}",
            serde_json::json!({
                "code": "W-STORY-DIR-ABSENT",
                "stories": [],
                "message": format!("{} 디렉터리가 없습니다.", story_dir.display()),
            })
        );
    } else {
        println!("스토리 없음: {} 디렉터리가 없습니다.", story_dir.display());
    }
    0
}

fn report_load_error(ctx: &crate::InspectCtx, e: &StoryLoadError) -> i32 {
    if ctx.json {
        println!(
            "{}",
            serde_json::json!({ "error": "story_load_failed", "message": e.to_string() })
        );
    } else {
        eprintln!("[bathos inspect story] {e}");
    }
    1
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn write_story(dir: &Path, file_name: &str, content: &str) {
        fs::write(dir.join(file_name), content).unwrap();
    }

    fn valid_story(key: &str) -> String {
        format!(
            r#"---
story_key: "{key}"
status: "ready-for-dev"
source_hash: "abc123"
---

## story_requirements
content [Source: x#a]

## developer_context
content [Source: x#b]

## architecture_compliance
content [Source: x#c]

## library_framework_requirements
content [Source: x#d]

## file_structure_requirements
content [Source: x#e]

## testing_requirements
content [Source: x#f]
"#
        )
    }

    // ── list_stories ─────────────────────────────────────────────────────────

    #[test]
    fn list_stories_returns_empty_ok_when_dir_absent() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("does-not-exist");
        let result = list_stories(&missing);
        assert!(result.is_ok(), "폴더 부재는 오류가 아니라 빈 Ok여야 함");
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn list_stories_enumerates_and_sorts_story_files() {
        let dir = tempfile::tempdir().unwrap();
        write_story(dir.path(), "story-2-1-b-kr.md", &valid_story("2-1-b"));
        write_story(dir.path(), "story-1-1-a-kr.md", &valid_story("1-1-a"));
        write_story(dir.path(), "not-a-story.md", "무시되어야 함");
        write_story(
            dir.path(),
            "readiness-report-kr.md",
            "무시되어야 함(story- 접두사 아님)",
        );

        let views = list_stories(dir.path()).unwrap();
        assert_eq!(views.len(), 2);
        assert_eq!(views[0].story_key.as_deref(), Some("1-1-a"));
        assert_eq!(views[1].story_key.as_deref(), Some("2-1-b"));
        assert!(views[0].ready_for_dev);
    }

    /// Backlog item 5 (natural sort) core regression: by lexicographic file name,
    /// "story-1-10-…" comes before "story-1-2-…" ('1'<'2' char comparison) — this is the wrong
    /// order. A natural sort that compares `story_key`'s epic/story numbers numerically must
    /// produce 1-1 < 1-2 < 1-10.
    #[test]
    fn list_stories_sorts_naturally_by_epic_and_story_number_not_lexicographically() {
        let dir = tempfile::tempdir().unwrap();
        write_story(
            dir.path(),
            "story-1-10-tenth-kr.md",
            &valid_story("1-10-tenth"),
        );
        write_story(
            dir.path(),
            "story-1-2-second-kr.md",
            &valid_story("1-2-second"),
        );
        write_story(
            dir.path(),
            "story-1-1-first-kr.md",
            &valid_story("1-1-first"),
        );
        write_story(
            dir.path(),
            "story-2-1-next-epic-kr.md",
            &valid_story("2-1-next-epic"),
        );

        let views = list_stories(dir.path()).unwrap();
        let keys: Vec<&str> = views
            .iter()
            .map(|v| v.story_key.as_deref().unwrap())
            .collect();
        assert_eq!(
            keys,
            vec!["1-1-first", "1-2-second", "1-10-tenth", "2-1-next-epic"]
        );
    }

    /// When `story_key` is absent or its numbers fail to parse, the same info is parsed back from
    /// the file name (natural sort maintained even without a story_key frontmatter).
    #[test]
    fn list_stories_falls_back_to_filename_when_story_key_absent() {
        let dir = tempfile::tempdir().unwrap();
        // No story_key frontmatter — epic/story parseable only from the file name.
        write_story(
            dir.path(),
            "story-1-10-tenth-kr.md",
            "---\nstatus: \"backlog\"\n---\n\n내용\n",
        );
        write_story(
            dir.path(),
            "story-1-2-second-kr.md",
            "---\nstatus: \"backlog\"\n---\n\n내용\n",
        );

        let views = list_stories(dir.path()).unwrap();
        assert_eq!(views[0].file_name, "story-1-2-second-kr.md");
        assert_eq!(views[1].file_name, "story-1-10-tenth-kr.md");
    }

    /// A non-standard entry whose numbers fail to parse entirely is also pushed to the end without
    /// crashing (u32::MAX fallback), while normally-parsed entries stay naturally sorted.
    #[test]
    fn list_stories_unparseable_entries_do_not_crash_and_sort_last() {
        let dir = tempfile::tempdir().unwrap();
        write_story(
            dir.path(),
            "story-nonstandard-name-kr.md",
            "---\nstory_key: \"not-numeric-at-all\"\nstatus: \"backlog\"\n---\n\n내용\n",
        );
        write_story(
            dir.path(),
            "story-1-1-normal-kr.md",
            &valid_story("1-1-normal"),
        );

        let views = list_stories(dir.path()).unwrap();
        assert_eq!(views.len(), 2);
        assert_eq!(views[0].story_key.as_deref(), Some("1-1-normal"));
        assert_eq!(views[1].story_key.as_deref(), Some("not-numeric-at-all"));
    }

    // ── find_story_file / render_story ──────────────────────────────────────

    #[test]
    fn find_story_file_locates_by_frontmatter_key() {
        let dir = tempfile::tempdir().unwrap();
        write_story(dir.path(), "story-1-1-a-kr.md", &valid_story("1-1-a"));
        let found = find_story_file(dir.path(), "1-1-a").unwrap();
        assert_eq!(
            found.file_name().unwrap().to_str().unwrap(),
            "story-1-1-a-kr.md"
        );
    }

    #[test]
    fn find_story_file_returns_not_found_for_unknown_key() {
        let dir = tempfile::tempdir().unwrap();
        write_story(dir.path(), "story-1-1-a-kr.md", &valid_story("1-1-a"));
        let result = find_story_file(dir.path(), "9-9-nope");
        assert!(matches!(result, Err(StoryLoadError::KeyNotFound { .. })));
    }

    #[test]
    fn render_story_returns_raw_markdown() {
        let dir = tempfile::tempdir().unwrap();
        let content = valid_story("1-1-a");
        write_story(dir.path(), "story-1-1-a-kr.md", &content);
        let rendered = render_story(&dir.path().join("story-1-1-a-kr.md")).unwrap();
        assert_eq!(rendered, content);
    }

    #[test]
    fn render_story_errors_when_file_missing() {
        let result = render_story(Path::new("/definitely/not/here-kr.md"));
        assert!(result.is_err());
    }

    // ── run_story (CLI handler integration) ─────────────────────────────────────────

    fn ctx(agent_team_path: PathBuf, json: bool) -> crate::InspectCtx {
        crate::InspectCtx {
            agent_team_path,
            json,
            verbose: false,
            strict: false,
        }
    }

    #[test]
    fn run_story_list_on_absent_dir_returns_exit_0() {
        let dir = tempfile::tempdir().unwrap();
        let code = run_story(&ctx(dir.path().to_path_buf(), false), None, false, false);
        assert_eq!(code, 0);
    }

    #[test]
    fn run_story_list_with_empty_dir_returns_exit_0() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("03-story-engineering")).unwrap();
        let code = run_story(&ctx(dir.path().to_path_buf(), false), None, false, false);
        assert_eq!(code, 0);
    }

    #[test]
    fn run_story_list_with_stories_returns_exit_0() {
        let dir = tempfile::tempdir().unwrap();
        let story_dir = dir.path().join("03-story-engineering");
        fs::create_dir_all(&story_dir).unwrap();
        write_story(&story_dir, "story-1-1-a-kr.md", &valid_story("1-1-a"));
        let code = run_story(&ctx(dir.path().to_path_buf(), false), None, false, false);
        assert_eq!(code, 0);
    }

    #[test]
    fn run_story_detail_unknown_key_returns_exit_1() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("03-story-engineering")).unwrap();
        let code = run_story(
            &ctx(dir.path().to_path_buf(), false),
            Some("nope"),
            false,
            false,
        );
        assert_eq!(code, 1);
    }

    #[test]
    fn run_story_detail_with_lint_fail_and_lint_flag_returns_exit_2() {
        let dir = tempfile::tempdir().unwrap();
        let story_dir = dir.path().join("03-story-engineering");
        fs::create_dir_all(&story_dir).unwrap();
        // All 6 sections missing → FAIL
        write_story(
            &story_dir,
            "story-1-1-a-kr.md",
            "---\nstory_key: \"1-1-a\"\nstatus: \"ready-for-dev\"\n---\n\n내용만 있음\n",
        );
        let code = run_story(
            &ctx(dir.path().to_path_buf(), false),
            Some("1-1-a"),
            true,
            false,
        );
        assert_eq!(code, 2);
    }

    /// Specifying only KEY without --lint returns exit 0 even if there is a FAIL (info display only, §A-3).
    #[test]
    fn run_story_detail_without_lint_flag_returns_exit_0_even_with_fail() {
        let dir = tempfile::tempdir().unwrap();
        let story_dir = dir.path().join("03-story-engineering");
        fs::create_dir_all(&story_dir).unwrap();
        write_story(
            &story_dir,
            "story-1-1-a-kr.md",
            "---\nstory_key: \"1-1-a\"\nstatus: \"ready-for-dev\"\n---\n\n내용만 있음\n",
        );
        let code = run_story(
            &ctx(dir.path().to_path_buf(), false),
            Some("1-1-a"),
            false,
            false,
        );
        assert_eq!(code, 0);
    }

    #[test]
    fn run_story_detail_json_mode_does_not_panic() {
        let dir = tempfile::tempdir().unwrap();
        let story_dir = dir.path().join("03-story-engineering");
        fs::create_dir_all(&story_dir).unwrap();
        write_story(&story_dir, "story-1-1-a-kr.md", &valid_story("1-1-a"));
        let code = run_story(
            &ctx(dir.path().to_path_buf(), true),
            Some("1-1-a"),
            true,
            true,
        );
        assert_eq!(code, 0);
    }

    #[test]
    fn run_story_list_json_mode_does_not_panic() {
        let dir = tempfile::tempdir().unwrap();
        let story_dir = dir.path().join("03-story-engineering");
        fs::create_dir_all(&story_dir).unwrap();
        write_story(&story_dir, "story-1-1-a-kr.md", &valid_story("1-1-a"));
        let code = run_story(&ctx(dir.path().to_path_buf(), true), None, false, true);
        assert_eq!(code, 0);
    }
}
