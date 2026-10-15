# 开发与验证

所有命令从仓库根目录执行。架构入口见 [architecture.md](architecture.md)，协作规则见 [AGENTS.md](../AGENTS.md)。

## 工具链

| 工具 | 用途 |
| --- | --- |
| Rust 1.95、rustfmt、Clippy | 由 `rust-toolchain.toml` 固定；构建三个 workspace 成员 |
| GPUI Kit 0.7.3 | 桌面依赖固定在 `crates/desktop/Cargo.toml`，由 Cargo 获取 |
| Node.js 22+ | Demo 启动器、示例生成和发布辅助脚本 |
| Bun | 仅安装或重建 `scripts/javascript/` 的开发依赖与 bundle |

普通 `cargo build` 和已构建的桌面程序不需要 Node.js/Bun。不要按历史 Tauri/React 版本的说明寻找根目录 `package.json` 或前端开发服务器。

开发和测试配置单独以 `opt-level = 2` 编译 `rquickjs-sys` 的 QuickJS 引擎。未优化的 MSVC 构建加载嵌入式 Bru 解析器会触发初始化超时；依赖级优化保留应用代码的开发配置和现有脚本执行时限。Cargo 的 `test` 配置继承这项 `dev` 覆盖，普通本地测试与 CI 使用同一设置。

Linux CI 使用以下系统库：

```sh
sudo apt-get update
sudo apt-get install clang libasound2-dev libxkbcommon-x11-dev libwayland-dev libx11-xcb-dev libvulkan-dev libssl-dev pkg-config libfontconfig1-dev libudev-dev
```

## 本地运行

以下桌面命令用于开发者手动运行，不属于 Agent 自动验证流程。

```sh
cargo run --locked -p apifuck-desktop -- examples/demo
cargo demo
cargo demo --headless
cargo build --release --locked -p apifuck-desktop
```

`cargo demo` 的入口是 `.cargo/config.toml` 中的 alias：先运行 `crates/demo/examples/launch.rs`，再转发给 `node scripts/demo.mjs`，因此仍需 Node.js。

| Demo 参数 | 行为 |
| --- | --- |
| 无参数 | 构建服务和桌面，复制/复用 `.demo-workspace/`，选择 local 并启动桌面 |
| `--headless` | 只构建/启动回环 API 和准备工作区；Ctrl+C 停止服务 |
| `--check` | 直接运行 `apifuck-demo` 的 playground 集成测试，结束后退出，不启动桌面 |
| `--fresh` | 将原工作区改名为时间戳备份，再复制默认 Playground |
| `--no-build` | 复用已有 `target/debug/` 二进制；调用者需确保已构建 |

`--check` 优先于启动流程，不会创建持久的 `.demo-workspace/`。示例场景见 [Playground](../examples/playground/README.md)。

## 自动检查

```sh
cargo test --workspace --locked
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo build --workspace --locked

node --check scripts/demo.mjs
node --check scripts/generate-demo.mjs
node --check scripts/javascript/build.mjs
node --test scripts/release/*.test.mjs
cargo demo --check
```

开发时可先跑相关目标，例如：

```sh
cargo test --locked -p apifuck-desktop variable_input
cargo test --locked -p apifuck-core --test variable_preview
cargo test --locked -p apifuck-demo --test playground
```

工作区测试已经包含 Playground 集成测试；`cargo demo --check` 额外验证启动器的无界面转发入口。测试数量随代码演进，不在文档中固定总数。

CI 的 `rust` job 在 macOS 14、Ubuntu 24.04、Windows 2022 上执行格式、测试、Clippy 和构建；`scripts` job 在 Ubuntu 上检查 Node 脚本、发布测试及 headless Demo。配置在 [.github/workflows/ci.yml](../.github/workflows/ci.yml)。配置覆盖某平台不代表某次运行已经通过该平台验证。

## 更新嵌入依赖与示例

运行必需的 bundle 和许可证保存在 `crates/core/resources/javascript/`，普通 Cargo 构建离线读取这些资源，不下载 Bruno 源码。开发入口与锁文件位于 `scripts/javascript/`；需要重建时，在该目录运行 `bun install --frozen-lockfile --ignore-scripts` 和 `bun run build`。重建需要 Git 和网络，会临时获取固定提交的 Bruno 转换源码、应用作用域修补，并在结束后清理源码。审查 bundle 和许可变更，并运行 Bru fixture 往返及脚本测试。

`scripts/generate-demo.mjs` 是重建已提交 Playground YAML/资源的开发工具，依赖 `scripts/javascript/` 中安装的 `yaml` 包。仅在需要更新默认示例时运行；运行后审查 `examples/playground/` 差异并执行 Playground 测试，避免覆盖手工调整。

## 验证边界

仓库禁止 Computer Use：不通过桌面窗口、截图、键鼠模拟、辅助功能 API 或 AppleScript 验证。允许测试、格式检查、静态分析、构建、CLI、日志、HTTP 和结构化 API。针对 Web 界面的非桌面 Playwright 可以使用，但不能替代 GPUI 原生界面的检查。

文档修改应核对本地链接、文件路径、命令及源码事实；代码修改应运行相应自动测试。报告必须区分已验证结果和未验证的视觉/交互行为。

人工验收清单集中在 [工作区交互](postman-layout.md)，包括浮动候选面板、变量 tooltip、焦点、输入法、标签导航、草稿恢复、分栏和明暗主题。真实发布升级则需按 [更新与发布](auto-update.md) 在目标平台单独验收。
