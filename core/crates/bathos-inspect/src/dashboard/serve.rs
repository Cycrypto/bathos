//! `serve` — live dashboard (P1, `feature = "serve"`, story 2-2).
//!
//! It builds the initial HTML by calling **exactly the same** `ProjectView →
//! DashboardVM → HTML` pipeline (`dashboard::build_report`) as report (P0, story 2-1);
//! this module only adds the "serve over HTTP + file-watch re-render + client reload"
//! shell — no reimplementation of render logic (ADR-P-0005). [Source:
//! story-2-2-dashboard-serve-kr.md developer_context, adr-kr.md ADR-P-0005]
//!
//! ## Dependency isolation (ADR-P-0001)
//! `tiny_http` (lightweight blocking HTTP, no async runtime needed) and `notify`
//! (file watch) exist only behind this crate's `serve` cargo feature (off by default).
//! `axum`/`tokio` are overkill for this purpose and not adopted.
//! [Source: story-2-2 library_framework_requirements]
//!
//! ## Absolute read-only discipline (CR-1)
//! This module **does not acquire and simply ignores** file locks (`.lock`)
//! (`W-LOCK-IGNORED`) and never writes state files — it reloads by calling only
//! `loader::load_project` (read-only). Because of the append-only / atomic-rename
//! properties, a partial write is never observed, so no separate lock-wait logic is needed.
//! [Source: exceptions-kr.md §5 W-LOCK-IGNORED, state-audit-contract-kr.md §0]
//!
//! ## Reload mechanism — polling chosen
//! The requirement specifies "the simpler of polling/SSE" for client reload (assuming a
//! single local client). SSE must keep a connection open and managed, and pairs poorly
//! with `tiny_http`'s blocking request loop (requiring a separate thread/buffering),
//! whereas short-interval polling (`/__bathos_serve_version`) only has to be laid on top
//! of the existing request loop, which is far simpler — and for a local dev tool the
//! latency (<=1s) is harmless too.
//! [Source: story-2-2 AC "the simpler of polling/SSE", service-stories.md SS-1.6]

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use notify::{recommended_watcher, RecursiveMode, Watcher};
use tiny_http::{Header, Response, Server};

use crate::cli::LangArg;
use crate::dashboard::StoryCardView;
use crate::loader::{self, LoadError, LoadOpts, ProjectView};
use crate::InspectCtx;

/// Watched subdirectories (verbatim from the AC) — relative paths under `.agent-team`.
/// [Source: story-2-2 AC "notify file watch (`_state/*`, `03-story-engineering/*`)"]
const WATCH_SUBDIRS: [&str; 2] = ["_state", "03-story-engineering"];

/// The version endpoint the client polls. When the value changes the client reloads the
/// whole page (simple polling, see the module doc "Reload mechanism" above).
const VERSION_PATH: &str = "/__bathos_serve_version";

/// Debounce window that defers re-render after a file-watch event (coalesces consecutive events into one).
/// [Source: story-2-2 AC "change event (debounce)"]
const DEBOUNCE: Duration = Duration::from_millis(300);

/// HTTP receive-wait timeout (only to periodically check the `stop` flag; it means
/// nothing during normal operation — `stop` is test-only).
const RECV_POLL: Duration = Duration::from_millis(200);

// ─────────────────────────────────────────────────────────────────────────────
// shared state
// ─────────────────────────────────────────────────────────────────────────────

/// The latest render result shared between the serving thread (HTTP response) and the watch thread (re-render).
///
/// Even if the `Mutex` is poisoned (extreme situation such as a watch-thread panic),
/// [`read_html`]/[`write_html`] recover from poison so it keeps serving the last value
/// without interruption (Boil the Ocean — even the extreme path does not crash).
struct ServeState {
    html: Mutex<String>,
    /// Version counter incremented on each re-render. Client polling detects a change in
    /// this value and reloads.
    version: AtomicU64,
}

fn read_html(m: &Mutex<String>) -> String {
    match m.lock() {
        Ok(guard) => guard.clone(),
        Err(poisoned) => poisoned.into_inner().clone(),
    }
}

