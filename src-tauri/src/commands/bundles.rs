use serde_json::Value;

use crate::core::{
    bundle::{self, BundleManifest},
    cli_bridge,
};

fn run(parts: &[&str]) -> Result<Value, String> {
    cli_bridge::run_bundled_json(
        &parts
            .iter()
            .map(|part| part.to_string())
            .collect::<Vec<_>>(),
    )
    .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn list_bundles() -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(|| run(&["bundles", "list"]))
        .await
        .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn save_bundle(
    manifest: BundleManifest,
    original_slug: Option<String>,
) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || {
        bundle::validate_structure(&manifest).map_err(|error| error.to_string())?;
        let mut temp = tempfile::NamedTempFile::new().map_err(|error| error.to_string())?;
        serde_yaml::to_writer(&mut temp, &manifest).map_err(|error| error.to_string())?;
        let path = temp.path().to_string_lossy().to_string();
        match original_slug {
            Some(slug) => run(&["bundles", "update", &slug, &path]),
            None => run(&["bundles", "import", &path]),
        }
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn delete_bundle(slug: String) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || run(&["bundles", "delete", &slug]))
        .await
        .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn deploy_bundle(
    slug: String,
    agent: String,
    dry_run: bool,
    undeploy: bool,
) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || {
        if agent != "codex" && agent != "hermes" {
            return Err("unsupported bundle agent".to_string());
        }
        let action = if undeploy { "undeploy" } else { "deploy" };
        let mut args = vec!["bundles", action, &slug, "--agent", &agent];
        if dry_run {
            args.push("--dry-run");
        }
        run(&args)
    })
    .await
    .map_err(|error| error.to_string())?
}
