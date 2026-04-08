# DevHub

DevHub 是一个跨平台桌面工具，用于统一管理开发环境中的 AI CLI 工具安装、更新和卸载流程。  
当前版本重点聚焦在 `Claude CLI`、`Gemini CLI`、`Codex CLI` 的基础生命周期管理。

## 核心能力

- 可视化安装、更新、卸载 AI CLI 工具
- 展示工具状态、版本和路径信息
- 统一任务执行反馈与错误提示
- 基于 Tauri 的跨平台桌面应用（Windows / macOS / Linux）

## 技术栈

- 前端：Vue 3 + TypeScript + Vite
- 桌面端：Rust + Tauri 2
- 测试：Vitest

## 当前平台状态

- 已完成并验证：macOS
- 开发中：Windows、Linux

## 快速开始

### 1) 环境要求

- Node.js 18+
- pnpm 9+
- Rust stable toolchain
- Tauri 2 运行依赖（按你的操作系统安装）

### Windows 说明

在 Windows 运行 `pnpm tauri dev` 前，请先满足系统依赖；常见报错与处理步骤见 `docs/FAQ.md`。

### 2) 安装依赖

```bash
pnpm install
```

### 3) 启动开发

```bash
pnpm tauri dev
```

### 4) 运行测试

```bash
pnpm test
```

### 5) 构建（调试模式，不打包）

```bash
pnpm tauri build --debug --no-bundle
```

## 文档

- 产品需求文档：`docs/PRD.md`
- 版本规划：`docs/ROADMAP.md`
- 常见问题：`docs/FAQ.md`
- 贡献指南：`CONTRIBUTING.md`

## 目录结构

```text
devhub/
  docs/                 # 产品与项目文档
  src/                  # Vue 前端代码
  src-tauri/            # Rust/Tauri 桌面端代码
  tests/                # 前端单测
```
