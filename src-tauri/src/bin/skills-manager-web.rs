//! Headless HTTP adapter for the same React UI and Rust core used by Tauri.
//! Bind to loopback by default. A non-loopback listener requires a bearer token.

use std::{net::IpAddr, path::PathBuf, process::Command, sync::Arc};

use anyhow::{anyhow, bail, Context, Result};
use app_lib::{
    commands::{agent_workspace, git_backup as git_commands, presets, projects, skills, tools},
    core::{app_state, bundle, central_repo, git_backup, skill_store::SkillStore, tool_service},
};
use axum::{
    extract::{DefaultBodyLimit, State},
    http::{header, HeaderMap, StatusCode},
    response::IntoResponse,
    routing::post,
    Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};
use tower_http::services::{ServeDir, ServeFile};

#[derive(Clone)]
struct WebState {
    store: Arc<SkillStore>,
    host: IpAddr,
    port: u16,
    token: Option<String>,
    cli: PathBuf,
}

#[derive(Deserialize)]
struct Request {
    name: String,
    #[serde(default)]
    args: Value,
}

fn field<'a>(args: &'a Value, key: &str) -> Result<&'a str> {
    args.get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty() && value.len() <= 4096)
        .ok_or_else(|| anyhow!("missing or invalid {key}"))
}

fn string_field<'a>(args: &'a Value, key: &str) -> Result<&'a str> {
    args.get(key)
        .and_then(Value::as_str)
        .filter(|value| value.len() <= 4096)
        .ok_or_else(|| anyhow!("missing or invalid {key}"))
}

fn optional<'a>(args: &'a Value, key: &str) -> Option<&'a str> {
    args.get(key).and_then(Value::as_str)
}

fn bool_field(args: &Value, key: &str) -> bool {
    args.get(key).and_then(Value::as_bool).unwrap_or(false)
}

fn cli_call(state: &WebState, args: &[String]) -> Result<Value> {
    let mut cmd = Command::new(&state.cli);
    cmd.arg("--json");
    if let Some(root) = std::env::var_os("SM_SKILLS_ROOT") {
        cmd.arg("--skills-root").arg(root);
    }
    cmd.args(args);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000);
    }
    let output = cmd.output().context("could not run skills-manager-cli")?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let message = serde_json::from_str::<Value>(&stderr)
            .ok()
            .and_then(|value| {
                value
                    .get("message")
                    .and_then(Value::as_str)
                    .map(str::to_string)
            })
            .unwrap_or_else(|| stderr.trim().to_string());
        bail!("{message}");
    }
    serde_json::from_str(stdout.trim()).context("CLI returned invalid JSON")
}

fn cli(state: &WebState, args: &[&str]) -> Result<Value> {
    cli_call(
        state,
        &args
            .iter()
            .map(|value| value.to_string())
            .collect::<Vec<_>>(),
    )
}

