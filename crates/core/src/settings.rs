use crate::collection::atomic_write;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
#[non_exhaustive]
pub struct NetworkSettings {
    pub timeout_ms: u64,
    pub connect_timeout_ms: u64,
    pub follow_redirects: bool,
    pub forward_authorization_header: bool,
    pub max_redirects: usize,
    pub allow_invalid_certificates: bool,
    /// None uses system proxies; an empty string disables proxies; a URL overrides them.
    pub proxy: Option<String>,
    pub max_response_bytes: usize,
    pub cookie_jar: bool,
    pub custom_ca_path: Option<PathBuf>,
    pub client_certificates: Vec<crate::tls::ClientCertificate>,
}
impl Default for NetworkSettings {
    fn default() -> Self {
        Self {
            timeout_ms: 0,
            connect_timeout_ms: 30_000,
            follow_redirects: true,
            forward_authorization_header: false,
            max_redirects: 10,
            allow_invalid_certificates: false,
            proxy: None,
            max_response_bytes: 32 * 1024 * 1024,
            cookie_jar: true,
            custom_ca_path: None,
            client_certificates: vec![],
        }
    }
}

impl NetworkSettings {
    pub fn validate(&self) -> Result<()> {
        anyhow::ensure!(self.connect_timeout_ms > 0, "连接超时必须大于 0");
        anyhow::ensure!(self.max_response_bytes > 0, "响应大小上限必须大于 0");
        if let Some(proxy) = &self.proxy
            && !proxy.is_empty()
        {
            let url = url::Url::parse(proxy).context("代理地址需要完整的协议和主机名")?;
            anyhow::ensure!(
                matches!(url.scheme(), "http" | "https" | "socks5" | "socks5h")
                    && url.host_str().is_some(),
                "代理仅支持 HTTP、HTTPS、SOCKS5 和 SOCKS5H"
            );
            reqwest::Proxy::all(proxy).context("代理地址无效")?;
        }
        if let Some(path) = &self.custom_ca_path {
            anyhow::ensure!(path.is_absolute(), "应用自定义 CA 路径必须是绝对路径");
        }
        for certificate in &self.client_certificates {
            certificate.validate()?;
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RequestLayout {
    #[default]
    Vertical,
    Horizontal,
}

/// Machine-local preferences, independent of the currently open collection.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct AppPreferences {
    #[serde(default = "default_language")]
    pub language: String,
    pub appearance: String,
    pub accent: String,
    pub request_layout: RequestLayout,
    pub auto_check_updates: bool,
    pub network: NetworkSettings,
}
impl Default for AppPreferences {
    fn default() -> Self {
        Self {
            language: default_language(),
            appearance: String::new(),
            accent: String::new(),
            request_layout: RequestLayout::default(),
            auto_check_updates: true,
            network: NetworkSettings::default(),
        }
    }
}
fn default_language() -> String {
    "zh-CN".into()
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RecentProject {
    pub path: PathBuf,
    pub name: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct RecentProjects {
    projects: Vec<RecentProject>,
}

impl RecentProjects {
    pub fn remove(&mut self, path: &Path) {
        self.projects.retain(|project| project.path != path);
    }
    pub fn entries(&self) -> &[RecentProject] {
        &self.projects
    }

    pub fn last_path(&self) -> Option<&Path> {
        self.projects.first().map(|project| project.path.as_path())
    }

    pub fn record(&mut self, path: PathBuf, name: String) {
        let path = std::fs::canonicalize(&path).unwrap_or(path);
        self.projects.retain(|project| project.path != path);
        self.projects.insert(0, RecentProject { path, name });
        self.projects.truncate(10);
    }

    pub fn load_from(path: &Path) -> Result<Self> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Stored {
            History(RecentProjects),
            Legacy(PathBuf),
        }
        let source = match std::fs::read_to_string(path) {
            Ok(source) => source,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Self::default());
            }
            Err(error) => return Err(error.into()),
        };
        let mut recent = Self::default();
        match serde_json::from_str(&source)? {
            Stored::History(history) => {
                for project in history.projects.into_iter().rev() {
                    recent.record(project.path, project.name);
                }
            }
            Stored::Legacy(path) => {
                let name = path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned();
                recent.record(path, name);
            }
        }
        Ok(recent)
    }

    pub fn save_to(&self, path: &Path) -> Result<()> {
        atomic_write(path, &serde_json::to_string_pretty(self)?)
    }
}

impl AppPreferences {
    pub fn draft_path() -> Result<PathBuf> {
        Ok(app_state_path()?.with_file_name("settings-draft.json"))
    }
    pub fn path() -> Result<PathBuf> {
        Ok(app_state_path()?.with_file_name("preferences.json"))
    }
    pub fn load_from(path: &Path) -> Result<Option<Self>> {
        match std::fs::read_to_string(path) {
            Ok(source) => Ok(Some(serde_json::from_str(&source)?)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.into()),
        }
    }
    pub fn save_to(&self, path: &Path) -> Result<()> {
        self.network.validate()?;
        atomic_write(path, &serde_json::to_string_pretty(self)?)
    }
    pub fn from_workspace(state: &WorkspaceState) -> Self {
        Self {
            language: default_language(),
            appearance: state.appearance.clone(),
            accent: state.accent.clone(),
            request_layout: RequestLayout::default(),
            auto_check_updates: true,
            network: state.network.clone(),
        }
    }
    pub fn apply_to(&self, state: &mut WorkspaceState) {
        state.appearance.clone_from(&self.appearance);
        state.accent.clone_from(&self.accent);
        state.network = self.network.clone();
    }
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
#[non_exhaustive]
pub struct WorkspaceState {
    pub tabs: Vec<String>,
    pub active: Option<String>,
    pub environment: Option<String>,
    pub drafts: std::collections::BTreeMap<String, String>,
    pub editor_drafts: std::collections::BTreeMap<String, Value>,
    pub ui_drafts: std::collections::BTreeMap<String, Value>,
    pub appearance: String,
    pub accent: String,
    pub collapsed_folders: std::collections::BTreeSet<String>,
    pub network: NetworkSettings,
}
impl WorkspaceState {
    pub fn load(root: &Path) -> Result<Self> {
        let path = root.join(".apifuck/workspace.json");
        if !path.exists() {
            return Ok(Self::default());
        }
        Ok(serde_json::from_str(&std::fs::read_to_string(path)?)?)
    }
    pub fn save(&self, root: &Path) -> Result<()> {
        atomic_write(
            &root.join(".apifuck/workspace.json"),
            &serde_json::to_string_pretty(self)?,
        )
    }
}
/// Raw editor state is independent of validated, applied settings.
pub fn load_raw_draft(path: &Path) -> Result<Option<Value>> {
    match std::fs::read_to_string(path) {
        Ok(source) => Ok(Some(serde_json::from_str(&source)?)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
    }
}

pub fn save_raw_draft(path: &Path, value: &Value) -> Result<()> {
    atomic_write(path, &serde_json::to_string_pretty(value)?)
}

pub fn overlay(root: &Path, env: Option<&str>) -> Result<Value> {
    let path = root.join(".apifuck/variables.json");
    if !path.exists() {
        return Ok(json!({}));
    }
    let data: Value = serde_json::from_str(&std::fs::read_to_string(path)?)?;
    Ok(data
        .get(
            env.map(|name| format!("env:{name}"))
                .unwrap_or_else(|| "global".into()),
        )
        .cloned()
        .unwrap_or_else(|| json!({})))
}
pub fn persist_variables(root: &Path, env: Option<&str>, mutations: &Value) -> Result<()> {
    if mutations.as_object().is_none_or(|m| m.is_empty()) {
        return Ok(());
    }
    let directory = root.join(".apifuck");
    std::fs::create_dir_all(&directory)?;
    let lock = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(directory.join("variables.lock"))?;
    lock.lock()?;
    let path = directory.join("variables.json");
    let mut data: Value = if path.exists() {
        serde_json::from_str(&std::fs::read_to_string(&path)?)?
    } else {
        json!({})
    };
    let key = env
        .map(|name| format!("env:{name}"))
        .unwrap_or_else(|| "global".into());
    if !data[&key].is_object() {
        data[&key] = json!({});
    }
    for (name, value) in mutations.as_object().context("无效变量写入")? {
        data[&key][name] = value.clone();
    }
    atomic_write(&path, &serde_json::to_string_pretty(&data)?)
}
pub fn app_state_path() -> Result<PathBuf> {
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .context("未找到用户目录")?;
    Ok(PathBuf::from(home).join(".apifuck-native/recent.json"))
}

#[cfg(test)]
mod recent_project_tests {
    use super::*;

    #[test]
    fn recent_projects_keep_ten_unique_paths_in_open_order_across_restarts() {
        let temp = tempfile::tempdir().unwrap();
        let mut recent = RecentProjects::default();
        for index in 0..12 {
            recent.record(
                temp.path().join(format!("project-{index}")),
                format!("Project {index}"),
            );
        }
        assert_eq!(recent.entries().len(), 10);
        assert_eq!(recent.entries()[0].name, "Project 11");
        assert_eq!(recent.entries()[9].name, "Project 2");
        let reopened = temp.path().join("project-5");
        recent.record(reopened.clone(), "Renamed project".into());
        assert_eq!(recent.last_path(), Some(reopened.as_path()));
        assert_eq!(recent.entries()[1].name, "Project 11");
        assert_eq!(recent.entries().len(), 10);
        assert_eq!(
            recent
                .entries()
                .iter()
                .filter(|entry| entry.path == reopened)
                .count(),
            1
        );
        let file = temp.path().join("recent.json");
        recent.save_to(&file).unwrap();
        let restored = RecentProjects::load_from(&file).unwrap();
        assert_eq!(restored.last_path(), Some(reopened.as_path()));
        assert_eq!(restored.entries()[0].name, "Renamed project");
        assert_eq!(restored.entries().len(), 10);
    }

    #[test]
    fn legacy_last_project_is_migrated_and_equivalent_paths_are_deduplicated() {
        let temp = tempfile::tempdir().unwrap();
        let project = temp.path().join("Legacy Project");
        std::fs::create_dir(&project).unwrap();
        let canonical = std::fs::canonicalize(&project).unwrap();
        let file = temp.path().join("recent.json");
        std::fs::write(&file, serde_json::to_string(&project).unwrap()).unwrap();
        let mut recent = RecentProjects::load_from(&file).unwrap();
        assert_eq!(recent.last_path(), Some(canonical.as_path()));
        assert_eq!(recent.entries()[0].name, "Legacy Project");
        recent.record(project.join("."), "Display name".into());
        assert_eq!(recent.entries().len(), 1);
        recent.save_to(&file).unwrap();
        let restored = RecentProjects::load_from(&file).unwrap();
        assert_eq!(restored.entries()[0].name, "Display name");
        assert_eq!(restored.last_path(), Some(canonical.as_path()));
    }

    #[test]
    fn loading_history_enforces_limit_and_reports_corrupt_files() {
        let temp = tempfile::tempdir().unwrap();
        let file = temp.path().join("recent.json");
        assert!(
            RecentProjects::load_from(&file)
                .unwrap()
                .entries()
                .is_empty()
        );
        let projects: Vec<_> = (0..15).map(|index| json!({"path":temp.path().join(format!("project-{index}")),"name":format!("Project {index}")})).collect();
        std::fs::write(
            &file,
            serde_json::to_string(&json!({"projects":projects})).unwrap(),
        )
        .unwrap();
        let recent = RecentProjects::load_from(&file).unwrap();
        assert_eq!(recent.entries().len(), 10);
        assert_eq!(recent.entries()[0].name, "Project 0");
        assert_eq!(recent.entries()[9].name, "Project 9");
        std::fs::write(&file, "broken json").unwrap();
        assert!(RecentProjects::load_from(&file).is_err());
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "broken json");
    }
}

#[cfg(test)]
mod preference_tests {
    use super::*;

