<p align="center">
  <img src="assets/apifuck.png" width="96" height="96" alt="ApiFuck Logo" />
</p>

# ApiFuck

基于 Rust 与 GPUI Kit 的原生桌面 API 调试工具。项目使用 OpenCollection YAML 存储，无需登录；请求、环境和目录配置可以纳入 Git，草稿与设备设置保存在本机。

当前仓库采用三个 Rust crate：`desktop` 负责原生界面，`core` 负责存储与请求执行，`demo` 提供本地 Playground。桌面资源、Bruno 转换器和脚本依赖编译嵌入，应用运行不需要 Node.js 或 Bun。

## 下载

原生桌面程序通过 [GitHub Releases](https://github.com/tyuan511/apifuck/releases) 分发。当前发布流程生成 macOS Apple Silicon、Linux x86_64 和 Windows x86_64 的独立可执行文件，附件名为 `apifuck-desktop-<target>`，Windows 以 `.exe` 结尾。

<!-- release-downloads:start -->
| 平台 | 最新稳定版 |
| --- | --- |
| macOS Apple Silicon | [下载](https://github.com/tyuan511/apifuck/releases/latest/download/apifuck-desktop-aarch64-apple-darwin.zip) |
| Windows x86_64 | [下载](https://github.com/tyuan511/apifuck/releases/latest/download/apifuck-desktop-x86_64-pc-windows-msvc.exe) |
| Linux x86_64 | [下载](https://github.com/tyuan511/apifuck/releases/latest/download/apifuck-desktop-x86_64-unknown-linux-gnu) |

首个原生版本发布后以上直链生效；历史版本见 [Releases](https://github.com/tyuan511/apifuck/releases)。
<!-- release-downloads:end -->

工作区版本以 [Cargo.toml](Cargo.toml) 为准；仓库代码不代表已经发布。历史发布说明保留在 [docs/release](docs/release)，其中旧版本的技术栈与安装包说明不适用于当前原生架构。

## 快速开始

源码运行需要 Rust 1.95；Demo 启动器另需 Node.js 22+。Cargo 会读取仓库根目录的 `rust-toolchain.toml`。Linux 系统依赖见 [开发指南](docs/development.md)。

```sh
# 启动本地 API、准备可编辑项目并打开桌面客户端
cargo demo

# 仅运行回环集成测试，不打开桌面客户端
cargo demo --check

# 直接打开已有 OpenCollection 项目
cargo run --locked -p apifuck-desktop -- examples/demo
```

完整 [Playground](examples/playground/README.md) 包含 32 个预填请求，覆盖认证、CRUD、上传、GraphQL、SSE、WebSocket 和预期错误。`cargo demo` 在 `.demo-workspace/` 中保留编辑，`--fresh` 先备份再重建，`--headless` 仅启动本地服务和准备工作区。

## 主要功能

| 范围 | 当前能力 |
| --- | --- |
| 项目与导航 | 新建/打开项目、最近 10 个项目、集合树与独立环境侧栏、搜索、标签与导航定位联动 |
| 文档管理 | 直接新建 HTTP/GraphQL/WebSocket 请求；目录与环境按显示名称创建；复制、移动、重命名、回收文档及同级排序 |
| 工作区 | 标签拖拽重排、批量关闭、长标题列表；默认上下请求/响应分栏，可切换左右布局 |
| 请求编辑 | Query/Path、Headers、Vars、Assert 表格，认证表单，多种 Body 模式，GraphQL Query/Variables，Script、Tests、Docs 和 YAML Source |
| 变量 | 环境继承、项目/目录/请求变量、Base URL、嵌套模板；输入框 badge、当前值 tooltip 和不撑开布局的浮动补全 |
| 编辑辅助 | 多语言语法高亮、后台格式化、内置 LSP 语法诊断；脚本 API 离线补全与悬停说明 |
| 协议与响应 | HTTP、GraphQL、SSE 增量响应与取消、WebSocket 收发；状态、耗时、大小、响应头、正文、时间线和测试结果 |
| 脚本与测试 | QuickJS 同步前后置脚本、Chai、AJV JSON Schema、声明式断言、响应变量提取和本机环境写回 |
| 导入导出 | 地址栏粘贴 cURL、Bru 请求/环境/目录转换、cURL/JavaScript/Python/Go 代码预览与复制 |
| 本机体验 | 原始输入草稿恢复、简体中文/English、浅色/深色/跟随系统、六种强调色、网络与代理设置、应用内更新 |

顶部项目菜单提供打开项目、最近打开、新建项目和导入 `.bru`。集合级变量、认证等配置通过集合侧栏标题旁的设置按钮打开；顶部菜单不再提供“项目设置”和“删除项目”。应用偏好设置位于左侧“设置”页面。

编辑环境与启用环境是两个操作：点击环境列表打开文档，右上角选择器决定请求使用的环境。变量预览使用当前静态配置，不执行前置脚本。

## 保存与数据

```text
project/
├── opencollection.yml       # 集合元数据与项目默认配置
├── environments/local.yml  # 环境变量与继承
├── users/
│   ├── folder.yml           # 目录默认配置
│   └── get-users.yml        # 请求文档
└── .apifuck/                # 本机草稿、变量覆盖、回收文档；不加入 Git
```

输入变化自动保存到本机草稿，包括未完成或无效内容；点击保存才写入共享 YAML。关闭标签、切换编辑模式和重启可恢复草稿。保存会校验文档、保留未修改区域的注释与格式，并拒绝覆盖外部改动。

语言、主题、网络偏好及最近项目保存在 `~/.apifuck-native/`。旧版 JSON 项目没有自动迁移层。格式、文件身份和持久化规则见 [存储说明](docs/storage.md)。

## 快捷键

macOS 使用 Cmd，其余平台使用 Ctrl。

| 操作 | 快捷键 |
| --- | --- |
| 打开项目 | Cmd/Ctrl+O |
| 新建请求 | Cmd/Ctrl+N |
| 保存当前文档或设置 | Cmd/Ctrl+S |
| 发送 / 停止请求 | Cmd/Ctrl+Enter |
| 关闭当前标签；设置页返回工作区 | Cmd/Ctrl+W |
| 退出 | Cmd/Ctrl+Q |
| 代码编辑器手动补全 | Ctrl+Space |

## 当前边界

Bruno 导入和部分脚本 API 兼容不等于完整 Bruno 客户端兼容。当前未提供 gRPC、高级认证（OAuth、AWS、Digest 等）、完整异步脚本/Node 模块、外部 Secret、dotenv、mTLS、Cookie jar、持久化请求历史或多项目并列工作区。详见 [执行规则](docs/execution.md) 与 [兼容性记录](docs/bruno-parity.md)。

集合运行器仅在核心库和无界面测试中使用，桌面入口尚未提供。原生 MCP 接入仍是规划；仓库没有独立 CLI 产品。Demo 命令是开发辅助工具。

## 开发与文档

| 文档 | 内容 |
| --- | --- |
| [架构说明](docs/architecture.md) | crate 边界、模块职责、编辑/执行/诊断数据流 |
| [工作区交互](docs/postman-layout.md) | 导航、表单、变量提示、设置及人工验收 |
| [存储说明](docs/storage.md) | OpenCollection、草稿、环境覆盖与原子写入 |
| [执行与脚本](docs/execution.md) | 继承、模板、脚本 API、扩展及执行限制 |
| [开发与验证](docs/development.md) | 工具链、构建、Demo、CI 与非 GUI 检查 |
| [更新与发布](docs/auto-update.md) | 版本、发布门禁、附件校验、应用内更新 |
| [AGENTS.md](AGENTS.md) | 仓库协作与实现约定 |

```sh
cargo test --workspace --locked
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo build --workspace --locked
node --test scripts/release/*.test.mjs
```

仓库禁止使用 Computer Use 验证。自动测试、构建和静态检查不等于实际视觉/交互验收；需要人工确认的流程集中在 [工作区交互](docs/postman-layout.md)。

## 发布与自动更新

推送 `v<版本号>` tag 后，GitHub Actions 检查并构建 macOS、Linux 和 Windows 程序，全部成功后发布 GitHub Release。发布准备使用 `$release-tag-publish` skill，包含代码检查、版本生成、发布说明与 CI 结果核对；详见 [发布流程](docs/auto-update.md)。

正式构建支持静默检查更新；设置里的更新面板展示新版本说明及下载进度，侧栏同步显示更新入口。下载完成后可稍后处理，点击“重启并安装”会先保存草稿。开发构建不会自动检查更新。
