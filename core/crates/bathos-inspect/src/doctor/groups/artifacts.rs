//! `artifacts` group — artifact-path existence + markdown-link (relative link / `[[..]]`)
//! integrity + (P1) sha256 comparison (CF-3.5, story 4-2).
//!
//! **Wikilink (`[[..]]`) vs relative-link severity distinction (O-4, must be observed):**
//! a broken relative link is `fail` (an actual file reference is severed), a wikilink that
//! cannot be found is `warn` (loose by upstream-project convention — an Obsidian-style project
//! practice, a lead-confirmation item). **Reversing this distinction floods normal wikilinks
//! with false-positive FAILs.**
//!
//! Uses only a lightweight manual scan (no regex, no heavy markdown parser, no `walkdir` —
//! ETHOS Search Before Building: at this pilot's scale a std recursive walk is enough).
//! [Source: story-4-2-doctor-links-policy-kr.md, exceptions-kr.md §4/§6 O-4]

use std::path::{Component, Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::loader::ProjectView;
use crate::story::{Finding, Severity};
use crate::InspectCtx;

use super::super::report::GroupResult;
use super::super::{collect_files_with_ext, relative_display};

pub const ARTIFACT_PATH_MISSING: &str = "artifact_path_missing";
/// M-1 (findings.md, must be observed): if `artifacts[].path` is absolute or contains a `..`
/// (parent-directory) component, it can escape outside the project root — this rule filters
/// such paths out "before reading the file" (protecting the read-only invariant). It is set to
/// severity=fail to stay consistent with the existing convention that a broken relative link
/// is a fail (O-4).
pub const ARTIFACT_PATH_OUTSIDE_ROOT: &str = "artifact_path_outside_root";
pub const MD_LINK_BROKEN: &str = "md_link_broken";
pub const ARTIFACT_SHA256_MISMATCH: &str = "artifact_sha256_mismatch";

pub const RULE_IDS: &[&str] = &[
    ARTIFACT_PATH_MISSING,
    ARTIFACT_PATH_OUTSIDE_ROOT,
    MD_LINK_BROKEN,
    ARTIFACT_SHA256_MISMATCH,
];

pub fn run(ctx: &InspectCtx, pv: Option<&ProjectView>) -> GroupResult {
    let mut findings = Vec::new();

    if let Some(pv) = pv {
        check_artifact_paths(pv, &ctx.agent_team_path, &mut findings);
    }

    check_markdown_links(&ctx.agent_team_path, &mut findings);

    GroupResult::new("artifacts", findings)
}

/// `Artifact.path` is a path relative to the project root (`.agent-team`'s parent)
/// (the `".agent-team/<wave>/..."` form — per the `bathos-state::model::Artifact.path`
/// comment). [Source: state-audit-contract-kr.md §1.2 artifacts[], model.rs Artifact]
fn project_root(agent_team_path: &Path) -> PathBuf {
    agent_team_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| agent_team_path.to_path_buf())
}

/// Returns `false` if `path` (a string taken verbatim from manifest.json) can escape the
/// project root (M-1 defense). It filters two things:
/// 1. **Absolute path** — if the argument is absolute, `Path::join` completely ignores `self`
///    (root) and returns the argument as-is (Rust standard behavior, empirically confirmed).
/// 2. **Contains a `..` (parent-directory) component** — since `artifacts[].path` is by
///    contract "a path relative to the project root" (state-audit-contract-kr.md §1.2), the
///    mere presence of a parent reference is already a contract violation. It does not
///    leniently allow even cases that, after normalization, actually stay within the root — it
///    chooses the simplest, safest rule decidable from the string alone without touching the
///    filesystem (the read-only invariant) (ETHOS Search Before Building: canonicalize-based
///    verification is overkill at this scale — the target file may not even exist, so
///    canonicalize itself could fail).
///
/// [Source: pilot/.agent-team/10-review/findings.md M-1]
fn path_stays_within_root(path: &str) -> bool {
    let p = Path::new(path);
    if p.is_absolute() {
        return false;
    }
    !p.components().any(|c| matches!(c, Component::ParentDir))
}

