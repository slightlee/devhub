# 常见问题（FAQ）

> 适用范围：Windows 环境下运行 `pnpm tauri dev`。

## 报错：`failed to run 'cargo metadata' ... program not found`

### 含义

系统找不到 `cargo` 可执行文件（未安装 Rust 或终端 PATH 未生效）。

### 快速检查（可选）

```powershell
cargo --version
rustc --version
```

### 解决步骤

1. 安装 Rust toolchain（含 `cargo`）：

```powershell
winget install -e --id Rustlang.Rustup --source winget --accept-source-agreements --accept-package-agreements
```

2. 重新打开终端（必须）。
3. 执行：

```powershell
rustup default stable-msvc
cargo --version
rustc --version
```

### 验证通过标准

- `pnpm tauri dev` 能正常启动。
- 日志中不再出现 `cargo metadata ... program not found`。

### 仍失败时

- 检查 `%USERPROFILE%\.cargo\bin` 是否在 PATH。
- 注意不同终端（PowerShell / Git Bash）PATH 可能不一致。

---

## 报错：`the msvc targets depend on the msvc linker but 'link.exe' was not found`

### 含义

Rust 使用 `*-msvc` 目标时，系统未找到 MSVC 链接器 `link.exe`。

### 快速检查（可选）

```powershell
where.exe link
where.exe cl
```

### 解决步骤

1. 安装 **Visual Studio Build Tools**：

```powershell
winget install -e --id Microsoft.VisualStudio.2022.BuildTools --source winget --accept-source-agreements --accept-package-agreements --override "--wait --passive --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
```

2. 重新打开终端后，直接重试：

```powershell
pnpm tauri dev
```

> 注意：VS Code 是编辑器，不包含 `link.exe`。

### 验证通过标准

- `pnpm tauri dev` 能正常启动。
- 日志中不再出现 `link.exe was not found`。

> 说明：上述 winget 命令已通过 `--add Microsoft.VisualStudio.Workload.VCTools` 指定 C++ 工作负载，通常无需在安装界面手动勾选。
