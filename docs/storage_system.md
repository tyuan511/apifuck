# 基于工作空间文件夹的接口存储方案

## Summary
采用“工作空间根清单 + 项目目录 + 集合目录树 + 单接口单文件”的存储模型，目标是可读、可 diff、可原子写入，并且后续能直接接 Git、S3、WebDAV 同步。

建议的磁盘结构如下：

```text
<workspace>/
  workspace.json
  default/
    metadata.json
    items/
      api_01HXYZ...__health-check.json
      auth/
        metadata.json
        api_01HXYZ...__login.json
        user/
          metadata.json
          api_01HXYZ...__get-profile.json
  project-b/
    metadata.json
    items/
      ...
```

约定：
- 一个工作空间就是一个根文件夹。
- 一个项目就是工作空间下的一个目录，默认初始化 `default/`。
- 项目目录名使用稳定 `slug`，显示名称改动时默认不改目录名。
- collection 用目录表示，目录内必须有 `metadata.json`。
- API 用单独 JSON 文件表示，文件名采用混合策略：`api_<ulid>__<slug>.json`。
- 文件名中的 `slug` 只在创建时生成，后续接口改名默认不重命名文件；真正身份只认 JSON 内部 `id`。

## Implementation Changes
### 1. JSON 模型
所有 JSON 文件统一包含：
- `schemaVersion: 1`
- `id: string`
- `createdAt: string`
- `updatedAt: string`

`workspace.json`
- `workspaceId`
- `name`
- `defaultProjectId`
- `lastOpenedProjectId`
- `projectOrder: string[]`
- `settings: {}`

项目 `metadata.json`
- `entityType: "project"`
- `id`
- `slug`
- `name`
- `description`
- `rootOrder: string[]`
- `docs: { enabled, basePath, info }`
- `mock: { enabled, baseUrl }`

collection `metadata.json`
- `entityType: "collection"`
- `id`
- `slug`
- `name`
- `description`
- `order: string[]`

API JSON
- `entityType: "api"`
- `id`
- `slug`
- `name`
- `method`
- `url`
- `description`
- `tags: string[]`
- `request`
- `documentation`
- `mock`

`request` 固定结构：
- `headers: KeyValue[]`
- `query: KeyValue[]`
- `pathParams: KeyValue[]`
- `cookies: KeyValue[]`
- `auth: AuthConfig`
- `body: BodySpec`

`KeyValue`
- `id`
- `key`
- `value`
- `enabled`
- `description`

`BodySpec`
- `mode: "none" | "raw" | "json" | "form-data" | "x-www-form-urlencoded" | "binary"`
- `raw`
- `json`
- `formData: KeyValue[]`
- `urlEncoded: KeyValue[]`
- `binary: { filePath?: string }`

`documentation`
- `summary`
- `description`
- `deprecated`
- `operationId`
- `groupName`

`mock`
- `enabled`
- `status`
- `latencyMs`
- `headers: KeyValue[]`
- `body`
- `contentType`

### 2. 路径与排序规则
- 工作空间启动时先读取 `workspace.json`；不存在则创建 `workspace.json` 和 `default/` 项目。
- 项目内容统一放在项目目录下的 `items/`。
- `rootOrder` 管项目顶层 collection/api 顺序。
- `collection.metadata.order` 管当前 collection 子项顺序。
- 排序数组只存子节点 `id`，真实文件位置通过扫描 `items/` 和子目录 `metadata.json` 反查。
- UI 渲染顺序优先按 `order`，缺失项追加到末尾，避免旧数据升级时丢节点。

### 3. Rust 存储层与 Tauri IPC
在 [src-tauri/src/lib.rs](/Users/yuantang/code/fuckapi/src-tauri/src/lib.rs) 下拆出 `storage` 模块，职责固定为：
- 路径解析与 slug/文件名生成
- JSON 读写与 schema 校验
- 工作空间初始化
- 项目/collection/API 的 CRUD
- reorder / move 操作
- 原子写入

Rust 命令统一返回 `Result<T, AppError>`，首批命令定为：
- `open_workspace(path) -> WorkspaceSnapshot`
- `bootstrap_workspace(path) -> WorkspaceSnapshot`
- `create_project(input) -> ProjectSummary`
- `update_project(input) -> ProjectSummary`
- `create_collection(input) -> CollectionSummary`
- `update_collection(input) -> CollectionSummary`
- `create_api(input) -> ApiDefinition`
- `update_api(input) -> ApiDefinition`
- `delete_node(input) -> ()`
- `move_node(input) -> ()`
- `reorder_children(input) -> ()`
- `read_api(id) -> ApiDefinition`

前端在 [src/app.tsx](/Users/yuantang/code/fuckapi/src/app.tsx) 后续接一层 workspace store，所有页面状态都以 `WorkspaceSnapshot` 为单一来源，不直接拼文件路径。

### 4. 同步友好约束
- 所有实体身份只认 `id`，不认路径。
- JSON 使用 UTF-8、2 空格缩进、末尾换行，字段顺序固定。
- 写入采用“临时文件 + rename”原子替换。
- 不在 v1 落盘请求历史、最近响应、运行态 UI 状态。
- 不在 v1 存环境变量和鉴权模板库；这些作为 v2 独立文件补充，避免首版模型过重。

## Test Plan
- Rust 单元测试覆盖：
  - 新工作空间初始化会生成 `workspace.json` 和 `default/metadata.json`
  - collection 树扫描与 `order` 解析正确
  - API 文件名生成符合 `api_<ulid>__<slug>.json`
  - 改接口名不会触发文件重命名
  - move / reorder 后目录结构和排序元数据一致
  - 非法 JSON、缺失 metadata、重复 id 能返回明确错误
  - 原子写入失败时不会留下半写入状态
- 手工验证：
  - Tauri 前端创建项目、集合、API 后重启仍可完整恢复
  - 删除、移动、排序后界面顺序和磁盘内容一致
  - Git diff 只出现实际改动文件，没有整库噪音

## Assumptions
- 工作空间路径由后续“选择文件夹”流程提供；本方案只定义选定后的存储模型，不包含首屏选目录交互。
- `default/` 是默认项目目录名，且首次初始化一定创建。
- 项目目录名和 collection 目录名使用稳定 `slug`，显示名修改不自动改路径。
- v1 目标是“核心定义可持久化并可同步”，不是完整运行态快照系统。
