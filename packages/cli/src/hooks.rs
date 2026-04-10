use serde_json::{Value, json};

use crate::error::CliError;

pub fn install_or_repair_hooks() -> Result<(), CliError> {
    let exe = std::env::current_exe()
        .map_err(|e| CliError::Internal(format!("failed to resolve executable path: {e}")))?;
    let exe = exe.to_string_lossy().to_string();
    install_claude_hooks(&exe)?;
    install_codex_hooks(&exe)?;
    ensure_codex_feature_enabled()?;
    Ok(())
}

pub fn capture_session_end() -> Result<(), CliError> {
    let cwd = std::env::current_dir()
        .map_err(|e| CliError::Internal(format!("failed to read current directory: {e}")))?;
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let payload = json!({
        "cwd": cwd.to_string_lossy(),
        "ended_at_unix": stamp
    });
    let state_dir = dirs::home_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join(".actionbook");
    std::fs::create_dir_all(&state_dir).ok();
    std::fs::write(
        state_dir.join("last-session.json"),
        serde_json::to_string_pretty(&payload).unwrap_or_else(|_| payload.to_string()),
    )
    .map_err(|e| CliError::Internal(format!("failed to write session state: {e}")))?;
    Ok(())
}

fn install_claude_hooks(exe: &str) -> Result<(), CliError> {
    let settings_path = dirs::home_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join(".claude")
        .join("settings.json");
    let mut root = load_or_empty_object(&settings_path);
    ensure_hook_command(
        &mut root,
        &["hooks", "SessionStart"],
        &format!("{exe} --hook-session-start"),
    );
    ensure_hook_command(
        &mut root,
        &["hooks", "SessionEnd"],
        &format!("{exe} --hook-session-end"),
    );
    write_json(&settings_path, &root)
}

fn install_codex_hooks(exe: &str) -> Result<(), CliError> {
    let hooks_path = dirs::home_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join(".codex")
        .join("hooks.json");
    let mut root = load_or_empty_object(&hooks_path);
    ensure_hook_command(
        &mut root,
        &["SessionStart"],
        &format!("{exe} --hook-session-start"),
    );
    ensure_hook_command(
        &mut root,
        &["SessionEnd"],
        &format!("{exe} --hook-session-end"),
    );
    write_json(&hooks_path, &root)
}

fn ensure_codex_feature_enabled() -> Result<(), CliError> {
    let path = dirs::home_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join(".codex")
        .join("config.toml");
    if !path.exists() {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        std::fs::write(&path, "[features]\ncodex_hooks = true\n")
            .map_err(|e| CliError::Internal(format!("failed to write codex config: {e}")))?;
        return Ok(());
    }
    let content = std::fs::read_to_string(&path).unwrap_or_default();
    if content.contains("codex_hooks = true") {
        return Ok(());
    }
    let updated = upsert_codex_feature(&content);
    std::fs::write(&path, updated)
        .map_err(|e| CliError::Internal(format!("failed to update codex config: {e}")))?;
    Ok(())
}

fn ensure_hook_command(root: &mut Value, path: &[&str], command: &str) {
    let mut cursor = root;
    for key in &path[..path.len() - 1] {
        if !cursor.get(*key).map(|v| v.is_object()).unwrap_or(false) {
            cursor[*key] = json!({});
        }
        cursor = &mut cursor[*key];
    }

    let leaf = path[path.len() - 1];
    if !cursor.get(leaf).map(|v| v.is_array()).unwrap_or(false) {
        cursor[leaf] = json!([]);
    }

    let arr = cursor[leaf]
        .as_array_mut()
        .expect("array ensured by previous block");
    let mut replaced = false;
    for item in arr.iter_mut() {
        if let Some(cmd) = item.get("command").and_then(|v| v.as_str())
            && is_managed_hook_command(cmd)
        {
            *item = json!({ "command": command });
            replaced = true;
            break;
        }
    }
    if !replaced {
        arr.push(json!({ "command": command }));
    }
}

fn is_managed_hook_command(command: &str) -> bool {
    let normalized = command.trim();
    normalized.ends_with("--hook-session-start") || normalized.ends_with("--hook-session-end")
}

fn upsert_codex_feature(content: &str) -> String {
    let mut lines: Vec<String> = content.lines().map(str::to_string).collect();
    let mut features_start = None;
    let mut features_end = lines.len();

    for (idx, line) in lines.iter().enumerate() {
        if line.trim() == "[features]" {
            features_start = Some(idx);
            continue;
        }
        if features_start.is_some() && line.trim().starts_with('[') {
            features_end = idx;
            break;
        }
    }

    match features_start {
        Some(start) => {
            let has_key = lines[start + 1..features_end]
                .iter()
                .any(|l| l.trim_start().starts_with("codex_hooks"));
            if !has_key {
                let mut insert_at = features_end;
                while insert_at > start + 1 && lines[insert_at - 1].trim().is_empty() {
                    insert_at -= 1;
                }
                lines.insert(insert_at, "codex_hooks = true".to_string());
            }
        }
        None => {
            if !lines.is_empty() && !lines.last().is_some_and(|l| l.is_empty()) {
                lines.push(String::new());
            }
            lines.push("[features]".to_string());
            lines.push("codex_hooks = true".to_string());
        }
    }

    let mut rendered = lines.join("\n");
    if !rendered.ends_with('\n') {
        rendered.push('\n');
    }
    rendered
}

fn load_or_empty_object(path: &std::path::Path) -> Value {
    if let Ok(content) = std::fs::read_to_string(path)
        && let Ok(value) = serde_json::from_str::<Value>(&content)
        && value.is_object()
    {
        return value;
    }
    json!({})
}

fn write_json(path: &std::path::Path, value: &Value) -> Result<(), CliError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    std::fs::write(
        path,
        serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string()),
    )
    .map_err(|e| CliError::Internal(format!("failed to write {}: {e}", path.display())))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_hook_command_is_idempotent() {
        let mut v = json!({});
        ensure_hook_command(
            &mut v,
            &["hooks", "SessionStart"],
            "/bin/actionbook --hook-session-start",
        );
        ensure_hook_command(
            &mut v,
            &["hooks", "SessionStart"],
            "/bin/actionbook --hook-session-start",
        );
        let arr = v["hooks"]["SessionStart"].as_array().expect("array");
        assert_eq!(arr.len(), 1);
    }

    #[test]
    fn upsert_codex_feature_inserts_inside_features_section() {
        let source = "[features]\na = true\n\n[other]\nx = 1\n";
        let out = upsert_codex_feature(source);
        assert!(out.contains("[features]\na = true\ncodex_hooks = true\n\n[other]"));
    }
}
