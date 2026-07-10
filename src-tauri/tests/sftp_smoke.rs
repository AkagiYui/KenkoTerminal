//! SFTP smoke test — lists `/` on the server and expects common root entries.
//! Runs only when `KENKO_SSH_*` env vars are set (no credentials in the repo).

use kenkoterminal_lib::sftp::list_once;
use kenkoterminal_lib::ssh::SshConfig;

#[tokio::test]
async fn sftp_lists_root() {
    let (host, user, pass) = match (
        std::env::var("KENKO_SSH_HOST"),
        std::env::var("KENKO_SSH_USER"),
        std::env::var("KENKO_SSH_PASS"),
    ) {
        (Ok(h), Ok(u), Ok(p)) => (h, u, p),
        _ => {
            eprintln!("skipping sftp_lists_root — set KENKO_SSH_HOST/USER/PASS to run");
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

    let entries = list_once(&cfg, "/").await.expect("sftp list / failed");
    assert!(!entries.is_empty(), "root listing was empty");
    assert!(
        entries.iter().any(|e| e == "etc"),
        "expected /etc in root listing, got: {entries:?}"
    );
}
