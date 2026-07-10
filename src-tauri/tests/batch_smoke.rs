//! Batch fan-out smoke test — runs one command across 2 targets (the same server
//! twice, standing in for a host group) and checks both come back ok.
//! Runs only when `KENKO_SSH_*` env vars are set (no credentials in the repo).

use kenkoterminal_lib::batch::{run_batch, BatchTarget};
use kenkoterminal_lib::ssh::SshConfig;

#[tokio::test]
async fn batch_runs_across_targets() {
    let (host, user, pass) = match (
        std::env::var("KENKO_SSH_HOST"),
        std::env::var("KENKO_SSH_USER"),
        std::env::var("KENKO_SSH_PASS"),
    ) {
        (Ok(h), Ok(u), Ok(p)) => (h, u, p),
        _ => {
            eprintln!("skipping batch_runs_across_targets — set KENKO_SSH_HOST/USER/PASS to run");
            return;
        }
    };
    let port = std::env::var("KENKO_SSH_PORT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(22);

    let cfg = SshConfig {
        host,
        port,
        user,
        password: Some(pass),
    };
    let targets = vec![
        BatchTarget { label: "a".into(), ssh: cfg.clone() },
        BatchTarget { label: "b".into(), ssh: cfg.clone() },
    ];

    let results = run_batch(targets, "echo kenko_batch_ok".into(), 4).await;
    assert_eq!(results.len(), 2, "expected 2 results");
    assert!(results.iter().all(|r| r.ok), "some target failed: {results:?}");
    assert!(
        results.iter().all(|r| r.output.contains("kenko_batch_ok")),
        "unexpected output: {results:?}"
    );
}
