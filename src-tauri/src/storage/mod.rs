use std::{
    collections::{BTreeMap, HashMap},
    fs,
    path::{Path, PathBuf},
};

use chrono::{SecondsFormat, Utc};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::Value;
use ulid::Ulid;

use crate::error::{AppError, AppResult};

const WORKSPACE_FILE: &str = "workspace.json";
const METADATA_FILE: &str = "metadata.json";
const ITEMS_DIR: &str = "items";
pub const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum EntityType {
    Project,
    Collection,
    Api,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceManifest {
    pub schema_version: u32,
    pub workspace_id: String,
    pub created_at: String,
    pub updated_at: String,
    pub name: String,
    pub default_project_id: String,
    pub last_opened_project_id: String,
    pub project_order: Vec<String>,
    #[serde(default)]
    pub settings: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ProjectDocsConfig {
    pub enabled: bool,
    #[serde(default)]
    pub base_path: String,
    #[serde(default)]
    pub info: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ProjectMockConfig {
    pub enabled: bool,
    #[serde(default)]
    pub base_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectMetadata {
    pub schema_version: u32,
    pub entity_type: EntityType,
    pub id: String,
    pub created_at: String,
    pub updated_at: String,
    pub slug: String,
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub root_order: Vec<String>,
    #[serde(default)]
    pub docs: ProjectDocsConfig,
    #[serde(default)]
    pub mock: ProjectMockConfig,
    #[serde(default)]
    pub active_environment_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollectionMetadata {
    pub schema_version: u32,
    pub entity_type: EntityType,
    pub id: String,
    pub created_at: String,
    pub updated_at: String,
    pub slug: String,
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub order: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct KeyValue {
    pub id: String,
    pub key: String,
    pub value: String,
    pub enabled: bool,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "kebab-case")]
pub enum AuthType {
    #[default]
    None,
    Basic,
    Bearer,
    ApiKey,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct BasicAuthConfig {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ApiKeyAuthConfig {
    pub key: String,
    pub value: String,
    pub add_to: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AuthConfig {
    pub auth_type: AuthType,
    pub basic: BasicAuthConfig,
    pub bearer_token: String,
    pub api_key: ApiKeyAuthConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "kebab-case")]
pub enum BodyMode {
    #[default]
    None,
    Raw,
    Json,
    FormData,
    XWwwFormUrlencoded,
    Binary,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct BinaryBodySpec {
    pub file_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct BodySpec {
    pub mode: BodyMode,
    pub raw: String,
    pub json: String,
    #[serde(default)]
    pub form_data: Vec<KeyValue>,
    #[serde(default)]
    pub url_encoded: Vec<KeyValue>,
    #[serde(default)]
    pub binary: BinaryBodySpec,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RequestDefinition {
    #[serde(default)]
    pub headers: Vec<KeyValue>,
    #[serde(default)]
    pub query: Vec<KeyValue>,
    #[serde(default)]
    pub path_params: Vec<KeyValue>,
    #[serde(default)]
    pub cookies: Vec<KeyValue>,
    #[serde(default)]
    pub auth: AuthConfig,
    #[serde(default)]
    pub body: BodySpec,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ApiDocumentation {
    pub summary: String,
    pub description: String,
    pub deprecated: bool,
    pub operation_id: String,
    pub group_name: String,
}

fn default_mock_status() -> u16 {
    200
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiMock {
    pub enabled: bool,
    #[serde(default = "default_mock_status")]
    pub status: u16,
    pub latency_ms: u64,
    #[serde(default)]
    pub headers: Vec<KeyValue>,
    #[serde(default)]
    pub body: Value,
    pub content_type: String,
}

impl Default for ApiMock {
    fn default() -> Self {
        Self {
            enabled: false,
            status: default_mock_status(),
            latency_ms: 0,
            headers: vec![],
            body: Value::Null,
            content_type: "application/json".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiDefinition {
    pub schema_version: u32,
    pub entity_type: EntityType,
    pub id: String,
    pub created_at: String,
    pub updated_at: String,
    pub slug: String,
    pub name: String,
    pub method: String,
    pub url: String,
    pub description: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub request: RequestDefinition,
    #[serde(default)]
    pub documentation: ApiDocumentation,
    #[serde(default)]
    pub mock: ApiMock,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiSummary {
    pub id: String,
    pub slug: String,
    pub name: String,
    pub method: String,
    pub url: String,
    pub description: String,
    #[serde(default)]
    pub tags: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Environment {
    pub id: String,
    pub name: String,
    pub base_url: String,
    #[serde(default)]
    pub variables: Vec<KeyValue>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollectionTreeNode {
    pub id: String,
    pub slug: String,
    pub name: String,
    pub description: String,
    pub created_at: String,
    pub updated_at: String,
    #[serde(default)]
    pub children: Vec<TreeNode>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "entityType", rename_all = "lowercase")]
pub enum TreeNode {
    Collection(CollectionTreeNode),
    Api(ApiSummary),
}

impl TreeNode {
    fn id(&self) -> &str {
        match self {
            TreeNode::Collection(node) => &node.id,
            TreeNode::Api(node) => &node.id,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectSnapshot {
    pub metadata: ProjectMetadata,
    #[serde(default)]
    pub children: Vec<TreeNode>,
    #[serde(default)]
    pub environments: Vec<Environment>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceSnapshot {
    pub schema_version: u32,
    pub workspace_id: String,
    pub created_at: String,
    pub updated_at: String,
    pub name: String,
    pub default_project_id: String,
    pub last_opened_project_id: String,
    #[serde(default)]
    pub project_order: Vec<String>,
    #[serde(default)]
    pub settings: BTreeMap<String, Value>,
    #[serde(default)]
    pub projects: Vec<ProjectSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateProjectInput {
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub slug: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateProjectInput {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub docs: ProjectDocsConfig,
    #[serde(default)]
    pub mock: ProjectMockConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateCollectionInput {
    pub project_id: String,
    pub parent_collection_id: Option<String>,
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub slug: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateCollectionInput {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateApiInput {
    pub project_id: String,
    pub parent_collection_id: Option<String>,
    pub name: String,
    pub method: String,
    pub url: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub request: RequestDefinition,
    #[serde(default)]
    pub documentation: ApiDocumentation,
    #[serde(default)]
    pub mock: ApiMock,
    pub slug: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateApiInput {
    pub id: String,
    pub name: String,
    pub method: String,
    pub url: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub request: RequestDefinition,
    #[serde(default)]
    pub documentation: ApiDocumentation,
    #[serde(default)]
    pub mock: ApiMock,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteNodeInput {
    pub id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveNodeInput {
    pub node_id: String,
    pub target_project_id: String,
    pub target_collection_id: Option<String>,
    pub position: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReorderChildrenInput {
    pub project_id: Option<String>,
    pub parent_collection_id: Option<String>,
    #[serde(default)]
    pub ordered_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateEnvironmentInput {
    pub project_id: String,
    pub name: String,
    #[serde(default)]
    pub base_url: String,
    #[serde(default)]
    pub variables: Vec<KeyValue>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateEnvironmentInput {
    pub id: String,
    pub project_id: String,
    pub name: String,
    #[serde(default)]
    pub base_url: String,
    #[serde(default)]
    pub variables: Vec<KeyValue>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteEnvironmentInput {
    pub id: String,
    pub project_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetActiveEnvironmentInput {
    pub project_id: String,
    pub environment_id: Option<String>,
}

pub type ProjectSummary = ProjectMetadata;
pub type CollectionSummary = CollectionMetadata;

#[derive(Debug, Clone)]
enum ParentRef {
    ProjectRoot(String),
    Collection(String),
}

#[derive(Debug, Clone)]
struct ProjectEntry {
    dir: PathBuf,
    items_dir: PathBuf,
    metadata: ProjectMetadata,
}

#[derive(Debug, Clone)]
struct CollectionEntry {
    dir: PathBuf,
    metadata_path: PathBuf,
    metadata: CollectionMetadata,
    project_id: String,
    parent: ParentRef,
}

#[derive(Debug, Clone)]
struct ApiEntry {
    path: PathBuf,
    definition: ApiDefinition,
    parent: ParentRef,
}

#[derive(Debug, Clone)]
struct WorkspaceIndex {
    root: PathBuf,
    manifest: WorkspaceManifest,
    projects: HashMap<String, ProjectEntry>,
    collections: HashMap<String, CollectionEntry>,
    apis: HashMap<String, ApiEntry>,
}

impl WorkspaceIndex {
    fn load(root: impl AsRef<Path>) -> AppResult<Self> {
        let root = root.as_ref().to_path_buf();
        let manifest_path = root.join(WORKSPACE_FILE);
        if !manifest_path.exists() {
            return Err(AppError::WorkspaceNotFound(root.display().to_string()));
        }

        let manifest: WorkspaceManifest = read_json_file(&manifest_path)?;
        if manifest.schema_version != SCHEMA_VERSION {
            return Err(AppError::InvalidWorkspace(format!(
                "unsupported workspace schema version {}",
                manifest.schema_version
            )));
        }

        let mut index = Self {
            root,
            manifest,
            projects: HashMap::new(),
            collections: HashMap::new(),
            apis: HashMap::new(),
        };

        let mut seen_ids = HashMap::new();

        let entries = fs::read_dir(&index.root)?;
        for entry in entries {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }
            if is_hidden(entry.file_name().to_string_lossy().as_ref()) {
                continue;
            }

            let project_dir = entry.path();
            let metadata_path = project_dir.join(METADATA_FILE);
            if !metadata_path.exists() {
                continue;
            }

            let metadata: ProjectMetadata = read_json_file(&metadata_path)?;
            validate_project_metadata(&metadata)?;
            register_id(&mut seen_ids, &metadata.id, "project")?;

            let project_id = metadata.id.clone();
            let items_dir = project_dir.join(ITEMS_DIR);
            let children = if items_dir.exists() {
                scan_children(
                    &project_id,
                    &items_dir,
                    &metadata.root_order,
                    &ParentRef::ProjectRoot(project_id.clone()),
                    &mut seen_ids,
                    &mut index.collections,
                    &mut index.apis,
                )?
            } else {
                vec![]
            };

            index.projects.insert(
                metadata.id.clone(),
                ProjectEntry {
                    dir: project_dir,
                    items_dir,
                    metadata,
                },
            );

            let project = index
                .projects
                .get_mut(&project_id)
                .expect("project inserted");
            project.metadata.root_order =
                ordered_child_ids(project.metadata.root_order.clone(), &children);
        }

        if !index
            .projects
            .contains_key(&index.manifest.default_project_id)
        {
            return Err(AppError::InvalidWorkspace(format!(
                "default project {} is missing",
                index.manifest.default_project_id
            )));
        }

        if !index
            .projects
            .contains_key(&index.manifest.last_opened_project_id)
        {
            return Err(AppError::InvalidWorkspace(format!(
                "last opened project {} is missing",
                index.manifest.last_opened_project_id
            )));
        }

        for project_id in &index.manifest.project_order {
            if !index.projects.contains_key(project_id) {
                return Err(AppError::InvalidWorkspace(format!(
                    "project order references missing project {}",
                    project_id
                )));
            }
        }

        Ok(index)
    }

    fn snapshot(&self) -> AppResult<WorkspaceSnapshot> {
        let mut projects = Vec::with_capacity(self.projects.len());
        for project_entry in sorted_projects(self) {
            let children = if project_entry.items_dir.exists() {
                scan_children(
                    &project_entry.metadata.id,
                    &project_entry.items_dir,
                    &project_entry.metadata.root_order,
                    &ParentRef::ProjectRoot(project_entry.metadata.id.clone()),
                    &mut HashMap::new(),
                    &mut HashMap::new(),
                    &mut HashMap::new(),
                )?
            } else {
                vec![]
            };
            let environments = read_environments(&self.root, &project_entry.metadata.id)?;
            projects.push(ProjectSnapshot {
                metadata: project_entry.metadata.clone(),
                children,
                environments,
            });
        }

        Ok(WorkspaceSnapshot {
            schema_version: self.manifest.schema_version,
            workspace_id: self.manifest.workspace_id.clone(),
            created_at: self.manifest.created_at.clone(),
            updated_at: self.manifest.updated_at.clone(),
            name: self.manifest.name.clone(),
            default_project_id: self.manifest.default_project_id.clone(),
            last_opened_project_id: self.manifest.last_opened_project_id.clone(),
            project_order: self.manifest.project_order.clone(),
            settings: self.manifest.settings.clone(),
            projects,
        })
    }
}

pub fn open_workspace(root: impl AsRef<Path>) -> AppResult<WorkspaceSnapshot> {
    WorkspaceIndex::load(root)?.snapshot()
}

pub fn bootstrap_workspace(root: impl AsRef<Path>) -> AppResult<WorkspaceSnapshot> {
    let root = root.as_ref();
    if !root.exists() {
        fs::create_dir_all(root)?;
    }

    let manifest_path = root.join(WORKSPACE_FILE);
    if !manifest_path.exists() {
        initialize_workspace(root)?;
    }

    open_workspace(root)
}

pub fn create_project(
    root: impl AsRef<Path>,
    input: CreateProjectInput,
) -> AppResult<ProjectSummary> {
    let root = root.as_ref();
    let mut index = WorkspaceIndex::load(root)?;
    let name = required_name(&input.name, "project")?;
    let slug = unique_project_slug(root, input.slug.as_deref().unwrap_or(&name))?;
    let id = new_id();
    let now = now_iso_string();
    let project_dir = root.join(&slug);
    if project_dir.exists() {
        return Err(AppError::Conflict(format!(
            "project directory already exists: {}",
            project_dir.display()
        )));
    }

    fs::create_dir_all(project_dir.join(ITEMS_DIR))?;

    let metadata = ProjectMetadata {
        schema_version: SCHEMA_VERSION,
        entity_type: EntityType::Project,
        id: id.clone(),
        created_at: now.clone(),
        updated_at: now,
        slug: slug.clone(),
        name,
        description: input.description,
        root_order: vec![],
        docs: ProjectDocsConfig::default(),
        mock: ProjectMockConfig::default(),
        active_environment_id: None,
    };

    write_json_file(&project_dir.join(METADATA_FILE), &metadata)?;

    index.manifest.project_order.push(id.clone());
    index.manifest.last_opened_project_id = id.clone();
    index.manifest.updated_at = now_iso_string();
    write_json_file(&root.join(WORKSPACE_FILE), &index.manifest)?;

    Ok(metadata)
}

pub fn update_project(
    root: impl AsRef<Path>,
    input: UpdateProjectInput,
) -> AppResult<ProjectSummary> {
    let index = WorkspaceIndex::load(root)?;
    let project = index
        .projects
        .get(&input.id)
        .ok_or_else(|| AppError::NotFound(format!("project {}", input.id)))?;

    let mut metadata = project.metadata.clone();
    metadata.name = required_name(&input.name, "project")?;
    metadata.description = input.description;
    metadata.docs = input.docs;
    metadata.mock = input.mock;
    metadata.updated_at = now_iso_string();
    write_json_file(&project.dir.join(METADATA_FILE), &metadata)?;

    Ok(metadata)
}

pub fn create_collection(
    root: impl AsRef<Path>,
    input: CreateCollectionInput,
) -> AppResult<CollectionSummary> {
    let root = root.as_ref();
    let index = WorkspaceIndex::load(root)?;
    ensure_project_parent_matches(
        &index,
        &input.project_id,
        input.parent_collection_id.as_deref(),
    )?;

    let name = required_name(&input.name, "collection")?;
    let id = new_id();
    let now = now_iso_string();
    let parent = resolve_parent(
        &index,
        &input.project_id,
        input.parent_collection_id.as_deref(),
    )?;
    let parent_dir = parent_directory(&index, &parent)?;
    let slug = unique_collection_slug(&parent_dir, input.slug.as_deref().unwrap_or(&name))?;
    let collection_dir = parent_dir.join(&slug);

    fs::create_dir_all(&collection_dir)?;
    let metadata = CollectionMetadata {
        schema_version: SCHEMA_VERSION,
        entity_type: EntityType::Collection,
        id: id.clone(),
        created_at: now.clone(),
        updated_at: now,
        slug,
        name,
        description: input.description,
        order: vec![],
    };
    write_json_file(&collection_dir.join(METADATA_FILE), &metadata)?;
    append_child_order(&index, &parent, &id)?;

    Ok(metadata)
}

pub fn update_collection(
    root: impl AsRef<Path>,
    input: UpdateCollectionInput,
) -> AppResult<CollectionSummary> {
    let index = WorkspaceIndex::load(root)?;
    let entry = index
        .collections
        .get(&input.id)
        .ok_or_else(|| AppError::NotFound(format!("collection {}", input.id)))?;

    let mut metadata = entry.metadata.clone();
    metadata.name = required_name(&input.name, "collection")?;
    metadata.description = input.description;
    metadata.updated_at = now_iso_string();
    write_json_file(&entry.metadata_path, &metadata)?;

    Ok(metadata)
}

pub fn create_api(root: impl AsRef<Path>, input: CreateApiInput) -> AppResult<ApiDefinition> {
    let root = root.as_ref();
    let index = WorkspaceIndex::load(root)?;
    ensure_project_parent_matches(
        &index,
        &input.project_id,
        input.parent_collection_id.as_deref(),
    )?;

    let name = required_name(&input.name, "api")?;
    let id = new_id();
    let slug = slugify(input.slug.as_deref().unwrap_or(&name), "api");
    let now = now_iso_string();
    let parent = resolve_parent(
        &index,
        &input.project_id,
        input.parent_collection_id.as_deref(),
    )?;
    let parent_dir = parent_directory(&index, &parent)?;
    let file_name = api_file_name(&id, &slug);

    let definition = ApiDefinition {
        schema_version: SCHEMA_VERSION,
        entity_type: EntityType::Api,
        id: id.clone(),
        created_at: now.clone(),
        updated_at: now,
        slug,
        name,
        method: normalized_method(&input.method),
        url: input.url,
        description: input.description,
        tags: input.tags,
        request: normalize_request(input.request),
        documentation: input.documentation,
        mock: input.mock,
    };

    write_json_file(&parent_dir.join(file_name), &definition)?;
    append_child_order(&index, &parent, &id)?;

    Ok(definition)
}

pub fn update_api(root: impl AsRef<Path>, input: UpdateApiInput) -> AppResult<ApiDefinition> {
    let index = WorkspaceIndex::load(root)?;
    let entry = index
        .apis
        .get(&input.id)
        .ok_or_else(|| AppError::NotFound(format!("api {}", input.id)))?;

    let mut definition = entry.definition.clone();
    definition.name = required_name(&input.name, "api")?;
    definition.method = normalized_method(&input.method);
    definition.url = input.url;
    definition.description = input.description;
    definition.tags = input.tags;
    definition.request = normalize_request(input.request);
    definition.documentation = input.documentation;
    definition.mock = input.mock;
    definition.updated_at = now_iso_string();
    write_json_file(&entry.path, &definition)?;

    Ok(definition)
}

pub fn delete_node(root: impl AsRef<Path>, input: DeleteNodeInput) -> AppResult<()> {
    let root = root.as_ref();
    let index = WorkspaceIndex::load(root)?;

    if let Some(entry) = index.apis.get(&input.id) {
        fs::remove_file(&entry.path)?;
        remove_child_order(&index, &entry.parent, &input.id)?;
        return Ok(());
    }

    if let Some(entry) = index.collections.get(&input.id) {
        fs::remove_dir_all(&entry.dir)?;
        remove_child_order(&index, &entry.parent, &input.id)?;
        return Ok(());
    }

    if let Some(entry) = index.projects.get(&input.id) {
        if input.id == index.manifest.default_project_id {
            return Err(AppError::Conflict(
                "default project cannot be deleted".to_string(),
            ));
        }

        fs::remove_dir_all(&entry.dir)?;
        let mut manifest = index.manifest.clone();
        manifest.project_order.retain(|id| id != &input.id);
        if manifest.last_opened_project_id == input.id {
            manifest.last_opened_project_id = manifest.default_project_id.clone();
        }
        manifest.updated_at = now_iso_string();
        write_json_file(&root.join(WORKSPACE_FILE), &manifest)?;
        return Ok(());
    }

    Err(AppError::NotFound(format!("node {}", input.id)))
}

pub fn move_node(root: impl AsRef<Path>, input: MoveNodeInput) -> AppResult<()> {
    let root = root.as_ref();
    let index = WorkspaceIndex::load(root)?;
    let target_parent = resolve_parent(
        &index,
        &input.target_project_id,
        input.target_collection_id.as_deref(),
    )?;
    let target_dir = parent_directory(&index, &target_parent)?;

    if let Some(api) = index.apis.get(&input.node_id) {
        let file_name = api
            .path
            .file_name()
            .ok_or_else(|| AppError::InvalidWorkspace("api file name is missing".to_string()))?;
        let destination = target_dir.join(file_name);
        if api.path != destination {
            fs::rename(&api.path, &destination)?;
        }

        update_move_order(
            root,
            &index,
            &api.parent,
            &target_parent,
            &input.node_id,
            input.position,
        )?;
        return Ok(());
    }

    if let Some(collection) = index.collections.get(&input.node_id) {
        if let Some(target_parent_id) = input.target_collection_id.as_deref() {
            if target_parent_id == input.node_id
                || is_descendant_collection(&index, target_parent_id, &input.node_id)
            {
                return Err(AppError::InvalidInput(
                    "collection cannot be moved into itself or its descendants".to_string(),
                ));
            }
        }

        let destination = target_dir.join(collection.dir.file_name().ok_or_else(|| {
            AppError::InvalidWorkspace("collection directory name is missing".to_string())
        })?);
        if collection.dir != destination {
            if destination.exists() {
                return Err(AppError::Conflict(format!(
                    "target already contains {}",
                    destination.display()
                )));
            }
            fs::rename(&collection.dir, &destination)?;
        }

        update_move_order(
            root,
            &index,
            &collection.parent,
            &target_parent,
            &input.node_id,
            input.position,
        )?;
        return Ok(());
    }

    Err(AppError::NotFound(format!("node {}", input.node_id)))
}

pub fn reorder_children(root: impl AsRef<Path>, input: ReorderChildrenInput) -> AppResult<()> {
    let root = root.as_ref();
    let index = WorkspaceIndex::load(root)?;

    let parent = match (&input.project_id, &input.parent_collection_id) {
        (Some(project_id), None) => ParentRef::ProjectRoot(project_id.clone()),
        (_, Some(collection_id)) => {
            if !index.collections.contains_key(collection_id) {
                return Err(AppError::NotFound(format!("collection {}", collection_id)));
            }
            ParentRef::Collection(collection_id.clone())
        }
        _ => {
            return Err(AppError::InvalidInput(
                "reorder_children requires either projectId or parentCollectionId".to_string(),
            ))
        }
    };

    let existing_ids = child_ids_for_parent(&index, &parent)?;
    validate_ordered_ids(&existing_ids, &input.ordered_ids)?;
    let next_order = merge_order(&existing_ids, &input.ordered_ids);
    set_parent_order(&index, &parent, next_order)?;

    Ok(())
}

pub fn read_api(root: impl AsRef<Path>, id: &str) -> AppResult<ApiDefinition> {
    let index = WorkspaceIndex::load(root)?;
    let entry = index
        .apis
        .get(id)
        .ok_or_else(|| AppError::NotFound(format!("api {}", id)))?;
    Ok(entry.definition.clone())
}

const ENVIRONMENTS_FILE: &str = "environments.json";

fn read_environments(root: &Path, project_id: &str) -> AppResult<Vec<Environment>> {
    let index = WorkspaceIndex::load(root)?;
    let project = index
        .projects
        .get(project_id)
        .ok_or_else(|| AppError::NotFound(format!("project {}", project_id)))?;
    let path = project.dir.join(ENVIRONMENTS_FILE);
    if !path.exists() {
        return Ok(vec![]);
    }
    read_json_file(&path)
}

fn write_environments(
    root: &Path,
    project_id: &str,
    environments: &[Environment],
) -> AppResult<()> {
    let index = WorkspaceIndex::load(root)?;
    let project = index
        .projects
        .get(project_id)
        .ok_or_else(|| AppError::NotFound(format!("project {}", project_id)))?;
    let path = project.dir.join(ENVIRONMENTS_FILE);
    write_json_file(&path, &environments.to_vec())
}

pub fn create_environment(
    root: impl AsRef<Path>,
    input: CreateEnvironmentInput,
) -> AppResult<Environment> {
    let root = root.as_ref();
    let index = WorkspaceIndex::load(root)?;
    if !index.projects.contains_key(&input.project_id) {
        return Err(AppError::NotFound(format!("project {}", input.project_id)));
    }

    let mut environments = read_environments(root, &input.project_id)?;
    let environment = Environment {
        id: new_id(),
        name: required_name(&input.name, "environment")?,
        base_url: input.base_url,
        variables: input.variables,
    };
    environments.push(environment.clone());
    write_environments(root, &input.project_id, &environments)?;
    Ok(environment)
}

pub fn update_environment(
    root: impl AsRef<Path>,
    input: UpdateEnvironmentInput,
) -> AppResult<Environment> {
    let root = root.as_ref();
    let mut environments = read_environments(root, &input.project_id)?;
    let env = environments
        .iter_mut()
        .find(|e| e.id == input.id)
        .ok_or_else(|| AppError::NotFound(format!("environment {}", input.id)))?;
    env.name = required_name(&input.name, "environment")?;
    env.base_url = input.base_url;
    env.variables = input.variables;
    let updated = env.clone();
    write_environments(root, &input.project_id, &environments)?;
    Ok(updated)
}

pub fn delete_environment(
    root: impl AsRef<Path>,
    input: DeleteEnvironmentInput,
) -> AppResult<()> {
    let root = root.as_ref();
    let mut environments = read_environments(root, &input.project_id)?;
    let original_len = environments.len();
    environments.retain(|e| e.id != input.id);
    if environments.len() == original_len {
        return Err(AppError::NotFound(format!("environment {}", input.id)));
    }
    write_environments(root, &input.project_id, &environments)?;

    // Clear active_environment_id if it was deleted
    let index = WorkspaceIndex::load(root)?;
    let project = index
        .projects
        .get(&input.project_id)
        .ok_or_else(|| AppError::NotFound(format!("project {}", input.project_id)))?;
    if project.metadata.active_environment_id.as_deref() == Some(&input.id) {
        let mut metadata = project.metadata.clone();
        metadata.active_environment_id = None;
        metadata.updated_at = now_iso_string();
        write_json_file(&project.dir.join(METADATA_FILE), &metadata)?;
    }

    Ok(())
}

pub fn list_environments(
    root: impl AsRef<Path>,
    project_id: &str,
) -> AppResult<Vec<Environment>> {
    let root = root.as_ref();
    let index = WorkspaceIndex::load(root)?;
    if !index.projects.contains_key(project_id) {
        return Err(AppError::NotFound(format!("project {}", project_id)));
    }
    read_environments(root, project_id)
}

pub fn set_active_environment(
    root: impl AsRef<Path>,
    input: SetActiveEnvironmentInput,
) -> AppResult<ProjectMetadata> {
    let root = root.as_ref();
    let index = WorkspaceIndex::load(root)?;
    let project = index
        .projects
        .get(&input.project_id)
        .ok_or_else(|| AppError::NotFound(format!("project {}", input.project_id)))?;

    // Validate environment exists if not None
    if let Some(ref env_id) = input.environment_id {
        let environments = read_environments(root, &input.project_id)?;
        if !environments.iter().any(|e| e.id == *env_id) {
            return Err(AppError::NotFound(format!("environment {}", env_id)));
        }
    }

    let mut metadata = project.metadata.clone();
    metadata.active_environment_id = input.environment_id;
    metadata.updated_at = now_iso_string();
    write_json_file(&project.dir.join(METADATA_FILE), &metadata)?;
    Ok(metadata)
}

fn initialize_workspace(root: &Path) -> AppResult<()> {
    let workspace_id = new_id();
    let project_id = new_id();
    let created_at = now_iso_string();
    let workspace_name = workspace_name_from_path(root);
    let project_dir = root.join("default");

    if project_dir.exists() && !project_dir.join(METADATA_FILE).exists() {
        return Err(AppError::Conflict(format!(
            "default project directory already exists without metadata: {}",
            project_dir.display()
        )));
    }

    fs::create_dir_all(project_dir.join(ITEMS_DIR))?;

    let project_metadata = ProjectMetadata {
        schema_version: SCHEMA_VERSION,
        entity_type: EntityType::Project,
        id: project_id.clone(),
        created_at: created_at.clone(),
        updated_at: created_at.clone(),
        slug: "default".to_string(),
        name: "默认项目".to_string(),
        description: "默认项目".to_string(),
        root_order: vec![],
        docs: ProjectDocsConfig::default(),
        mock: ProjectMockConfig::default(),
        active_environment_id: None,
    };
    write_json_file(&project_dir.join(METADATA_FILE), &project_metadata)?;

    let manifest = WorkspaceManifest {
        schema_version: SCHEMA_VERSION,
        workspace_id,
        created_at: created_at.clone(),
        updated_at: created_at,
        name: workspace_name,
        default_project_id: project_id.clone(),
        last_opened_project_id: project_id,
        project_order: vec![project_metadata.id.clone()],
        settings: BTreeMap::new(),
    };
    write_json_file(&root.join(WORKSPACE_FILE), &manifest)?;

    Ok(())
}

fn scan_children(
    project_id: &str,
    directory: &Path,
    stored_order: &[String],
    parent: &ParentRef,
    seen_ids: &mut HashMap<String, String>,
    collections: &mut HashMap<String, CollectionEntry>,
    apis: &mut HashMap<String, ApiEntry>,
) -> AppResult<Vec<TreeNode>> {
    let mut children = vec![];
    let entries = fs::read_dir(directory)?;
    for entry in entries {
        let entry = entry?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if is_hidden(&name) {
            continue;
        }

        let path = entry.path();
        if entry.file_type()?.is_dir() {
            let metadata_path = path.join(METADATA_FILE);
            if !metadata_path.exists() {
                return Err(AppError::InvalidWorkspace(format!(
                    "collection directory {} is missing metadata.json",
                    path.display()
                )));
            }

            let metadata: CollectionMetadata = read_json_file(&metadata_path)?;
            validate_collection_metadata(&metadata)?;
            register_id(seen_ids, &metadata.id, "collection")?;
            let collection_parent = ParentRef::Collection(metadata.id.clone());
            let children_nodes = scan_children(
                project_id,
                &path,
                &metadata.order,
                &collection_parent,
                seen_ids,
                collections,
                apis,
            )?;

            collections.insert(
                metadata.id.clone(),
                CollectionEntry {
                    dir: path.clone(),
                    metadata_path,
                    metadata: metadata.clone(),
                    project_id: project_id.to_string(),
                    parent: parent.clone(),
                },
            );

            children.push(TreeNode::Collection(CollectionTreeNode {
                id: metadata.id.clone(),
                slug: metadata.slug.clone(),
                name: metadata.name.clone(),
                description: metadata.description.clone(),
                created_at: metadata.created_at.clone(),
                updated_at: metadata.updated_at.clone(),
                children: children_nodes,
            }));
            continue;
        }

        if !name.ends_with(".json") || name == METADATA_FILE {
            continue;
        }

        let definition: ApiDefinition = read_json_file(&path)?;
        validate_api_definition(&definition)?;
        register_id(seen_ids, &definition.id, "api")?;
        apis.insert(
            definition.id.clone(),
            ApiEntry {
                path,
                definition: definition.clone(),
                parent: parent.clone(),
            },
        );

        children.push(TreeNode::Api(ApiSummary {
            id: definition.id,
            slug: definition.slug,
            name: definition.name,
            method: definition.method,
            url: definition.url,
            description: definition.description,
            tags: definition.tags,
            created_at: definition.created_at,
            updated_at: definition.updated_at,
        }));
    }

    Ok(sort_children(stored_order, children))
}

fn validate_project_metadata(metadata: &ProjectMetadata) -> AppResult<()> {
    if metadata.schema_version != SCHEMA_VERSION {
        return Err(AppError::InvalidWorkspace(format!(
            "project {} has unsupported schema version {}",
            metadata.id, metadata.schema_version
        )));
    }
    if metadata.entity_type != EntityType::Project {
        return Err(AppError::InvalidWorkspace(format!(
            "project {} has invalid entityType",
            metadata.id
        )));
    }
    if metadata.id.is_empty() {
        return Err(AppError::InvalidWorkspace(
            "project id is empty".to_string(),
        ));
    }
    Ok(())
}

fn validate_collection_metadata(metadata: &CollectionMetadata) -> AppResult<()> {
    if metadata.schema_version != SCHEMA_VERSION {
        return Err(AppError::InvalidWorkspace(format!(
            "collection {} has unsupported schema version {}",
            metadata.id, metadata.schema_version
        )));
    }
    if metadata.entity_type != EntityType::Collection {
        return Err(AppError::InvalidWorkspace(format!(
            "collection {} has invalid entityType",
            metadata.id
        )));
    }
    if metadata.id.is_empty() {
        return Err(AppError::InvalidWorkspace(
            "collection id is empty".to_string(),
        ));
    }
    Ok(())
}

fn validate_api_definition(definition: &ApiDefinition) -> AppResult<()> {
    if definition.schema_version != SCHEMA_VERSION {
        return Err(AppError::InvalidWorkspace(format!(
            "api {} has unsupported schema version {}",
            definition.id, definition.schema_version
        )));
    }
    if definition.entity_type != EntityType::Api {
        return Err(AppError::InvalidWorkspace(format!(
            "api {} has invalid entityType",
            definition.id
        )));
    }
    if definition.id.is_empty() {
        return Err(AppError::InvalidWorkspace("api id is empty".to_string()));
    }
    Ok(())
}

fn sorted_projects(index: &WorkspaceIndex) -> Vec<&ProjectEntry> {
    let mut ordered = vec![];
    let mut remaining = index.projects.values().collect::<Vec<_>>();
    remaining.sort_by(|left, right| left.metadata.name.cmp(&right.metadata.name));

    for project_id in &index.manifest.project_order {
        if let Some(project) = index.projects.get(project_id) {
            ordered.push(project);
        }
    }

    for project in remaining {
        if !ordered
            .iter()
            .any(|existing| existing.metadata.id == project.metadata.id)
        {
            ordered.push(project);
        }
    }

    ordered
}

fn ordered_child_ids(mut stored_order: Vec<String>, children: &[TreeNode]) -> Vec<String> {
    let existing_ids = children
        .iter()
        .map(|child| child.id().to_string())
        .collect::<Vec<_>>();
    stored_order.retain(|id| existing_ids.contains(id));
    merge_order(&existing_ids, &stored_order)
}

fn sort_children(stored_order: &[String], children: Vec<TreeNode>) -> Vec<TreeNode> {
    let mut by_id = children
        .into_iter()
        .map(|node| (node.id().to_string(), node))
        .collect::<HashMap<_, _>>();
    let mut ordered = vec![];

    for id in stored_order {
        if let Some(node) = by_id.remove(id) {
            ordered.push(node);
        }
    }

    let mut leftovers = by_id.into_values().collect::<Vec<_>>();
    leftovers.sort_by(|left, right| left.id().cmp(right.id()));
    ordered.extend(leftovers);
    ordered
}

fn register_id(seen_ids: &mut HashMap<String, String>, id: &str, kind: &str) -> AppResult<()> {
    match seen_ids.insert(id.to_string(), kind.to_string()) {
        Some(existing) => Err(AppError::DuplicateId(format!(
            "{} is already used by {}",
            id, existing
        ))),
        None => Ok(()),
    }
}

fn ensure_project_parent_matches(
    index: &WorkspaceIndex,
    project_id: &str,
    parent_collection_id: Option<&str>,
) -> AppResult<()> {
    if !index.projects.contains_key(project_id) {
        return Err(AppError::NotFound(format!("project {}", project_id)));
    }

    if let Some(collection_id) = parent_collection_id {
        let collection = index
            .collections
            .get(collection_id)
            .ok_or_else(|| AppError::NotFound(format!("collection {}", collection_id)))?;
        if collection.project_id != project_id {
            return Err(AppError::InvalidInput(format!(
                "collection {} does not belong to project {}",
                collection_id, project_id
            )));
        }
    }

    Ok(())
}

fn resolve_parent(
    index: &WorkspaceIndex,
    project_id: &str,
    parent_collection_id: Option<&str>,
) -> AppResult<ParentRef> {
    ensure_project_parent_matches(index, project_id, parent_collection_id)?;
    Ok(match parent_collection_id {
        Some(collection_id) => ParentRef::Collection(collection_id.to_string()),
        None => ParentRef::ProjectRoot(project_id.to_string()),
    })
}

fn parent_directory(index: &WorkspaceIndex, parent: &ParentRef) -> AppResult<PathBuf> {
    match parent {
        ParentRef::ProjectRoot(project_id) => {
            let project = index
                .projects
                .get(project_id)
                .ok_or_else(|| AppError::NotFound(format!("project {}", project_id)))?;
            Ok(project.items_dir.clone())
        }
        ParentRef::Collection(collection_id) => {
            let collection = index
                .collections
                .get(collection_id)
                .ok_or_else(|| AppError::NotFound(format!("collection {}", collection_id)))?;
            Ok(collection.dir.clone())
        }
    }
}

fn child_ids_for_parent(index: &WorkspaceIndex, parent: &ParentRef) -> AppResult<Vec<String>> {
    match parent {
        ParentRef::ProjectRoot(project_id) => {
            let project = index
                .projects
                .get(project_id)
                .ok_or_else(|| AppError::NotFound(format!("project {}", project_id)))?;
            let children = if project.items_dir.exists() {
                scan_children(
                    project_id,
                    &project.items_dir,
                    &project.metadata.root_order,
                    parent,
                    &mut HashMap::new(),
                    &mut HashMap::new(),
                    &mut HashMap::new(),
                )?
            } else {
                vec![]
            };
            Ok(children
                .into_iter()
                .map(|node| node.id().to_string())
                .collect())
        }
        ParentRef::Collection(collection_id) => {
            let collection = index
                .collections
                .get(collection_id)
                .ok_or_else(|| AppError::NotFound(format!("collection {}", collection_id)))?;
            let children = scan_children(
                &collection.project_id,
                &collection.dir,
                &collection.metadata.order,
                parent,
                &mut HashMap::new(),
                &mut HashMap::new(),
                &mut HashMap::new(),
            )?;
            Ok(children
                .into_iter()
                .map(|node| node.id().to_string())
                .collect())
        }
    }
}

fn validate_ordered_ids(existing_ids: &[String], ordered_ids: &[String]) -> AppResult<()> {
    for id in ordered_ids {
        if !existing_ids.contains(id) {
            return Err(AppError::InvalidInput(format!(
                "ordered id {} is not a child of the selected parent",
                id
            )));
        }
    }
    Ok(())
}

fn merge_order(existing_ids: &[String], ordered_ids: &[String]) -> Vec<String> {
    let mut next_order = vec![];
    for id in ordered_ids {
        if !next_order.contains(id) {
            next_order.push(id.clone());
        }
    }
    for id in existing_ids {
        if !next_order.contains(id) {
            next_order.push(id.clone());
        }
    }
    next_order
}

fn append_child_order(index: &WorkspaceIndex, parent: &ParentRef, child_id: &str) -> AppResult<()> {
    let mut order = child_ids_for_parent(index, parent)?;
    order.retain(|id| id != child_id);
    order.push(child_id.to_string());
    set_parent_order(index, parent, order)
}

fn remove_child_order(index: &WorkspaceIndex, parent: &ParentRef, child_id: &str) -> AppResult<()> {
    let mut order = child_ids_for_parent(index, parent)?;
    order.retain(|id| id != child_id);
    set_parent_order(index, parent, order)
}

fn update_move_order(
    root: &Path,
    index: &WorkspaceIndex,
    source_parent: &ParentRef,
    target_parent: &ParentRef,
    node_id: &str,
    position: Option<usize>,
) -> AppResult<()> {
    if same_parent(source_parent, target_parent) {
        let mut order = child_ids_for_parent(index, source_parent)?;
        order.retain(|id| id != node_id);
        insert_id(&mut order, node_id, position);
        return set_parent_order(index, source_parent, order);
    }

    let mut source_order = child_ids_for_parent(index, source_parent)?;
    source_order.retain(|id| id != node_id);
    set_parent_order(index, source_parent, source_order)?;

    let refreshed_index = WorkspaceIndex::load(root)?;
    let mut target_order = child_ids_for_parent(&refreshed_index, target_parent)?;
    target_order.retain(|id| id != node_id);
    insert_id(&mut target_order, node_id, position);
    set_parent_order(&refreshed_index, target_parent, target_order)
}

fn set_parent_order(
    index: &WorkspaceIndex,
    parent: &ParentRef,
    order: Vec<String>,
) -> AppResult<()> {
    match parent {
        ParentRef::ProjectRoot(project_id) => {
            let project = index
                .projects
                .get(project_id)
                .ok_or_else(|| AppError::NotFound(format!("project {}", project_id)))?;
            let mut metadata = project.metadata.clone();
            metadata.root_order = order;
            metadata.updated_at = now_iso_string();
            write_json_file(&project.dir.join(METADATA_FILE), &metadata)
        }
        ParentRef::Collection(collection_id) => {
            let collection = index
                .collections
                .get(collection_id)
                .ok_or_else(|| AppError::NotFound(format!("collection {}", collection_id)))?;
            let mut metadata = collection.metadata.clone();
            metadata.order = order;
            metadata.updated_at = now_iso_string();
            write_json_file(&collection.metadata_path, &metadata)
        }
    }
}

fn same_parent(left: &ParentRef, right: &ParentRef) -> bool {
    match (left, right) {
        (ParentRef::ProjectRoot(left_id), ParentRef::ProjectRoot(right_id)) => left_id == right_id,
        (ParentRef::Collection(left_id), ParentRef::Collection(right_id)) => left_id == right_id,
        _ => false,
    }
}

fn insert_id(order: &mut Vec<String>, node_id: &str, position: Option<usize>) {
    let position = position.unwrap_or(order.len()).min(order.len());
    order.insert(position, node_id.to_string());
}

fn is_descendant_collection(index: &WorkspaceIndex, candidate_id: &str, ancestor_id: &str) -> bool {
    let mut current = Some(candidate_id.to_string());
    while let Some(collection_id) = current {
        if collection_id == ancestor_id {
            return true;
        }
        current = index
            .collections
            .get(&collection_id)
            .and_then(|entry| match &entry.parent {
                ParentRef::Collection(parent_id) => Some(parent_id.clone()),
                ParentRef::ProjectRoot(_) => None,
            });
    }
    false
}

fn api_file_name(id: &str, slug: &str) -> String {
    format!("api_{}__{}.json", id, slug)
}

fn unique_project_slug(root: &Path, input: &str) -> AppResult<String> {
    unique_slug(root, input, "project", true)
}

fn unique_collection_slug(root: &Path, input: &str) -> AppResult<String> {
    unique_slug(root, input, "collection", true)
}

fn unique_slug(
    root: &Path,
    input: &str,
    fallback: &str,
    directories_only: bool,
) -> AppResult<String> {
    let base = slugify(input, fallback);
    let mut slug = base.clone();
    let mut counter = 2;
    loop {
        let candidate = root.join(&slug);
        let exists = if directories_only {
            candidate.exists()
        } else {
            candidate.is_file()
        };
        if !exists {
            return Ok(slug);
        }
        slug = format!("{}-{}", base, counter);
        counter += 1;
    }
}

fn normalized_method(method: &str) -> String {
    let trimmed = method.trim();
    if trimmed.is_empty() {
        "GET".to_string()
    } else {
        trimmed.to_ascii_uppercase()
    }
}

fn normalize_request(mut request: RequestDefinition) -> RequestDefinition {
    for item in request
        .headers
        .iter_mut()
        .chain(request.query.iter_mut())
        .chain(request.path_params.iter_mut())
        .chain(request.cookies.iter_mut())
        .chain(request.body.form_data.iter_mut())
        .chain(request.body.url_encoded.iter_mut())
    {
        if item.id.is_empty() {
            item.id = new_id();
        }
    }
    request
}

fn read_json_file<T: DeserializeOwned>(path: &Path) -> AppResult<T> {
    let content = fs::read_to_string(path)?;
    Ok(serde_json::from_str(&content)?)
}

fn write_json_file<T: Serialize>(path: &Path, value: &T) -> AppResult<()> {
    let parent = path
        .parent()
        .ok_or_else(|| AppError::InvalidInput(format!("path {} has no parent", path.display())))?;
    fs::create_dir_all(parent)?;

    let mut json = serde_json::to_vec_pretty(value)?;
    json.push(b'\n');

    let temp_path = parent.join(format!(
        ".{}.tmp-{}",
        path.file_name()
            .and_then(|file_name| file_name.to_str())
            .ok_or_else(|| AppError::InvalidInput(format!(
                "invalid file name {}",
                path.display()
            )))?,
        new_id()
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

fn workspace_name_from_path(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.trim().is_empty())
        .unwrap_or("Workspace")
        .to_string()
}

fn required_name(input: &str, label: &str) -> AppResult<String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(AppError::InvalidInput(format!(
            "{} name cannot be empty",
            label
        )));
    }
    Ok(trimmed.to_string())
}

fn new_id() -> String {
    Ulid::new().to_string()
}

fn now_iso_string() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true)
}

fn slugify(input: &str, fallback: &str) -> String {
    let mut slug = String::with_capacity(input.len());
    let mut previous_dash = false;

    for character in input.trim().chars() {
        if character.is_ascii_alphanumeric() {
            slug.push(character.to_ascii_lowercase());
            previous_dash = false;
            continue;
        }

        if !previous_dash {
            slug.push('-');
            previous_dash = true;
        }
    }

    let slug = slug.trim_matches('-').to_string();
    if slug.is_empty() {
        fallback.to_string()
    } else {
        slug
    }
}

fn is_hidden(name: &str) -> bool {
    name.starts_with('.')
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::*;

    #[test]
    fn bootstrap_workspace_creates_manifest_and_default_project() {
        let temp_dir = tempdir().expect("create temp dir");
        let snapshot = bootstrap_workspace(temp_dir.path()).expect("bootstrap workspace");

        assert_eq!(snapshot.project_order.len(), 1);
        assert!(temp_dir.path().join(WORKSPACE_FILE).exists());
        assert!(temp_dir.path().join("default").join(METADATA_FILE).exists());
        assert!(temp_dir.path().join("default").join(ITEMS_DIR).exists());
    }

    #[test]
    fn api_file_name_uses_mixed_strategy() {
        let file_name = api_file_name("01HXYZABC", "health-check");
        assert_eq!(file_name, "api_01HXYZABC__health-check.json");
    }

    #[test]
    fn create_and_scan_collection_tree_respects_order() {
        let temp_dir = tempdir().expect("create temp dir");
        let snapshot = bootstrap_workspace(temp_dir.path()).expect("bootstrap workspace");
        let project_id = snapshot.default_project_id;

        let auth = create_collection(
            temp_dir.path(),
            CreateCollectionInput {
                project_id: project_id.clone(),
                parent_collection_id: None,
                name: "Auth".to_string(),
                description: String::new(),
                slug: None,
            },
        )
        .expect("create auth collection");

        let user = create_collection(
            temp_dir.path(),
            CreateCollectionInput {
                project_id: project_id.clone(),
                parent_collection_id: Some(auth.id.clone()),
                name: "User".to_string(),
                description: String::new(),
                slug: None,
            },
        )
        .expect("create user collection");

        let root_api = create_api(
            temp_dir.path(),
            CreateApiInput {
                project_id: project_id.clone(),
                parent_collection_id: None,
                name: "Health".to_string(),
                method: "get".to_string(),
                url: "https://example.com/health".to_string(),
                description: String::new(),
                tags: vec![],
                request: RequestDefinition::default(),
                documentation: ApiDocumentation::default(),
                mock: ApiMock::default(),
                slug: None,
            },
        )
        .expect("create root api");

        reorder_children(
            temp_dir.path(),
            ReorderChildrenInput {
                project_id: Some(project_id.clone()),
                parent_collection_id: None,
                ordered_ids: vec![root_api.id.clone(), auth.id.clone()],
            },
        )
        .expect("reorder root");

        let refreshed = open_workspace(temp_dir.path()).expect("open workspace");
        let project = refreshed.projects.first().expect("project snapshot");
        assert_eq!(project.children.len(), 2);
        assert_eq!(project.children[0].id(), root_api.id);
        assert_eq!(project.children[1].id(), auth.id);

        match &project.children[1] {
            TreeNode::Collection(node) => {
                assert_eq!(node.children.len(), 1);
                assert_eq!(node.children[0].id(), user.id);
            }
            TreeNode::Api(_) => panic!("expected collection node"),
        }
    }

    #[test]
    fn api_update_keeps_file_name_stable() {
        let temp_dir = tempdir().expect("create temp dir");
        let snapshot = bootstrap_workspace(temp_dir.path()).expect("bootstrap workspace");

        let api = create_api(
            temp_dir.path(),
            CreateApiInput {
                project_id: snapshot.default_project_id.clone(),
                parent_collection_id: None,
                name: "Login".to_string(),
                method: "post".to_string(),
                url: "https://example.com/login".to_string(),
                description: String::new(),
                tags: vec![],
                request: RequestDefinition::default(),
                documentation: ApiDocumentation::default(),
                mock: ApiMock::default(),
                slug: None,
            },
        )
        .expect("create api");

        let project_dir = temp_dir.path().join("default").join(ITEMS_DIR);
        let original_path = find_api_path(&project_dir, &api.id);

        let updated = update_api(
            temp_dir.path(),
            UpdateApiInput {
                id: api.id.clone(),
                name: "User Login".to_string(),
                method: "put".to_string(),
                url: "https://example.com/sessions".to_string(),
                description: "renamed".to_string(),
                tags: vec!["auth".to_string()],
                request: RequestDefinition::default(),
                documentation: ApiDocumentation::default(),
                mock: ApiMock::default(),
            },
        )
        .expect("update api");

        let refreshed_path = find_api_path(&project_dir, &updated.id);
        assert_eq!(original_path, refreshed_path);
        assert_eq!(updated.slug, api.slug);
    }

    #[test]
    fn move_node_updates_directory_and_order() {
        let temp_dir = tempdir().expect("create temp dir");
        let snapshot = bootstrap_workspace(temp_dir.path()).expect("bootstrap workspace");
        let project_id = snapshot.default_project_id.clone();

        let source = create_collection(
            temp_dir.path(),
            CreateCollectionInput {
                project_id: project_id.clone(),
                parent_collection_id: None,
                name: "Source".to_string(),
                description: String::new(),
                slug: None,
            },
        )
        .expect("create source");

        let target = create_collection(
            temp_dir.path(),
            CreateCollectionInput {
                project_id: project_id.clone(),
                parent_collection_id: None,
                name: "Target".to_string(),
                description: String::new(),
                slug: None,
            },
        )
        .expect("create target");

        let api = create_api(
            temp_dir.path(),
            CreateApiInput {
                project_id: project_id.clone(),
                parent_collection_id: Some(source.id.clone()),
                name: "Login".to_string(),
                method: "post".to_string(),
                url: "https://example.com/login".to_string(),
                description: String::new(),
                tags: vec![],
                request: RequestDefinition::default(),
                documentation: ApiDocumentation::default(),
                mock: ApiMock::default(),
                slug: None,
            },
        )
        .expect("create api");

        move_node(
            temp_dir.path(),
            MoveNodeInput {
                node_id: api.id.clone(),
                target_project_id: project_id.clone(),
                target_collection_id: Some(target.id.clone()),
                position: Some(0),
            },
        )
        .expect("move api");

        let refreshed = open_workspace(temp_dir.path()).expect("open workspace");
        let project = refreshed.projects.first().expect("project snapshot");
        let target_node = project
            .children
            .iter()
            .find_map(|node| match node {
                TreeNode::Collection(collection) if collection.id == target.id => Some(collection),
                _ => None,
            })
            .expect("target node");

        assert_eq!(target_node.children.len(), 1);
        assert_eq!(target_node.children[0].id(), api.id);
    }

    #[test]
    fn invalid_json_is_reported() {
        let temp_dir = tempdir().expect("create temp dir");
        bootstrap_workspace(temp_dir.path()).expect("bootstrap workspace");
        fs::write(temp_dir.path().join(WORKSPACE_FILE), "{invalid").expect("write invalid file");

        let error = open_workspace(temp_dir.path()).expect_err("workspace should fail");
        assert!(matches!(error, AppError::Json(_)));
    }

    #[test]
    fn duplicate_ids_are_reported() {
        let temp_dir = tempdir().expect("create temp dir");
        let snapshot = bootstrap_workspace(temp_dir.path()).expect("bootstrap workspace");
        let duplicate_id = snapshot.default_project_id.clone();
        let api_path = temp_dir
            .path()
            .join("default")
            .join(ITEMS_DIR)
            .join(api_file_name(&duplicate_id, "duplicate"));

        let definition = ApiDefinition {
            schema_version: SCHEMA_VERSION,
            entity_type: EntityType::Api,
            id: duplicate_id,
            created_at: now_iso_string(),
            updated_at: now_iso_string(),
            slug: "duplicate".to_string(),
            name: "Duplicate".to_string(),
            method: "GET".to_string(),
            url: "https://example.com".to_string(),
            description: String::new(),
            tags: vec![],
            request: RequestDefinition::default(),
            documentation: ApiDocumentation::default(),
            mock: ApiMock::default(),
        };
        write_json_file(&api_path, &definition).expect("write duplicate api");

        let error = open_workspace(temp_dir.path()).expect_err("workspace should fail");
        assert!(matches!(error, AppError::DuplicateId(_)));
    }

    #[cfg(unix)]
    #[test]
    fn atomic_write_failure_keeps_original_file() {
        use std::os::unix::fs::PermissionsExt;

        let temp_dir = tempdir().expect("create temp dir");
        let file_path = temp_dir.path().join("sample.json");
        let original = serde_json::json!({ "ok": true });
        write_json_file(&file_path, &original).expect("seed file");

        let mut permissions = fs::metadata(temp_dir.path())
            .expect("metadata")
            .permissions();
        permissions.set_mode(0o500);
        fs::set_permissions(temp_dir.path(), permissions).expect("set readonly");

        let result = write_json_file(&file_path, &serde_json::json!({ "ok": false }));

        let mut restored_permissions = fs::metadata(temp_dir.path())
            .expect("metadata")
            .permissions();
        restored_permissions.set_mode(0o700);
        fs::set_permissions(temp_dir.path(), restored_permissions).expect("restore permissions");

        assert!(result.is_err());
        let current = fs::read_to_string(&file_path).expect("read original file");
        assert!(current.contains("\"ok\": true"));
    }

    fn find_api_path(root: &Path, api_id: &str) -> PathBuf {
        for entry in fs::read_dir(root).expect("read dir") {
            let entry = entry.expect("entry");
            let path = entry.path();
            if entry.file_type().expect("file type").is_dir() {
                let nested = find_api_path(&path, api_id);
                if nested.exists() {
                    return nested;
                }
                continue;
            }

            if path
                .file_name()
                .and_then(|value| value.to_str())
                .map(|value| value.starts_with(&format!("api_{}__", api_id)))
                .unwrap_or(false)
            {
                return path;
            }
        }

        PathBuf::new()
    }
}
