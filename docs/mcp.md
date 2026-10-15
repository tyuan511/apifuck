# 原生 MCP

当前 Rust / GPUI 桌面应用提供内置 MCP 服务。在侧栏打开 **设置 → MCP**，启用开关后，按页面中的 **Claude Code、Codex、OpenCode、Pi** 教程添加连接。无需 Node、Bun、WebView 或独立 CLI；使用时保持桌面应用运行。

MCP 开关立即生效，不依赖「保存设置」。默认关闭；开启状态持久化，随后随应用启动。关闭服务停止监听并取消 MCP HTTP/SSE、集合运行和 WebSocket 任务；重新开启后，必要时在 Agent 中刷新连接。正在执行的外部请求可能已经产生副作用，取消不会回滚远端数据。

## 接入与设备设置

- 默认地址：`http://127.0.0.1:47831/mcp`，传输为 Streamable HTTP。
- 需要 `Authorization: Bearer <token>` 请求头。设置页按当前地址和令牌生成配置，提供复制按钮和对应官方文档。
- 配置文件是 `~/.apifuck-native/mcp.json`，字段为 `enabled`、`port`、`token`，独立于应用偏好和未提交的设置草稿。
- 仅绑定 IPv4 回环地址，校验 Host、Origin、令牌和协议版本。没有公开网络或浏览器跨域入口，云端 Agent 不能直接连接本机地址。
- 端口冲突会显示错误，开关不会假报成功。关闭占用程序后点击「启动 / 重试」，或退出应用后修改 `port` 再重启。配置文件无效时不会被静默覆盖，修复后重启应用。
- 令牌以随机 UUID 生成，配置通过核心同目录临时文件原子写入（Unix 文件权限 0600）。配置包含访问权限，仅交给可信 Agent。

项目没有自动将旧 Tauri 的 `~/.apifuck/config.json` 迁移为原生设置，也不会读取已退役前端的 MCP 配置。

## 四种 Agent 教程

优先复制应用生成的配置。以下 `YOUR_TOKEN` 仅是占位符，合并时保留已有的其他 MCP 服务。

### Claude Code

```sh
claude mcp add --transport http --scope user apifuck http://127.0.0.1:47831/mcp --header "Authorization: Bearer YOUR_TOKEN"
```

