//! End-to-end SSH smoke test.
//!
//! Runs ONLY when `KENKO_SSH_HOST` / `KENKO_SSH_USER` / `KENKO_SSH_PASS` are set,
//! so no credentials ever live in the repo. Without them it is a no-op pass, so
//! CI stays green while local runs exercise a real server.

use kenkoterminal_lib::ssh::{exec_capture, SshConfig};

#[tokio::test]
async fn ssh_echo_smoke() {
    let (host, user, pass) = match (
        std::env::var("KENKO_SSH_HOST"),
        std::env::var("KENKO_SSH_USER"),
        std::env::var("KENKO_SSH_PASS"),
    ) {
        (Ok(h), Ok(u), Ok(p)) => (h, u, p),
        _ => {
            eprintln!("skipping ssh_echo_smoke — set KENKO_SSH_HOST/USER/PASS to run");
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

    let (out, code) = exec_capture(&cfg, "echo kenko_ok && uname -s")
        .await
        .expect("ssh exec failed");

    assert!(out.contains("kenko_ok"), "unexpected output: {out:?}");
    assert_eq!(code, 0, "non-zero exit; output: {out:?}");
}