    #[test]
    fn automatic_update_preference_defaults_on_and_preserves_opt_out() {
        let legacy: AppPreferences = serde_json::from_value(json!({"language":"en"})).unwrap();
        assert!(legacy.auto_check_updates);
        let opted_out = AppPreferences {
            auto_check_updates: false,
            ..Default::default()
        };
        let restored: AppPreferences =
            serde_json::from_value(serde_json::to_value(opted_out).unwrap()).unwrap();
        assert!(!restored.auto_check_updates);
    }

    #[test]
    fn preferences_survive_restart_and_apply_without_replacing_workspace() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("preferences.json");
        assert!(AppPreferences::load_from(&path).unwrap().is_none());
        let mut preferences = AppPreferences {
            language: "en".into(),
            appearance: "dark".into(),
            accent: "blue".into(),
            request_layout: RequestLayout::Horizontal,
            ..Default::default()
        };
        preferences.network.proxy = Some("socks5h://127.0.0.1:1080".into());
        preferences.network.forward_authorization_header = true;
        preferences.save_to(&path).unwrap();
        let restored = AppPreferences::load_from(&path).unwrap().unwrap();
        assert_eq!(restored.language, "en");
        assert_eq!(restored.request_layout, RequestLayout::Horizontal);
        let mut workspace = WorkspaceState {
            tabs: vec!["request.yml".into()],
            environment: Some("local".into()),
            ..Default::default()
        };
        restored.apply_to(&mut workspace);
        assert_eq!(workspace.tabs, ["request.yml"]);
        assert_eq!(workspace.environment.as_deref(), Some("local"));
        assert_eq!(workspace.appearance, "dark");
        assert_eq!(workspace.accent, "blue");
        assert!(workspace.network.forward_authorization_header);
        assert_eq!(
            workspace.network.proxy.as_deref(),
            Some("socks5h://127.0.0.1:1080")
        );
    }