fn check_artifact_paths(pv: &ProjectView, agent_team_path: &Path, findings: &mut Vec<Finding>) {
    let root = project_root(agent_team_path);

    for (idx, artifact) in pv.artifacts.iter().enumerate() {
        let loc = format!("_state/manifest.json#artifacts[{idx}]");

        if !path_stays_within_root(&artifact.path) {
            findings.push(Finding::new(
                ARTIFACT_PATH_OUTSIDE_ROOT,
                Severity::Fail,
                loc,
                format!(
                    "artifacts[{idx}].path='{}'가 프로젝트 루트를 벗어날 수 있습니다\
                     (절대경로 또는 '..' 상위 디렉터리 참조) — read-only 불변식 보호를 \
                     위해 이 경로의 파일을 읽지 않습니다(존재 확인·sha256 계산 모두 스킵).",
                    artifact.path
                ),
                "path를 프로젝트 루트(.agent-team의 부모) 기준 상대경로로 정정하세요\
                 ('..' 미포함, 절대경로 금지)."
                    .to_string(),
            ));
            continue; // An escapable path is not even checked for existence (M-1).
        }

        let full_path = root.join(&artifact.path);

        if !full_path.is_file() {
            findings.push(Finding::new(
                ARTIFACT_PATH_MISSING,
                Severity::Fail,
                loc,
                format!(
                    "artifacts[{idx}].path='{}' 실파일이 없습니다.",
                    artifact.path
                ),
                "경로를 정정하거나 산출물을 재생성하세요.".to_string(),
            ));
            continue; // If the file is missing, sha256 comparison is meaningless.
        }

        if let Some(expected) = &artifact.sha256 {
            match sha256_of_file(&full_path) {
                Ok(actual) if &actual == expected => {}
                Ok(actual) => {
                    findings.push(Finding::new(
                        ARTIFACT_SHA256_MISMATCH,
                        Severity::Warn,
                        loc,
                        format!(
                            "artifacts[{idx}].sha256 불일치 — manifest={expected}, \
                             실파일={actual}(산출물 수정 후 미갱신 가능성)."
                        ),
                        "산출물 sha256을 재계산해 manifest.json을 갱신하세요.".to_string(),
                    ));
                }
                // Read failure (permissions, etc.) — existence is already confirmed, so skip silently, no crash.
                Err(_) => {}
            }
        }
    }
}

fn sha256_of_file(path: &Path) -> std::io::Result<String> {
    let bytes = std::fs::read(path)?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    Ok(hex::encode(hasher.finalize()))
}

// ─────────────────────────────────────────────────────────────────────────────
// Markdown link scan
// ─────────────────────────────────────────────────────────────────────────────

fn check_markdown_links(agent_team_path: &Path, findings: &mut Vec<Finding>) {
    let md_files = collect_files_with_ext(agent_team_path, "md");

    for file in &md_files {
        let Ok(content) = std::fs::read_to_string(file) else {
            continue;
        };
        let display = relative_display(agent_team_path, file);

        // Track fenced-code-block (``` / ~~~) state across the whole file — `](`/`)` notation
        // inside a code example is not an actual link.
        let mut in_fence = false;

        for (idx, line) in content.lines().enumerate() {
            let line_no = idx + 1;
            let trimmed = line.trim_start();

            if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
                in_fence = !in_fence;
                continue; // The fence-marker line itself is just a code-boundary marker; not scanned.
            }
            if in_fence {
                continue; // Inside a fenced code block is a code example — out of scan scope (false-positive prevention).
            }

            // **False-positive regression (found empirically via dogfooding, must be kept):**
            // notation like `](`/`)` inside an inline code span (a single backtick pair) — e.g.
            // the `` `](`/`)` `` phrasing this crate's own impl-note uses when describing the
            // scanner — must not be mistaken for an actual markdown link. Before scanning for
            // links, build and pass a "sanitized" line with inline code spans removed.
            let sanitized = strip_inline_code_spans(line);

            for target in extract_relative_link_targets(&sanitized) {
                check_relative_link(file, &target, &display, line_no, findings);
            }
            for target in extract_wiki_link_targets(&sanitized) {
                check_wiki_link(&md_files, &target, &display, line_no, findings);
            }
        }
    }
}

/// Builds a line with the contents of inline code spans (single backtick pairs) removed. The
/// backticks themselves are removed too (leaving them would not hurt false-positive
/// prevention, but removing them is cleaner). An unmatched backtick (not closed within the
/// line) is treated as "inside code" until the end of the line, so the rest is not preserved —
/// this case too is handled safely without crashing (lightweight scanner, no regex/markdown
/// parser needed).
fn strip_inline_code_spans(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut in_code = false;
    for ch in line.chars() {
        if ch == '`' {
            in_code = !in_code;
            continue;
        }
        if !in_code {
            out.push(ch);
        }
    }
    out
}