fn dispatch(state: &WebState, request: Request) -> Result<Value> {
    let args = &request.args;
    let store = &state.store;
    match request.name.as_str() {
        "get_presets" => Ok(serde_json::to_value(presets::list_presets_internal(
            store,
        )?)?),
        "get_active_preset" => Ok(serde_json::to_value(presets::active_preset_internal(
            store,
        )?)?),
        "get_tool_status" => Ok(serde_json::to_value(tools::list_tool_status_internal(
            store,
        )?)?),
        "get_managed_skills" => Ok(serde_json::to_value(skills::list_managed_skills_internal(
            store,
        )?)?),
        "get_skills_for_preset" => Ok(serde_json::to_value(
            skills::list_skills_for_preset_internal(store, field(args, "presetId")?)?,
        )?),
        "get_skill_document" => Ok(serde_json::to_value(skills::skill_document_internal(
            store,
            field(args, "skillId")?,
        )?)?),
        "get_projects" => Ok(serde_json::to_value(projects::list_projects_internal(
            store,
        )?)?),
        "get_global_local_skills" => Ok(serde_json::to_value(
            agent_workspace::list_global_local_skills_internal(store, field(args, "agent")?)?,
        )?),
        "get_global_local_skill_document" => Ok(serde_json::to_value(
            agent_workspace::global_local_skill_document_internal(
                store,
                field(args, "agent")?,
                field(args, "skillRelativePath")?,
            )?,
        )?),
        "get_settings" => Ok(json!(store.get_setting(field(args, "key")?)?)),
        "set_settings" => {
            store.set_setting(field(args, "key")?, string_field(args, "value")?)?;
            Ok(Value::Null)
        }
        "get_central_repo_path" => Ok(json!(central_repo::base_dir().to_string_lossy())),
        "get_central_repo_path_override" => Ok(json!(
            central_repo::configured_base_dir().map(|path| path.to_string_lossy().to_string())
        )),
        "get_central_repo_warnings" => Ok(json!(central_repo::startup_warnings())),
        "set_central_repo_path" => {
            if std::env::var_os("SM_SKILLS_ROOT").is_some() {
                bail!("SM_SKILLS_ROOT controls this server's library path; change it and restart the server");
            }
            let path = optional(args, "path").map(str::to_string);
            Ok(json!(
                central_repo::set_base_dir_override(path)?.to_string_lossy()
            ))
        }
        "get_tool_order_cmd" => Ok(json!(tool_service::get_tool_order(store))),
        "set_tool_order_cmd" => {
            let order: Vec<String> =
                serde_json::from_value(args.get("order").cloned().context("missing order")?)?;
            tool_service::set_tool_order(store, &order)?;
            Ok(Value::Null)
        }
        "get_all_tags" => Ok(json!(store.get_all_tags()?)),
        "set_skill_tags" => {
            let tags: Vec<String> =
                serde_json::from_value(args.get("tags").cloned().context("missing tags")?)?;
            skills::set_skill_tags_internal(store, field(args, "skillId")?, &tags)?;
            Ok(Value::Null)
        }
        "rename_tag" => {
            skills::rename_tag_internal(store, field(args, "oldName")?, field(args, "newName")?)?;
            Ok(Value::Null)
        }
        "delete_tag" => {
            skills::delete_tag_internal(store, field(args, "name")?)?;
            Ok(Value::Null)
        }
        "delete_managed_skill" => {
            let deleted = skills::delete_managed_skills_by_ids(
                store,
                &[field(args, "skillId")?.to_string()],
            )?;
            if deleted.deleted == 0 {
                bail!("skill not found");
            }
            Ok(Value::Null)
        }
        "delete_managed_skills" => {
            let ids: Vec<String> =
                serde_json::from_value(args.get("skillIds").cloned().context("missing skillIds")?)?;
            Ok(serde_json::to_value(skills::delete_managed_skills_by_ids(
                store, &ids,
            )?)?)
        }
        "preview_git_install" => Ok(serde_json::to_value(skills::preview_git_install_internal(
            store,
            field(args, "repoUrl")?,
            None,
            None,
        )?)?),
        "confirm_git_install" => {
            let items: Vec<skills::SkillInstallItem> =
                serde_json::from_value(args.get("items").cloned().context("missing items")?)?;
            skills::confirm_git_install_internal(
                store,
                field(args, "repoUrl")?,
                field(args, "tempDir")?,
                &items,
            )?;
            Ok(Value::Null)
        }
        "cancel_git_preview" => {
            skills::cancel_git_preview_internal(field(args, "tempDir")?)?;
            Ok(Value::Null)
        }
        "get_preset_skill_order" => Ok(json!(
            store.get_skill_ids_for_scenario(field(args, "presetId")?)?
        )),
        "reorder_preset_skills" => {
            let ids: Vec<String> =
                serde_json::from_value(args.get("skillIds").cloned().context("missing skillIds")?)?;
            presets::reorder_preset_skills_internal(store, field(args, "presetId")?, &ids)?;
            Ok(Value::Null)
        }
        "reorder_presets" => {
            let ids: Vec<String> =
                serde_json::from_value(args.get("ids").cloned().context("missing ids")?)?;
            presets::reorder_presets_internal(store, &ids)?;
            Ok(Value::Null)
        }
        "git_backup_status" => Ok(serde_json::to_value(git_backup::get_status(
            &central_repo::skills_dir(),
        )?)?),
        "set_tool_enabled" => {
            tools::set_tool_enabled_internal(
                store,
                field(args, "key")?,
                bool_field(args, "enabled"),
            )?;
            Ok(Value::Null)
        }
        "set_all_tools_enabled" => {
            tools::set_all_tools_enabled_internal(store, bool_field(args, "enabled"))?;
            Ok(Value::Null)
        }
        "set_custom_tool_path" => {
            tools::apply_tool_skills_dir(store, field(args, "key")?, field(args, "path")?)?;
            Ok(Value::Null)
        }
        "reset_custom_tool_path" => {
            tools::reset_custom_tool_path_internal(store, field(args, "key")?)?;
            Ok(Value::Null)
        }
        "set_custom_tool_project_path" => {
            tools::apply_tool_project_skills_dir(
                store,
                field(args, "key")?,
                optional(args, "projectRelativeSkillsDir"),
            )?;
            Ok(Value::Null)
        }
        "reset_custom_tool_project_path" => {
            tools::reset_custom_tool_project_path_internal(store, field(args, "key")?)?;
            Ok(Value::Null)
        }
        "add_custom_tool" => {
            tools::add_custom_tool_internal(
                store,
                field(args, "key")?,
                field(args, "displayName")?,
                field(args, "skillsDir")?,
                optional(args, "projectRelativeSkillsDir"),
            )?;
            Ok(Value::Null)
        }
        "remove_custom_tool" => {
            tools::remove_custom_tool_internal(store, field(args, "key")?)?;
            Ok(Value::Null)
        }
        "git_backup_sanitize_remote_url" => Ok(json!(git_commands::sanitize_remote_url_internal(
            field(args, "url")?
        )?)),
        "git_backup_remove_remote" => {
            git_commands::disconnect_local(store, &central_repo::skills_dir())?;
            Ok(Value::Null)
        }
        "git_backup_fetch" => {
            git_backup::fetch_remote(&central_repo::skills_dir())?;
            Ok(Value::Null)
        }
        "git_backup_list_versions" => {
            let limit = args
                .get("limit")
                .and_then(Value::as_u64)
                .map(|n| n.min(500) as usize);
            Ok(serde_json::to_value(git_backup::list_snapshot_versions(
                &central_repo::skills_dir(),
                limit,
            )?)?)
        }
        "git_backup_size_report" => Ok(serde_json::to_value(git_backup::size_report(
            &central_repo::skills_dir(),
        )?)?),
        "git_backup_pending_conflicts" => {
            Ok(serde_json::to_value(store.list_pending_conflicts()?)?)
        }
        "git_backup_migrate_credentials" => {
            Ok(json!(git_commands::migrate_embedded_credentials(store)?))
        }
        "backup_device_name" => Ok(json!(store
            .get_setting("backup_device_name")?
            .filter(|name| !name.trim().is_empty())
            .unwrap_or_else(git_backup::default_device_name))),
        "backup_set_device_name" => {
            let name = git_backup::sanitize_device_name(field(args, "name")?);
            if name.is_empty() {
                bail!("device name is empty");
            }
            store.set_setting("backup_device_name", &name)?;
            Ok(json!(name))
        }
        "git_backup_sync" => Ok(serde_json::to_value(
            git_commands::git_backup_sync_internal(store, field(args, "message")?)?,
        )?),
        "log_startup_event" | "check_last_panic" => Ok(Value::Null),
        "list_bundles" => cli(state, &["bundles", "list"]),
        "save_bundle" => {
            let manifest: bundle::BundleManifest =
                serde_json::from_value(args.get("manifest").cloned().context("missing manifest")?)?;
            bundle::validate_structure(&manifest)?;
            let file = tempfile::NamedTempFile::new()?;
            std::fs::write(file.path(), serde_json::to_vec(&manifest)?)?;
            let path = file.path().to_string_lossy();
            match optional(args, "originalSlug") {
                Some(slug) => cli(state, &["bundles", "update", slug, &path]),
                None => cli(state, &["bundles", "import", &path]),
            }
        }
        "delete_bundle" => cli(state, &["bundles", "delete", field(args, "slug")?]),
        "deploy_bundle" => {
            let agent = field(args, "agent")?;
            if agent != "codex" && agent != "hermes" {
                bail!("bundle deployment supports codex and hermes");
            }
            let mut command = vec![
                "bundles",
                if bool_field(args, "undeploy") {
                    "undeploy"
                } else {
                    "deploy"
                },
                field(args, "slug")?,
                "--agent",
                agent,
            ];
            if bool_field(args, "dryRun") {
                command.push("--dry-run");
            }
            cli(state, &command)
        }
        "create_preset" => {
            let mut command = vec!["presets", "create", field(args, "name")?];
            if let Some(description) = optional(args, "description") {
                command.extend(["--description", description]);
            }
            if let Some(icon) = optional(args, "icon") {
                command.extend(["--icon", icon]);
            }
            cli(state, &command)
        }
        "update_preset" => {
            presets::update_preset_internal(
                store,
                field(args, "id")?,
                field(args, "name")?,
                optional(args, "description"),
                optional(args, "icon"),
            )?;
            Ok(Value::Null)
        }
        "delete_preset" => cli(state, &["presets", "delete", field(args, "id")?, "--yes"]),
        "switch_preset" | "apply_preset_to_default" => {
            cli(state, &["presets", "apply", field(args, "id")?])
        }
        "add_skill_to_preset" => cli(
            state,
            &[
                "presets",
                "add-skill",
                field(args, "presetId")?,
                field(args, "skillId")?,
            ],
        ),
        "remove_skill_from_preset" => cli(
            state,
            &[
                "presets",
                "remove-skill",
                field(args, "presetId")?,
                field(args, "skillId")?,
            ],
        ),
        "sync_skill_to_tool" => cli(
            state,
            &[
                "skills",
                "deploy",
                field(args, "skillId")?,
                "--agent",
                field(args, "tool")?,
            ],
        ),
        "unsync_skill_from_tool" => cli(
            state,
            &[
                "skills",
                "undeploy",
                field(args, "skillId")?,
                "--agent",
                field(args, "tool")?,
            ],
        ),
        "git_backup_init" => cli(state, &["git", "init"]),
        "git_backup_push" => cli(state, &["git", "push"]),
        "git_backup_pull" => cli(state, &["git", "pull"]),
        "git_backup_commit" => cli(
            state,
            &["git", "commit", "--message", field(args, "message")?],
        ),
        "git_backup_set_remote" => {
            let url = git_commands::sanitize_remote_url_internal(field(args, "url")?)?;
            git_backup::set_remote(&central_repo::skills_dir(), &url)?;
            Ok(json!(url))
        }
        "git_backup_clone" => {
            git_commands::git_backup_clone_internal(store, field(args, "url")?)?;
            Ok(Value::Null)
        }
        "git_backup_reclone" => {
            git_commands::git_backup_reclone_internal(store, field(args, "url")?)?;
            Ok(Value::Null)
        }
        _ => bail!("Web mode does not support {} yet", request.name),
    }
}