    #[test]
    fn disabled_proxy_survives_preferences_save_and_reload() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("preferences.json");
        let mut preferences = AppPreferences::default();
        preferences.network.proxy = Some(String::new());
        preferences.save_to(&path).unwrap();
        let restored = AppPreferences::load_from(&path).unwrap().unwrap();
        assert_eq!(restored.network.proxy.as_deref(), Some(""));
    }

    #[test]
    fn invalid_proxy_does_not_overwrite_saved_preferences() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("preferences.json");
        let mut preferences = AppPreferences::default();
        preferences.save_to(&path).unwrap();
        let before = std::fs::read(&path).unwrap();
        for proxy in ["missing-protocol", "file:///tmp/proxy", "ftp://localhost"] {
            preferences.network.proxy = Some(proxy.into());
            assert!(preferences.save_to(&path).is_err());
            assert_eq!(std::fs::read(&path).unwrap(), before);
        }
    }

    #[test]
    fn existing_preferences_default_to_simplified_chinese() {
        let preferences: AppPreferences = serde_json::from_str(r#"{"appearance":"dark"}"#).unwrap();
        assert_eq!(preferences.language, "zh-CN");
        assert_eq!(preferences.request_layout, RequestLayout::Vertical);
    }
}

#[cfg(test)]
mod draft_tests {
    use super::*;