fn check_relative_link(
    file: &Path,
    target: &str,
    display: &str,
    line_no: usize,
    findings: &mut Vec<Finding>,
) {
    let path_part = target.split('#').next().unwrap_or("").trim();
    if path_part.is_empty() {
        return; // A link with only a pure anchor (`#section`) — not a file reference, out of scope.
    }
    if !looks_like_file_path(path_part) {
        // Empirical false positive (found via dogfooding, must be kept): this document set uses
        // a convention of appending a parenthetical description after "field-name[]", like
        // `owned_paths[](freeze boundary)` / `artifacts[](path·owner·updated)`. `[]`+`(...)` is
        // literally the same as markdown's "empty-text link" syntax, so the lightweight scanner
        // mistakes it for a link. A target with no slash and no known extension (including
        // whitespace/Korean prose) is very likely not an actual file reference, so it is
        // filtered out here — without this filter, FAILs flood this document set.
        return;
    }
    let resolved = file.parent().unwrap_or(Path::new(".")).join(path_part);
    if !resolved.exists() {
        findings.push(Finding::new(
            MD_LINK_BROKEN,
            Severity::Fail,
            format!("{display}:{line_no}"),
            format!(
                "상대링크 '{target}'이 resolve되지 않습니다({}).",
                resolved.display()
            ),
            "링크 경로를 정정하거나 대상 파일을 생성하세요.".to_string(),
        ));
    }
}

fn check_wiki_link(
    md_files: &[PathBuf],
    target: &str,
    display: &str,
    line_no: usize,
    findings: &mut Vec<Finding>,
) {
    if !wiki_link_resolves(md_files, target) {
        findings.push(Finding::new(
            MD_LINK_BROKEN,
            Severity::Warn,
            format!("{display}:{line_no}"),
            format!(
                "위키링크 '[[{target}]]'을 `.agent-team` 안에서 찾지 못했습니다\
                 (관례상 warn — 상위 프로젝트 문서일 수 있음)."
            ),
            "대상 문서를 `.agent-team` 안에 추가하거나 링크명을 확인하세요.".to_string(),
        ));
    }
}

/// From the targets inside `](...)`, keep only those regarded as "relative links" (excluding
/// scheme-bearing URLs, pure anchors, and absolute paths). [Source: exceptions-kr.md §4]
fn extract_relative_link_targets(line: &str) -> Vec<String> {
    extract_between(line, "](", ")")
        .into_iter()
        .filter(|t| is_relative_link_target(t))
        .collect()
}

fn is_relative_link_target(target: &str) -> bool {
    let t = target.trim();
    if t.is_empty() || t.starts_with('#') || t.starts_with('/') {
        return false;
    }
    let lower = t.to_ascii_lowercase();
    !(lower.starts_with("http://")
        || lower.starts_with("https://")
        || lower.starts_with("mailto:")
        || lower.contains("://"))
}

/// Known document/asset extensions (this project's relative-link convention, `.md` being the vast majority).
const KNOWN_FILE_EXTENSIONS: &[&str] = &[
    "md", "html", "htm", "json", "yaml", "yml", "toml", "txt", "pdf", "png", "jpg", "jpeg", "svg",
    "css", "js",
];

/// Decides "whether it looks like an actual file path" (the core false-positive-prevention heuristic).
///
/// It must contain a slash (`/`) or end with a known extension. If it contains whitespace it is
/// immediately excluded — actual paths in this document set have no whitespace at all, whereas
/// false-positive candidates like `owned_paths[](freeze boundary)` were always Korean prose
/// mixed with whitespace (dogfooding-measured). Without this filter, bogus `md_link_broken`
/// fails flood every document that has `[]`+`(...)` notation.
fn looks_like_file_path(path_part: &str) -> bool {
    if path_part.chars().any(char::is_whitespace) {
        return false;
    }
    let lower = path_part.to_ascii_lowercase();
    path_part.contains('/')
        || KNOWN_FILE_EXTENSIONS
            .iter()
            .any(|ext| lower.ends_with(&format!(".{ext}")))
}

