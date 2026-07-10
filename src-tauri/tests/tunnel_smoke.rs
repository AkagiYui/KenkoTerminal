//! End-to-end tunnel (direct-tcpip) smoke test.
//!
//! Opens a forward to the server's own loopback SSH (127.0.0.1:22) and checks it
//! reads back an SSH banner — proving the forward pipes data. Runs only when
//! `KENKO_SSH_*` env vars are set, so no credentials live in the repo.

use kenkoterminal_lib::ssh::SshConfig;
use kenkoterminal_lib::tunnel::probe_forward;

#[tokio::test]
async fn forward_reads_remote_ssh_banner() {
    let (host, user, pass) = match (
        std::env::var("KENKO_SSH_HOST"),
        std::env::var("KENKO_SSH_USER"),
        std::env::var("KENKO_SSH_PASS"),
    ) {
        (Ok(h), Ok(u), Ok(p)) => (h, u, p),
        _ => {
            eprintln!("skipping forward smoke — set KENKO_SSH_HOST/USER/PASS to run");
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

    let data = probe_forward(&cfg, "127.0.0.1", 22, 64)
        .await
        .expect("forward failed");
    let banner = String::from_utf8_lossy(&data);
    assert!(banner.starts_with("SSH-"), "expected SSH banner, got: {banner:?}");
}
