use std::{
    env, fs,
    path::{Path, PathBuf},
};

use chrono::{SecondsFormat, Utc};
use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};

const APP_CONFIG_DIR: &str = ".apifuck";
const LEGACY_APP_CONFIG_DIR: &str = ".fuckapi";
const APP_CONFIG_FILE: &str = "config.json";
const DEFAULT_WORKSPACE_DIR: &str = "workspace";
const APP_CONFIG_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum AppTheme {
    Light,
    Dark,
    #[default]
    System,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    pub schema_version: u32,
    pub created_at: String,
    pub updated_at: String,
    pub last_opened_workspace_path: Option<String>,
    pub theme: AppTheme,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateAppConfigInput {
    pub last_opened_workspace_path: Option<String>,
    pub theme: AppTheme,
}

pub fn read_app_config() -> AppResult<AppConfig> {
    read_app_config_at(&app_config_path()?)
}

pub fn resolve_startup_workspace_path() -> AppResult<PathBuf> {
    resolve_startup_workspace_path_at(&app_config_path()?)
}

pub fn update_app_config(input: UpdateAppConfigInput) -> AppResult<AppConfig> {
    update_app_config_at(&app_config_path()?, input)
}

pub fn update_last_opened_workspace_path(path: Option<String>) -> AppResult<AppConfig> {
    let current = read_app_config()?;
    update_app_config(UpdateAppConfigInput {
        last_opened_workspace_path: path,
        theme: current.theme,
    })
}

fn read_app_config_at(path: &Path) -> AppResult<AppConfig> {
    if !path.exists() {
        let config = default_app_config();
        write_json_file(path, &config)?;
        return Ok(config);
    }

    let content = fs::read_to_string(path)?;
    let config: AppConfig = serde_json::from_str(&content)?;
    validate_config(&config)?;
    Ok(config)
}

fn update_app_config_at(path: &Path, input: UpdateAppConfigInput) -> AppResult<AppConfig> {
    let current = read_app_config_at(path)?;
    let next = AppConfig {
        schema_version: current.schema_version,
        created_at: current.created_at,
        updated_at: now_iso_string(),
        last_opened_workspace_path: input
            .last_opened_workspace_path
            .and_then(|value| normalize_optional_string(&value)),
        theme: input.theme,
    };
    write_json_file(path, &next)?;
    Ok(next)
}

fn resolve_startup_workspace_path_at(path: &Path) -> AppResult<PathBuf> {
    let config = read_app_config_at(path)?;
    if let Some(last_opened_workspace_path) = config.last_opened_workspace_path {
        let candidate = PathBuf::from(&last_opened_workspace_path);
        if candidate.is_dir() {
            return Ok(candidate);
        }
    }

    let root = path.parent().ok_or_else(|| {
        AppError::InvalidInput(format!("config path {} has no parent", path.display()))
    })?;
    Ok(default_workspace_path_from_root(root))
}

fn default_app_config() -> AppConfig {
    let now = now_iso_string();
    AppConfig {
        schema_version: APP_CONFIG_SCHEMA_VERSION,
        created_at: now.clone(),
        updated_at: now,
        last_opened_workspace_path: None,
        theme: AppTheme::System,
    }
}

fn validate_config(config: &AppConfig) -> AppResult<()> {
    if config.schema_version != APP_CONFIG_SCHEMA_VERSION {
        return Err(AppError::InvalidWorkspace(format!(
            "unsupported app config schema version {}",
            config.schema_version
        )));
    }
    Ok(())
}

fn normalize_optional_string(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    }
    else {
        Some(trimmed.to_string())
    }
}

fn app_config_path() -> AppResult<PathBuf> {
    Ok(app_config_root()?.join(APP_CONFIG_FILE))
}

fn app_config_root() -> AppResult<PathBuf> {
    let home_dir = home_dir().ok_or_else(|| {
        AppError::InvalidInput("could not determine the current user's home directory".to_string())
    })?;
    let new_root = home_dir.join(APP_CONFIG_DIR);
    let legacy_root = home_dir.join(LEGACY_APP_CONFIG_DIR);

    if new_root.exists() {
        return Ok(new_root);
    }

    if legacy_root.exists() {
        match fs::rename(&legacy_root, &new_root) {
            Ok(_) => return Ok(new_root),
            Err(_) => return Ok(legacy_root),
        }
    }

    Ok(new_root)
}

fn default_workspace_path_from_root(root: &Path) -> PathBuf {
    root.join(DEFAULT_WORKSPACE_DIR)
}