在 Claude Code 中运行 `/mcp` 检查连接。[官方文档](https://code.claude.com/docs/en/mcp)。

### Codex

合并到 `~/.codex/config.toml`，重新打开会话：

```toml
[mcp_servers.apifuck]
url = "http://127.0.0.1:47831/mcp"
http_headers = { Authorization = "Bearer YOUR_TOKEN" }
tool_timeout_sec = 120
```

[官方文档](https://developers.openai.com/codex/mcp)。

### OpenCode

合并到项目 `opencode.json` 后运行 `opencode mcp list`：

```json
{
  "$schema": "https://opencode.ai/config.json",
  "mcp": {
    "apifuck": {
      "type": "remote",
      "url": "http://127.0.0.1:47831/mcp",
      "headers": { "Authorization": "Bearer YOUR_TOKEN" },
      "enabled": true,
      "oauth": false
    }
  }
}
```

[官方文档](https://opencode.ai/docs/mcp-servers/)。

### Pi

使用内置 MCP 的 Pi 版本，旧版先升级。合并到 `~/.pi/agent/mcp.json`；也可放入需要授予项目信任的 `.pi/mcp.json`：

```json
{
  "mcpServers": {
    "apifuck": {
      "url": "http://127.0.0.1:47831/mcp",
      "headers": { "Authorization": "Bearer YOUR_TOKEN" },
      "timeout": 120
    }
  }
}
```

运行 `pi mcp list`。已有会话执行 `/reload`，再用 `/mcp` 检查。[官方文档](https://github.com/earendil-works/pi/blob/main/packages/coding-agent/docs/mcp.md)。

## 工具与工作区语义

先调用 `get_workspace` 查看当前项目、文档相对路径和环境。工具完整参数通过 `tools/list` 提供，服务端在执行前按 JSON Schema 校验。

| 范围 | 工具 |
| --- | --- |
| 项目 | `get_workspace`、`list_projects`、`open_project`、`create_project` |
| 请求、目录、环境 | `read_document`、`create_request`、`create_folder`、`create_environment` |
| 文档编辑 | `update_document`、`save_document_source`、`duplicate_request`、`move_document`、`trash_document`、`reorder_document` |
| 环境选择与预览 | `set_environment`、`preview_variables` |
| HTTP、GraphQL、SSE | `send_request`、`get_job`、`cancel_job` |
| WebSocket | `connect_websocket`、`send_websocket_message`、`get_job`、`cancel_job` |
| 导入导出 | `import_curl`、`import_document`、`convert_bru`、`export_request` |
| 核心集合运行器 | `run_collection`、`get_job`、`cancel_job` |

工具操作已保存的 OpenCollection YAML。`read_document` 返回 `source` 与解析后的 `value`：

- `update_document` 提交完整 `value`，通过 `editing::update` 保留未编辑节点的注释和格式；可编辑名称、请求体、认证、参数、脚本、Tests、Docs、项目/目录/环境变量等核心支持的字段。
- `save_document_source` 提交完整 YAML。两种保存以及移动/回收都需要上一轮读取的 `expectedSource`，发现外部修改即拒绝覆盖；核心仍执行 schema 校验及原子存储。
- 修改显示名不会改文件名，只有 `move_document` 明确改变路径。回收文档使用 `.apifuck/trash/`，不支持删除根集合。
- 新建环境不会激活它，只有 `set_environment` 改变桌面选择。请求的 `environment` 省略时使用桌面选择，显式 `null` 不选择环境。
- 写入、移动、回收和切换项目时，如果工作区有未保存草稿或原生/MCP 请求仍在运行，会返回错误。MCP 操作通过 GPUI 工作区调度，成功后刷新树、环境和干净的已打开文档；不会用 MCP 数据覆盖手动草稿。
- 文件路径限制在共享项目内，拒绝 `..`、绝对文档路径和隐藏应用状态路径。项目打开/创建的 `path` 则指向用户指定的项目目录。

## 执行与限制

HTTP、GraphQL、SSE、WebSocket 和集合运行均复用原生核心，不另建请求实现。继承、URL/Query、认证、QuickJS、断言、Chai/AJV 和本机环境变量写回遵循 [执行规则](execution.md)。`preview_variables` 和代码导出不执行脚本。`convert_bru` 只在内存转换；转换结果可用 `import_document` 保存。cURL 导入只解析文本，不运行 shell。

发送/连接/集合运行返回任务 `id`，通过 `get_job` 读取状态、响应、测试结果和最近事件；`cancel_job` 取消请求或断开 WebSocket。最多保留 32 个任务，达到上限按完成时间淘汰最早结束的任务（包括成功、失败和取消），不按随机任务 ID 排序，也不淘汰仍在运行的任务；32 个任务全部运行中时，新任务会被拒绝。结果并非永久保留，客户端应及时读取。每个请求保留最近 100 个事件，超大事件文本截断，完整 HTTP 响应受核心网络设置的大小上限控制。任务与连接不跨应用重启保存。

协议支持 `2025-03-26`、`2025-06-18`、`2025-11-25`，使用无会话 JSON 响应模式；GET/DELETE 返回 405，不提供旧 SSE 传输。普通工具等待上限 120 秒，超时不意味着写入回滚，重试前先读取工作区核对。

## 验证

`crates/core/tests/mcp.rs` 使用临时项目、临时配置与真实回环 HTTP/WebSocket 服务，验证 MCP 握手/鉴权、启停/端口冲突、YAML CRUD/冲突保护、HTTP/GraphQL/SSE、脚本环境写回、WebSocket 与取消、四种配置格式。桌面编译和本地化测试覆盖原生设置集成。

这些检查不等于 GPUI 视觉验收或四个 Agent 的实际客户端联调。人工验收步骤见 [工作区交互](postman-layout.md)。
