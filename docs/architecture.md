# 原生架构

ApiFuck 是根目录的 Rust workspace，三个成员共享版本和锁文件。桌面使用 GPUI Kit 0.7.3；网络执行使用 Tokio，脚本和 Bru 转换在嵌入的 QuickJS 中完成。没有 WebView 前端或运行时 Node 服务。

## 依赖边界

```text
GPUI desktop ───────────────┐
                           v
                    apifuck-core
                           ^
Demo 集成测试 / 原生 MCP ───┘

apifuck-demo → 回环 HTTP / GraphQL / SSE / WebSocket 服务
cargo demo → Rust launch example → Node 启动器 → Demo + desktop
```

`core` 不依赖 GPUI。`demo` 是开发和测试服务，不是桌面的生产后端；桌面直接通过核心发送请求。MCP 的 HTTP 传输和工具实现在核心的 `mcp/`，通过通道将操作交给 GPUI 工作区调度，复用相同存储和执行 API。

## 核心模块

路径相对于 `crates/core/src/`。

| 模块 | 职责 |
| --- | --- |
| `collection.rs` | 文档类型、schema 校验、项目扫描、生成文件身份、增删改/移动/排序、环境与目录作用域 |
| `editing.rs` | 通过 YAML 语法树修改指定区域，保留未修改内容 |
| `settings.rs` | 工作区草稿、应用偏好、最近项目、本机环境覆盖和变量写入锁 |
| `url_editor.rs` | URL 与 Query 模型、Base URL、相对地址和主机规范化 |
| `engine.rs` | prepare/execute、静态变量预览、模板展开、继承、认证、正文、HTTP/GraphQL/SSE |
| `websocket.rs` | WebSocket 连接、发送、接收与关闭 |
| `scripts.rs`、`testing.rs` | QuickJS Hook、脚本报告、Chai/AJV、声明式断言和响应变量提取 |
| `runner.rs` | 顺序集合运行、迭代、延迟、取消、失败停止、跨请求运行变量 |
| `bru.rs` | 嵌入式 Bru/OpenCollection 转换 |
| `exchange.rs`、`codegen.rs` | cURL 导入、代码导出，复用执行准备与 URL 解析 |
| `mcp/` | 本机鉴权 HTTP MCP、工具与任务、四种 Agent 配置；不依赖 GPUI |
| `update.rs` | Release 筛选、版本比较、发布说明、下载校验和程序替换 |

主要公共入口从 `lib.rs` 导出：`Collection`、`Document`、`prepare`、`execute`、`Event`、`Response` 和 `NetworkSettings`。各专用模块提供更细的 API。

## 桌面模块

路径相对于 `crates/desktop/src/`。

| 模块 | 职责 |
| --- | --- |
| `main.rs` | Tokio runtime、资源注册、窗口、平台菜单和快捷键 |
| `workbench.rs` | 项目与环境导航、集合树、标签管理、侧栏联动、草稿持久化和设置/更新接入 |
| `document_tab.rs` | 单文档草稿、编辑分区、URL、发送/取消和响应状态 |
| `request_form.rs`、`key_value_editor.rs` | 认证、正文、GraphQL、参数/变量/请求头/断言等表单与表格 |
| `variable_input.rs` | 普通输入框的变量 badge、值提示、浮动候选面板，以及 JSON 编辑器的变量高亮与悬停 |
| `response_view.rs`、`export_view.rs` | 响应与流消息、测试报告、代码预览 |
| `settings_view.rs`、`network_editor.rs` | 应用设置分类、设置草稿、网络参数编辑 |
| `mcp_view.rs` | 原生设置中的 MCP 即时开关、状态、四种教程及配置复制 |
| `update_view.rs` | 更新检查、下载进度、重试与安装阶段的界面状态 |
| `languages.rs`、`syntax.rs` | 格式化、语法解析、高亮与模板范围 |
| `language_server.rs`、`editor_tools.rs` | 内置 LSP、版本化诊断、编辑器生命周期和后台格式化 |
| `script_completion.rs` | 当前 Hook 阶段的脚本 API 补全与悬停说明 |
| `appearance.rs`、`surfaces.rs` | 主题/强调色、共享面板和工具栏 |
| `brand.rs`、`icons.rs`、`i18n.rs`、`feedback.rs` | 嵌入资源、图标、双语文案和通知呈现 |
| `tab_drag.rs` | 文档标签拖拽数据 |

## 编辑和持久化

1. `Collection::open` 加载项目和文档，核心校验 OpenCollection 格式。
2. 文档标签从原文初始化编辑状态，再恢复对应本机草稿；表单草稿保留原始输入和不同模式的输入。
3. 输入变化触发工作区草稿保存。共享 YAML 和本机草稿分别持久化，草稿允许不完整内容。
4. 显式保存时生成文档，校验并比较磁盘原文，再原子替换；结构化修改保留其他 YAML 区域。
5. 切换/关闭/恢复标签同步导航和草稿，不能让关闭后台标签改变当前请求，也不能把环境文档的编辑操作当成启用环境。

存储位置和外部修改规则见 [storage.md](storage.md)。

## 请求和变量

桌面把文档交给核心准备并执行。核心合并环境、本机覆盖和项目/目录/请求配置，执行前置脚本、展开变量、规范化 URL，再建立网络请求。响应事件回到文档标签和响应面板；结束后完成响应脚本、断言、测试及允许的环境写回。

代码导出、集合运行器和 MCP 请求复用核心规则。静态 tooltip 使用 `engine::preview_variables` 与 `interpolate`，不会运行脚本；不能将脚本运行后的值伪装为静态预览。完整语义见 [execution.md](execution.md)。

## 编辑器服务

结构化输入和代码编辑器分开实现。普通输入框的变量候选是浮动面板；代码编辑器由 GPUI 编辑状态承载，使用内置 LSP 消息传递语法诊断，并提供脚本 API 补全。

语言服务在应用进程内后台运行，支持打开、修改、关闭文档与版本化诊断。输入停止约 250ms 后提交检查，语言切换和撤销也触发更新。格式化在后台完成；结果应用前检查内容/语言是否仍匹配。诊断只分析语法，不执行脚本、不提供完整类型检查或 GraphQL schema 验证。

## 资源、测试和分发

`assets/` 中的品牌素材、Lucide 图标以及 `crates/core/resources/javascript/` 下的转换器和运行时 bundle 编译嵌入。修改 JavaScript bundle 才需要 Bun；普通 Rust 构建和应用运行无需 JavaScript 工具链。

核心与 Demo 集成测试通过回环服务验证行为；桌面单元测试验证草稿、导航、变量、语言服务等模型。CI 配置覆盖 macOS/Linux/Windows。发布输出独立可执行文件，不包含签名安装器。

开发命令见 [development.md](development.md)，发布门禁与校验见 [auto-update.md](auto-update.md)。

MCP 接入和工具范围见 [mcp.md](mcp.md)。
