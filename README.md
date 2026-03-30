# ApiFuck

<p align="center">
  <img src="https://github.com/tyuan511/apifuck/raw/main/src-tauri/icons/128x128@2x.png" width="80" alt="ApiFuck Logo" />
</p>

<p align="center">
  <strong>ApiFuck</strong> · 轻量、极速、本地优先的 API 调试工具
</p>

<p align="center">
  <a href="https://github.com/tyuan511/apifuck/releases/latest">
    <img src="https://img.shields.io/github/v/release/tyuan511/apifuck?label=%E4%B8%8B%E8%BD%BD" alt="Download" />
  </a>
  <a href="https://github.com/tyuan511/apifuck/releases/latest">
    <img src="https://img.shields.io/github/downloads/tyuan511/apifuck/total?label=%E4%B8%8B%E8%BD%BD%E6%AC%A1%E6%95%B0" alt="Downloads" />
  </a>
  <img src="https://img.shields.io/badge/license-MIT-green.svg" alt="License" />
</p>

---

## 下载地址

[![GitHub Release](https://img.shields.io/github/v/release/tyuan511/apifuck?include_prereleases&label=%E6%9C%80%E6%96%B0%E7%89%88%E6%9C%AC)](https://github.com/tyuan511/apifuck/releases/latest)

<!-- release-downloads:start -->
| 平台 | 下载地址 |
|------|---------|
| macOS | [ApiFuck_0.0.5_aarch64.dmg](https://github.com/tyuan511/apifuck/releases/download/v0.0.5/ApiFuck_0.0.5_aarch64.dmg) |
| Windows | [ApiFuck_0.0.5_x64-setup.exe](https://github.com/tyuan511/apifuck/releases/download/v0.0.5/ApiFuck_0.0.5_x64-setup.exe) |
| Linux | [ApiFuck_0.0.5_amd64.AppImage](https://github.com/tyuan511/apifuck/releases/download/v0.0.5/ApiFuck_0.0.5_amd64.AppImage) |
<!-- release-downloads:end -->

> 桌面端内置自动更新机制，发布新版本后会主动提示升级。

---

## 功能特色

### 核心功能

- **项目化管理** — 支持项目、集合、请求三级结构，按业务模块组织接口
- **多标签页编辑** — 同时打开多个请求，支持切换、重排、关闭，自动恢复上次工作状态
- **完整请求构建** — 支持 HTTP 方法、URL、Query、Headers、Auth、Body
- **认证方式** — None / Basic / Bearer / API Key
- **响应查看** — 状态码、耗时、大小、响应头、JSON 格式化与语法高亮
- **环境变量** — 多环境配置、快速切换、按环境变量替换请求内容
- **请求脚本** — Pre-request Script / Post-request Script，支持读写环境变量和脚本变量

### 差异化特点

- **本地优先** — 基于文件夹 + JSON 存储，无需数据库，内容可读、可备份、可 Git 管理
- **极速响应** — Tauri v2 + Rust 后端，启动快、体积小、资源占用低
- **原生体验** — 适配 macOS 窗口拖拽区域与交通灯按钮，融入系统桌面
- **自动更新** — 通过 GitHub Releases 自动分发新版本

---

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

```
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

---

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

# 启动桌面开发模式（新窗口）
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

---

## 如何贡献

欢迎提交 Issue 或 Pull Request。

### 协作约定

在开始贡献前，建议先阅读：

- [AGENTS.md](./AGENTS.md) — 团队协作约定
- [docs/auto-update.md](./docs/auto-update.md) — 自动更新机制说明

### 提交流程

提交前请确保执行以下检查：

```bash
# 前端代码检查
bun run lint
bun run build

# Rust 代码测试（如有涉及）
cd src-tauri && cargo test
```

---

## 许可证

本项目使用 [MIT License](./LICENSE)。

[English](./README_en.md)
