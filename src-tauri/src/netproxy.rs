//! Proxy settings for update and extension fetches.
//!
//! curl honours `http_proxy`/`https_proxy`/`all_proxy` on its own, but it never reads the
//! macOS or Windows system proxy that tools like Clash or v2rayN set, and an app opened from
//! Finder or the Start menu has no proxy variables. The updater plugin's HTTP client is built
//! without system-proxy support too. Without this, GitHub times out for users who browse fine.
//! Loopback is always excluded so local mock servers and tests are never proxied.

use url::{Host, Url};

const NO_PROXY: &str = "127.0.0.1,localhost,::1";

pub fn curl_proxy_args() -> Vec<String> {
    let mut args = vec!["--noproxy".to_string(), NO_PROXY.to_string()];
    if let Some(proxy) = fallback_proxy() {
        args.push("--proxy".to_string());
        args.push(proxy);
    }
    args
}

/// Proxy for the updater plugin's check and download of `endpoint`. Its HTTP client reads
/// proxy variables but not the system proxy, and cannot speak SOCKS, so only HTTP proxies
/// are passed on.
pub fn updater_proxy(endpoint: &Url) -> Option<Url> {
    if is_loopback(endpoint) {
        return None;
    }
    http_proxy_url(&fallback_proxy()?)
}

/// The system proxy, unless a proxy variable is set, which curl and the plugin already honour.
fn fallback_proxy() -> Option<String> {
    if env_proxy_set() {
        return None;
    }
    system_proxy()
}

fn http_proxy_url(raw: &str) -> Option<Url> {
    let url = Url::parse(raw).ok()?;
    matches!(url.scheme(), "http" | "https").then_some(url)
}

fn is_loopback(url: &Url) -> bool {
    match url.host() {
        Some(Host::Domain(domain)) => domain.eq_ignore_ascii_case("localhost"),
        Some(Host::Ipv4(ip)) => ip.is_loopback(),
        Some(Host::Ipv6(ip)) => ip.is_loopback(),
        None => false,
    }
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

#[cfg(target_os = "macos")]
fn system_proxy() -> Option<String> {
    use crate::nowindow::NoWindow;
    let out = std::process::Command::new("/usr/sbin/scutil")
        .arg("--proxy")
        .no_window()
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    parse_scutil_proxy(&String::from_utf8_lossy(&out.stdout))
}

#[cfg(not(any(windows, target_os = "macos")))]
fn system_proxy() -> Option<String> {
    None
}

/// Top-level `Key : value` lines of `scutil --proxy` into a curl proxy URL, preferring
/// HTTPS, then HTTP, then SOCKS. Nested blocks such as `ExceptionsList` are skipped.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn parse_scutil_proxy(output: &str) -> Option<String> {
    let mut depth = 0_usize;
    let mut fields = std::collections::HashMap::new();
    for line in output.lines().map(str::trim) {
        if line.starts_with('}') {
            depth = depth.saturating_sub(1);
            continue;
        }
        let opens = line.ends_with('{');
        if depth == 1 && !opens {
            if let Some((key, value)) = line.split_once(" : ") {
                fields.insert(key.trim(), value.trim());
            }
        }
        if opens {
            depth += 1;
        }
    }
    let entry = |kind: &str, scheme: &str| {
        if fields.get(format!("{kind}Enable").as_str()) != Some(&"1") {
            return None;
        }
        let host = *fields.get(format!("{kind}Proxy").as_str())?;
        if host.is_empty() {
            return None;
        }
        let host = if host.contains(':') {
            format!("[{host}]")
        } else {
            host.to_string()
        };
        let address = match fields.get(format!("{kind}Port").as_str()) {
            Some(port) => format!("{host}:{port}"),
            None => host,
        };
        Some(with_scheme(&address, scheme))
    };
    entry("HTTPS", "http")
        .or_else(|| entry("HTTP", "http"))
        .or_else(|| entry("SOCKS", "socks5"))
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

#[cfg_attr(not(any(windows, target_os = "macos")), allow(dead_code))]
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

    const SCUTIL: &str = "<dictionary> {
  ExceptionsList : <array> {
    0 : localhost
    1 : 127.0.0.0/8
  }
  ExcludeSimpleHostnames : 1
  HTTPEnable : 1
  HTTPPort : 7890
  HTTPProxy : 127.0.0.1
  HTTPSEnable : 1
  HTTPSPort : 7891
  HTTPSProxy : 127.0.0.1
  ProxyAutoConfigEnable : 0
  SOCKSEnable : 1
  SOCKSPort : 7892
  SOCKSProxy : 127.0.0.1
}
";

    #[test]
    fn scutil_prefers_https() {
        assert_eq!(
            parse_scutil_proxy(SCUTIL).as_deref(),
            Some("http://127.0.0.1:7891")
        );
    }

    #[test]
    fn scutil_falls_back_to_http_then_socks() {
        let http = SCUTIL.replace("HTTPSEnable : 1", "HTTPSEnable : 0");
        assert_eq!(
            parse_scutil_proxy(&http).as_deref(),
            Some("http://127.0.0.1:7890")
        );
        let socks = http.replace("HTTPEnable : 1", "HTTPEnable : 0");
        assert_eq!(
            parse_scutil_proxy(&socks).as_deref(),
            Some("socks5://127.0.0.1:7892")
        );
    }

    #[test]
    fn scutil_without_enabled_proxy_is_none() {
        let off = "<dictionary> {\n  HTTPEnable : 0\n  HTTPSEnable : 0\n  SOCKSEnable : 0\n}\n";
        assert_eq!(parse_scutil_proxy(off), None);
        assert_eq!(parse_scutil_proxy(""), None);
    }

    #[test]
    fn scutil_ignores_nested_blocks() {
        let scoped = "<dictionary> {
  __SCOPED__ : <dictionary> {
    en0 : <dictionary> {
      HTTPSEnable : 1
      HTTPSPort : 9999
      HTTPSProxy : 10.0.0.1
    }
  }
  HTTPSEnable : 0
}
";
        assert_eq!(parse_scutil_proxy(scoped), None);
    }

    #[test]
    fn scutil_brackets_ipv6_hosts() {
        let v6 = "<dictionary> {\n  HTTPSEnable : 1\n  HTTPSPort : 8080\n  HTTPSProxy : ::1\n}\n";
        assert_eq!(parse_scutil_proxy(v6).as_deref(), Some("http://[::1]:8080"));
    }

    #[test]
    fn updater_only_takes_http_proxies() {
        assert!(http_proxy_url("http://127.0.0.1:7890").is_some());
        assert!(http_proxy_url("socks5://127.0.0.1:7892").is_none());
        assert!(http_proxy_url("not a url").is_none());
    }

    #[test]
    fn updater_never_proxies_loopback_endpoints() {
        for endpoint in [
            "http://127.0.0.1:4010/latest.json",
            "http://localhost:4010/latest.json",
            "http://[::1]:4010/latest.json",
        ] {
            let url = Url::parse(endpoint).unwrap();
            assert!(is_loopback(&url));
            assert_eq!(updater_proxy(&url), None);
        }
        assert!(!is_loopback(
            &Url::parse(crate::updater::STABLE_ENDPOINT).unwrap()
        ));
    }

    #[test]
    fn loopback_is_never_proxied() {
        assert!(curl_proxy_args()
            .windows(2)
            .any(|w| w[0] == "--noproxy" && w[1].contains("127.0.0.1")));
    }
}
