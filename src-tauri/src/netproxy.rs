//! Proxy arguments for the `curl` children we spawn for update and extension fetches.
//!
//! curl honours `http_proxy`/`https_proxy`/`all_proxy` on its own, but on Windows it never
//! reads the system proxy that tools like Clash or v2rayN set, so GitHub times out for users
//! who browse fine. Loopback is always excluded so local mock servers and tests are never proxied.

const NO_PROXY: &str = "127.0.0.1,localhost,::1";

pub fn curl_proxy_args() -> Vec<String> {
    let mut args = vec!["--noproxy".to_string(), NO_PROXY.to_string()];
    if !env_proxy_set() {
        if let Some(proxy) = system_proxy() {
            args.push("--proxy".to_string());
            args.push(proxy);
        }
    }
    args
}

fn env_proxy_set() -> bool {
    ["https_proxy", "HTTPS_PROXY", "all_proxy", "ALL_PROXY"]
        .iter()
        .any(|name| std::env::var(name).is_ok_and(|v| !v.trim().is_empty()))
}

#[cfg(windows)]
fn system_proxy() -> Option<String> {
    use crate::nowindow::NoWindow;
    const KEY: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Internet Settings";
    let query = |name: &str| {
        let out = std::process::Command::new("reg")
            .args(["query", KEY, "/v", name])
            .no_window()
            .output()
            .ok()?;
        out.status
            .success()
            .then(|| String::from_utf8_lossy(&out.stdout).into_owned())
    };
    let enabled = reg_value(&query("ProxyEnable")?, "ProxyEnable")?;
    if enabled
        .trim_start_matches("0x")
        .trim_start_matches('0')
        .is_empty()
    {
        return None;
    }
    parse_proxy_server(&reg_value(&query("ProxyServer")?, "ProxyServer")?)
}

#[cfg(not(windows))]
fn system_proxy() -> Option<String> {
    None
}

/// Value column of one `reg query` output line: `    Name    REG_SZ    data`.
#[cfg_attr(not(windows), allow(dead_code))]
fn reg_value(output: &str, name: &str) -> Option<String> {
    output.lines().find_map(|line| {
        let mut parts = line.split_whitespace();
        if parts.next()? != name {
            return None;
        }
        parts.next()?; // REG_SZ / REG_DWORD
        let value = parts.collect::<Vec<_>>().join(" ");
        (!value.is_empty()).then_some(value)
    })
}

/// `host:port` or `http=h:p;https=h:p;socks=h:p` into a curl proxy URL, preferring https.
#[cfg_attr(not(windows), allow(dead_code))]
fn parse_proxy_server(raw: &str) -> Option<String> {
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }
    if !raw.contains('=') {
        return Some(with_scheme(raw, "http"));
    }
    let find = |key: &str| {
        raw.split(';').find_map(|entry| {
            let (k, v) = entry.split_once('=')?;
            (k.trim().eq_ignore_ascii_case(key) && !v.trim().is_empty()).then(|| v.trim())
        })
    };
    find("https")
        .or_else(|| find("http"))
        .map(|v| with_scheme(v, "http"))
        .or_else(|| find("socks").map(|v| with_scheme(v, "socks5")))
}

#[cfg_attr(not(windows), allow(dead_code))]
fn with_scheme(value: &str, scheme: &str) -> String {
    if value.contains("://") {
        value.to_string()
    } else {
        format!("{scheme}://{value}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bare_host_port() {
        assert_eq!(
            parse_proxy_server("127.0.0.1:7890").as_deref(),
            Some("http://127.0.0.1:7890")
        );
    }

    #[test]
    fn per_protocol_prefers_https() {
        assert_eq!(
            parse_proxy_server("http=a:1;https=b:2;socks=c:3").as_deref(),
            Some("http://b:2")
        );
        assert_eq!(
            parse_proxy_server("socks=c:3").as_deref(),
            Some("socks5://c:3")
        );
    }

    #[test]
    fn empty_is_none() {
        assert_eq!(parse_proxy_server("  "), None);
    }

    #[test]
    fn reads_reg_output() {
        let out = "\r\nHKEY_CURRENT_USER\\...\\Internet Settings\r\n    ProxyEnable    REG_DWORD    0x1\r\n";
        assert_eq!(reg_value(out, "ProxyEnable").as_deref(), Some("0x1"));
        assert_eq!(reg_value(out, "ProxyServer"), None);
    }

    #[test]
    fn loopback_is_never_proxied() {
        assert!(curl_proxy_args()
            .windows(2)
            .any(|w| w[0] == "--noproxy" && w[1].contains("127.0.0.1")));
    }
}
