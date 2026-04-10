use serde_json::{Map, Value, json};

use crate::api::ApiClient;
use crate::cli::Cli;
use crate::config;
use crate::error::CliError;
use crate::output::{
    AxiRenderOptions, render_axi_success, resolve_stdout_mode, truncate_text_preview,
};

pub async fn run(cli: &Cli, area_id: &str, fields: Option<&str>, full: bool) -> Result<(), CliError> {
    let mut config = config::load_config()?;
    if let Some(ref key) = cli.api_key {
        config.api.api_key = Some(key.clone());
    }
    let client = ApiClient::from_config(&config)?;

    let raw = client.get_action_by_area_id_json(area_id).await?;

    let selected_fields = parse_fields(fields);
    let mut data = Map::new();
    let detail = if raw.get("raw_text").is_none() {
        raw.as_object()
    } else {
        None
    };

    data.insert("id".to_string(), Value::String(area_id.to_string()));
    data.insert(
        "title".to_string(),
        Value::String(
            detail
                .and_then(|d| pick_str_obj(d, &["title", "name"]))
                .unwrap_or("-")
                .to_string(),
        ),
    );
    data.insert(
        "status".to_string(),
        Value::String(
            detail
                .and_then(|d| pick_str_obj(d, &["status", "state"]))
                .unwrap_or("unknown")
                .to_string(),
        ),
    );

    if let Some(d) = detail {
        if let Some(url) = pick_str_obj(d, &["url"]) {
            data.insert("url".to_string(), Value::String(url.to_string()));
        }

        let body_candidate = pick_str_obj(d, &["description", "body", "content"]).unwrap_or("");
        if !body_candidate.is_empty() {
            if full {
                data.insert("body".to_string(), Value::String(body_candidate.to_string()));
            } else {
                let (preview, truncated, total) = truncate_text_preview(body_candidate, 900);
                data.insert("body".to_string(), Value::String(preview));
                if truncated {
                    data.insert("body_truncated".to_string(), Value::Bool(true));
                    data.insert("body_total_chars".to_string(), Value::Number(total.into()));
                    data.insert(
                        "help".to_string(),
                        Value::Array(vec![Value::String(format!(
                            "Run actionbook get \"{area_id}\" --full"
                        ))]),
                    );
                }
            }
        }

        for field in selected_fields {
            if data.contains_key(&field) {
                continue;
            }
            if let Some(v) = d.get(&field) {
                data.insert(field, v.clone());
            }
        }
    } else if let Some(text) = raw.get("raw_text").and_then(|v| v.as_str()) {
        if full {
            data.insert("body".to_string(), Value::String(text.to_string()));
        } else {
            let (preview, truncated, total) = truncate_text_preview(text, 900);
            data.insert("body".to_string(), Value::String(preview));
            if truncated {
                data.insert("body_truncated".to_string(), Value::Bool(true));
                data.insert("body_total_chars".to_string(), Value::Number(total.into()));
                data.insert(
                    "help".to_string(),
                    Value::Array(vec![Value::String(format!(
                        "Run actionbook get \"{area_id}\" --full"
                    ))]),
                );
            }
        }
    }

    let legacy_text = raw
        .get("raw_text")
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .unwrap_or_else(|| serde_json::to_string_pretty(&raw).unwrap_or_else(|_| raw.to_string()));

    let mode = resolve_stdout_mode(cli.json, cli.legacy_output);
    println!(
        "{}",
        render_axi_success(
            "get",
            Value::Object(data),
            &legacy_text,
            AxiRenderOptions {
                mode,
                duration: std::time::Duration::ZERO,
                include_meta: false,
            }
        )
    );

    Ok(())
}

fn parse_fields(fields: Option<&str>) -> Vec<String> {
    fields
        .unwrap_or("")
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect()
}

fn pick_str_obj<'a>(obj: &'a serde_json::Map<String, Value>, keys: &[&str]) -> Option<&'a str> {
    keys.iter().find_map(|k| obj.get(*k).and_then(|v| v.as_str()))
}
