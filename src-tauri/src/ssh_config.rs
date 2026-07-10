//! Minimal `~/.ssh/config` reader: resolve a Host alias to HostName / Port / User /
//! IdentityFile so aliases and configured keys "just work" (R2).

use std::path::PathBuf;

#[derive(Default, Debug, Clone)]
pub struct ResolvedHost {
    pub hostname: Option<String>,
    pub port: Option<u16>,
    pub user: Option<String>,
    pub identity_files: Vec<PathBuf>,
}

fn matches(pattern: &str, host: &str) -> bool {
    if pattern == "*" || pattern == host {
        return true;
    }
    if let Some(star) = pattern.find('*') {
        let (pre, post) = (&pattern[..star], &pattern[star + 1..]);
        return host.len() >= pre.len() + post.len() && host.starts_with(pre) && host.ends_with(post);
    }
    false
}

fn expand(path: &str) -> PathBuf {
    if let Some(rest) = path.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest);
        }
    }
    PathBuf::from(path)
}

pub fn resolve(host: &str) -> ResolvedHost {
    let mut out = ResolvedHost::default();
    let path = match dirs::home_dir() {
        Some(h) => h.join(".ssh").join("config"),
        None => return out,
    };
    let text = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(_) => return out,
    };
    let mut applies = false;
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut parts = line.splitn(2, |c: char| c.is_whitespace() || c == '=');
        let key = parts.next().unwrap_or("").to_lowercase();
        let val = parts.next().unwrap_or("").trim().trim_start_matches('=').trim();
        if key == "host" {
            applies = val.split_whitespace().any(|pat| matches(pat, host));
        } else if applies {
            match key.as_str() {
                "hostname" => out.hostname = Some(val.to_string()),
                "port" => out.port = val.parse().ok(),
                "user" => out.user = Some(val.to_string()),
                "identityfile" => out.identity_files.push(expand(val)),
                _ => {}
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::matches;

    #[test]
    fn glob_and_exact() {
        assert!(matches("*", "anything"));
        assert!(matches("web1", "web1"));
        assert!(matches("*.example.com", "a.example.com"));
        assert!(matches("web*", "web42"));
        assert!(!matches("web*", "db1"));
    }
}
