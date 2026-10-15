# ApiFuck 官网

独立静态官网，以蓝青极光动画首屏、特点轮换打字标题、浮动导航和真实产品预览开场，四段左右交错的图文讲述 Rust 原生、本地与 Git、多种请求协议、AI Native / MCP。原生、Git、协议、MCP 区块分别使用蓝、薄荷绿、紫和玫瑰色渐变，明暗主题各有配色；下载区使用蓝绿渐变。中文 `/`，英文 `/en/`，支持系统主题和手动主题切换。它与 Rust 桌面应用没有构建或运行依赖。

首屏动画采用 [React Bits Aurora](https://reactbits.dev/backgrounds/aurora) 的原生 WebGL2 shader，蓝、青、靛紫波纹位于标题附近，流动光带轻微跟随鼠标；另有两条 CSS 光带横向漂移，在无 WebGL 时仍提供可见动效。只集成该效果，无 React、OGL 或其他运行依赖；完整第三方许可与固定源码来源保留在 `public/third-party/`。WebGL 绘制限制为最多 30fps、1440px 宽及约 100 万像素，不按高 DPI 放大；离屏、后台和手动暂停时停止持续绘制，减少动态效果时保留静态帧。WebGL 不可用或初始化失败时回退到 CSS 光带与渐变，上下文恢复后可重新绘制。

首屏使用用户提供的真实桌面截图 `public/workbench-screenshot.png`（2560 × 1680），原图保持不变，在中英文和明暗主题下等比例完整显示，点击可查看原图。构建时给样式、脚本、截图及 Agent 图标 URL 加内容版本，避免浏览器或 CDN 将新页面与旧资源混用。产品能力以根目录 README、`docs/execution.md`、`docs/storage.md` 与 `docs/mcp.md` 为准，不宣传独立 CLI 或尚未实现的协议。

- **原生架构**：对比 Rust + GPUI 与 Electron 的组成，不使用未经同机测量的内存、速度数字。页面链接到 Electron 官方文档及 Bruno、Insomnia 的官方源码。
- **本地与 Git**：展示 YAML 文件、可读 diff 与分支提交动画。Git 图是外部工作流示意，不表示应用内置提交/同步功能；`.apifuck/` 本机状态需忽略。
- **多种协议**：HTTP、GraphQL、SSE、WebSocket 标签可用键盘切换，重放按钮只运行本地示例动画，不发送网络请求。
- **AI Native**：展示 Claude Code、Codex、OpenCode 与 Pi 通过 MCP 连接工作台。图标来源与归属见 `public/agents/SOURCES.md`；Pi 使用文字 π，不声称官方认证或四端实测。

首屏以“原生 API 工作台”为固定标题，轮换 Rust 性能、本地/Git、多协议与 MCP 四句特点描述，先删字再逐字输入。所有句子预留相同区域，切换不推动下方按钮与截图；读屏使用完整静态描述，无 JavaScript 时展示完整首句。全局暂停会补全当前句，减少动态效果时固定展示首句，离开首屏或切到后台暂停计时。

滚动进入时两列交错移动，示意动画仅在视口内播放。可手动暂停；离屏、后台标签页和 `prefers-reduced-motion` 下停止动画。正文不会因脚本未运行而隐藏；无 JavaScript 时展示全部协议的静态示例。

## 开发与验证

Node.js 22+；生成站点本身无需安装依赖。只有浏览器检查需要 Playwright。

```sh
npm --prefix website ci
npm --prefix website run build
npm --prefix website run dev  # http://127.0.0.1:9092
npm --prefix website run check
npx --prefix website playwright install chromium
npm --prefix website run test:browser
# 只检查极光渲染与生命周期
npm --prefix website run test:aurora
```

可设置 `PORT` 更改本地预览端口；测试使用 `WEBSITE_URL`，`CHROME_PATH` 可指定已有 Chrome 的可执行文件。浏览器验证仅使用 headless Playwright 的 DOM、布局和交互断言，不使用截图或桌面级 Computer Use，不验证 GPUI 原生应用。

- `src/content.mjs`：中英文文案。
- `scripts/build.mjs`：两种语言的完整静态 HTML、SEO 和站点地图。
- `src/styles.css`：共享响应式布局与主题。
- `src/feature-sections.mjs`：构建时生成四段图文、SVG 与可访问示例。
- `src/features.css`：交错布局、示意图与动画。
- `src/site.js`：导航、键盘标签页、请求演示、视口动画管理和下载状态。
- `src/hero-aurora.js`：React Bits Aurora shader 的原生 WebGL2 适配与渲染生命周期。
- `src/releases.mjs`：只接受当前原生架构的精确资产名、非空文件和 GitHub 仓库下载 URL。
- `dist/`：生成物，忽略提交，由 Docker 构建时生成。

下载区读取 GitHub 最新稳定发布。旧 Tauri 安装器不会冒充当前原生构建；无匹配文件、网络失败或 API 限流时保留发布页和源码入口。未添加遥测、账号或后台服务。无 JavaScript 时仍可读取正文、FAQ、语言页面和发布链接。

极光专项检查在 headless WebGL2 中验证 shader 编译/链接、实际绘制调用、帧率和像素预算、暂停/离屏/后台停帧、主题与尺寸重绘、context loss/restore 以及不可用时的 CSS 动效回退；只记录 API 调用与状态，不截图或读取像素。

浏览器检查覆盖双语 × 明暗主题 × 320/375/768/1024/1440px、图标加载、两列交错/手机堆叠、标题轮换与尺寸稳定、请求重放与键盘切换、减少动态效果、手动暂停/恢复、离屏暂停、无 JS 内容，以及下载成功/回退。

手动验收可访问预览/线上页面，查看首屏极光流动、鼠标轻微跟随与四句打字轮换，确认标题对比度，再从原生段滚动到 MCP 段，检查渐变背景、交错移动节奏、图文视觉比例、字体和真实截图展示；切换协议重放示例并查看 Agent 图标。自动 DOM 验证不代表像素级审美或 GPUI 原生桌面验收。

## 自动部署

域名：`https://apifuck.fastvibe.dev`。服务器：us02。

沿用 FastVibe 的 **GitHub Actions → GHCR → SSH → Docker Compose** 流程：

1. `main` 上的 `website/**` 或 `.github/workflows/website.yml` 变动触发验证；PR 只验证。
2. 两种语言、主题、五档屏宽、交互、下载回退检查通过后构建 Linux amd64 镜像。
3. 推送 `ghcr.io/tyuan511/apifuck/website:<commit-sha>`。
4. 部署到独立 `/opt/apifuck-website`，Compose 项目名 `apifuck-website`，只绑定 `127.0.0.1:9092`。
5. 容器健康检查、英文页和 `/health.json` 的精确 commit 校验通过后保存 `.env` 的版本。容器部署失败会恢复已有版本；最后再次检查公共 HTTPS 入口。

需要仓库 Secrets：

- `DEPLOY_SSH_KEY`：us02 上的部署密钥。
- `SSH_KNOWN_HOSTS`：通过已信任 SSH 会话核实的主机公钥。

GHCR 登录使用本次工作流的 `GITHUB_TOKEN`，不需要长期 PAT。镜像只用 commit SHA，无不确定的 latest 标签。服务器上 `.env` 保存最后成功的 `IMAGE_TAG`，可显式指定之前版本回滚。

`deploy/edge.http.conf` 是首次 HTTP 站点模板。先安装为 `/etc/nginx/conf.d/apifuck.fastvibe.dev.conf`，通过 `nginx -t` 后 reload，使用 Certbot webroot `/var/www/letsencrypt` 为该域名单独签发证书；然后替换为 `deploy/edge.conf` 的 HTTPS 配置并再次检查和 reload。Certbot 管理证书续签，日常工作流不覆盖宿主机 nginx 或其他网站。

```sh
# 本地验证与流水线相同的镜像
# 如果直接部署，请为 SITE_REVISION 提供真实 Git commit SHA。
docker build -t apifuck-website:check website
IMAGE_TAG=local docker compose -f website/deploy/docker-compose.yml config
```

官网构建不需要 Cargo、Bun、Next.js 或 npm 生产依赖。原生程序的 release 工作流保持独立。
