//! TG-605: `packages/core` drives a real server from a non-DOM host. Bundles
//! `hosts/node/smoke.ts` with Bun and runs it under plain Node (no DOM, no polyfill) against
//! this test's server: sign up → list chats → create one → join its socket → send → receive the
//! broadcast → read history. Skips with a notice when `node` or `bun` is not installed.

mod poll_support;

use std::process::Command;

fn available(tool: &str) -> bool {
    Command::new(tool)
        .arg("--version")
        .output()
        .is_ok_and(|output| output.status.success())
}

#[tokio::test]
async fn core_runs_end_to_end_in_a_non_dom_node_host() {
    if !available("node") || !available("bun") {
        eprintln!("SKIPPED: node and bun are needed for the TG-605 host check");
        return;
    }
    let server = poll_support::serve_memory().await;
    let root = env!("CARGO_MANIFEST_DIR");
    // Typechecks with no DOM lib at all (hosts/node/tsconfig.json), core included.
    let typed = Command::new("bunx")
        .args(["tsc", "-p", "hosts/node"])
        .current_dir(root)
        .output()
        .expect("tsc");
    assert!(
        typed.status.success(),
        "{}",
        String::from_utf8_lossy(&typed.stdout)
    );
    let bundle = std::env::temp_dir().join(format!("core-node-host-{}.mjs", std::process::id()));
    let built = Command::new("bun")
        .args(["build", "hosts/node/smoke.ts", "--target=node", "--outfile"])
        .arg(&bundle)
        .current_dir(root)
        .output()
        .expect("bun build");
    assert!(
        built.status.success(),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    let source = std::fs::read_to_string(&bundle).unwrap();
    assert!(
        !source.contains("document.") && !source.contains("window."),
        "the core bundle must not reach for DOM globals"
    );

    let base = server.base.clone();
    let bundle_path = bundle.clone();
    let run = tokio::task::spawn_blocking(move || {
        Command::new("node")
            .arg(&bundle_path)
            .arg(&base)
            .output()
            .expect("node")
    })
    .await
    .unwrap();
    let _ = std::fs::remove_file(&bundle);
    let stdout = String::from_utf8_lossy(&run.stdout);
    assert!(
        run.status.success(),
        "{stdout}\n{}",
        String::from_utf8_lossy(&run.stderr)
    );
    for step in [
        "signed up as",
        "listed",
        "created chat",
        "received its broadcast",
        "read it back from history",
        "core-node-host: OK",
    ] {
        assert!(stdout.contains(step), "missing step `{step}`:\n{stdout}");
    }
}
