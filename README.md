<p align="center">
  <img src="https://github.com/tyuan511/apifuck/raw/main/src-tauri/icons/128x128@2x.png" width="96" height="96" alt="ApiFuck Logo" />
</p>

<h1 align="center">ApiFuck</h1>

<p align="center">
  <strong>轻量、极速、本地优先的桌面 API 调试工具。</strong>
</p>

<p align="center">
  不用登录，不依赖云端，不把接口数据锁进黑盒。
</p>

<p align="center">
  基于 Tauri v2、React 与 Rust 构建，适合希望把接口调试、项目管理和 Git 工作流放在同一条线上完成的团队。
</p>

<p align="center">
  <a href="https://github.com/tyuan511/apifuck/releases/latest"><img src="https://img.shields.io/github/v/release/tyuan511/apifuck?style=flat-square&label=Release" alt="Latest Release" /></a>
  <a href="https://github.com/tyuan511/apifuck/releases/latest"><img src="https://img.shields.io/github/downloads/tyuan511/apifuck/total?style=flat-square&label=Downloads" alt="Downloads" /></a>
  <a href="https://github.com/tyuan511/apifuck/actions/workflows/release.yml"><img src="https://img.shields.io/github/actions/workflow/status/tyuan511/apifuck/release.yml?style=flat-square&label=Release" alt="Release Workflow" /></a>
  <a href="./LICENSE"><img src="https://img.shields.io/badge/license-MIT-green.svg?style=flat-square" alt="MIT License" /></a>
</p>

<p align="center">
  <a href="#下载">下载</a> &bull;
  <a href="#为什么是-apifuck">为什么是 ApiFuck</a> &bull;
  <a href="#功能亮点">功能亮点</a> &bull;
  <a href="#技术栈">技术栈</a> &bull;
  <a href="#本地开发">本地开发</a> &bull;
  <a href="#如何贡献">如何贡献</a>
</p>

<p align="center">
  <a href="./README_en.md">English</a>
</p>

---

## 下载

