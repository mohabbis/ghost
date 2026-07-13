//! Probe common localhost inference runtimes — never scans the wider network.

use serde::Serialize;
use std::time::Duration;

const PROBE_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Clone, Debug, Serialize)]
pub struct LocalRuntimeDiscovery {
    pub backend: String,
    pub base_url: String,
    pub models: Vec<String>,
    pub is_localhost: bool,
}

/// Check Ollama and LM Studio on localhost only.
pub fn discover_local_runtimes() -> Vec<LocalRuntimeDiscovery> {
    let client = match reqwest::blocking::Client::builder()
        .timeout(PROBE_TIMEOUT)
        .build()
    {
        Ok(c) => c,
        Err(_) => return vec![],
    };

    let mut found = Vec::new();
    if let Some(d) = probe_ollama(&client) {
        found.push(d);
    }
    if let Some(d) = probe_lm_studio(&client) {
        found.push(d);
    }
    found
}

fn probe_ollama(client: &reqwest::blocking::Client) -> Option<LocalRuntimeDiscovery> {
    let base = "http://127.0.0.1:11434";
    let res = client.get(format!("{base}/api/tags")).send().ok()?;
    if !res.status().is_success() {
        return None;
    }
    #[derive(serde::Deserialize)]
    struct Tags {
        models: Vec<TagModel>,
    }
    #[derive(serde::Deserialize)]
    struct TagModel {
        name: String,
    }
    let tags: Tags = res.json().ok()?;
    let models: Vec<String> = tags.models.into_iter().map(|m| m.name).collect();
    if models.is_empty() {
        return None;
    }
    Some(LocalRuntimeDiscovery {
        backend: "ollama".to_string(),
        base_url: format!("{base}/v1"),
        models,
        is_localhost: true,
    })
}

fn probe_lm_studio(client: &reqwest::blocking::Client) -> Option<LocalRuntimeDiscovery> {
    let base = "http://127.0.0.1:1234/v1";
    let res = client.get(format!("{base}/models")).send().ok()?;
    if !res.status().is_success() {
        return None;
    }
    #[derive(serde::Deserialize)]
    struct Models {
        data: Vec<ModelEntry>,
    }
    #[derive(serde::Deserialize)]
    struct ModelEntry {
        id: String,
    }
    let body: Models = res.json().ok()?;
    let models: Vec<String> = body.data.into_iter().map(|m| m.id).collect();
    if models.is_empty() {
        return None;
    }
    Some(LocalRuntimeDiscovery {
        backend: "lm_studio".to_string(),
        base_url: base.to_string(),
        models,
        is_localhost: true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discover_does_not_panic_without_servers() {
        let _ = discover_local_runtimes();
    }
}
