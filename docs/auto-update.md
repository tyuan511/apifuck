# 发布与自动更新

版本以根目录 `Cargo.toml` 的 `[workspace.package].version` 为准，所有 crate 继承此版本，`Cargo.lock` 一起提交。推送 `v<semver>` tag 会触发 `.github/workflows/release.yml`；仅修改版本号或推送 main 不会发布。

## 发布流程

使用仓库 skill `$release-tag-publish`，例如“发布下一个 patch 版本”或“发布 0.1.0”。它先检查代码和远端版本，再编写发布说明、同步版本、提交并推送 main、创建带注释的 tag，最后核对 CI 和公开 Release。只要求准备发布文件时不会自动推送。

版本生成辅助命令（只打印，不写文件；先 fetch 远端 tags）：

```sh
git fetch origin main --tags
git ls-remote --tags origin
node scripts/release/next-version.mjs auto
node scripts/release/next-version.mjs patch
# 也支持 minor、major、具体版本号
```

`auto` 优先保留尚未发布且高于已有 tag 的 workspace 版本，否则生成下一稳定版本。写好 `docs/release/<version>.md` 后执行：

```sh
node scripts/release/sync-version.mjs 0.1.0 --verify-release-notes
node scripts/release/validate-release.mjs v0.1.0
cargo fmt --all --check
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo build --release --locked -p apifuck-desktop
node --test scripts/release/*.test.mjs
```

版本同步失败会还原 Cargo.toml 和 Cargo.lock。检查通过、发布说明与最终变更一致后，将发布提交推送至 main，再创建并推送对应 annotated tag。不要覆盖已有公开版本或移动已有 tag。

## GitHub Actions

1. **Validate**：tag 必须与 workspace 版本一致，锁文件有效，每个 workspace crate 版本相同，且存在非空的版本说明；运行发布脚本测试。
2. **Build**：macOS Apple Silicon、Linux x86_64、Windows x86_64 分别运行格式、测试和 Clippy 检查，再用明确的 Rust target 构建 Release 二进制。macOS 封装 `ApiFuck.app`（版本、图标及最低系统版本取自本次构建），使用临时 keychain 导入 Developer ID Application 证书并启用 Hardened Runtime 和可信时间戳签名。先公证应用包，给 `.app` 装订并验证票据，再生成含 Applications 快捷方式的 DMG；DMG 也须签名、公证并装订票据。全部成功才生成最终校验值。打包只调用命令行工具，不运行 Finder 或 UI 脚本。
3. **Publish**：只有三个构建全部成功才进入。验证完整附件及哈希，创建或继续草稿 Release，只上传 DMG、EXE、AppImage 与 `SHA256SUMS`，再次核对 GitHub 附件的大小和 digest 后公开。已公开版本拒绝覆盖；失败时不公开不完整的草稿。

`v0.2.0-beta.1` 一类 tag 会发布为 prerelease，不会成为 latest，客户端也不会自动安装。晚发布的旧稳定版本不会替换更高版本的 latest。暂时性失败可通过 Actions 重跑，或手动运行 Release workflow 并指定原 tag；源码有修复时使用新的版本和 tag。

GitHub 发布权限使用自动提供的 `GITHUB_TOKEN`，只有 Publish job 有 `contents: write`；不需要 GitHub 个人 token。macOS 签名与公证另外需要下述 Apple 凭据。工作流从 tag checkout，不修改 main，README 使用稳定的 latest 下载链接。分支保护不会阻止 Release job 写 README，因为它不会写 README。

## 发布附件

| 平台 | 唯一安装附件 |
| --- | --- |
| macOS Apple Silicon | `ApiFuck-<version>-arm64.dmg` |
| Windows x86_64 | `ApiFuck-<version>.exe` |
| Linux x86_64 | `ApiFuck-<version>.AppImage` |

文件名的 `<version>` 来自 Cargo 工作区版本，不含 tag 的 `v` 前缀；例如 `ApiFuck-0.1.2-arm64.dmg`、`ApiFuck-0.1.2.exe`、`ApiFuck-0.1.2.AppImage`。参考 electron-builder 的 DMG、portable 和 AppImage 默认规则及 GitHub 安全文件名处理：arm64 保留架构后缀，默认 x64 省略；Windows 是便携 EXE，文件名中的空格转换为连字符。发布前按目标 tag 校验文件名版本，不能混用其他版本的产物；Actions 中间 artifact 名也带 tag。

新命名从后续构建生效，不重命名已公开附件。官网、下载链接辅助脚本和新客户端优先使用版本化名称，同时兼容旧的 `apifuck-desktop-<target>` 安装附件。已发布的旧客户端只识别旧名称，因此命名切换后的首个版本需要手动下载一次。README 的长期下载入口使用 latest Release 页面，避免版本变化后链接失效。

