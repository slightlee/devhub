# Contributing

感谢你关注 DevHub。为保证质量和协作效率，提交前请遵循以下约定。

## 本地开发

当前主验证平台为 macOS，Windows/Linux 功能正在开发中。

```bash
pnpm install
pnpm tauri dev
```

## 提交前检查

```bash
pnpm test
pnpm build
pnpm tauri build --debug --no-bundle
```

## 分支建议

- 功能开发：`feat/<short-name>`
- 问题修复：`fix/<short-name>`
- 文档变更：`docs/<short-name>`

## Commit 建议

建议使用 Conventional Commits：

- `feat: ...`
- `fix: ...`
- `docs: ...`
- `refactor: ...`
- `chore: ...`

## PR 建议

- 描述问题背景与改动目标
- 列出关键改动点
- 给出验证方式与结果
- 如有 UI 变更，附截图或录屏

## Issue 建议

- 明确系统环境（OS、Node、Rust、pnpm 版本）
- 提供复现步骤
- 附关键日志或报错信息