/// The target inside `[[...]]` (the optional `|display-name` alias is discarded).
fn extract_wiki_link_targets(line: &str) -> Vec<String> {
    extract_between(line, "[[", "]]")
        .into_iter()
        .filter_map(|raw| {
            let name = raw.split('|').next().unwrap_or(&raw).trim().to_string();
            (!name.is_empty()).then_some(name)
        })
        .collect()
}

/// A lightweight scanner that extracts, in order, the strings between `open` and `close`
/// (no regex, processing non-nested multiple links within one line sequentially).
fn extract_between(line: &str, open: &str, close: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cursor = 0usize;
    while let Some(open_rel) = line[cursor..].find(open) {
        let content_start = cursor + open_rel + open.len();
        match line[content_start..].find(close) {
            Some(close_rel) => {
                let content_end = content_start + close_rel;
                out.push(line[content_start..content_end].to_string());
                cursor = content_end + close.len();
            }
            None => break,
        }
    }
    out
}

/// Whether the wikilink target matches the stem of some `.md` file inside `.agent-team`
/// (directory-independent, Obsidian-style convention). It also matches when the extension is
/// written too (`name.md`).
fn wiki_link_resolves(md_files: &[PathBuf], target: &str) -> bool {
    let want = target.trim_end_matches(".md");
    md_files
        .iter()
        .any(|p| p.file_stem().and_then(|s| s.to_str()) == Some(want))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::loader::{ArtifactView, ChainStatus, ManifestForm, ProjectMeta, ProjectStatus};

    fn ctx(path: PathBuf) -> InspectCtx {
        InspectCtx {
            agent_team_path: path,
            json: false,
            verbose: false,
            strict: false,
        }
    }

    fn empty_project_view(artifacts: Vec<ArtifactView>) -> ProjectView {
        ProjectView {
            form: ManifestForm::Descriptive,
            meta: ProjectMeta {
                codename: None,
                current_level: None,
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
            artifacts,
            routing: vec![],
            modules: vec![],
            stale_story_keys: vec![],
            audit: vec![],
            chain_status: ChainStatus::Absent,
            audit_skipped: 0,
            warnings: vec![],
        }
    }

    // ── artifacts[].path existence ───────────────────────────────────────────

    #[test]
    fn missing_artifact_path_is_fail() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".agent-team")).unwrap();
        let agent_team = dir.path().join(".agent-team");

        let pv = empty_project_view(vec![ArtifactView {
            path: ".agent-team/does-not-exist.md".to_string(),
            owner_role: None,
            sha256: None,
            updated: None,
            wave_id: None,
        }]);

        let group = run(&ctx(agent_team.clone()), Some(&pv));
        assert!(group
            .findings
            .iter()
            .any(|f| f.rule_id == ARTIFACT_PATH_MISSING && f.severity == Severity::Fail));
    }

    #[test]
    fn existing_artifact_path_passes() {
        let dir = tempfile::tempdir().unwrap();
        let agent_team = dir.path().join(".agent-team");
        std::fs::create_dir_all(&agent_team).unwrap();
        std::fs::write(agent_team.join("real.md"), "# ok").unwrap();

        let pv = empty_project_view(vec![ArtifactView {
            path: ".agent-team/real.md".to_string(),
            owner_role: None,
            sha256: None,
            updated: None,
            wave_id: None,
        }]);

        let group = run(&ctx(agent_team), Some(&pv));
        assert!(!group
            .findings
            .iter()
            .any(|f| f.rule_id == ARTIFACT_PATH_MISSING));
    }

    // ── M-1: path-escape defense (absolute path / '..') ─────────────────────

    /// **M-1 repro (absolute path):** if `artifacts[].path` is absolute, `Path::join`
    /// completely ignores `root` and returns that absolute path as-is — without this rule
    /// doctor would compute the sha256 of an arbitrary file outside the project root (e.g.
    /// `/etc/hosts`). Here an actually-readable file is placed outside the root to
    /// empirically prove "it would have been read had there been no defense", and it also
    /// confirms that after the defense `ARTIFACT_PATH_MISSING` (any trace of attempting an
    /// existence check) never appears (= existence is not even checked, read-only protection).
    #[test]
    fn absolute_artifact_path_is_rejected_without_reading_file() {
        let dir = tempfile::tempdir().unwrap();
        let agent_team = dir.path().join(".agent-team");
        std::fs::create_dir_all(&agent_team).unwrap();

        // Place an actual file "outside" the project root (the target that would be read without the defense).
        let outside_dir = tempfile::tempdir().unwrap();
        let secret = outside_dir.path().join("secret.md");
        std::fs::write(&secret, "outside-root-content").unwrap();

        let pv = empty_project_view(vec![ArtifactView {
            path: secret.to_string_lossy().to_string(), // absolute path
            owner_role: None,
            sha256: Some("deadbeef".to_string()),
            updated: None,
            wave_id: None,
        }]);

        let group = run(&ctx(agent_team), Some(&pv));

        let outside = group
            .findings
            .iter()
            .find(|f| f.rule_id == ARTIFACT_PATH_OUTSIDE_ROOT)
            .expect("절대경로는 artifact_path_outside_root fail이어야 함(M-1)");
        assert_eq!(outside.severity, Severity::Fail);

        // An escaped path skips both the existence check and sha256 computation — no other
        // rule_id (especially sha256 mismatch) should appear at all.
        assert!(!group
            .findings
            .iter()
            .any(|f| f.rule_id == ARTIFACT_PATH_MISSING));
        assert!(!group
            .findings
            .iter()
            .any(|f| f.rule_id == ARTIFACT_SHA256_MISMATCH));
    }

    /// **M-1 repro (`..` parent reference):** even a relative path can escape the project root
    /// if it contains `..` — likewise it is reported as a fail without reading the file.
    #[test]
    fn parent_dir_traversal_artifact_path_is_rejected_without_reading_file() {
        let dir = tempfile::tempdir().unwrap();
        let agent_team = dir.path().join(".agent-team");
        std::fs::create_dir_all(&agent_team).unwrap();
        // Place an actual file outside the project root (=dir.path()) so it is provable that
        // it would be read if "leaked".
        std::fs::write(dir.path().join("outside.md"), "leaked").unwrap();

        let pv = empty_project_view(vec![ArtifactView {
            path: "../outside.md".to_string(),
            owner_role: None,
            sha256: None,
            updated: None,
            wave_id: None,
        }]);

        let group = run(&ctx(agent_team), Some(&pv));
        assert!(group
            .findings
            .iter()
            .any(|f| f.rule_id == ARTIFACT_PATH_OUTSIDE_ROOT && f.severity == Severity::Fail));
        assert!(!group
            .findings
            .iter()
            .any(|f| f.rule_id == ARTIFACT_PATH_MISSING));
    }

    /// A normal relative path (no `..`) must diagnose normally as before (no regression).
    #[test]
    fn normal_relative_artifact_path_still_passes_through_m1_guard() {
        let dir = tempfile::tempdir().unwrap();
        let agent_team = dir.path().join(".agent-team");
        std::fs::create_dir_all(&agent_team).unwrap();
        std::fs::write(agent_team.join("real.md"), "# ok").unwrap();

        let pv = empty_project_view(vec![ArtifactView {
            path: ".agent-team/real.md".to_string(),
            owner_role: None,
            sha256: None,
            updated: None,
            wave_id: None,
        }]);

        let group = run(&ctx(agent_team), Some(&pv));
        assert!(!group
            .findings
            .iter()
            .any(|f| f.rule_id == ARTIFACT_PATH_OUTSIDE_ROOT));
        assert!(!group
            .findings
            .iter()
            .any(|f| f.rule_id == ARTIFACT_PATH_MISSING));
    }

    #[test]
    fn path_stays_within_root_rejects_absolute_and_parent_dir() {
        assert!(!path_stays_within_root("/etc/passwd"));
        assert!(!path_stays_within_root("../outside.md"));
        assert!(!path_stays_within_root("a/../../outside.md"));
        assert!(path_stays_within_root(".agent-team/real.md"));
        assert!(path_stays_within_root("04-architecture/x.md"));
    }

    /// (P1) sha256 mismatch → warn (not fail, merely a possibility the artifact was modified but not refreshed).
    #[test]
    fn sha256_mismatch_is_warn_not_fail() {
        let dir = tempfile::tempdir().unwrap();
        let agent_team = dir.path().join(".agent-team");
        std::fs::create_dir_all(&agent_team).unwrap();
        std::fs::write(agent_team.join("real.md"), "changed content").unwrap();

        let pv = empty_project_view(vec![ArtifactView {
            path: ".agent-team/real.md".to_string(),
            owner_role: None,
            sha256: Some(
                "0000000000000000000000000000000000000000000000000000000000000000".to_string(),
            ),
            updated: None,
            wave_id: None,
        }]);

        let group = run(&ctx(agent_team), Some(&pv));
        assert!(group
            .findings
            .iter()
            .any(|f| f.rule_id == ARTIFACT_SHA256_MISMATCH && f.severity == Severity::Warn));
        assert_eq!(group.status, Severity::Warn);
    }

    // ── Markdown links ───────────────────────────────────────────────────────

    /// **False-positive regression (found empirically via dogfooding, must be kept):** the
    /// document convention of appending a parenthetical description after "field-name[]", like
    /// `owned_paths[](freeze boundary)` / `artifacts[](path·owner·updated)`, must not be
    /// false-flagged as `md_link_broken` (it is whitespace/Korean prose, not a file path, per
    /// the `looks_like_file_path` filter).
    #[test]
    fn field_array_notation_followed_by_prose_parens_is_not_flagged() {
        let dir = tempfile::tempdir().unwrap();
        let agent_team = dir.path().join(".agent-team");
        std::fs::create_dir_all(&agent_team).unwrap();
        std::fs::write(
            agent_team.join("a.md"),
            "| doctor CLI | 파일 부재→[!](부재≠오류 시) |\n\
             `owned_paths[](freeze 경계)`, `status(enum: ...)`\n\
             artifacts[](path·owner·updated) 트리\n\
             처리: 정규화](정규화) 아님\n",
        )
        .unwrap();

        let group = run(&ctx(agent_team), None);
        assert!(group.findings.is_empty(), "findings: {:?}", group.findings);
    }

    /// **False-positive regression (core, found by the lead via dogfooding):** `](`/`)`
    /// notation inside an inline code span (prose describing the scanner itself, actually
    /// occurring in `08-impl-notes/backend-w5-doctor.md`) must not be mistaken for a relative link.
    #[test]
    fn inline_code_span_containing_link_syntax_is_not_flagged() {
        let dir = tempfile::tempdir().unwrap();
        let agent_team = dir.path().join(".agent-team");
        std::fs::create_dir_all(&agent_team).unwrap();
        std::fs::write(
            agent_team.join("a.md"),
            "경량 수동 스캔(정규식·마크다운 파서 없이 `](`/`)` 문자열 탐색)으로 상대링크를 찾다가,\n",
        )
        .unwrap();

        let group = run(&ctx(agent_team), None);
        assert!(group.findings.is_empty(), "findings: {:?}", group.findings);
    }

    /// **False-positive regression:** a `](x)` code example inside a fenced code block
    /// (``` ``` ```) must not be mistaken for a link either.
    #[test]
    fn fenced_code_block_containing_link_syntax_is_not_flagged() {
        let dir = tempfile::tempdir().unwrap();
        let agent_team = dir.path().join(".agent-team");
        std::fs::create_dir_all(&agent_team).unwrap();
        std::fs::write(
            agent_team.join("a.md"),
            "설명 텍스트\n\n```rust\nlet s = \"](not-a-real-path.md)\";\n```\n\n마무리.\n",
        )
        .unwrap();

        let group = run(&ctx(agent_team), None);
        assert!(group.findings.is_empty(), "findings: {:?}", group.findings);
    }

    /// The code-span/fence removal logic does not affect normal link/wikilink decisions
    /// (confirming no regression — the fail/warn distinction is unchanged).
    #[test]
    fn code_span_stripping_does_not_affect_real_link_detection() {
        let dir = tempfile::tempdir().unwrap();
        let agent_team = dir.path().join(".agent-team");
        std::fs::create_dir_all(agent_team.join("04-architecture")).unwrap();
        std::fs::write(
            agent_team.join("04-architecture").join("target.md"),
            "# 대상",
        )
        .unwrap();
        std::fs::write(
            agent_team.join("04-architecture").join("a.md"),
            "코드 `foo()` 설명 뒤 정상 링크 [링크](./target.md)와 \
             깨진 링크 [깨짐](./nope.md) 둘 다 있음.",
        )
        .unwrap();

        let group = run(&ctx(agent_team), None);
        assert!(group.findings.iter().any(|f| f.rule_id == MD_LINK_BROKEN
            && f.severity == Severity::Fail
            && f.message.contains("nope.md")));
        assert!(!group
            .findings
            .iter()
            .any(|f| f.message.contains("target.md")));
    }

    #[test]
    fn broken_relative_link_is_fail() {
        let dir = tempfile::tempdir().unwrap();
        let agent_team = dir.path().join(".agent-team");
        std::fs::create_dir_all(agent_team.join("04-architecture")).unwrap();
        std::fs::write(
            agent_team.join("04-architecture").join("a.md"),
            "참고: [링크](../nope/does-not-exist.md) 입니다.",
        )
        .unwrap();

        let group = run(&ctx(agent_team), None);
        assert!(group
            .findings
            .iter()
            .any(|f| f.rule_id == MD_LINK_BROKEN && f.severity == Severity::Fail));
    }

    #[test]
    fn valid_relative_link_passes() {
        let dir = tempfile::tempdir().unwrap();
        let agent_team = dir.path().join(".agent-team");
        std::fs::create_dir_all(agent_team.join("04-architecture")).unwrap();
        std::fs::write(
            agent_team.join("04-architecture").join("target.md"),
            "# 대상",
        )
        .unwrap();
        std::fs::write(
            agent_team.join("04-architecture").join("a.md"),
            "참고: [링크](./target.md) 입니다.",
        )
        .unwrap();

        let group = run(&ctx(agent_team), None);
        assert!(!group.findings.iter().any(|f| f.rule_id == MD_LINK_BROKEN));
    }

    #[test]
    fn broken_wiki_link_is_warn_not_fail() {
        let dir = tempfile::tempdir().unwrap();
        let agent_team = dir.path().join(".agent-team");
        std::fs::create_dir_all(&agent_team).unwrap();
        std::fs::write(agent_team.join("a.md"), "정책: [[docs-emoji-and-es]] 참고.").unwrap();

        let group = run(&ctx(agent_team), None);
        let f = group
            .findings
            .iter()
            .find(|f| f.rule_id == MD_LINK_BROKEN)
            .expect("깨진 위키링크 finding 있어야 함");
        assert_eq!(
            f.severity,
            Severity::Warn,
            "위키링크는 fail이 아니라 warn이어야 함(O-4)"
        );
        // When only the wikilink is broken, the group status must not be promoted to fail.
        assert_ne!(group.status, Severity::Fail);
    }

    #[test]
    fn resolvable_wiki_link_passes() {
        let dir = tempfile::tempdir().unwrap();
        let agent_team = dir.path().join(".agent-team");
        std::fs::create_dir_all(&agent_team).unwrap();
        std::fs::write(agent_team.join("target-doc.md"), "# 대상").unwrap();
        std::fs::write(agent_team.join("a.md"), "참고: [[target-doc]].").unwrap();

        let group = run(&ctx(agent_team), None);
        assert!(!group.findings.iter().any(|f| f.rule_id == MD_LINK_BROKEN));
    }

    /// `[Source: file.md#section]` notation is not a markdown link, so it is not a scan target
    /// (false-positive-prevention regression — square-bracket notation without `](`).
    #[test]
    fn source_bracket_notation_is_not_treated_as_link() {
        let dir = tempfile::tempdir().unwrap();
        let agent_team = dir.path().join(".agent-team");
        std::fs::create_dir_all(&agent_team).unwrap();
        std::fs::write(agent_team.join("a.md"), "내용 [Source: nope.md#section]").unwrap();

        let group = run(&ctx(agent_team), None);
        assert!(group.findings.is_empty(), "findings: {:?}", group.findings);
    }

    /// External URLs (`http(s)://`) are not treated as relative links (false-positive prevention).
    #[test]
    fn external_url_is_not_flagged() {
        let dir = tempfile::tempdir().unwrap();
        let agent_team = dir.path().join(".agent-team");
        std::fs::create_dir_all(&agent_team).unwrap();
        std::fs::write(
            agent_team.join("a.md"),
            "참고: [외부](https://example.com/x).",
        )
        .unwrap();

        let group = run(&ctx(agent_team), None);
        assert!(group.findings.is_empty());
    }

    #[test]
    fn rule_ids_table_has_four_entries() {
        // M-1 added one artifact_path_outside_root, so 3 → 4.
        assert_eq!(RULE_IDS.len(), 4);
    }
}