fn write_html(m: &Mutex<String>, new_html: String) {
    match m.lock() {
        Ok(mut guard) => *guard = new_html,
        Err(poisoned) => *poisoned.into_inner() = new_html,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// public entry point — must exactly match the `lib.rs` dispatch contract.
// ─────────────────────────────────────────────────────────────────────────────

/// `bathos inspect serve` handler — the point `lib.rs`'s `run()` delegates to when
/// `feature = "serve"`. The normal path ends only via Ctrl+C (process termination) and never returns.
///
/// [Source: api-contracts-kr.md §A-2, story-2-2 AC, lib.rs `run()` dispatch]
#[allow(clippy::missing_errors_doc)]
pub fn run_serve(ctx: &InspectCtx, port: u16, lang: LangArg, open: bool) -> i32 {
    run_serve_inner(ctx, port, lang, open, None, None)
}

/// Test-only internal entry point. The public API (`run_serve`) always blocks (infinite
/// loop) and cannot be called directly from automated tests, so we split out an internal
/// version that can terminate via a `stop` signal and can report the actually-bound port
/// (0 = supports OS arbitrary assignment) via `ready_tx`. The operational path
/// (`run_serve`) calls this function with `stop=None` (= never stops), so behavior is identical.
fn run_serve_inner(
    ctx: &InspectCtx,
    port: u16,
    lang: LangArg,
    open: bool,
    stop: Option<Arc<AtomicBool>>,
    ready_tx: Option<mpsc::Sender<u16>>,
) -> i32 {
    let stop = stop.unwrap_or_else(|| Arc::new(AtomicBool::new(false)));

    // 1) initial load — same pipeline as report (P0), read-only.
    let pv = match loader::load_project(&ctx.agent_team_path, LoadOpts { verify_chain: true }) {
        Ok(pv) => pv,
        Err(e) => return initial_load_error(&e, ctx.json),
    };
    let initial_html = initial_page_html(&pv, lang);
    let state = Arc::new(ServeState {
        html: Mutex::new(initial_html),
        version: AtomicU64::new(0),
    });

    // 2) HTTP bind — E-PORT-IN-USE if the port is taken. [Source: exceptions-kr.md §5]
    let server = match Server::http(("127.0.0.1", port)) {
        Ok(s) => s,
        Err(e) => return port_in_use_error(port, e.as_ref(), ctx.json),
    };
    let bound_port = server
        .server_addr()
        .to_ip()
        .map(|a| a.port())
        .unwrap_or(port);
    if let Some(tx) = ready_tx {
        let _ = tx.send(bound_port);
    }
    if !ctx.json {
        eprintln!(
            "[bathos inspect serve] http://127.0.0.1:{bound_port}/ (Ctrl+C 종료 · read-only 감시 중)"
        );
    }
    if open {
        try_open_browser(bound_port);
    }

    // 3) file-watch thread — watches `_state/*`, `03-story-engineering/*` (read-only).
    let watch_root = ctx.agent_team_path.clone();
    let watch_state = Arc::clone(&state);
    let watch_stop = Arc::clone(&stop);
    let verbose = ctx.verbose;
    let watcher_handle =
        std::thread::spawn(move || watch_loop(watch_root, lang, watch_state, verbose, watch_stop));

    // 4) HTTP serving loop (blocking). Does not return until `stop` is set.
    let code = serve_loop(server, state, stop);
    let _ = watcher_handle.join();
    code
}

// ─────────────────────────────────────────────────────────────────────────────
// render (initial page) — reuses the report (P0) pipeline + inserts the live region
// ─────────────────────────────────────────────────────────────────────────────

/// Calls `dashboard::build_report` as-is, then appends only a serve-specific live-update
/// region (`aria-live="polite"`) and a polling script just before `</body>`.
///
/// **Render equivalence (AC)**: it **includes the report pipeline output verbatim as a
/// prefix** with no reimplementation — the appended live region is a serve-unique element
/// not present in report, so it is not byte-identical, but it satisfies the AC's real
/// requirement of "reusing the same pipeline" (test:
/// `initial_page_html_reuses_report_pipeline_*`).
/// [Source: story-2-2 AC "initial render reuses build_report", testing_requirements
///          "render equivalence", design-handoff-kr.md §1.6 aria-live]
fn initial_page_html(pv: &ProjectView, lang: LangArg) -> String {
    let stories: [StoryCardView; 0] = [];
    let html = crate::dashboard::build_report(pv, &stories, lang);
    inject_live_reload(&html)
}

/// Inserts the live-update region + polling script just before `</body>`. Even when
/// `</body>` is not found (should not happen) it does not crash and appends to the end of the document.
fn inject_live_reload(html: &str) -> String {
    let injected = format!(
        r#"<div id="bathos-serve-status" role="status" aria-live="polite" style="position:fixed;right:0;bottom:0;padding:.25rem .5rem;font:12px monospace;opacity:.7;">
  <span class="en">Live — watching for changes</span><span class="kr">실시간 감시 중</span>
</div>
<script>
(function () {{
  // serve 전용 폴링 리로드 — 로컬 단일 클라이언트 전제(SS-1.6). 서버가
  // 재시작/일시 응답불가 상태여도 fetch 실패를 조용히 무시하고 계속 폴링한다.
  var lastVersion = null;
  function poll() {{
    fetch('{endpoint}', {{ cache: 'no-store' }})
      .then(function (r) {{ return r.text(); }})
      .then(function (v) {{
        if (lastVersion === null) {{ lastVersion = v; return; }}
        if (v !== lastVersion) {{ location.reload(); }}
      }})
      .catch(function () {{ /* 무시 — 다음 주기에 재시도 */ }});
  }}
  setInterval(poll, 1000);
  poll();
}})();
</script>
</body>"#,
        endpoint = VERSION_PATH,
    );
    match html.rfind("</body>") {
        Some(idx) => {
            let mut out = String::with_capacity(html.len() + injected.len());
            out.push_str(&html[..idx]);
            out.push_str(&injected);
            out.push_str(&html[idx + "</body>".len()..]);
            out
        }
        // defensive fallback (unreachable on the normal path; render.rs always includes </body>).
        None => format!("{html}{injected}"),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// file watch + debounced re-render
// ─────────────────────────────────────────────────────────────────────────────

/// Watches `_state/*`, `03-story-engineering/*` and re-renders on changes with debounce.
/// Terminates soon after `stop` is set (prevents a thread leak in tests; in operation it
/// is always in progress and is cleaned up together on process termination).
///
/// **read-only discipline (CR-1)**: this function only watches — `notify` does not lock
/// files, and this function neither acquires a lock nor writes state. Even when
/// create/delete events for `.lock` files arrive, they are given no special treatment and
/// are used merely as a re-render trigger (`W-LOCK-IGNORED`). [Source: exceptions-kr.md §5]
fn watch_loop(
    root: PathBuf,
    lang: LangArg,
    state: Arc<ServeState>,
    verbose: bool,
    stop: Arc<AtomicBool>,
) {
    let (tx, rx) = mpsc::channel::<notify::Result<notify::Event>>();
    let mut watcher = match recommended_watcher(move |res| {
        let _ = tx.send(res);
    }) {
        Ok(w) => w,
        Err(e) => {
            // A file-watch init failure is not fatal — the initial render is already being
            // served, so we degrade to "keep serving without live updates" and just notify.
            eprintln!(
                "[bathos inspect serve] 파일와치 초기화 실패(라이브 갱신 비활성, 서빙은 계속): {e}"
            );
            return;
        }
    };

    for sub in WATCH_SUBDIRS {
        let p = root.join(sub);
        if p.exists() {
            if let Err(e) = watcher.watch(&p, RecursiveMode::Recursive) {
                eprintln!(
                    "[bathos inspect serve] 파일와치 등록 실패({}): {e}",
                    p.display()
                );
            }
        } else if verbose {
            // absence is normal (Absent-OK) — may be not yet created, or a directory this project does not use.
            eprintln!(
                "[bathos inspect serve] 감시 대상 경로 부재(정상): {}",
                p.display()
            );
        }
    }

    let mut dirty = false;
    while !stop.load(Ordering::SeqCst) {
        match rx.recv_timeout(DEBOUNCE) {
            Ok(Ok(_event)) => dirty = true,
            Ok(Err(e)) => {
                if verbose {
                    eprintln!("[bathos inspect serve] 파일와치 이벤트 오류(무시하고 계속): {e}");
                }
            }
            Err(RecvTimeoutError::Timeout) => {
                if dirty {
                    dirty = false;
                    reload_and_store(&root, lang, &state, verbose);
                }
            }
            Err(RecvTimeoutError::Disconnected) => break,
        }
    }
}

/// One read-only reload + re-render + state update (+ version increment). Even on
/// failure it keeps serving the last good HTML (no crash, SS-C.1 tolerance principle).
fn reload_and_store(root: &Path, lang: LangArg, state: &Arc<ServeState>, verbose: bool) {
    match loader::load_project(root, LoadOpts { verify_chain: true }) {
        Ok(pv) => {
            let html = initial_page_html(&pv, lang);
            write_html(&state.html, html);
            let v = state.version.fetch_add(1, Ordering::SeqCst) + 1;
            if verbose {
                eprintln!("[bathos inspect serve] 재렌더 완료(version={v})");
            }
        }
        Err(e) => {
            // Even a Fatal load failure (e.g. the manifest briefly disappears) keeps the
            // service running — retains the last successful render as-is ("update deferred", without fabrication).
            eprintln!("[bathos inspect serve] 재로드 실패(마지막 정상 상태 유지): {e}");
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// HTTP serving loop
// ─────────────────────────────────────────────────────────────────────────────

/// Receives requests and responds with `state.html` (latest render) or the version string.
/// When `stop` is set it terminates within a `RECV_POLL` cycle (test-only path; in
/// operation `stop` is never set, so it serves indefinitely until Ctrl+C).
fn serve_loop(server: Server, state: Arc<ServeState>, stop: Arc<AtomicBool>) -> i32 {
    loop {
        if stop.load(Ordering::SeqCst) {
            return 0;
        }
        match server.recv_timeout(RECV_POLL) {
            Ok(Some(request)) => handle_request(request, &state),
            Ok(None) => continue,
            Err(e) => {
                eprintln!("[bathos inspect serve] HTTP 수신 오류(서버 종료): {e}");
                return 1;
            }
        }
    }
}

fn handle_request(request: tiny_http::Request, state: &Arc<ServeState>) {
    let path = request.url().split('?').next().unwrap_or("/").to_string();

    let (status, content_type, body): (u16, &str, String) = if path == "/" || path == "/index.html"
    {
        (200, "text/html; charset=utf-8", read_html(&state.html))
    } else if path == VERSION_PATH {
        (
            200,
            "text/plain; charset=utf-8",
            state.version.load(Ordering::SeqCst).to_string(),
        )
    } else {
        (
            404,
            "text/plain; charset=utf-8",
            "404 not found".to_string(),
        )
    };

    let header = Header::from_bytes(&b"Content-Type"[..], content_type.as_bytes())
        .expect("정적 Content-Type 문자열은 항상 유효한 HTTP 헤더값");
    let response = Response::from_string(body)
        .with_status_code(status)
        .with_header(header);
    // The client may have already dropped the connection (polling cancel, etc.) — a response failure is harmless, so ignore it.
    let _ = request.respond(response);
}

// ─────────────────────────────────────────────────────────────────────────────
// error messages (CLI contract — exceptions-kr.md §5)
// ─────────────────────────────────────────────────────────────────────────────

fn initial_load_error(e: &LoadError, json: bool) -> i32 {
    if json {
        eprintln!(
            "{}",
            serde_json::json!({ "error": "load_failed", "message": e.to_string() })
        );
    } else {
        eprintln!("[bathos inspect serve] {e}");
    }
    1
}

/// `E-PORT-IN-USE` — suggests a different port and exits 1. [Source: exceptions-kr.md §5]
fn port_in_use_error(
    port: u16,
    e: &(dyn std::error::Error + Send + Sync + 'static),
    json: bool,
) -> i32 {
    let alt = port.wrapping_add(1);
    let message = format!(
        "127.0.0.1:{port} 포트를 사용할 수 없습니다({e}). 다른 포트를 지정하세요: --port {alt}"
    );
    if json {
        eprintln!(
            "{}",
            serde_json::json!({ "error": "E-PORT-IN-USE", "message": message, "port": port })
        );
    } else {
        eprintln!("[E-PORT-IN-USE] {message}");
    }
    1
}

// ─────────────────────────────────────────────────────────────────────────────
// automatic browser open (`--open`, default false) — a convenience; serving continues even on failure.
// ─────────────────────────────────────────────────────────────────────────────

fn try_open_browser(port: u16) {
    let url = format!("http://127.0.0.1:{port}/");
    let result = open_url_platform(&url);
    if let Err(e) = result {
        eprintln!("[bathos inspect serve] 브라우저 자동 열기 실패(무시하고 계속): {e}");
    }
}

#[cfg(target_os = "macos")]
fn open_url_platform(url: &str) -> std::io::Result<std::process::Child> {
    std::process::Command::new("open").arg(url).spawn()
}

#[cfg(target_os = "linux")]
fn open_url_platform(url: &str) -> std::io::Result<std::process::Child> {
    std::process::Command::new("xdg-open").arg(url).spawn()
}

#[cfg(target_os = "windows")]
fn open_url_platform(url: &str) -> std::io::Result<std::process::Child> {
    std::process::Command::new("cmd")
        .args(["/C", "start", "", url])
        .spawn()
}

#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
fn open_url_platform(_url: &str) -> std::io::Result<std::process::Child> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "지원되지 않는 플랫폼",
    ))
}

// ─────────────────────────────────────────────────────────────────────────────
// tests (testing_requirements — feature-on smoke / render equivalence / read-only / debounce)
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::{Read, Write};

    fn write_manifest(dir: &Path, json: &str) {
        let state_dir = dir.join("_state");
        fs::create_dir_all(&state_dir).unwrap();
        fs::write(state_dir.join("manifest.json"), json).unwrap();
    }

    fn ctx_for(dir: &Path) -> InspectCtx {
        InspectCtx {
            agent_team_path: dir.to_path_buf(),
            json: false,
            verbose: false,
            strict: false,
        }
    }

    // ── render equivalence ────────────────────────────────────────────────────

    /// serve's initial HTML includes the same pipeline output as report (P0), verbatim
    /// and without reimplementation (no render reimplementation).
    ///
    /// **Note (comparison scope):** `viewmodel::build_vm` stamps a fresh
    /// `generated_at_iso = chrono::Utc::now()` on every call (intended behavior — a live
    /// dashboard's "generated time" should differ on each re-render), so two independent
    /// report calls differ **only in the footer's generated time**. Therefore this test
    /// verifies that everything **before** the `<footer>` start point (= the entire header,
    /// banners, waves, gates, audit log, artifact tree) is byte-identical — this region has
    /// no time-dependent value (this test's manifest has no audit-log/artifacts), enabling a
    /// complete equivalence check. [Source: story-2-2 testing_requirements
    /// "render equivalence", adr-kr.md ADR-P-0005, viewmodel.rs `generated_at_iso`]
    #[test]
    fn initial_page_html_reuses_report_pipeline_and_adds_live_region() {
        let dir = tempfile::tempdir().unwrap();
        write_manifest(dir.path(), r#"{"project": "Serve Smoke"}"#);
        let pv = loader::load_project(dir.path(), LoadOpts { verify_chain: true }).unwrap();

        let expected = crate::dashboard::build_report(&pv, &[], LangArg::Both);
        let served = initial_page_html(&pv, LangArg::Both);

        let footer_idx = expected
            .find("<footer>")
            .expect("report 출력은 항상 <footer>를 포함해야 함");
        assert!(
            served.starts_with(&expected[..footer_idx]),
            "serve 초기 HTML은 footer(생성 시각) 이전까지 build_report 산출물과 완전히 동일해야 함(ADR-P-0005)"
        );
        assert!(
            served.contains("<footer>"),
            "serve도 report와 동일하게 footer를 포함해야 함"
        );
        assert!(
            served.contains(r#"aria-live="polite""#),
            "라이브 갱신 영역에 aria-live=polite 필요(AC)"
        );
        assert!(served.contains("</body>") && served.contains("</html>"));
    }

    // ── read-only (CR-1) ──────────────────────────────────────────────────────

    /// serve's reload path never writes state files — mtime/content unchanged.
    /// [Source: exceptions-kr.md §5, story-2-2 testing_requirements "read-only"]
    #[test]
    fn reload_never_modifies_watched_state_files() {
        let dir = tempfile::tempdir().unwrap();
        write_manifest(dir.path(), r#"{"project": "ReadOnly"}"#);
        let manifest_path = dir.path().join("_state").join("manifest.json");
        let before_meta = fs::metadata(&manifest_path).unwrap().modified().unwrap();
        let before_bytes = fs::read(&manifest_path).unwrap();

        let state = Arc::new(ServeState {
            html: Mutex::new(String::new()),
            version: AtomicU64::new(0),
        });
        reload_and_store(dir.path(), LangArg::Both, &state, false);
        reload_and_store(dir.path(), LangArg::Both, &state, false);

        let after_meta = fs::metadata(&manifest_path).unwrap().modified().unwrap();
        let after_bytes = fs::read(&manifest_path).unwrap();
        assert_eq!(
            before_meta, after_meta,
            "serve는 상태 파일 mtime을 절대 바꾸지 않아야 함(CR-1)"
        );
        assert_eq!(
            before_bytes, after_bytes,
            "serve는 상태 파일 내용을 절대 바꾸지 않아야 함(CR-1)"
        );
        assert!(
            state.version.load(Ordering::SeqCst) >= 2,
            "성공한 재로드마다 버전이 증가해야 함"
        );
    }

    /// Even if a `.lock` file exists, the tool ignores it without taking a lock and still reads successfully.
    /// [Source: exceptions-kr.md §5 W-LOCK-IGNORED]
    #[test]
    fn reload_ignores_lock_file_presence_and_still_loads() {
        let dir = tempfile::tempdir().unwrap();
        write_manifest(dir.path(), r#"{"project": "LockIgnored"}"#);
        fs::write(dir.path().join("_state").join("audit-log.jsonl.lock"), b"").unwrap();

        let state = Arc::new(ServeState {
            html: Mutex::new(String::new()),
            version: AtomicU64::new(0),
        });
        reload_and_store(dir.path(), LangArg::Both, &state, false);

        assert_eq!(
            state.version.load(Ordering::SeqCst),
            1,
            ".lock 존재해도 재로드는 성공해야 함(W-LOCK-IGNORED)"
        );
        assert!(read_html(&state.html).contains("BATHOS inspect"));
    }

    /// Even on a reload failure (e.g. manifest deletion) it keeps the last good HTML as-is
    /// (no crash, no fabrication — degraded to "update deferred").
    #[test]
    fn reload_failure_keeps_last_good_html_without_crashing() {
        let dir = tempfile::tempdir().unwrap();
        write_manifest(dir.path(), r#"{"project": "WillDisappear"}"#);
        let state = Arc::new(ServeState {
            html: Mutex::new("<html>LAST-GOOD</html>".to_string()),
            version: AtomicU64::new(5),
        });

        // Delete manifest.json itself to induce a Fatal load failure.
        fs::remove_file(dir.path().join("_state").join("manifest.json")).unwrap();
        reload_and_store(dir.path(), LangArg::Both, &state, false);

        assert_eq!(
            read_html(&state.html),
            "<html>LAST-GOOD</html>",
            "실패 시 마지막 정상 HTML 유지"
        );
        assert_eq!(
            state.version.load(Ordering::SeqCst),
            5,
            "실패 시 버전 증가 없음(갱신 없음)"
        );
    }

    // ── feature-on smoke (real tiny_http bind → GET / → HTML response) ──────

    /// Smoke-verifies that a `--features serve` build actually binds to
    /// 127.0.0.1:<arbitrary port> and that `GET /` responds 200 + HTML. Binds on port 0
    /// and receives the OS-assigned actual port via `ready_tx` to avoid parallel-test collisions.
    /// [Source: story-2-2 testing_requirements "feature on smoke"]
    #[test]
    fn feature_on_http_get_root_returns_ok_html_smoke() {
        let dir = tempfile::tempdir().unwrap();
        write_manifest(dir.path(), r#"{"project": "HttpSmoke"}"#);
        let ctx = ctx_for(dir.path());

        let (ready_tx, ready_rx) = mpsc::channel::<u16>();
        let stop = Arc::new(AtomicBool::new(false));
        let stop2 = Arc::clone(&stop);

        let handle = std::thread::spawn(move || {
            run_serve_inner(&ctx, 0, LangArg::Both, false, Some(stop2), Some(ready_tx))
        });

        let port = ready_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("서버가 바인딩된 포트를 알려줘야 함(스모크 타임아웃)");

        let mut stream = std::net::TcpStream::connect(("127.0.0.1", port)).expect("서버 연결 실패");
        stream
            .write_all(b"GET / HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n")
            .unwrap();
        let mut response = String::new();
        stream.read_to_string(&mut response).unwrap();

        stop.store(true, Ordering::SeqCst);
        let code = handle.join().expect("serve 스레드 join 실패");

        assert!(
            response.starts_with("HTTP/1.1 200"),
            "GET / 응답이 200이어야 함: {response}"
        );
        assert!(
            response.to_lowercase().contains("<!doctype html"),
            "응답 본문에 HTML 문서가 포함돼야 함"
        );
        assert!(
            response.contains("aria-live=\"polite\""),
            "라이브 리전이 응답에 포함돼야 함"
        );
        assert_eq!(code, 0, "정상 stop 신호로 종료 시 exit code 0");
    }

    /// Confirms the `/__bathos_serve_version` polling endpoint changes value after a
    /// re-render (the basis signal for client polling reload). [Source: SS-1.6, story-2-2 AC]
    #[test]
    fn version_endpoint_changes_after_reload() {
        let dir = tempfile::tempdir().unwrap();
        write_manifest(dir.path(), r#"{"project": "VersionPoll"}"#);
        let ctx = ctx_for(dir.path());

        let (ready_tx, ready_rx) = mpsc::channel::<u16>();
        let stop = Arc::new(AtomicBool::new(false));
        let stop2 = Arc::clone(&stop);
        let handle = std::thread::spawn(move || {
            run_serve_inner(&ctx, 0, LangArg::Both, false, Some(stop2), Some(ready_tx))
        });
        let port = ready_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("포트 통지 타임아웃");

        let fetch_version = |port: u16| -> String {
            let mut stream = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
            stream
                .write_all(
                    format!("GET {VERSION_PATH} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n")
                        .as_bytes(),
                )
                .unwrap();
            let mut resp = String::new();
            stream.read_to_string(&mut resp).unwrap();
            resp
        };

        let before = fetch_version(port);
        assert!(
            before.contains("\r\n\r\n0"),
            "초기 버전은 0이어야 함: {before}"
        );

        // Update the manifest to trigger a file-watch event. An OS file-event backend
        // (e.g. macOS FSEvents) can lag more than the debounce window (300ms), so instead
        // of a single fixed sleep we re-poll at short intervals for up to 5 seconds (passes
        // reliably even on slow backends — avoids timing flakiness).
        write_manifest(dir.path(), r#"{"project": "VersionPoll-Changed"}"#);
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        let mut after = before.clone();
        while std::time::Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(150));
            after = fetch_version(port);
            if after != before {
                break;
            }
        }

        stop.store(true, Ordering::SeqCst);
        let _ = handle.join();

        assert_ne!(
            before, after,
            "파일 변경 후 버전이 증가해야 함(디바운스 재렌더, 5초 내)"
        );
    }
}
