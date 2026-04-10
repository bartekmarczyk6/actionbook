use serde_json::{Map, Value, json};

use crate::api::{ApiClient, SearchActionsParams};
use crate::cli::Cli;
use crate::config;
use crate::error::CliError;
use crate::output::{AxiRenderOptions, render_axi_success, resolve_stdout_mode};

pub async fn run(
    cli: &Cli,
    query: &str,
    domain: Option<&str>,
    url: Option<&str>,
    page: u32,
    page_size: u32,
    fields: Option<&str>,
) -> Result<(), CliError> {
    let mut config = config::load_config()?;
    if let Some(ref key) = cli.api_key {
        config.api.api_key = Some(key.clone());
    }
    let client = ApiClient::from_config(&config)?;

    let params = SearchActionsParams {
        query: query.to_string(),
        domain: domain.map(|s| s.to_string()),
        url: url.map(|s| s.to_string()),
        page: Some(page),
        page_size: Some(page_size),
        background: None,
    };

    let raw = client.search_actions_json(params).await?;

    let selected_fields = parse_fields(fields);
    let items = extract_items(&raw);
    let total = extract_total(&raw).unwrap_or(items.len() as u64);

    let mut out_items = Vec::new();
    for item in &items {
        let mut row = Map::new();
        let id = pick_str(item, &["area_id", "id", "action_id"]).unwrap_or("(no id)");
        let title = pick_str(item, &["title", "name", "summary"]).unwrap_or("(untitled)");
        let status = pick_str(item, &["status", "state"]).unwrap_or("unknown");
        row.insert("id".to_string(), Value::String(id.to_string()));
        row.insert("title".to_string(), Value::String(title.to_string()));
        row.insert("status".to_string(), Value::String(status.to_string()));

        for field in &selected_fields {
            if row.contains_key(field) {
                continue;
            }
            if let Some(v) = item.get(field) {
                row.insert(field.clone(), v.clone());
            }
        }

        out_items.push(Value::Object(row));
    }

    let empty = out_items.is_empty();
    let data = if empty {
        json!({
            "query": query,
            "count": { "shown": 0, "total": total },
            "message": format!("0 actions found for query \"{query}\""),
            "help": [
                "Run actionbook search \"<query>\" --domain \"<domain>\"",
                "Run actionbook search \"<query>\" --url \"<url>\""
            ]
        })
    } else {
        json!({
            "query": query,
            "count": { "shown": out_items.len(), "total": total },
            "items": out_items,
            "help": [
                "Run actionbook get \"<id>\"",
                "Run actionbook search \"<query>\" --fields \"id,title,status,url,score\""
            ]
        })
    };

    let legacy_text = raw
        .get("raw_text")
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .unwrap_or_else(|| serde_json::to_string_pretty(&raw).unwrap_or_else(|_| raw.to_string()));

    let mode = resolve_stdout_mode(cli.json, cli.legacy_output);
    println!(
        "{}",
        render_axi_success(
            "search",
            data,
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

fn extract_items(raw: &Value) -> Vec<Value> {
    if let Some(items) = raw.get("items").and_then(|v| v.as_array()) {
        return items.clone();
    }
    if let Some(items) = raw.get("results").and_then(|v| v.as_array()) {
        return items.clone();
    }
    if let Some(items) = raw
        .get("data")
        .and_then(|v| v.get("items"))
        .and_then(|v| v.as_array())
    {
        return items.clone();
    }
    Vec::new()
}

fn extract_total(raw: &Value) -> Option<u64> {
    raw.get("total")
        .and_then(|v| v.as_u64())
        .or_else(|| raw.get("count").and_then(|v| v.as_u64()))
        .or_else(|| {
            raw.get("pagination")
                .and_then(|v| v.get("total"))
                .and_then(|v| v.as_u64())
        })
}

fn pick_str<'a>(item: &'a Value, keys: &[&str]) -> Option<&'a str> {
    keys.iter()
        .find_map(|k| item.get(k).and_then(|v| v.as_str()))
}
