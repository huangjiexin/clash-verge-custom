use super::Group;
use crate::config::IProfiles;
use anyhow::Result;
use serde_yaml_ng::{Mapping, Value};

pub(super) fn legacy(config: &mut Mapping, profiles: &IProfiles) -> Result<Option<Group>> {
    let Some(value) = config.remove("codex-screening") else {
        return Ok(None);
    };
    if value.get("enabled").and_then(Value::as_bool) != Some(true) {
        return Ok(None);
    }
    let mut sources: Vec<String> = value
        .get("profiles")
        .and_then(Value::as_sequence)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect();
    if sources.is_empty() {
        sources.extend(profiles.current.iter().map(ToString::to_string));
    }
    let options = serde_json::json!({
        "interval": value.get("interval").and_then(Value::as_u64).unwrap_or(300),
        "interface-name": value.get("interface-name").and_then(Value::as_str).unwrap_or(""),
        "automatic": true, "sort": "delay",
        "checks": [
            {"url":"https://api.openai.com/v1/models","expected-status":401,"validation":"auth-json"},
            {"url":"https://chatgpt.com/backend-api/codex/models?client_version=0.167.2","expected-status":401,"validation":"auth-json"},
            {"url":"https://chatgpt.com/","expected-status":200,"validation":"html"}
        ]
    });
    Ok(Some(Group {
        id: "legacy-codex".into(),
        name: "Codex可用".into(),
        enabled: true,
        sources,
        script: format!(
            "function main(config) {{\n  config.screening = {};\n  return config;\n}}",
            serde_json::to_string_pretty(&options)?
        ),
    }))
}

pub(super) fn automatic_name(group: &Group) -> String {
    if group.id == "legacy-codex" {
        "Codex自动选择".into()
    } else {
        format!("{} · 自动选择", group.name)
    }
}

pub(super) fn remove_legacy_groups(config: &mut Mapping) {
    if let Some(groups) = config.get_mut("proxy-groups").and_then(Value::as_sequence_mut) {
        groups.retain(|g| ![Some("Codex可用"), Some("Codex自动选择")].contains(&g.get("name").and_then(Value::as_str)));
    }
}
