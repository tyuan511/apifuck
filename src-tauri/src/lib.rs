mod app_config;
mod error;
mod http;
mod storage;

use std::{
    path::PathBuf,
    sync::{Mutex, MutexGuard},
};

use app_config::{
    read_app_config as read_app_config_storage, resolve_startup_project_path,
    update_app_config as update_app_config_storage, update_last_opened_project_path, AppConfig,
    OpenRequestTab, UpdateAppConfigInput, UpdateTabStateInput,
};
use error::{AppError, AppResult};
use http::{send_request as send_request_http, SendRequestInput, SendRequestResponse};
use storage::{
    bootstrap_project as bootstrap_project_storage, create_api as create_api_storage,
    create_collection as create_collection_storage,
    create_environment as create_environment_storage, delete_environment as delete_environment_storage,
    delete_node as delete_node_storage, delete_project_files as delete_project_files_storage,
    list_environments as list_environments_storage,
    move_node as move_node_storage, open_project as open_project_storage,
    read_api as read_api_storage, read_project_summary as read_project_summary_storage,
    reorder_children as reorder_children_storage,
    set_active_environment as set_active_environment_storage,
    update_api as update_api_storage, update_collection as update_collection_storage,
    update_environment as update_environment_storage, update_project as update_project_storage,
    ApiDefinition, CollectionSummary, CreateApiInput, CreateCollectionInput, CreateEnvironmentInput,
    CreateProjectInput, DeleteEnvironmentInput, DeleteNodeInput, Environment, MoveNodeInput,
    ProjectSnapshot, ProjectSummary, ReorderChildrenInput, SetActiveEnvironmentInput,
    UpdateApiInput, UpdateCollectionInput, UpdateEnvironmentInput, UpdateProjectInput,
};

