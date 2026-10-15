# ApiFuck Playground

32 个已经填写好的请求，全部访问本机服务，无需账号、外网 API 或手动准备上传文件。

## 一条命令开始

需要 Rust 1.95 和 Node.js 22+。在仓库根目录运行：

```sh
cargo demo
```

启动器会：

1. 编译原生客户端和 Demo 服务。
2. 在空闲回环端口启动 API，自动填入 HTTP / WebSocket 地址。
3. 把本目录复制为可编辑的 `.demo-workspace/`，自动选择 `local` 环境。
4. 首次创建工作区时打开健康检查、用户列表、JSON 正文、GraphQL 四个标签页；后续启动恢复已保存的标签。

先发送“服务健康检查”，预期状态 **200**，Tests 中所有断言通过。

后续启动保留你的请求编辑和工作台布局。需要从头测试时运行：

```sh
cargo demo --fresh
```

原工作区会先改名为带时间戳的备份，不会直接删除。

## 场景

| 目录 | 可以测试什么 | 预期 |
|---|---|---|
| 开始使用 | 健康检查、项目/目录继承请求头、Trace ID、JSON Schema | 200 |
| 认证 | 登录写回 Token、Bearer、Basic、Header/Query API Key、错误 Token | 正常 200；错误 Token 401 |
| 用户工作流 | 列表分页、创建、路径参数、更新、删除、响应变量传递 | 200 / 201 / 204 |
| 参数与请求头 | 中文、空格、重复 Query、禁用项、Path 参数 | 200 |
| 正文与文件 | 嵌套 JSON、URL 编码表单、Multipart、二进制、XML、多行文本 | 200；上传 201 |
| GraphQL | 真实 Schema、字段选择、Query 和 Variables | 200 |
| 实时连接 | SSE 增量事件、WebSocket 欢迎消息和 JSON 回声 | 200 / 已连接 |
| 响应与边界 | 404、422、500、302 重定向、慢响应取消 | 错误码是预期；Tests 仍通过 |
| 工具 | 重置服务内存数据与 created_user_id | 200 |

用户工作流按 **创建 → 查看 → 更新 → 删除** 顺序。`created_user_id` 会由创建请求自动更新；初始值 3 对应可直接查看的示例用户。删除后再次测试，请先创建，或发送“恢复初始 Demo 数据”。

SSE 默认约 3 秒收到 8 条事件。慢响应请求等待 3 秒，可点击 Cancel 验证取消。WebSocket 连接后先收到欢迎消息，消息输入框已填写 JSON，点击 Send message 会收到原样回声。

## 预填认证

这些值仅用于本地 Demo：

- 登录：`demo@apifuck.dev` / `demo123`
- Basic：`demo` / `demo123`
- Bearer：`demo-token-apifuck`
- API Key：`af_demo_local_2026`

Token 已预置，因此可单独测试认证请求；登录请求也会通过响应提取更新 Token。

上传文件 `assets/apifuck.png` 与现有应用 logo 一致，`assets/sample.bin` 是固定的 256 字节样本。文件路径相对请求文件解析，不需要改成本机绝对路径。

## 无界面验证

```sh
cargo demo --check
```

它运行 Rust 集成测试，直接调用核心引擎创建临时工作区、校验 YAML，顺序执行 31 个 HTTP/GraphQL 请求与断言，并验证 WebSocket 回声。不会启动桌面界面。Runner 中 WebSocket 标记为 skipped，因为它是交互式长连接。

只启动 API 和预配置工作区：

```sh
cargo demo --headless
```

终端会打印工作区路径，可以在客户端中打开它。按 Ctrl+C 关闭本地服务。

如果单独启动固定端口服务：

```sh
cargo run -p apifuck-demo
```

默认 HTTP 地址 `http://127.0.0.1:8787`，WebSocket 地址 `ws://127.0.0.1:8787/socket`。直接打开本目录并选择 `local` 即可使用。

## 界面操作提示

顶部选择器的 `local` 是请求启用环境；在左侧点击其他环境文档仅打开编辑器，不会切换请求环境。集合级配置从集合标题旁的设置按钮打开。

URL 和表格中的 `{{变量}}` 在非编辑状态显示为 badge，悬停可查看当前解析值。输入 `{{` 会打开浮动候选面板，点击候选完成引用，不会撑高表格行。前置脚本生成的 `trace_id` 可能与静态预览不同。

使用自动验证时只运行 `cargo demo --check`；`cargo demo` 会启动桌面，留给开发者手动操作。人工验收及限制见 [工作区交互](../../docs/postman-layout.md)，其他命令见 [开发指南](../../docs/development.md)。