fn home_dir() -> Option<PathBuf> {
    env::var_os("HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("USERPROFILE").map(PathBuf::from))
        .or_else(|| {
            let drive = env::var_os("HOMEDRIVE")?;
            let path = env::var_os("HOMEPATH")?;
            Some(PathBuf::from(format!(
                "{}{}",
                PathBuf::from(drive).display(),
                PathBuf::from(path).display()
            )))
        })
}

fn write_json_file<T: Serialize>(path: &Path, value: &T) -> AppResult<()> {
    let parent = path
        .parent()
        .ok_or_else(|| AppError::InvalidInput(format!("path {} has no parent", path.display())))?;
    fs::create_dir_all(parent)?;

    let mut json = serde_json::to_vec_pretty(value)?;
    json.push(b'\n');

    let temp_path = parent.join(format!(
        ".{}.tmp",
        path.file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| AppError::InvalidInput(format!("invalid config path {}", path.display())))?
    ));

    let write_result = fs::write(&temp_path, json);
    if let Err(error) = write_result {
        let _ = fs::remove_file(&temp_path);
        return Err(error.into());
    }

    #[cfg(target_os = "windows")]
    {
        if path.exists() {
            fs::remove_file(path)?;
        }
    }

    let rename_result = fs::rename(&temp_path, path);
    if let Err(error) = rename_result {
        let _ = fs::remove_file(&temp_path);
        return Err(error.into());
    }

    Ok(())
}

fn now_iso_string() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::*;

    #[test]
    fn app_config_is_created_with_defaults() {
        let temp_dir = tempdir().expect("create temp dir");
        let config_path = temp_dir.path().join(APP_CONFIG_FILE);

        let config = read_app_config_at(&config_path).expect("read config");

        assert_eq!(config.schema_version, APP_CONFIG_SCHEMA_VERSION);
        assert!(config.last_opened_workspace_path.is_none());
        assert!(config_path.exists());
    }

    #[test]
    fn app_config_update_persists_workspace_path_and_theme() {
        let temp_dir = tempdir().expect("create temp dir");
        let config_path = temp_dir.path().join(APP_CONFIG_FILE);

        let updated = update_app_config_at(
            &config_path,
            UpdateAppConfigInput {
                last_opened_workspace_path: Some("/tmp/apifuck-workspace".to_string()),
                theme: AppTheme::Dark,
            },
        )
        .expect("update config");

        assert!(matches!(updated.theme, AppTheme::Dark));
        assert_eq!(
            updated.last_opened_workspace_path.as_deref(),
            Some("/tmp/apifuck-workspace")
        );

        let reloaded = read_app_config_at(&config_path).expect("reload config");
        assert!(matches!(reloaded.theme, AppTheme::Dark));
    }

    #[test]
    fn startup_workspace_path_falls_back_to_default_workspace() {
        let temp_dir = tempdir().expect("create temp dir");
        let config_path = temp_dir.path().join(APP_CONFIG_FILE);

        let startup_path = resolve_startup_workspace_path_at(&config_path).expect("startup path");

        assert_eq!(startup_path, temp_dir.path().join(DEFAULT_WORKSPACE_DIR));
    }

    #[test]
    fn startup_workspace_path_prefers_existing_last_opened_workspace() {
        let temp_dir = tempdir().expect("create temp dir");
        let config_path = temp_dir.path().join(APP_CONFIG_FILE);
        let existing_workspace = temp_dir.path().join("existing-workspace");
        fs::create_dir_all(&existing_workspace).expect("create workspace");

        update_app_config_at(
            &config_path,
            UpdateAppConfigInput {
                last_opened_workspace_path: Some(existing_workspace.display().to_string()),
                theme: AppTheme::System,
            },
        )
        .expect("update config");

        let startup_path = resolve_startup_workspace_path_at(&config_path).expect("startup path");

        assert_eq!(startup_path, existing_workspace);
    }

    #[test]
    fn startup_workspace_path_ignores_missing_last_opened_workspace() {
        let temp_dir = tempdir().expect("create temp dir");
        let config_path = temp_dir.path().join(APP_CONFIG_FILE);

        update_app_config_at(
            &config_path,
            UpdateAppConfigInput {
                last_opened_workspace_path: Some(
                    temp_dir
                        .path()
                        .join("missing-workspace")
                        .display()
                        .to_string(),
                ),
                theme: AppTheme::System,
            },
        )
        .expect("update config");

        let startup_path = resolve_startup_workspace_path_at(&config_path).expect("startup path");

        assert_eq!(startup_path, temp_dir.path().join(DEFAULT_WORKSPACE_DIR));
    }

    #[cfg(unix)]
    #[test]
    fn atomic_write_failure_keeps_previous_config() {
        use std::os::unix::fs::PermissionsExt;

        let temp_dir = tempdir().expect("create temp dir");
        let config_path = temp_dir.path().join(APP_CONFIG_FILE);
        let initial = default_app_config();
        write_json_file(&config_path, &initial).expect("seed config");

        let mut permissions = fs::metadata(temp_dir.path())
            .expect("metadata")
            .permissions();
        permissions.set_mode(0o500);
        fs::set_permissions(temp_dir.path(), permissions).expect("set readonly");

        let result = write_json_file(
            &config_path,
            &AppConfig {
                theme: AppTheme::Dark,
                ..initial.clone()
            },
        );

        let mut restored_permissions = fs::metadata(temp_dir.path())
            .expect("metadata")
            .permissions();
        restored_permissions.set_mode(0o700);
        fs::set_permissions(temp_dir.path(), restored_permissions).expect("restore permissions");

        assert!(result.is_err());
        let content = fs::read_to_string(&config_path).expect("read config");
        assert!(content.contains("\"theme\": \"system\""));
    }
}