命名依据：[electron-builder 平台命名](https://github.com/electron-userland/electron-builder/blob/master/packages/app-builder-lib/src/platformPackager.ts)、[NSIS / portable 命名](https://github.com/electron-userland/electron-builder/blob/master/packages/app-builder-lib/src/targets/win/nsis/NsisTarget.ts)。

macOS DMG 包含 `ApiFuck.app` 和 Applications 快捷方式；应用和 DMG 均签名、公证并装订票据。Linux 使用固定版本及 SHA-256 校验的 linuxdeploy 打包共享库，检查 AppImage 头、解包目录与动态库解析，不启动 GUI。CI 构建基于 Ubuntu 24.04，AppImage 不保证能在更旧的 glibc 系统运行。

`SHA256SUMS` 覆盖三个安装附件；原始中间二进制留在构建目录，macOS 不额外上传裸程序、ZIP 或更新归档，Linux 不上传裸程序或 DEB。GitHub 自动生成的 Source code 链接不是构建附件。

从 **v0.1.2** 起使用版本化文件名；旧客户端需手动下载一次以迁移到新命名。每次常规发布使用新的版本号，更新器不会安装相同版本。当前没有 Intel Mac、Linux ARM64 附件。

本地无需签名凭据即可验证结构（输出目录必须尚不存在）：

```sh
cargo build --release --locked -p apifuck-desktop
node scripts/release/macos-package.mjs target/release/apifuck-desktop target/macos-package-check
node --test scripts/release/macos-package.test.mjs
```

本地产物未签名、未公证，只用于包结构验证。测试使用临时 Mach-O 程序，验证 plist、图标、权限并通过 `hdiutil` 只读挂载检查 DMG 内容，不会启动程序或打开 Finder；macOS CI 会运行这项测试。

## macOS 签名凭据

ApiFuck 的代码签名标识为 `dev.fastvibe.apifuck`。

在仓库的 Settings → Secrets and variables → Actions 添加以下 **Repository secrets**，名称与 FastVibe 保持一致：

| Secret | 内容 |
| --- | --- |
| `APPLE_CERT_P12` | Developer ID Application 证书及其私钥导出为 `.p12` 后的 Base64；不是 Apple Development 或 Developer ID Installer 证书 |
| `APPLE_CERT_PASSWORD` | 导出 `.p12` 时设置的非空密码 |
| `APPLE_API_KEY_P8` | App Store Connect **Team API key** 的完整 `.p8` PEM 文本，不是 Base64，也不是本地路径 |
| `APPLE_API_KEY_ID` | 该 API key 的 Key ID |
| `APPLE_API_ISSUER` | 该 Team API key 的 Issuer ID（UUID） |

Apple 账号需已开通 Developer Program，证书有效且包含私钥，API key 有公证权限。P12 内必须恰好有一个有效 Developer ID Application 签名身份，脚本会自动选取，无需另填签名身份或 Team ID。当前脚本使用 Team API key；Individual API key 的认证参数不同，不适用这份配置。

使用 GitHub CLI 设置凭据的示例（文件保留在本地，不提交进 Git）：

```sh
base64 -i DeveloperIDApplication.p12 > certificate.p12.base64
gh secret set APPLE_CERT_P12 --repo tyuan511/apifuck < certificate.p12.base64
gh secret set APPLE_CERT_PASSWORD --repo tyuan511/apifuck
gh secret set APPLE_API_KEY_P8 --repo tyuan511/apifuck < AuthKey_YOUR_KEY_ID.p8
gh secret set APPLE_API_KEY_ID --repo tyuan511/apifuck
gh secret set APPLE_API_ISSUER --repo tyuan511/apifuck
```

不带输入重定向的命令会交互式询问值。旧 `TAURI_SIGNING_PRIVATE_KEY` 是旧更新协议的签名密钥，不能替代 Apple Developer ID 证书或公证 API key。

流程会在编译前检查凭据是否齐全，缺少任何项都停止 macOS 发布，不自动降级为无签名产物。私钥文件只写入 runner 临时目录（文件权限 0600），临时 keychain 使用随机密码；正常完成、失败及取消后的 always-cleanup 步骤都会清理文件并恢复 keychain 搜索列表。

应用提交和 DMG 提交各自的公证等待上限为 30 分钟，整个打包签名步骤上限 75 分钟。拒绝或未完成都会阻止 Release；能取得请求 ID 时尝试下载 Apple 诊断日志，失败工作流的 `macos-notarization-diagnostics` artifact 保留 7 天。凭据文件不进入任何 artifact。超时后 Apple 可能仍在处理，请先查看该请求状态；不要把未完成当成通过。

旧 Tauri 版使用另一套更新协议；首次切换到原生版需要手动下载，本自动更新链路用于原生版本之间升级。

## 更新交互

更新流程参考 FastVibe 本地项目的 `use-app-update`、`sidebar-update-button` 和 `update-dialog`：侧栏和设置中的面板使用同一个更新状态。

- Release 构建在启动 3 秒后静默检查，随后每 10 分钟检查；“关于与更新”里的“自动检查更新”可关闭，保存设置后生效。开发构建不自动检查更新，仍可手动检查。
- 后台检查不会打开弹窗、自动下载或中断当前操作；发现更新后侧栏显示“更新”。无新版本或网络失败时保持安静，已有可用版本不会被后台失败清掉。
- 手动“检查更新”打开一个更新面板，展示检查状态、新版本说明、“稍后”和“立即更新”。关闭面板不会取消下载，重新打开会继续显示相同进度。
- 下载开始先显示 spinner，收到字节后显示百分比与已下载/总大小；侧栏同步显示进度。
- 超过 60 秒的旧提醒在下载前复查。如果版本变化或撤回，先展示最新状态，重新等待确认，不悄悄替换用户已确认的版本。
- 下载并校验完成后显示“重启并安装”，可以继续工作；只有用户点击才保存所有草稿、校验并安装、再次保存最新草稿并重启。保存失败会阻止安装/重启并显示 toast。
- 重复点击不会创建并行下载或并行安装。网络、校验、权限等失败使用统一 toast，并可在面板重试。下载缓存在当前进程拥有的临时文件里；退出且未选择安装时会清理，下次需要重新下载。

## 校验与安装

更新元数据来自本仓库 GitHub Release。跳过草稿、预发布、同版及旧版，只选择当前架构的精确附件名；下载 URL 必须精确对应此仓库、此 tag 和此附件。SHA-256 digest 和声明大小缺失或非法时拒绝更新。

下载通过 HTTPS 流式写入临时文件，限制大小并校验 SHA-256；失败自动清理文件，避免一次性把整个附件放入内存。安装前再次校验。Windows 使用 `self-replace`。macOS 应用只选择 DMG：先校验镜像哈希，只读挂载且禁止自动打开，检查应用签名和版本，再在本地生成私有临时归档交给安装库；发布页不需要该归档。安装前卸载镜像，所有失败路径清理挂载。Linux 只选择 AppImage：根据 APPIMAGE/APPDIR 和当前进程路径定位外部文件，校验 ELF/AppImage 头后更新外部镜像，不覆盖只读挂载目录中的程序。整包安装复用固定版本 `self_update 1.3.0`：通过自定义 release source 和仅能读取已下载文件的 transport 交付用户已确认的版本，不重新访问网络或切换版本。库在安装目录同一文件系统解包、备份旧包并整体替换，替换失败时尝试回退；回退属于尽力恢复，不宣称断电或磁盘损坏下的强事务保证。安装前校验新包必须满足已安装应用的 designated code-signing requirement，并匹配应用 ID、可执行文件名及已确认版本，拒绝不可信签名和被篡改资源。现有“稍后”和草稿保存流程继续有效，重启目标为整个 `.app` 或外部 AppImage 文件。只读目录或在 macOS 磁盘映像中运行时会提示移至可写目录。SHA-256 不是独立数字签名，信任来源仍是 HTTPS 和仓库的发布权限。

协议依据：[GitHub Releases REST API](https://docs.github.com/en/rest/releases/releases)、[GitHub CLI Release](https://cli.github.com/manual/gh_release_edit)。

## 验证边界

本地自动化测试使用合成 GitHub 元数据、回环 HTTP 服务、模拟 gh 和模拟 Apple 命令，验证版本选择、进度/大小/哈希检查、临时文件清理、两阶段公证及票据装订失败门禁、最终哈希和完整附件要求。macOS 另用真实命令行工具验证临时应用的签名与资源篡改检测，以及 DMG 结构；通过 `self_update` 对临时目录执行整包更新，验证资源替换及拒绝错误更新时旧包不变。不会启动 GUI、发布真实 Release 或替换当前运行的程序。状态测试验证下载中的状态不会被检查覆盖，以及过期版本需要重新确认。

真实 Apple 签名与公证只有在上述凭据配置后才能验证；本地模拟测试不代表 Apple 已接受该程序。实际 GitHub 托管 runner、目标平台窗口和安装后重启需要发布候选版本验证。手工验收：从正式 DMG 将 ApiFuck.app 拖入 Applications，双击确认正常启动；再查看设置中的更新说明，开始下载后关闭并重开面板确认进度连续，下载完成选择“稍后”继续编辑，再点击“重启并安装”，核对新版本和草稿恢复；网络失败及只读安装位置应显示 toast。遵守仓库规定，不以 Computer Use 验证。

实现参考：[self_update 整包更新](https://docs.rs/self_update/1.3.0/self_update/)、[Tauri 更新归档及签名](https://tauri.app/plugin/updater/)。Tauri 插件需要 AppHandle，因此这里使用独立 Rust 库，不引入 Tauri/WebView。
