# ApiFuck

[![GitHub Release](https://img.shields.io/github/v/release/tyuan511/apifuck?label=%E6%9C%80%E6%96%B0%E7%89%88%E6%9C%AC)](https://github.com/tyuan511/apifuck/releases/latest)
[![License](https://img.shields.io/badge/license-MIT-green.svg)](./LICENSE)

一款基于 Tauri v2、React 和 TypeScript 构建的桌面 API 客户端，面向本地调试、项目化接口管理和高密度工作台场景。

[English](./README_en.md)

## 功能特性

- **项目化管理**：支持项目、集合、请求三级结构，适合按业务模块组织接口。
- **多标签请求编辑**：支持同时打开多个请求，快速切换、重排、关闭，并恢复上次标签状态。
- **完整请求构建**：支持 HTTP 方法、URL、Query、Headers、Auth、Body 等常见请求配置。
- **认证方式支持**：支持 `None`、`Basic`、`Bearer`、`API Key`。
- **响应结果查看**：展示状态码、耗时、大小、响应头和响应体，并对 JSON 做格式化与高亮。
- **环境变量管理**：支持多个环境配置、切换当前激活环境、按环境替换请求内容。
- **请求脚本能力**：支持 Pre-request Script 和 Post-request Script，可读写环境变量与脚本变量。
- **工作区文件存储**：项目数据基于文件夹与 JSON 存储，便于迁移、备份和 Git 管理。
- **桌面自动更新**：支持通过 GitHub Releases 分发新版本。
- **原生桌面体验**：基于 Tauri v2 构建，适配 macOS 窗口拖拽区域与交通灯按钮。

## 使用方式

### 下载

- 最新版本：[GitHub Releases](https://github.com/tyuan511/apifuck/releases/latest)

### 环境要求

- Node.js 18+
- Bun
- Rust 1.70+

### 本地开发

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
| `bun run lint:fix` | 自动修复部分 ESLint 问题 |
| `bun run tauri dev` | 启动桌面开发环境 |
| `bun run tauri build` | 构建桌面应用 |
| `cd src-tauri && cargo test` | 运行 Rust 测试 |

## 实现方式

### 技术栈

- **桌面容器**：Tauri v2
- **前端**：React 19 + TypeScript + Vite
- **UI**：shadcn/ui + Tailwind CSS v4
- **状态管理**：Zustand
- **代码编辑器**：Monaco Editor
- **HTTP 请求执行**：Rust `reqwest`

### 目录结构

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
├── docs/                     # 文档与 release notes
├── scripts/                  # 发布辅助脚本
└── AGENTS.md                 # 协作约定
```

### 数据存储

工作区采用基于文件夹和 JSON 的结构化存储：

```text
<workspace>/
├── workspace.json
└── <project>/
    ├── metadata.json
    └── items/
        api_<ulid>__<slug>.json
        <collection>/
            ├── metadata.json
            └── items/
```

这种方式的特点是：

- 内容可读、可备份
- 易于版本管理
- 不依赖数据库

## 如何贡献

欢迎提交 Issue 或 Pull Request。

在开始贡献前，建议先阅读：

- [AGENTS.md](./AGENTS.md)
- [docs/auto-update.md](./docs/auto-update.md)

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
