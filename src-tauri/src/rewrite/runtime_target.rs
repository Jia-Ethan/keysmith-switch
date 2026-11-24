//! Runtime-aware input-rewrite links.
//!
//! The relay stays on Windows. For WSL targets, config files are read and atomically written
//! by the selected Linux user through `runtime-helper.py`; only a short-lived staging copy is
//! parsed by the existing format-specific link code. Link records remain in Windows app data
//! and are keyed by the complete distribution/user identity.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};

use super::{
    claude, grok, link, service, zcode, ClaudeView, CodexView, GrokView, RewriteView,
    ZcodeProviderView, ZcodeView,
};
use crate::adapter::AdapterOptions;
use crate::db::Store;
use crate::error::{Error, Result};
use crate::models::{RuntimeTarget, ToolKind};
use crate::paths::{atomic_write, AppPaths};

const CODEX_FILES: &[&str] = &["config.toml"];
const CLAUDE_FILES: &[&str] = &["settings.json"];
const ZCODE_FILES: &[&str] = &["provider_config.json"];
const GROK_FILES: &[&str] = &["config.toml", "models_cache.json"];

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RemoteFile {
    kind: String,
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    sha256: Option<String>,
    #[serde(default)]
    mode: Option<u32>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RemoteRead {
    directory_exists: bool,
    #[serde(default)]
    files: HashMap<String, RemoteFile>,
}

struct StagedConfig {
    _temp: tempfile::TempDir,
    pub dir: PathBuf,
    real_dir: PathBuf,
    target: RuntimeTarget,
    files: HashMap<String, RemoteFile>,
}

impl StagedConfig {
    fn load(target: &RuntimeTarget, real_dir: &Path, names: &[&str]) -> Result<Self> {
        let response = crate::runtime::invoke_helper(
            target,
            &json!({
                "command": "rewriteRead",
                "directory": real_dir,
                "files": names,
            }),
        )?;
        let read: RemoteRead = serde_json::from_value(response)?;
        let temp = tempfile::tempdir()?;
        let dir = temp.path().join("config");
        std::fs::create_dir(&dir)?;
        if read.directory_exists {
            for (name, file) in &read.files {
                if file.kind == "file" {
                    let content = file
                        .content
                        .as_deref()
                        .ok_or_else(|| Error::invalid("WSL rewrite file has no content"))?;
                    std::fs::write(dir.join(name), content)?;
                }
            }
        }
        Ok(Self {
            _temp: temp,
            dir,
            real_dir: real_dir.to_path_buf(),
            target: target.clone(),
            files: read.files,
        })
    }

    fn flush(&self, names: &[&str]) -> Result<()> {
        let mut operations = Vec::new();
        for name in names {
            let staged = self.dir.join(name);
            let Some(content) = std::fs::read_to_string(&staged).ok() else {
                continue;
            };
            let after = hex::encode(Sha256::digest(content.as_bytes()));
            let before = self.files.get(*name);
            if before.and_then(|file| file.sha256.as_deref()) == Some(after.as_str()) {
                continue;
            }
            operations.push(json!({
                "name": name,
                "beforeSha256": before.and_then(|file| file.sha256.as_deref()),
                "content": content,
                "mode": before.and_then(|file| file.mode),
            }));
        }
        if operations.is_empty() {
            return Ok(());
        }
        crate::runtime::invoke_helper(
            &self.target,
            &json!({
                "command": "rewriteWrite",
                "directory": self.real_dir,
                "operations": operations,
            }),
        )?;
        Ok(())
    }
}

fn target_dir(paths: &AppPaths, target: &RuntimeTarget) -> PathBuf {
    let digest = hex::encode(Sha256::digest(target.storage_key().as_bytes()));
    super::rewrite_dir(paths)
        .join("targets")
        .join(&digest[..24])
}

fn record_path(paths: &AppPaths, target: &RuntimeTarget, name: &str) -> PathBuf {
    target_dir(paths, target).join(format!("{name}-link.json"))
}

fn read_record<T: DeserializeOwned>(
    paths: &AppPaths,
    target: &RuntimeTarget,
    name: &str,
) -> Option<T> {
    std::fs::read(record_path(paths, target, name))
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
}

fn write_record<T: Serialize>(
    paths: &AppPaths,
    target: &RuntimeTarget,
    name: &str,
    record: &T,
) -> Result<()> {
    let path = record_path(paths, target, name);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    atomic_write(&path, &serde_json::to_string_pretty(record)?)
}

fn remove_record(paths: &AppPaths, target: &RuntimeTarget, name: &str) {
    let _ = std::fs::remove_file(record_path(paths, target, name));
}

fn config_dir(opts: &AdapterOptions, tool: ToolKind) -> Result<PathBuf> {
    if let Some(dir) = opts.config_dirs.get(&tool) {
        return Ok(if tool == ToolKind::Zcode {
            crate::runtime::join_path(dir, "v2")
        } else {
            dir.clone()
        });
    }
    let home = opts
        .home
        .as_deref()
        .ok_or_else(|| Error::invalid("the runtime home folder is unknown"))?;
    let child = match tool {
        ToolKind::Codex => ".codex",
        ToolKind::Claude => ".claude",
        ToolKind::Grok => ".grok",
        ToolKind::Zcode => ".zcode/v2",
        _ => {
            return Err(Error::unavailable(
                "input rewrite is not supported for this agent",
            ))
        }
    };
    Ok(crate::runtime::join_path(home, child))
}

fn runtime_prefix(target: &RuntimeTarget) -> String {
    super::runtime_profile(target).expect("WSL targets have a rewrite profile")
}

fn route_id(target: &RuntimeTarget, upstream: &str) -> String {
    format!("{}-{upstream}", runtime_prefix(target))
}

fn relay_config(
    paths: &AppPaths,
    target: &RuntimeTarget,
) -> Result<(service::RelayConfig, String)> {
    let host = crate::runtime::relay_address(target)?;
    let mut config = service::ensure_config(paths)?;
    if host != "127.0.0.1" && !config.listen_addresses.contains(&host) {
        config.listen_addresses.push(host.clone());
        config.listen_addresses.sort();
        config.listen_addresses.dedup();
    }
    Ok((config, host))
}

fn staged_codex_record(
    paths: &AppPaths,
    target: &RuntimeTarget,
    dir: &Path,
) -> Option<link::LinkRecord> {
    let mut record: link::LinkRecord = read_record(paths, target, "codex")?;
    record.codex_dir = dir.to_path_buf();
    Some(record)
}

fn staged_claude_record(
    paths: &AppPaths,
    target: &RuntimeTarget,
    dir: &Path,
) -> Option<claude::ClaudeLinkRecord> {
    let mut record: claude::ClaudeLinkRecord = read_record(paths, target, "claude")?;
    record.claude_dir = dir.to_path_buf();
    Some(record)
}

fn staged_zcode_record(
    paths: &AppPaths,
    target: &RuntimeTarget,
    dir: &Path,
) -> Option<zcode::ZcodeLinkRecord> {
    let mut record: zcode::ZcodeLinkRecord = read_record(paths, target, "zcode")?;
    record.zcode_dir = dir.to_path_buf();
    Some(record)
}

fn staged_grok_record(
    paths: &AppPaths,
    target: &RuntimeTarget,
    dir: &Path,
) -> Option<grok::GrokLinkRecord> {
    let mut record: grok::GrokLinkRecord = read_record(paths, target, "grok")?;
    record.grok_dir = dir.to_path_buf();
    Some(record)
}

fn codex_view(paths: &AppPaths, opts: &AdapterOptions) -> Result<CodexView> {
    let real = config_dir(opts, ToolKind::Codex)?;
    let stage = StagedConfig::load(&opts.runtime_target, &real, CODEX_FILES)?;
    let record = staged_codex_record(paths, &opts.runtime_target, &stage.dir);
    let link_state = link::state(record.as_ref(), &stage.dir);
    let (provider, unsupported) = match link::current_provider(&stage.dir) {
        Ok(provider) => (Some(provider), None),
        Err(reason) => (None, Some(reason.code().to_string())),
    };
    Ok(CodexView {
        link: link_state,
        service: service::status(paths),
        provider,
        unsupported,
        codex_dir: Some(real.to_string_lossy().into_owned()),
    })
}

fn claude_view(paths: &AppPaths, opts: &AdapterOptions) -> Result<ClaudeView> {
    let real = config_dir(opts, ToolKind::Claude)?;
    let stage = StagedConfig::load(&opts.runtime_target, &real, CLAUDE_FILES)?;
    let record = staged_claude_record(paths, &opts.runtime_target, &stage.dir);
    let link_state = claude::state(record.as_ref(), &stage.dir);
    let (upstream, unsupported) = match (&link_state, &record) {
        (claude::ClaudeLinkState::Linked, Some(record)) => (
            Some(
                record
                    .previous
                    .clone()
                    .unwrap_or_else(|| claude::DEFAULT_UPSTREAM.into()),
            ),
            None,
        ),
        _ => match claude::current_upstream(&stage.dir) {
            Ok(upstream) => (Some(upstream), None),
            Err(reason) => (None, Some(reason.code().to_string())),
        },
    };
    Ok(ClaudeView {
        link: link_state,
        service: service::status(paths),
        upstream_host: upstream.as_deref().and_then(claude::upstream_host),
        unsupported,
        settings_path: Some(
            crate::runtime::join_path(&real, "settings.json")
                .to_string_lossy()
                .into_owned(),
        ),
    })
}

fn zcode_view(paths: &AppPaths, opts: &AdapterOptions) -> Result<ZcodeView> {
    let real = config_dir(opts, ToolKind::Zcode)?;
    let stage = StagedConfig::load(&opts.runtime_target, &real, ZCODE_FILES)?;
    let record = staged_zcode_record(paths, &opts.runtime_target, &stage.dir);
    let link_state = zcode::state(record.as_ref(), &stage.dir);
    let (providers, unsupported) = match zcode::providers(&stage.dir) {
        Ok(found) => (
            found
                .into_iter()
                .map(|provider| {
                    let real_url = record
                        .as_ref()
                        .and_then(|record| {
                            record
                                .providers
                                .iter()
                                .find(|linked| linked.provider_id == provider.id)
                        })
                        .filter(|linked| linked.relay_base_url == provider.base_url)
                        .map(|linked| linked.previous.clone())
                        .unwrap_or(provider.base_url);
                    ZcodeProviderView {
                        name: provider.name,
                        host: claude::upstream_host(&real_url),
                    }
                })
                .collect(),
            None,
        ),
        Err(reason) => (Vec::new(), Some(reason.code().to_string())),
    };
    Ok(ZcodeView {
        link: link_state,
        service: service::status(paths),
        providers,
        unsupported,
        config_path: Some(
            crate::runtime::join_path(&real, "provider_config.json")
                .to_string_lossy()
                .into_owned(),
        ),
    })
}

fn grok_view(paths: &AppPaths, opts: &AdapterOptions) -> Result<GrokView> {
    let real = config_dir(opts, ToolKind::Grok)?;
    let stage = StagedConfig::load(&opts.runtime_target, &real, GROK_FILES)?;
    let record = staged_grok_record(paths, &opts.runtime_target, &stage.dir);
    let link_state = grok::state(record.as_ref(), &stage.dir);
    let (hosts, unsupported) = match grok::routable(&stage.dir) {
        Ok(models) => {
            let mut hosts: Vec<String> = models
                .iter()
                .filter_map(|model| claude::upstream_host(&model.base_url))
                .collect();
            hosts.sort();
            hosts.dedup();
            (hosts, None)
        }
        Err(reason) => (Vec::new(), Some(reason.code().to_string())),
    };
    Ok(GrokView {
        link: link_state,
        service: service::status(paths),
        hosts,
        left_out: grok::left_out(&stage.dir),
        unsupported,
        config_path: Some(
            crate::runtime::join_path(&real, "config.toml")
                .to_string_lossy()
                .into_owned(),
        ),
    })
}

pub fn view(store: &Store, opts: &AdapterOptions) -> Result<RewriteView> {
    let switches = store.rewrite_switches(&opts.runtime_target)?;
    let mut tables = store.list_rule_tables_for(&opts.runtime_target)?;
    for table in &mut tables {
        table.seal_for_view();
    }
    Ok(RewriteView {
        enabled: switches.enabled,
        codex_enabled: switches.codex_enabled,
        claude_enabled: switches.claude_enabled,
        grok_enabled: switches.grok_enabled,
        zcode_enabled: switches.zcode_enabled,
        tables,
        codex: codex_view(store.paths(), opts)?,
        claude: claude_view(store.paths(), opts)?,
        zcode: zcode_view(store.paths(), opts)?,
        grok: grok_view(store.paths(), opts)?,
    })
}

pub fn connect(paths: &AppPaths, tool: ToolKind, opts: &AdapterOptions) -> Result<()> {
    let target = &opts.runtime_target;
    let (mut config, host) = relay_config(paths, target)?;
    match tool {
        ToolKind::Codex => {
            let real = config_dir(opts, tool)?;
            let stage = StagedConfig::load(target, &real, CODEX_FILES)?;
            let previous = staged_codex_record(paths, target, &stage.dir);
            let keep = previous.as_ref().filter(|_| link::uses_managed(&stage.dir));
            let (provider_id, upstream) = match keep {
                Some(record) => {
                    let id = route_id(target, &record.provider);
                    let upstream = config.upstreams.get(&id).cloned().ok_or_else(|| {
                        Error::drift("relay has no upstream for the linked provider")
                    })?;
                    (record.provider.clone(), upstream)
                }
                None => {
                    let provider = link::current_provider(&stage.dir).map_err(|reason| {
                        Error::unavailable(format!("{}: Codex cannot be linked", reason.code()))
                    })?;
                    (provider.id, provider.base_url)
                }
            };
            let id = route_id(target, &provider_id);
            config.upstreams.insert(id.clone(), upstream);
            service::write_config(paths, &config)?;
            service::install(paths)?;
            let (mut record, _) =
                link::link(&stage.dir, keep, &service::base_url_at(&config, &host, &id))?;
            stage.flush(CODEX_FILES)?;
            record.codex_dir = real;
            write_record(paths, target, "codex", &record)?;
        }
        ToolKind::Claude => {
            let real = config_dir(opts, tool)?;
            let stage = StagedConfig::load(target, &real, CLAUDE_FILES)?;
            let previous = staged_claude_record(paths, target, &stage.dir);
            let still_linked = previous.as_ref().filter(|record| {
                claude::state(Some(record), &stage.dir) == claude::ClaudeLinkState::Linked
            });
            let upstream = match still_linked {
                Some(record) => record
                    .previous
                    .clone()
                    .unwrap_or_else(|| claude::DEFAULT_UPSTREAM.into()),
                None => claude::current_upstream(&stage.dir).map_err(|reason| {
                    Error::unavailable(format!("{}: Claude Code cannot be linked", reason.code()))
                })?,
            };
            let id = route_id(target, "anthropic");
            config.routes.insert(format!("claude/{id}"), upstream);
            service::write_config(paths, &config)?;
            service::install(paths)?;
            let mut record = claude::link(
                &stage.dir,
                still_linked,
                &service::route_url_at(&config, &host, "claude", &id),
            )?;
            stage.flush(CLAUDE_FILES)?;
            record.claude_dir = real;
            write_record(paths, target, "claude", &record)?;
        }
        ToolKind::Zcode => {
            let real = config_dir(opts, tool)?;
            let stage = StagedConfig::load(target, &real, ZCODE_FILES)?;
            let previous = staged_zcode_record(paths, target, &stage.dir);
            let providers = zcode::providers(&stage.dir).map_err(|reason| {
                Error::unavailable(format!("{}: ZCode cannot be linked", reason.code()))
            })?;
            for provider in &providers {
                let id = route_id(target, &provider.id);
                let still = previous.as_ref().and_then(|record| {
                    record.providers.iter().find(|linked| {
                        linked.provider_id == provider.id
                            && linked.relay_base_url == provider.base_url
                    })
                });
                config.routes.insert(
                    format!("zcode/{id}"),
                    still
                        .map(|linked| linked.previous.clone())
                        .unwrap_or_else(|| provider.base_url.clone()),
                );
            }
            service::write_config(paths, &config)?;
            service::install(paths)?;
            let mut record = zcode::link(&stage.dir, previous.as_ref(), |provider| {
                service::route_url_at(&config, &host, "zcode", &route_id(target, provider))
            })?;
            stage.flush(ZCODE_FILES)?;
            record.zcode_dir = real;
            write_record(paths, target, "zcode", &record)?;
        }
        ToolKind::Grok => {
            let real = config_dir(opts, tool)?;
            let stage = StagedConfig::load(target, &real, GROK_FILES)?;
            let models = grok::routable(&stage.dir).map_err(|reason| {
                Error::unavailable(format!("{}: Grok cannot be linked", reason.code()))
            })?;
            let mut table = std::collections::BTreeMap::new();
            for model in models {
                let digest = hex::encode(Sha256::digest(model.base_url.as_bytes()));
                let upstream = route_id(target, &format!("u{}", &digest[..12]));
                config
                    .routes
                    .insert(format!("grok/{upstream}"), model.base_url);
                table.insert(
                    model.id,
                    service::route_url_at(&config, &host, "grok", &upstream),
                );
            }
            service::write_config(paths, &config)?;
            service::install(paths)?;
            let mut record = grok::link(&stage.dir, table)?;
            stage.flush(GROK_FILES)?;
            record.grok_dir = real;
            write_record(paths, target, "grok", &record)?;
        }
        _ => {
            return Err(Error::unavailable(
                "input rewrite is not supported for this agent",
            ))
        }
    }
    Ok(())
}

pub fn disconnect(paths: &AppPaths, tool: ToolKind, opts: &AdapterOptions) -> Result<bool> {
    let target = &opts.runtime_target;
    let disconnected = match tool {
        ToolKind::Codex => {
            let Some(mut record) = read_record::<link::LinkRecord>(paths, target, "codex") else {
                return Ok(false);
            };
            let real = record.codex_dir.clone();
            let stage = StagedConfig::load(target, &real, CODEX_FILES)?;
            record.codex_dir = stage.dir.clone();
            link::unlink(&record)?;
            stage.flush(CODEX_FILES)?;
            remove_record(paths, target, "codex");
            true
        }
        ToolKind::Claude => {
            let Some(mut record) = read_record::<claude::ClaudeLinkRecord>(paths, target, "claude")
            else {
                return Ok(false);
            };
            let real = record.claude_dir.clone();
            let stage = StagedConfig::load(target, &real, CLAUDE_FILES)?;
            record.claude_dir = stage.dir.clone();
            claude::unlink(&record)?;
            stage.flush(CLAUDE_FILES)?;
            remove_record(paths, target, "claude");
            true
        }
        ToolKind::Zcode => {
            let Some(mut record) = read_record::<zcode::ZcodeLinkRecord>(paths, target, "zcode")
            else {
                return Ok(false);
            };
            let real = record.zcode_dir.clone();
            let stage = StagedConfig::load(target, &real, ZCODE_FILES)?;
            record.zcode_dir = stage.dir.clone();
            zcode::unlink(&record)?;
            stage.flush(ZCODE_FILES)?;
            remove_record(paths, target, "zcode");
            true
        }
        ToolKind::Grok => {
            let Some(mut record) = read_record::<grok::GrokLinkRecord>(paths, target, "grok")
            else {
                return Ok(false);
            };
            let real = record.grok_dir.clone();
            let stage = StagedConfig::load(target, &real, GROK_FILES)?;
            record.grok_dir = stage.dir.clone();
            grok::unlink(&record)?;
            stage.flush(GROK_FILES)?;
            remove_record(paths, target, "grok");
            true
        }
        _ => false,
    };
    super::stop_service_if_unused(paths)?;
    Ok(disconnected)
}

pub fn has_record(paths: &AppPaths, target: &RuntimeTarget, tool: ToolKind) -> bool {
    let name = match tool {
        ToolKind::Codex => "codex",
        ToolKind::Claude => "claude",
        ToolKind::Zcode => "zcode",
        ToolKind::Grok => "grok",
        _ => return false,
    };
    record_path(paths, target, name).is_file()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn target(distribution: &str, user: &str) -> RuntimeTarget {
        RuntimeTarget::Wsl {
            distribution: distribution.into(),
            user: Some(user.into()),
        }
    }

    #[test]
    fn link_records_and_routes_are_isolated_by_distribution_and_user() {
        let root = tempfile::tempdir().unwrap();
        let paths = AppPaths::from_home(root.path().join("app"));
        let alice = target("Ubuntu-24.04", "alice");
        let root_user = target("Ubuntu-24.04", "root");
        let debian = target("Debian", "alice");

        let record_paths = [
            record_path(&paths, &alice, "codex"),
            record_path(&paths, &root_user, "codex"),
            record_path(&paths, &debian, "codex"),
        ];
        assert_ne!(record_paths[0], record_paths[1]);
        assert_ne!(record_paths[0], record_paths[2]);
        assert_ne!(
            route_id(&alice, "provider"),
            route_id(&root_user, "provider")
        );
        assert_ne!(route_id(&alice, "provider"), route_id(&debian, "provider"));
    }

    #[test]
    fn runtime_route_ids_remain_one_safe_path_segment() {
        let id = route_id(&target("Ubuntu Dev", "build-user"), "upstream_1");
        assert!(id.len() < 128);
        assert!(id
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.')));
    }
}