async fn invoke(
    State(state): State<Arc<WebState>>,
    headers: HeaderMap,
    Json(request): Json<Request>,
) -> impl IntoResponse {
    let expected_host = format!("{}:{}", state.host, state.port);
    let host = headers
        .get(header::HOST)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    let loopback_host = format!("localhost:{}", state.port);
    if host != expected_host && !(state.host.is_loopback() && host == loopback_host) {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"error": "invalid host"})),
        )
            .into_response();
    }
    if let Some(origin) = headers
        .get(header::ORIGIN)
        .and_then(|value| value.to_str().ok())
    {
        if origin != format!("http://{host}") {
            return (
                StatusCode::FORBIDDEN,
                Json(json!({"error": "invalid origin"})),
            )
                .into_response();
        }
    }
    if let Some(token) = &state.token {
        let authorization = headers
            .get(header::AUTHORIZATION)
            .and_then(|value| value.to_str().ok());
        if authorization != Some(format!("Bearer {token}").as_str()) {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "Authentication required"})),
            )
                .into_response();
        }
    }
    match tokio::task::spawn_blocking(move || dispatch(&state, request)).await {
        Ok(Ok(value)) => (StatusCode::OK, Json(value)).into_response(),
        Ok(Err(error)) => (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": error.to_string()})),
        )
            .into_response(),
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": error.to_string()})),
        )
            .into_response(),
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let host: IpAddr = std::env::var("SM_WEB_HOST")
        .unwrap_or_else(|_| "127.0.0.1".into())
        .parse()?;
    let port: u16 = std::env::var("SM_WEB_PORT")
        .unwrap_or_else(|_| "1420".into())
        .parse()?;
    let token = std::env::var("SM_WEB_TOKEN")
        .ok()
        .filter(|value| !value.is_empty());
    if !host.is_loopback() && token.as_ref().is_none_or(|value| value.len() < 24) {
        bail!("non-loopback binding requires SM_WEB_TOKEN (at least 24 characters)");
    }
    if let Some(root) = std::env::var_os("SM_SKILLS_ROOT") {
        let root = PathBuf::from(root);
        central_repo::set_runtime_base_dir_override(Some(central_repo::external_base_dir(&root)));
        central_repo::set_runtime_skills_dir_override(Some(root));
    }
    let store = app_state::initialize_cli_store()?;
    let exe = std::env::current_exe()?;
    let cli = std::env::var_os("SM_CLI_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            exe.parent()
                .expect("web executable has a parent directory")
                .join(if cfg!(windows) {
                    "skills-manager-cli.exe"
                } else {
                    "skills-manager-cli"
                })
        });
    if !cli.is_file() {
        bail!(
            "skills-manager-cli is missing next to web server: {}",
            cli.display()
        );
    }
    let dist = std::env::var_os("SM_WEB_DIST")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("..")
                .join("dist")
        });
    let index = dist.join("index.html");
    if !index.is_file() {
        bail!("build the React frontend first: {}", index.display());
    }
    let state = Arc::new(WebState {
        store,
        host,
        port,
        token,
        cli,
    });
    let app = Router::new()
        .route("/api/invoke", post(invoke))
        .fallback_service(ServeDir::new(&dist).not_found_service(ServeFile::new(index)))
        .layer(DefaultBodyLimit::max(1_000_000))
        .with_state(state);
    let listener = tokio::net::TcpListener::bind((host, port)).await?;
    println!("Skills Manager Web: http://{host}:{port}/");
    axum::serve(listener, app).await?;
    Ok(())
}
