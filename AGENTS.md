# Repository Guidelines

## 项目结构与模块组织
DevHub 是一个 `Vue 3 + TypeScript + Tauri 2` 的桌面应用。

- `src/`：前端应用代码（组件、组合式函数、类型、样式）
- `src-tauri/`：Rust 后端与 Tauri 配置（核心命令与系统交互）
- `tests/`：Vitest 单元测试，当前主要覆盖 composables
- `public/`：静态资源
- `docs/`：产品和协作文档（`PRD.md`、`ROADMAP.md` 等）
- `dist/`：前端构建产物（由构建命令生成）

优先在对应层内改动代码，避免跨层耦合；例如前端状态逻辑放在 `src/composables/`，系统命令放在 `src-tauri/src/lib.rs`。

## 构建、测试与开发命令
- `pnpm install`：安装前端依赖
- `pnpm dev`：仅启动 Vite 前端开发服务
- `pnpm tauri dev`：启动完整桌面开发环境（推荐日常开发）
- `pnpm test`：运行 Vitest 单测（一次性执行）
- `pnpm build`：前端类型检查并构建
- `pnpm tauri build --debug --no-bundle`：调试模式构建桌面端（不打包）
- `pnpm preview`：本地预览前端构建结果

## 代码风格与命名约定
- TypeScript/Vue 使用 2 空格缩进、双引号、保留分号，遵循现有文件风格。
- 组件文件使用 `PascalCase`，如 `ToolSection.vue`。
- 组合式函数使用 `useXxx` 命名，如 `useToolActions.ts`。
- 测试文件使用 `*.test.ts`，与被测模块语义对应（如 `useToolActions.test.ts`）。
- Rust 代码遵循 `rustfmt` 默认风格与 `snake_case` 命名。

## 测试指南
测试框架为 Vitest，测试目录为 `tests/`。新增或修改前端状态逻辑（尤其 composables）时，应补充对应单测，覆盖成功路径、失败路径和边界输入。当前仓库未配置强制覆盖率阈值，但提交前至少确保 `pnpm test` 全部通过。

## 提交与 PR 规范
当前分支尚无历史提交，请按 `CONTRIBUTING.md` 执行 Conventional Commits：

- `feat: ...`
- `fix: ...`
- `docs: ...`
- `refactor: ...`
- `chore: ...`

建议分支命名：`feat/<short-name>`、`fix/<short-name>`、`docs/<short-name>`。PR 需包含改动背景、关键变更点、验证步骤/结果；涉及 UI 时附截图或录屏。
