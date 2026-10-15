# ApiFuck 官网

独立静态官网，参考 FastVibe Services 的蓝紫首屏、浮动毛玻璃导航和大幅产品预览。中文 `/`，英文 `/en/`，支持系统主题和手动主题切换。它与 Rust 桌面应用没有构建或运行依赖。

首屏是标注为示例数据的 HTML 工作台示意，不是原生应用截图；发送按钮只演示本地状态变化，不发送网络请求。产品能力以根目录 README 与 `docs/execution.md` 为准，未宣传 CLI、MCP 或尚未实现的协议。

## 开发与验证

Node.js 22+；生成站点本身无需安装依赖。只有浏览器检查需要 Playwright。

```sh
npm --prefix website ci
npm --prefix website run build
npm --prefix website run dev  # http://127.0.0.1:9092
npm --prefix website run check
npx --prefix website playwright install chromium
npm --prefix website run test:browser
```

可设置 `PORT` 更改本地预览端口；测试使用 `WEBSITE_URL`，`CHROME_PATH` 可指定已有 Chrome 的可执行文件。浏览器验证仅使用 headless Playwright 的 DOM、布局和交互断言，不使用截图或桌面级 Computer Use，不验证 GPUI 原生应用。

- `src/content.mjs`：中英文文案。
- `scripts/build.mjs`：两种语言的完整静态 HTML、SEO 和站点地图。
- `src/styles.css`：响应式布局与主题。
- `src/site.js`：导航、键盘标签页、本地示例和下载状态。
- `src/releases.mjs`：只接受当前原生架构的精确资产名、非空文件和 GitHub 仓库下载 URL。
- `dist/`：生成物，忽略提交，由 Docker 构建时生成。

下载区读取 GitHub 最新稳定发布。旧 Tauri 安装器不会冒充当前原生构建；无匹配文件、网络失败或 API 限流时保留发布页和源码入口。未添加遥测、账号或后台服务。无 JavaScript 时仍可读取正文、FAQ、语言页面和发布链接。

手动验收可访问预览/线上页面，检查整体视觉、字体和工作台示意是否符合期望；自动 DOM 验证不代表像素或原生桌面验收。

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