    #[test]
    fn raw_edits_and_closed_tab_drafts_survive_workspace_round_trip() {
        let root = tempfile::tempdir().unwrap();
        let raw = json!({
            "dirty": true, "name":"未完成的名称", "source":"http: [",
            "sectionText":"{ unfinished", "url":"{{base_url}}/a?x=",
            "message":"{\"event\":", "method":"POST",
            "formDraft": {"kind":"Body", "raw":{
                "mode":"multipart-form", "editor":"{unfinished",
                "table":[{"original":{"type":"file"},"name":"","value":"","description":"","enabled":false}],
                "fields":{"timeout":"01e","proxy":"http://"}
            }, "modes":{"body:json":{"editor":"{\"unfinished\":"},"auth:basic":{"fields":{"username":"someone","password":"draft"}}}},
            "formDrafts":{"\"Headers\"":{"raw":{"table":[{"name":"X-Partial","value":""}]}}}
        });
        let mut state = WorkspaceState {
            tabs: vec!["request.yml".into()],
            active: Some("request.yml".into()),
            ..Default::default()
        };
        state
            .editor_drafts
            .insert("request.yml".into(), raw.clone());
        state.drafts.insert("request.yml".into(), "http: [".into());
        state
            .ui_drafts
            .insert("new:http".into(), json!("unfinished/path"));
        state
            .ui_drafts
            .insert("rename:request.yml".into(), json!("new-name"));
        state.save(root.path()).unwrap();
        let mut restored = WorkspaceState::load(root.path()).unwrap();
        assert_eq!(restored.editor_drafts["request.yml"], raw);
        assert_eq!(restored.ui_drafts, state.ui_drafts);
        assert_eq!(restored.drafts["request.yml"], "http: [");
        // Closing a tab updates the session without deleting its recovery data.
        restored.tabs.clear();
        restored.active = None;
        restored.save(root.path()).unwrap();
        let reopened = WorkspaceState::load(root.path()).unwrap();
        assert!(reopened.tabs.is_empty());
        assert_eq!(reopened.editor_drafts["request.yml"], raw);
    }

    #[test]
    fn settings_drafts_preserve_invalid_input_without_applying_it() {
        let dir = tempfile::tempdir().unwrap();
        let preferences_path = dir.path().join("preferences.json");
        let draft_path = dir.path().join("settings-draft.json");
        AppPreferences::default()
            .save_to(&preferences_path)
            .unwrap();
        let applied = std::fs::read(&preferences_path).unwrap();
        assert!(load_raw_draft(&draft_path).unwrap().is_none());
        let raw = json!({"language":"en","network":{"timeout":"-", "proxy":"http://", "limit":"", "follow":false}});
        save_raw_draft(&draft_path, &raw).unwrap();
        assert_eq!(load_raw_draft(&draft_path).unwrap(), Some(raw));
        assert_eq!(std::fs::read(&preferences_path).unwrap(), applied);
        std::fs::write(&draft_path, "{broken").unwrap();
        assert!(load_raw_draft(&draft_path).is_err());
        assert_eq!(std::fs::read_to_string(&draft_path).unwrap(), "{broken");
    }

    #[test]
    fn legacy_workspace_keeps_its_drafts_and_defaults_new_fields() {
        let legacy = r#"{"tabs":["old.yml"],"drafts":{"old.yml":"partial: ["},"editor_drafts":{"old.yml":{"source":"partial: ["}}}"#;
        let state: WorkspaceState = serde_json::from_str(legacy).unwrap();
        assert!(state.ui_drafts.is_empty());
        assert_eq!(state.editor_drafts["old.yml"]["source"], "partial: [");
    }
}
