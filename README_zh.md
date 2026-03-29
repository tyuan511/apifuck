# ApiFuck

一款基于 Tauri v2、React 和 TypeScript 构建的强大桌面 API 客户端。提供专业的高密度工作区界面，支持项目管理、集合管理和 API 请求调试。

[English](./README.md)

## 功能特性

- **项目化组织**: 将 API 整理到项目和集合中
- **多标签请求编辑器**: 支持同时处理多个请求的标签页界面
- **请求构建器**: 支持 Query 参数、Headers、Body（Raw/JSON）、Auth（Basic/Bearer/API Key）
- **响应查看器**: 自动 JSON 格式化与语法高亮，纯文本降级支持
- **工作空间存储**: 基于文件的工作空间模型，Git 友好
- **桌面原生**: 原生窗口控制，支持 macOS 交通灯按钮

## 技术栈

- **后端**: Tauri v2 (Rust)
- **前端**: React 19 + TypeScript + Vite
- **UI**: shadcn/ui 组件库 + Tailwind CSS v4
- **状态管理**: Zustand
- **编辑器**: Monaco Editor
- **HTTP 客户端**: Rust reqwest

## 快速开始

### 环境要求

- Node.js 18+
- Bun（推荐）或 npm
- Rust 1.70+

### 安装运行

```bash
# 安装依赖
bun install

# Web 开发模式
bun run dev

# 桌面应用开发模式
bun run tauri dev

# 生产构建
bun run tauri build
```

### 可用命令

| 命令 | 说明 |
|------|------|
| `bun run dev` | 启动 Vite 开发服务器 |
| `bun run build` | TypeScript 检查 + 生产构建 |
| `bun run lint` | ESLint 检查 |
| `bun run lint:fix` | 应用 ESLint 自动修复 |
| `bun run tauri dev` | 以开发模式启动桌面应用 |
| `bun run tauri build` | 构建原生桌面应用 |
| `cd src-tauri && cargo test` | 运行 Rust 测试 |

## 项目结构

```
apifuck/
├── src/                      # React 前端
│   ├── app.tsx               # 主应用入口
│   ├── main.tsx              # React 引导
│   ├── components/ui/        # shadcn/ui 基础组件
│   ├── features/workbench/    # 主工作台功能
│   │   ├── components/       # UI 组件
│   │   ├── hooks/           # 自定义 React Hooks
│   │   ├── store/           # Zustand 状态管理
│   │   ├── types.ts         # 功能类型定义
│   │   └── utils.ts         # 工具函数
│   └── lib/                  # 共享工具
├── src-tauri/                # Tauri/Rust 后端
│   ├── src/
│   │   ├── main.rs          # 入口点
│   │   ├── http.rs          # HTTP 请求处理
│   │   └── error.rs         # 错误类型定义
│   ├── capabilities/        # Tauri 权限配置
│   └── tauri.conf.json      # Tauri 配置
├── docs/                     # 设计文档
└── CLAUDE.md                 # 项目开发指南
```

## 工作空间存储模型

应用采用基于文件夹的工作空间存储系统：

```
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

## 开发指南

### 添加 shadcn 组件

```bash
bun shadcn add <component>
```

### 代码规范

- React 组件: 命名导出，文件名 PascalCase
- 工具函数: camelCase
- 使用 `@antfu/eslint-config` 进行代码检查
- 通过 ESLint 实现 Prettier 格式化

## License

私有项目