#[derive(Default)]
struct ProjectState {
    current_root: Mutex<Option<PathBuf>>,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct AppStartupState {
    app_config: AppConfig,
    project_path: String,
    project_snapshot: ProjectSnapshot,
}

#[tauri::command]
fn open_startup_project(state: tauri::State<'_, ProjectState>) -> AppResult<AppStartupState> {
    let root = resolve_startup_project_path()?;
    let snapshot = bootstrap_project_storage(&root, None)?;
    set_project_root(&state, root.clone())?;
    let app_config = update_last_opened_project_path(Some(root.display().to_string()))?;

    Ok(AppStartupState {
        app_config,
        project_path: root.display().to_string(),
        project_snapshot: snapshot,
    })
}

#[tauri::command]
fn open_project(
    path: String,
    state: tauri::State<'_, ProjectState>,
) -> AppResult<ProjectSnapshot> {
    let root = PathBuf::from(path);
    let snapshot = open_project_storage(&root)?;
    set_project_root(&state, root)?;
    sync_last_opened_project_path(&state)?;
    Ok(snapshot)
}

#[tauri::command]
fn read_project_summary(path: String) -> AppResult<ProjectSummary> {
    read_project_summary_storage(PathBuf::from(path))
}

#[tauri::command]
fn delete_project_files(path: String) -> AppResult<()> {
    delete_project_files_storage(PathBuf::from(path))
}

#[tauri::command]
fn bootstrap_project(
    path: String,
    input: Option<CreateProjectInput>,
    state: tauri::State<'_, ProjectState>,
) -> AppResult<ProjectSnapshot> {
    let root = PathBuf::from(path);
    let snapshot = bootstrap_project_storage(&root, input)?;
    set_project_root(&state, root)?;
    sync_last_opened_project_path(&state)?;
    Ok(snapshot)
}

#[tauri::command]
fn read_app_config() -> AppResult<AppConfig> {
    read_app_config_storage()
}

#[tauri::command]
fn update_app_config(input: UpdateAppConfigInput) -> AppResult<AppConfig> {
    update_app_config_storage(input)
}

#[tauri::command]
fn read_tab_state() -> AppResult<(Vec<OpenRequestTab>, Option<String>)> {
    app_config::read_tab_state()
}

#[tauri::command]
fn update_tab_state(input: UpdateTabStateInput) -> AppResult<AppConfig> {
    app_config::update_tab_state(input)
}

#[tauri::command]
fn update_project(
    input: UpdateProjectInput,
    state: tauri::State<'_, ProjectState>,
) -> AppResult<ProjectSummary> {
    update_project_storage(current_project_root(&state)?, input)
}

#[tauri::command]
fn create_collection(
    input: CreateCollectionInput,
    state: tauri::State<'_, ProjectState>,
) -> AppResult<CollectionSummary> {
    create_collection_storage(current_project_root(&state)?, input)
}

#[tauri::command]
fn update_collection(
    input: UpdateCollectionInput,
    state: tauri::State<'_, ProjectState>,
) -> AppResult<CollectionSummary> {
    update_collection_storage(current_project_root(&state)?, input)
}

#[tauri::command]
fn create_api(
    input: CreateApiInput,
    state: tauri::State<'_, ProjectState>,
) -> AppResult<ApiDefinition> {
    create_api_storage(current_project_root(&state)?, input)
}

#[tauri::command]
fn update_api(
    input: UpdateApiInput,
    state: tauri::State<'_, ProjectState>,
) -> AppResult<ApiDefinition> {
    update_api_storage(current_project_root(&state)?, input)
}

#[tauri::command]
fn delete_node(input: DeleteNodeInput, state: tauri::State<'_, ProjectState>) -> AppResult<()> {
    delete_node_storage(current_project_root(&state)?, input)
}

#[tauri::command]
fn move_node(input: MoveNodeInput, state: tauri::State<'_, ProjectState>) -> AppResult<()> {
    move_node_storage(current_project_root(&state)?, input)
}

#[tauri::command]
fn reorder_children(
    input: ReorderChildrenInput,
    state: tauri::State<'_, ProjectState>,
) -> AppResult<()> {
    reorder_children_storage(current_project_root(&state)?, input)
}

#[tauri::command]
fn read_api(id: String, state: tauri::State<'_, ProjectState>) -> AppResult<ApiDefinition> {
    read_api_storage(current_project_root(&state)?, &id)
}

#[tauri::command]
fn create_environment(
    input: CreateEnvironmentInput,
    state: tauri::State<'_, ProjectState>,
) -> AppResult<Environment> {
    create_environment_storage(current_project_root(&state)?, input)
}

#[tauri::command]
fn update_environment(
    input: UpdateEnvironmentInput,
    state: tauri::State<'_, ProjectState>,
) -> AppResult<Environment> {
    update_environment_storage(current_project_root(&state)?, input)
}

#[tauri::command]
fn delete_environment(
    input: DeleteEnvironmentInput,
    state: tauri::State<'_, ProjectState>,
) -> AppResult<()> {
    delete_environment_storage(current_project_root(&state)?, input)
}

#[tauri::command]
fn list_environments(
    project_id: String,
    state: tauri::State<'_, ProjectState>,
) -> AppResult<Vec<Environment>> {
    list_environments_storage(current_project_root(&state)?, &project_id)
}

#[tauri::command]
fn set_active_environment(
    input: SetActiveEnvironmentInput,
    state: tauri::State<'_, ProjectState>,
) -> AppResult<ProjectSummary> {
    set_active_environment_storage(current_project_root(&state)?, input)
}

#[tauri::command]
async fn send_request(input: SendRequestInput) -> AppResult<SendRequestResponse> {
    send_request_http(input).await
}

fn current_project_root(state: &tauri::State<'_, ProjectState>) -> AppResult<PathBuf> {
    let guard = lock_project_state(state)?;
    guard.clone().ok_or(AppError::ProjectNotOpen)
}

fn set_project_root(state: &tauri::State<'_, ProjectState>, root: PathBuf) -> AppResult<()> {
    let mut guard = lock_project_state(state)?;
    *guard = Some(root);
    Ok(())
}

fn lock_project_state<'a>(
    state: &'a tauri::State<'_, ProjectState>,
) -> AppResult<MutexGuard<'a, Option<PathBuf>>> {
    state
        .current_root
        .lock()
        .map_err(|_| AppError::InvalidInput("project state is unavailable".to_string()))
}

fn sync_last_opened_project_path(state: &tauri::State<'_, ProjectState>) -> AppResult<()> {
    let root = current_project_root(state)?;
    update_last_opened_project_path(Some(root.display().to_string()))?;
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(ProjectState::default())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            open_startup_project,
            read_app_config,
            update_app_config,
            read_tab_state,
            update_tab_state,
            open_project,
            read_project_summary,
            delete_project_files,
            bootstrap_project,
            update_project,
            create_collection,
            update_collection,
            create_api,
            update_api,
            delete_node,
            move_node,
            reorder_children,
            read_api,
            create_environment,
            update_environment,
            delete_environment,
            list_environments,
            set_active_environment,
            send_request
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
