mod compat;
mod worker;

use crate::{
    cmd::{CmdResult, StringifyErr as _},
    config::{Config, IProfiles, profiles::PROFILE_WRITE_LOCK},
    utils::{dirs, help},
};
use anyhow::{Context as _, Result, bail};
use serde::{Deserialize, Serialize};
use serde_yaml_ng::Mapping;
use std::collections::HashSet;
use tauri_plugin_mihomo::models::Proxies;
pub(crate) use worker::{Request, finish_on_exit, start};

#[derive(Clone, Debug, Deserialize, Serialize, specta::Type)]
#[serde(deny_unknown_fields)]
pub struct Group {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub sources: Vec<String>,
    pub script: String,
}

#[derive(Clone, Default, Deserialize, Serialize, specta::Type)]
#[serde(deny_unknown_fields)]
pub struct Settings {
    pub groups: Vec<Group>,
}

#[derive(Serialize, specta::Type)]
pub struct Overview {
    pub settings: Settings,
    pub reports: Vec<worker::Report>,
    pub config_path: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, specta::Type)]
pub struct NodeLabel {
    pub name: String,
    pub source: String,
}

#[derive(Debug, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct GroupPolicy {
    pub sort: String,
    pub automatic_name: Option<String>,
    pub hide_failed: bool,
}

async fn load() -> Result<Option<Settings>> {
    let path = dirs::app_home_dir()?.join("subscription-groups.yaml");
    match tokio::fs::read_to_string(path).await {
        Ok(data) => Ok(Some(serde_yaml_ng::from_str(&data)?)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
    }
}

pub(crate) async fn apply(mut config: Mapping, profiles: &IProfiles) -> Result<(Mapping, Vec<Request>)> {
    let legacy = compat::legacy(&mut config, profiles)?;
    if legacy.is_some() {
        compat::remove_legacy_groups(&mut config);
    }
    let settings = load().await?.unwrap_or_else(|| Settings {
        groups: legacy.into_iter().collect(),
    });
    let mut requests = Vec::new();
    for group in settings.groups.iter().filter(|g| g.enabled) {
        let request = match worker::prepare(group, profiles, &config).await {
            Ok(request) => request,
            Err(error) => worker::failed_request(group, &config, error.to_string()),
        };
        worker::append(&mut config, &request)?;
        requests.push(request);
    }
    Ok((config, requests))
}

#[tauri::command]
#[specta::specta]
pub async fn get_subscription_groups() -> CmdResult<Overview> {
    let settings = match load().await.stringify_err()? {
        Some(settings) => settings,
        None => Settings {
            groups: Config::runtime()
                .await
                .data_arc()
                .subscription_groups
                .iter()
                .map(|r| r.group.clone())
                .collect(),
        },
    };
    Ok(Overview {
        settings,
        reports: worker::reports(&Config::runtime().await.data_arc().subscription_groups),
        config_path: dirs::app_home_dir()
            .stringify_err()?
            .join("subscription-groups.yaml")
            .display()
            .to_string(),
    })
}

fn validate_settings(settings: &Settings) -> Result<()> {
    let mut ids = HashSet::new();
    let mut names = HashSet::new();
    for group in &settings.groups {
        if group.id.is_empty()
            || !group.id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
            || !ids.insert(&group.id)
        {
            bail!("Group IDs must be unique, nonempty and alphanumeric");
        }
        if group.name.trim().is_empty() || !names.insert(&group.name) {
            bail!("Group names must be unique and nonempty");
        }
        if group.enabled && group.sources.is_empty() {
            bail!("Select at least one source subscription for {}", group.name);
        }
    }
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn save_subscription_groups(settings: Settings) -> CmdResult<()> {
    async {
        validate_settings(&settings)?;
        let _write = PROFILE_WRITE_LOCK.lock().await;
        let profiles = Config::profiles().await.data_arc();
        let config = Config::runtime().await.data_arc().config.clone().unwrap_or_default();
        // Validate every script before persisting, without network work or runtime activation.
        for group in settings.groups.iter().filter(|g| g.enabled) {
            worker::prepare(group, &profiles, &config)
                .await
                .with_context(|| format!("Invalid group {}", group.name))?;
        }
        let path = dirs::app_home_dir()?.join("subscription-groups.yaml");
        help::save_yaml(&path, &settings, Some("# Independent subscription groups")).await?;
        Ok::<_, anyhow::Error>(())
    }
    .await
    .stringify_err()
}

#[tauri::command]
#[specta::specta]
pub async fn rescan_subscription_group(id: String) -> CmdResult<()> {
    if !Config::runtime()
        .await
        .data_arc()
        .subscription_groups
        .iter()
        .any(|r| r.group.id == id)
    {
        return Err("Apply this group before scanning".into());
    }
    worker::rescan(id);
    Ok(())
}

pub(crate) fn prepare_proxy_view(requests: &[Request], proxies: &mut Proxies) {
    for request in requests {
        let Some(check) = request.options.checks.first() else {
            continue;
        };
        let prefix = format!("SG/{}/", request.group.id);
        for (name, proxy) in &mut proxies.proxies {
            if name.starts_with(&prefix)
                && let Some(health) = proxy.extra.remove(&check.url)
            {
                proxy.history = health.history;
            }
        }
    }
}

pub(crate) fn decorate_proxy_view(requests: &[Request], view: &mut crate::core::proxy_view::ProxyViewV1) {
    for request in requests {
        view.subscription_labels.extend(request.node_labels());
        let automatic = request.automatic_name();
        for group in &mut view.groups {
            if group.name == request.group.name || (request.options.automatic && group.name == automatic) {
                group.subscription_group = Some(GroupPolicy {
                    sort: match request.options.sort {
                        worker::Sort::Delay => "delay",
                        worker::Sort::Name => "name",
                        worker::Sort::Original => "original",
                    }
                    .into(),
                    automatic_name: request.options.automatic.then(|| automatic.clone()),
                    hide_failed: !request.options.checks.is_empty(),
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn accepts_many_sources_and_rejects_ambiguous_group_identity() {
        let group = Group {
            id: "one".into(),
            name: "Any service".into(),
            enabled: true,
            sources: (0..100).map(|i| format!("subscription-{i}")).collect(),
            script: "function main(c) {return c}".into(),
        };
        assert!(
            validate_settings(&Settings {
                groups: vec![group.clone()]
            })
            .is_ok()
        );
        assert!(
            validate_settings(&Settings {
                groups: vec![group.clone(), group]
            })
            .is_err()
        );
    }
}