[![GitHub Release](https://img.shields.io/github/v/release/tyuan511/apifuck?include_prereleases&label=%E6%9C%80%E6%96%B0%E7%89%88%E6%9C%AC)](https://github.com/tyuan511/apifuck/releases/latest)

<!-- release-downloads:start -->
| 平台 | 下载地址 |
|------|---------|
| macOS | [ApiFuck_0.0.8_aarch64.dmg](https://github.com/tyuan511/apifuck/releases/download/v0.0.8/ApiFuck_0.0.8_aarch64.dmg) |
| Windows | [ApiFuck_0.0.8_x64-setup.exe](https://github.com/tyuan511/apifuck/releases/download/v0.0.8/ApiFuck_0.0.8_x64-setup.exe) |
| Linux | [ApiFuck_0.0.8_amd64.AppImage](https://github.com/tyuan511/apifuck/releases/download/v0.0.8/ApiFuck_0.0.8_amd64.AppImage) |
<!-- release-downloads:end -->

> 桌面端内置自动更新机制，发布新版本后会主动提示升级。

<details>
<summary><strong>从源码构建</strong></summary>

```bash
bun install
bun run tauri build
```

</details>

## 为什么是 ApiFuck

| 维度 | 常见云端/重量级 API 工具 | **ApiFuck** |
|---|---|---|
| 数据归属 | 数据常被绑定到账号、工作区或私有格式 | **本地优先，基于文件夹和 JSON 存储** |
| 启动与体积 | 常见 Electron 方案更重 | **Tauri v2 + Rust，启动快、占用低** |
| Git 协作 | 导出、同步、比对不够直接 | **内容可读，可备份，可直接纳入 Git** |
| 桌面体验 | 跨平台一致，但系统融合感一般 | **原生窗口体验，更贴近系统桌面** |
| 扩展请求配置 | 多层继承通常不够直观 | **项目、目录、请求三级配置链路清晰** |

## 功能亮点

**项目化管理**  
支持项目、集合、请求三级结构，适合按业务域、服务边界和接口模块组织请求。

**多标签页工作台**  
多个请求可以同时打开、切换、重排与恢复，适合并行调试、响应对照和参数试验。

**完整请求构建能力**  
覆盖 HTTP 方法、URL、Query、Headers、Auth、Body，满足日常 REST API 调试所需。

**环境变量与 Base URL 继承**  
支持多环境切换，并在项目、目录、请求三层之间解析 Base URL 与变量替换。

**响应查看体验**  
可查看状态码、耗时、大小、响应头与响应体，支持 JSON 格式化与语法高亮。

**流式响应支持**  
支持 `text/event-stream`，适合调试大模型输出、日志流和增量事件。

**请求脚本**  
支持 Pre-request Script 和 Post-request Script，可读写环境变量与脚本变量。

**请求导出**  
支持将当前请求复制为 `cURL`、`JavaScript`、`Python`、`Go` 代码，方便分享和复现。

**自动更新**  
通过 GitHub Releases 分发新版本，桌面端可感知更新并提示升级。

## 核心能力

| 分类 | 能力 |
|---|---|
| 请求编辑 | HTTP 方法、URL、Query、Headers、Auth、Body |
| 认证方式 | `None` / `Basic` / `Bearer` / `API Key` |
| 响应查看 | 状态码、耗时、大小、响应头、JSON 高亮 |
| 环境管理 | 多环境配置、变量替换、快速切换 |
| 工作流 | 多标签页、自动恢复、项目化结构 |
| 扩展能力 | 请求脚本、请求导出、自动更新 |

## 技术栈

| 层级 | 技术 |
|------|------|
| 桌面容器 | Tauri v2 |
| 前端框架 | React 19 + TypeScript |
| 构建工具 | Vite |
| UI 组件 | shadcn/ui + Tailwind CSS v4 |
| 状态管理 | Zustand |
| 代码编辑器 | Monaco Editor |
| HTTP 执行 | Rust reqwest |

### 项目结构

```text
apifuck/
├── src/                      # React 前端
│   ├── app.tsx               # 主应用入口
│   ├── main.tsx              # React 引导
│   ├── components/ui/        # 通用 UI 组件
│   ├── features/workbench/   # 主工作台功能
│   └── lib/                  # 前端公共逻辑
├── src-tauri/                # Tauri / Rust 后端
│   ├── src/
│   ├── capabilities/
│   └── tauri.conf.json
├── docs/                     # 文档与 Release Notes
├── scripts/                  # 发布辅助脚本
└── AGENTS.md                 # 协作约定
```

## 本地开发

### 环境要求

- Node.js 18+
- Bun
- Rust 1.70+

### 快速开始

```bash
# 安装依赖
bun install

# 启动前端开发模式
bun run dev

# 启动桌面开发模式
bun run tauri dev
```

### 生产构建

```bash
bun run build
bun run tauri build
```

### 常用命令

| 命令 | 说明 |
|------|------|
| `bun install` | 安装依赖 |
| `bun run dev` | 启动 Vite 开发服务器 |
| `bun run build` | TypeScript 检查并构建前端 |
| `bun run lint` | 运行 ESLint |
| `bun run lint:fix` | 自动修复 ESLint 问题 |
| `bun run tauri dev` | 启动桌面开发环境 |
| `bun run tauri build` | 构建桌面应用 |
| `cd src-tauri && cargo test` | 运行 Rust 测试 |

## 如何贡献

欢迎提交 Issue 或 Pull Request。

在开始贡献前，建议先阅读：

- [AGENTS.md](./AGENTS.md) — 团队协作约定
- [docs/auto-update.md](./docs/auto-update.md) — 自动更新机制说明

提交前建议至少执行：

```bash
bun run lint
bun run build
```

如果涉及 Rust 逻辑，再执行：

```bash
cd src-tauri && cargo test
```

## 许可证

本项目使用 [MIT License](./LICENSE)。
