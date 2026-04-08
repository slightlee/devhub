// Tauri 后端核心：负责工具状态探测、命令执行、日志持久化与事件分发。
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    process::Stdio,
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};
use tauri::{AppHandle, Emitter, Manager, State, Window};
use tauri_plugin_opener::OpenerExt;
use tokio::fs;
use tokio::io::AsyncWriteExt;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::Mutex as AsyncMutex;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ToolState {
    id: String,
    name: String,
    vendor: String,
    vendor_icon: String,
    status: String,
    current_version: String,
    latest_version: String,
    path: String,
    config_path: String,
    path_needs_setup: bool,
    supports_path_fix: bool,
    shell_config_file: String,
    progress: u8,
    active_action: Option<String>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ToolProgressEvent {
    tool_id: String,
    progress: u8,
    status: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ToolUpdatedEvent {
    tool: ToolState,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ToolLogEvent {
    timestamp: i64,
    message: String,
    status: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ToolActionResultEvent {
    timestamp: i64,
    tool_id: String,
    action: String,
    success: bool,
    message: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct BatchUpdateFailure {
    tool_id: String,
    reason: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct BatchUpdateResult {
    started: Vec<String>,
    failed: Vec<BatchUpdateFailure>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SourceCheckResult {
    overall: String,
    npm: String,
    claude: String,
    checked_at: i64,
}

#[derive(Clone, Serialize, Deserialize)]
struct LogLine {
    timestamp: i64,
    status: String,
    message: String,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct SettingsState {
    auto_refresh_on_launch: bool,
    proxy_enabled: bool,
    proxy_url: String,
    log_persistence_enabled: bool,
    log_retention_days: u32,
}

impl Default for SettingsState {
    fn default() -> Self {
        Self {
            auto_refresh_on_launch: true,
            proxy_enabled: false,
            proxy_url: String::new(),
            log_persistence_enabled: true,
            log_retention_days: 7,
        }
    }
}

struct ToolSpec {
    id: &'static str,
    name: &'static str,
    vendor: &'static str,
    vendor_icon: &'static str,
    bin: &'static str,
    config_dir: &'static str,
    install_cmd: &'static str,
    update_cmd: &'static str,
    uninstall_cmd: &'static str,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum PlatformKind {
    Windows,
    Unix,
}

#[derive(Clone, Copy)]
struct PlatformCapabilities {
    supports_path_fix: bool,
    preferred_shell: &'static str,
}

#[derive(Clone)]
struct ToolRollback {
    status: String,
    current_version: String,
    latest_version: String,
    path: String,
    config_path: String,
    path_needs_setup: bool,
    supports_path_fix: bool,
    shell_config_file: String,
}

struct AppState {
    inner: Mutex<InnerState>,
    log_lock: AsyncMutex<()>,
}

struct InnerState {
    tools: Vec<ToolState>,
    settings: SettingsState,
}

const PATH_MARKER: &str = "# devhub";
const PATH_EXPORT_LINE: &str = "export PATH=\"$HOME/.local/bin:$PATH\"";

const TOOL_SPECS: [ToolSpec; 3] = [
    ToolSpec {
        id: "claude",
        name: "Claude CLI",
        vendor: "Anthropic",
        vendor_icon: "/assets/anthropic.svg",
        bin: "claude",
        config_dir: ".claude",
        install_cmd: "curl -fsSL https://claude.ai/install.sh | bash",
        update_cmd: "claude update",
        uninstall_cmd: "rm -f \"$HOME/.local/bin/claude\" && rm -rf \"$HOME/.local/share/claude\"",
    },
    ToolSpec {
        id: "gemini",
        name: "Gemini CLI",
        vendor: "Google",
        vendor_icon: "/assets/google.svg",
        bin: "gemini",
        config_dir: ".gemini",
        install_cmd: "npm i -g @google/gemini-cli@latest",
        update_cmd: "npm i -g @google/gemini-cli@latest",
        uninstall_cmd: "npm uninstall -g @google/gemini-cli",
    },
    ToolSpec {
        id: "codex",
        name: "Codex CLI",
        vendor: "OpenAI",
        vendor_icon: "/assets/openai.svg",
        bin: "codex",
        config_dir: ".codex",
        install_cmd: "npm i -g @openai/codex@latest",
        update_cmd: "npm i -g @openai/codex@latest",
        uninstall_cmd: "npm uninstall -g @openai/codex",
    },
];

impl AppState {
    fn new() -> Self {
        Self {
            inner: Mutex::new(InnerState {
                tools: initial_tools(),
                settings: SettingsState::default(),
            }),
            log_lock: AsyncMutex::new(()),
        }
    }
}

fn initial_tools() -> Vec<ToolState> {
    let platform = current_platform();
    let shell_config_file = shell_config_path_string(platform);
    TOOL_SPECS
        .iter()
        .map(|tool| ToolState {
            id: tool.id.into(),
            name: tool.name.into(),
            vendor: tool.vendor.into(),
            vendor_icon: tool.vendor_icon.into(),
            status: "not_installed".into(),
            current_version: "--".into(),
            latest_version: "--".into(),
            path: "--".into(),
            config_path: config_path_for(tool),
            path_needs_setup: false,
            supports_path_fix: supports_path_fix(tool.id, platform),
            shell_config_file: shell_config_file.clone(),
            progress: 0,
            active_action: None,
        })
        .collect()
}

fn tool_spec(tool_id: &str) -> Option<&'static ToolSpec> {
    TOOL_SPECS.iter().find(|tool| tool.id == tool_id)
}

fn resolve_home_dir() -> Option<PathBuf> {
    if let Ok(home) = std::env::var("HOME") {
        let trimmed = home.trim();
        if !trimmed.is_empty() {
            return Some(PathBuf::from(trimmed));
        }
    }

    if let Ok(profile) = std::env::var("USERPROFILE") {
        let trimmed = profile.trim();
        if !trimmed.is_empty() {
            return Some(PathBuf::from(trimmed));
        }
    }

    let drive = std::env::var("HOMEDRIVE").ok()?;
    let path = std::env::var("HOMEPATH").ok()?;
    if drive.trim().is_empty() || path.trim().is_empty() {
        return None;
    }

    Some(PathBuf::from(format!("{}{}", drive.trim(), path.trim())))
}

fn config_path_for(tool: &ToolSpec) -> String {
    resolve_home_dir()
        .map(|home| home.join(tool.config_dir).to_string_lossy().to_string())
        .unwrap_or_else(|| "--".to_string())
}

fn settings_dir() -> Option<PathBuf> {
    resolve_home_dir().map(|home| home.join(".devhub"))
}

fn settings_path() -> Option<PathBuf> {
    settings_dir().map(|dir| dir.join("settings.json"))
}

fn logs_dir() -> Option<PathBuf> {
    resolve_home_dir().map(|home| home.join(".devhub").join("logs"))
}

fn logs_file_path() -> Option<PathBuf> {
    logs_dir().map(|dir| dir.join("app.log"))
}

#[tauri::command]
async fn get_logs_dir() -> Result<String, String> {
    let dir = logs_dir().ok_or_else(|| "无法解析日志目录。".to_string())?;
    fs::create_dir_all(&dir)
        .await
        .map_err(|error| format!("创建日志目录失败：{}", error))?;
    Ok(dir.to_string_lossy().to_string())
}

#[tauri::command]
async fn open_logs_dir(app: AppHandle) -> Result<(), String> {
    let dir = logs_dir().ok_or_else(|| "无法解析日志目录。".to_string())?;
    fs::create_dir_all(&dir)
        .await
        .map_err(|error| format!("创建日志目录失败：{}", error))?;
    app.opener()
        .open_path(dir.to_string_lossy().to_string(), None::<&str>)
        .map_err(|error| format!("打开日志目录失败：{}", error))?;
    Ok(())
}

fn sanitize_settings(mut settings: SettingsState) -> SettingsState {
    let trimmed = settings.proxy_url.trim().to_string();
    settings.proxy_url = trimmed;
    if settings.proxy_url.is_empty() {
        settings.proxy_enabled = false;
    }
    if settings.log_retention_days == 0 {
        settings.log_retention_days = 7;
    }
    settings
}

async fn load_settings_from_disk() -> SettingsState {
    let path = match settings_path() {
        Some(value) => value,
        None => return SettingsState::default(),
    };
    let content = match fs::read_to_string(&path).await {
        Ok(value) => value,
        Err(_) => return SettingsState::default(),
    };
    match serde_json::from_str::<SettingsState>(&content) {
        Ok(value) => sanitize_settings(value),
        Err(_) => SettingsState::default(),
    }
}

async fn save_settings_to_disk(settings: &SettingsState) -> Result<(), String> {
    let dir = settings_dir().ok_or_else(|| "无法解析用户目录。".to_string())?;
    fs::create_dir_all(&dir)
        .await
        .map_err(|error| format!("创建配置目录失败：{}", error))?;
    let path = dir.join("settings.json");
    let payload = serde_json::to_string_pretty(settings)
        .map_err(|error| format!("序列化设置失败：{}", error))?;
    fs::write(&path, payload)
        .await
        .map_err(|error| format!("写入设置失败：{}", error))?;
    Ok(())
}

fn shell_config_path(platform: PlatformKind) -> Option<PathBuf> {
    if platform == PlatformKind::Windows {
        return None;
    }
    let home = resolve_home_dir()?;
    let shell = std::env::var("SHELL")
        .unwrap_or_default()
        .to_ascii_lowercase();
    if shell.ends_with("zsh") {
        return Some(home.join(".zshrc"));
    }
    if shell.ends_with("bash") {
        return Some(home.join(".bashrc"));
    }
    if Path::new("/bin/zsh").exists() {
        return Some(home.join(".zshrc"));
    }
    Some(home.join(".bashrc"))
}

fn shell_config_path_string(platform: PlatformKind) -> String {
    shell_config_path(platform)
        .map(|path| path.to_string_lossy().to_string())
        .unwrap_or_else(|| "--".to_string())
}

fn claude_fallback_path() -> Option<PathBuf> {
    resolve_home_dir().map(|home| home.join(".local/bin/claude"))
}

fn npm_package_for(tool_id: &str) -> Option<&'static str> {
    match tool_id {
        "claude" => Some("@anthropic-ai/claude-code"),
        "gemini" => Some("@google/gemini-cli"),
        "codex" => Some("@openai/codex"),
        _ => None,
    }
}

fn status_for_action(action: &str) -> Option<&'static str> {
    match action {
        "install" => Some("installing"),
        "update" => Some("updating"),
        "uninstall" => Some("uninstalling"),
        _ => None,
    }
}

fn command_for_action(tool_id: &str, action: &str, platform: PlatformKind) -> Option<String> {
    tool_spec(tool_id)?;
    match action {
        "install" | "update" | "uninstall" => Some(resolve_action_command(tool_id, action, platform)),
        _ => None,
    }
}

fn action_label(action: &str) -> &'static str {
    match action {
        "install" => "安装",
        "update" => "更新",
        "uninstall" => "卸载",
        _ => "操作",
    }
}

fn current_platform() -> PlatformKind {
    if cfg!(target_os = "windows") {
        PlatformKind::Windows
    } else {
        PlatformKind::Unix
    }
}

fn platform_capabilities(platform: PlatformKind) -> PlatformCapabilities {
    match platform {
        PlatformKind::Windows => PlatformCapabilities {
            supports_path_fix: false,
            preferred_shell: "cmd.exe",
        },
        PlatformKind::Unix => PlatformCapabilities {
            supports_path_fix: true,
            preferred_shell: if Path::new("/bin/zsh").exists() {
                "/bin/zsh"
            } else {
                "/bin/bash"
            },
        },
    }
}

fn supports_path_fix(tool_id: &str, platform: PlatformKind) -> bool {
    tool_id == "claude" && platform_capabilities(platform).supports_path_fix
}

fn resolve_action_command(tool_id: &str, action: &str, platform: PlatformKind) -> String {
    if tool_id == "claude" {
        return match (platform, action) {
            (PlatformKind::Windows, "install") => {
                "powershell -NoProfile -Command \"iwr https://claude.ai/install.ps1 -UseBasicParsing | iex\"".to_string()
            }
            (PlatformKind::Windows, "uninstall") => {
                "npm uninstall -g @anthropic-ai/claude-code".to_string()
            }
            (_, "install") => "curl -fsSL https://claude.ai/install.sh | bash".to_string(),
            (_, "update") => "claude update".to_string(),
            (_, "uninstall") => {
                "rm -f \"$HOME/.local/bin/claude\" && rm -rf \"$HOME/.local/share/claude\"".to_string()
            }
            _ => String::new(),
        };
    }

    if let Some(spec) = tool_spec(tool_id) {
        return match action {
            "install" => spec.install_cmd.to_string(),
            "update" => spec.update_cmd.to_string(),
            "uninstall" => spec.uninstall_cmd.to_string(),
            _ => String::new(),
        };
    }

    String::new()
}

fn build_action_commands_map() -> BTreeMap<String, BTreeMap<String, String>> {
    let platform = current_platform();
    let mut map = BTreeMap::new();
    for spec in TOOL_SPECS {
        let mut commands = BTreeMap::new();
        commands.insert(
            "install".to_string(),
            resolve_action_command(spec.id, "install", platform),
        );
        commands.insert(
            "update".to_string(),
            resolve_action_command(spec.id, "update", platform),
        );
        commands.insert(
            "uninstall".to_string(),
            resolve_action_command(spec.id, "uninstall", platform),
        );
        if supports_path_fix(spec.id, platform) {
            commands.insert(
                "fix_path".to_string(),
                format!("{}\n{}", PATH_MARKER, PATH_EXPORT_LINE),
            );
        }
        map.insert(spec.id.to_string(), commands);
    }
    map
}

fn preferred_shell() -> &'static str {
    platform_capabilities(current_platform()).preferred_shell
}

fn is_windows_shell(shell: &str) -> bool {
    shell.eq_ignore_ascii_case("cmd")
        || shell.eq_ignore_ascii_case("cmd.exe")
        || shell.to_ascii_lowercase().ends_with("\\cmd.exe")
}

fn wrap_shell_command(shell: &str, command: &str) -> (&'static str, String) {
    if is_windows_shell(shell) {
        ("/C", command.to_string())
    } else {
        ("-lc", format!("set -o pipefail; {}", command))
    }
}

fn path_lookup_command(shell: &str, bin: &str) -> String {
    if is_windows_shell(shell) {
        format!("where {}", bin)
    } else {
        format!("command -v {}", bin)
    }
}

fn extract_version_token(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    let mut start = None;
    for (idx, ch) in trimmed.char_indices() {
        if ch.is_ascii_digit() {
            start = Some(idx);
            break;
        }
    }
    let start = start?;
    let mut end = trimmed.len();
    for (offset, ch) in trimmed[start..].char_indices() {
        if !(ch.is_ascii_alphanumeric() || ch == '.' || ch == '-' || ch == '_') {
            end = start + offset;
            break;
        }
    }
    let token = trimmed[start..end].trim();
    if token.is_empty() {
        None
    } else {
        Some(token.to_string())
    }
}

fn format_version_display(token: &str) -> String {
    if token.starts_with('v') || token.starts_with('V') {
        token.to_string()
    } else {
        format!("v{}", token)
    }
}

fn parse_version(raw: &str) -> (String, Option<String>) {
    if let Some(token) = extract_version_token(raw) {
        (format_version_display(&token), Some(token))
    } else {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            ("--".to_string(), None)
        } else {
            (trimmed.to_string(), None)
        }
    }
}

async fn resolve_command_path(shell: &str, bin: &str) -> Option<String> {
    let lookup = path_lookup_command(shell, bin);
    let (flag, wrapped) = wrap_shell_command(shell, &lookup);
    let output = Command::new(shell)
        .arg(flag)
        .arg(wrapped)
        .output()
        .await
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let path = String::from_utf8_lossy(&output.stdout)
        .lines()
        .next()
        .unwrap_or("")
        .trim()
        .to_string();
    if path.is_empty() {
        None
    } else {
        Some(path)
    }
}

async fn read_command_output_with_env(
    shell: &str,
    command: &str,
    envs: &[(String, String)],
) -> Result<String, String> {
    let (flag, wrapped) = wrap_shell_command(shell, command);
    let output = Command::new(shell)
        .arg(flag)
        .arg(wrapped)
        .envs(envs.iter().cloned())
        .output()
        .await
        .map_err(|error| format!("命令执行失败: {}", error))?;
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if !output.status.success() {
        let message = if !stderr.is_empty() {
            stderr
        } else if !stdout.is_empty() {
            stdout
        } else {
            "命令执行失败".to_string()
        };
        return Err(message);
    }
    if !stdout.is_empty() {
        Ok(stdout)
    } else {
        Ok(stderr)
    }
}

async fn read_command_output(shell: &str, command: &str) -> Result<String, String> {
    read_command_output_with_env(shell, command, &[]).await
}

async fn get_tool_version(shell: &str, bin: &str) -> Option<(String, Option<String>)> {
    let output = read_command_output(shell, &format!("{} --version", bin))
        .await
        .ok()?;
    let line = output.lines().next().unwrap_or("").trim();
    if line.is_empty() {
        None
    } else {
        Some(parse_version(line))
    }
}

async fn get_tool_version_at_path(shell: &str, path: &Path) -> Option<(String, Option<String>)> {
    let command = format!("\"{}\" --version", path.to_string_lossy());
    let output = read_command_output(shell, &command).await.ok()?;
    let line = output.lines().next().unwrap_or("").trim();
    if line.is_empty() {
        None
    } else {
        Some(parse_version(line))
    }
}

async fn shell_config_has_local_bin(config_path: &str) -> bool {
    if config_path.is_empty() || config_path == "--" {
        return false;
    }
    let content = fs::read_to_string(config_path).await.unwrap_or_default();
    let lower = content.to_ascii_lowercase();
    lower.contains(".local/bin")
}

async fn get_npm_latest_version_with_env(
    shell: &str,
    package: &str,
    envs: &[(String, String)],
) -> Option<(String, Option<String>)> {
    if resolve_command_path(shell, "npm").await.is_none() {
        return None;
    }
    let command = format!("npm view {} version", package);
    let npm_envs = with_npm_timeout_envs(envs);
    let output = read_command_output_with_env(shell, &command, &npm_envs)
        .await
        .ok()?;
    if output.trim().is_empty() {
        None
    } else {
        Some(parse_version(&output))
    }
}

fn reconcile_latest_status(
    current_status: &str,
    current_version: &str,
    latest_norm: Option<&str>,
) -> String {
    if !matches!(current_status, "installed" | "update_available") {
        return current_status.to_string();
    }
    let Some(latest) = latest_norm else {
        return current_status.to_string();
    };
    let current_norm = parse_version(current_version).1;
    let Some(current) = current_norm.as_deref() else {
        return current_status.to_string();
    };
    if current != latest {
        "update_available".to_string()
    } else {
        "installed".to_string()
    }
}

async fn ensure_dependency(shell: &str, bin: &str) -> Result<(), String> {
    if resolve_command_path(shell, bin).await.is_some() {
        Ok(())
    } else {
        Err(format!("缺少依赖：{}", bin))
    }
}

fn current_settings(app: &AppHandle) -> SettingsState {
    let state = app.state::<AppState>();
    state
        .inner
        .lock()
        .map(|inner| inner.settings.clone())
        .unwrap_or_default()
}

fn build_proxy_envs(settings: &SettingsState) -> Vec<(String, String)> {
    if !settings.proxy_enabled || settings.proxy_url.trim().is_empty() {
        return Vec::new();
    }
    let url = settings.proxy_url.trim().to_string();
    vec![
        ("HTTP_PROXY".to_string(), url.clone()),
        ("HTTPS_PROXY".to_string(), url.clone()),
        ("ALL_PROXY".to_string(), url.clone()),
        ("http_proxy".to_string(), url.clone()),
        ("https_proxy".to_string(), url.clone()),
        ("all_proxy".to_string(), url.clone()),
        ("NPM_CONFIG_PROXY".to_string(), url.clone()),
        ("NPM_CONFIG_HTTPS_PROXY".to_string(), url.clone()),
        ("npm_config_proxy".to_string(), url.clone()),
        ("npm_config_https_proxy".to_string(), url),
    ]
}

fn with_npm_timeout_envs(envs: &[(String, String)]) -> Vec<(String, String)> {
    let mut next: Vec<(String, String)> = envs.to_vec();
    next.push(("NPM_CONFIG_FETCH_TIMEOUT".to_string(), "4000".to_string()));
    next.push(("NPM_CONFIG_FETCH_RETRIES".to_string(), "0".to_string()));
    next.push(("npm_config_fetch_timeout".to_string(), "4000".to_string()));
    next.push(("npm_config_fetch_retries".to_string(), "0".to_string()));
    next
}

async fn check_npm_package(shell: &str, envs: &[(String, String)], package: &str) -> String {
    if resolve_command_path(shell, "npm").await.is_none() {
        return "fail".to_string();
    }
    let command = format!("npm view {} version", package);
    let npm_envs = with_npm_timeout_envs(envs);
    match read_command_output_with_env(shell, &command, &npm_envs).await {
        Ok(output) => {
            if output.trim().is_empty() {
                "fail".to_string()
            } else {
                "ok".to_string()
            }
        }
        Err(_) => "fail".to_string(),
    }
}

async fn check_claude_script(shell: &str, envs: &[(String, String)], platform: PlatformKind) -> String {
    let command = if platform == PlatformKind::Windows {
        "powershell -NoProfile -Command \"(iwr https://claude.ai/install.ps1 -UseBasicParsing -TimeoutSec 8).Content\""
    } else {
        "curl -fsSL --max-time 8 https://claude.ai/install.sh"
    };
    match read_command_output_with_env(shell, command, envs).await {
        Ok(output) => {
            let trimmed = output.trim_start();
            if trimmed.is_empty() {
                return "fail".to_string();
            }
            let lower = trimmed.to_ascii_lowercase();
            if lower.starts_with("<!doctype html") || lower.starts_with("<html") {
                return "fail".to_string();
            }
            if platform == PlatformKind::Windows {
                if !lower.contains("anthropic") && !lower.contains("claude") {
                    return "fail".to_string();
                }
            } else if !trimmed.starts_with("#!") {
                return "fail".to_string();
            }
            "ok".to_string()
        }
        Err(_) => "fail".to_string(),
    }
}

#[tauri::command]
async fn check_sources(
    app: AppHandle,
    tool_id: Option<String>,
    action: Option<String>,
) -> Result<SourceCheckResult, String> {
    let action = action.unwrap_or_default();
    let tool_id = tool_id.unwrap_or_default();

    let mut npm_packages: Vec<&'static str> = Vec::new();
    let mut check_claude = false;

    if action == "batch_update" {
        let tool_ids = {
            let state = app.state::<AppState>();
            let inner = state.inner.lock().map_err(|_| "锁失败".to_string())?;
            inner
                .tools
                .iter()
                .filter(|tool| tool.status == "update_available")
                .map(|tool| tool.id.clone())
                .collect::<Vec<_>>()
        };
        for id in tool_ids {
            if id == "claude" {
                check_claude = true;
                continue;
            }
            if let Some(package) = npm_package_for(&id) {
                npm_packages.push(package);
            }
        }
    } else if action == "install" || action == "update" {
        if tool_id == "claude" {
            check_claude = true;
        } else if let Some(package) = npm_package_for(&tool_id) {
            npm_packages.push(package);
        }
    }

    let platform = current_platform();
    let shell = preferred_shell();
    let envs = build_proxy_envs(&current_settings(&app));
    let npm = if npm_packages.is_empty() {
        "unknown".to_string()
    } else {
        let mut status = "ok".to_string();
        for package in npm_packages {
            let result = check_npm_package(shell, &envs, package).await;
            if result != "ok" {
                status = "fail".to_string();
                break;
            }
        }
        status
    };

    let claude = if check_claude {
        let fetch_ready = if platform == PlatformKind::Windows {
            resolve_command_path(shell, "powershell").await.is_some()
        } else {
            resolve_command_path(shell, "curl").await.is_some()
        };
        if !fetch_ready {
            "fail".to_string()
        } else {
            check_claude_script(shell, &envs, platform).await
        }
    } else {
        "unknown".to_string()
    };

    let any_checked = npm != "unknown" || claude != "unknown";
    let any_failed = npm == "fail" || claude == "fail";
    let overall = if !any_checked {
        "unknown".to_string()
    } else if any_failed {
        "fail".to_string()
    } else {
        "ok".to_string()
    };
    Ok(SourceCheckResult {
        overall,
        npm,
        claude,
        checked_at: now_timestamp(),
    })
}

#[tauri::command]
async fn clear_logs(_app: AppHandle) -> Result<(), String> {
    let path = logs_file_path().ok_or_else(|| "无法解析日志目录。".to_string())?;
    if fs::metadata(&path).await.is_err() {
        return Ok(());
    }
    fs::remove_file(&path)
        .await
        .map_err(|error| format!("清理日志失败：{}", error))?;
    Ok(())
}

#[tauri::command]
fn get_action_commands() -> BTreeMap<String, BTreeMap<String, String>> {
    build_action_commands_map()
}

fn now_timestamp() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or_default()
}

fn emit_log(app: &AppHandle, message: &str, status: &str) {
    let timestamp = now_timestamp();
    let _ = app.emit(
        "tool-log",
        ToolLogEvent {
            timestamp,
            message: message.to_string(),
            status: status.to_string(),
        },
    );
    persist_log(app, timestamp, message, status);
}

fn emit_action_result(app: &AppHandle, tool_id: &str, action: &str, success: bool, message: &str) {
    let _ = app.emit(
        "tool-action-result",
        ToolActionResultEvent {
            timestamp: now_timestamp(),
            tool_id: tool_id.to_string(),
            action: action.to_string(),
            success,
            message: message.to_string(),
        },
    );
}

fn persist_log(app: &AppHandle, timestamp: i64, message: &str, status: &str) {
    let settings = current_settings(app);
    if !settings.log_persistence_enabled {
        return;
    }
    let log_path = match logs_file_path() {
        Some(path) => path,
        None => return,
    };
    let retention_days = settings.log_retention_days;
    let line = LogLine {
        timestamp,
        status: status.to_string(),
        message: message.to_string(),
    };
    let app_handle = app.clone();
    tauri::async_runtime::spawn(async move {
        let state = app_handle.state::<AppState>();
        let _guard = state.log_lock.lock().await;
        if let Some(dir) = logs_dir() {
            let _ = fs::create_dir_all(&dir).await;
        }
        let payload = match serde_json::to_string(&line) {
            Ok(value) => value,
            Err(_) => return,
        };
        if let Ok(mut file) = tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)
            .await
        {
            let _ = file.write_all(payload.as_bytes()).await;
            let _ = file.write_all(b"\n").await;
        }
        let _ = prune_logs(&log_path, retention_days).await;
    });
}

async fn prune_logs(path: &Path, retention_days: u32) -> Result<(), String> {
    if retention_days == 0 {
        return Ok(());
    }
    let cutoff = now_timestamp().saturating_sub(retention_days as i64 * 24 * 60 * 60 * 1000);
    let content = fs::read_to_string(path).await.unwrap_or_default();
    let mut kept: Vec<String> = Vec::new();
    for line in content.lines() {
        if line.trim().is_empty() {
            continue;
        }
        if let Ok(entry) = serde_json::from_str::<LogLine>(line) {
            if entry.timestamp >= cutoff {
                kept.push(line.to_string());
            }
        }
    }
    let mut next = kept.join("\n");
    if !next.is_empty() {
        next.push('\n');
    }
    fs::write(path, next)
        .await
        .map_err(|error| format!("写入日志失败：{}", error))?;
    Ok(())
}

fn emit_tool_updated(app: &AppHandle, tool: &ToolState) {
    let _ = app.emit("tool-updated", ToolUpdatedEvent { tool: tool.clone() });
}

fn emit_progress(app: &AppHandle, tool_id: &str, progress: u8, status: &str) {
    let _ = app.emit(
        "tool-progress",
        ToolProgressEvent {
            tool_id: tool_id.to_string(),
            progress,
            status: status.to_string(),
        },
    );
}

fn strip_ansi(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == 0x1b {
            i += 1;
            if i >= bytes.len() {
                break;
            }
            match bytes[i] {
                b'[' => {
                    i += 1;
                    while i < bytes.len() {
                        let b = bytes[i];
                        i += 1;
                        if b >= 0x40 && b <= 0x7e {
                            break;
                        }
                    }
                }
                b']' => {
                    i += 1;
                    while i < bytes.len() {
                        if bytes[i] == 0x07 {
                            i += 1;
                            break;
                        }
                        if bytes[i] == 0x1b && i + 1 < bytes.len() && bytes[i + 1] == b'\\' {
                            i += 2;
                            break;
                        }
                        i += 1;
                    }
                }
                _ => {
                    i += 1;
                }
            }
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).to_string()
}

fn stderr_log_level(line: &str) -> &'static str {
    let lower = line.trim().to_ascii_lowercase();
    if lower.starts_with("npm warn") {
        return "warn";
    }
    if lower.starts_with("npm notice") {
        return "info";
    }
    "error"
}

fn set_tool_status(tool: &mut ToolState, action: &str) {
    match action {
        "install" => tool.status = "installing".into(),
        "update" => tool.status = "updating".into(),
        "uninstall" => tool.status = "uninstalling".into(),
        _ => {}
    }
}

fn update_tool_state<F>(app: &AppHandle, tool_id: &str, updater: F) -> Option<ToolState>
where
    F: FnOnce(&mut ToolState),
{
    let state = app.state::<AppState>();
    let mut inner = state.inner.lock().ok()?;
    let tool = inner.tools.iter_mut().find(|tool| tool.id == tool_id)?;
    updater(tool);
    Some(tool.clone())
}

fn update_tool_progress(app: &AppHandle, tool_id: &str, progress: u8, status: &str) {
    let snapshot = update_tool_state(app, tool_id, |tool| {
        tool.progress = progress;
        tool.status = status.to_string();
    });
    if snapshot.is_some() {
        emit_progress(app, tool_id, progress, status);
    }
}

async fn refresh_tool_state(app: &AppHandle, tool_id: &str, shell: &str) -> Option<ToolState> {
    let spec = tool_spec(tool_id)?;
    let bin = spec.bin;
    let platform = current_platform();
    let config_path = config_path_for(spec);
    let shell_config_file = shell_config_path_string(platform);
    let mut path_needs_setup = false;
    let tool_supports_path_fix = supports_path_fix(tool_id, platform);

    let path = resolve_command_path(shell, bin).await;
    let (status, current_version, current_norm, path_value) = if let Some(path) = path {
        if let Some((display, norm)) = get_tool_version(shell, bin).await {
            ("installed".to_string(), display, norm, path)
        } else {
            ("installed".to_string(), "--".to_string(), None, path)
        }
    } else if tool_id == "claude" {
        if let Some(fallback) = claude_fallback_path() {
            if fs::metadata(&fallback).await.is_ok() {
                let (display, norm) = match get_tool_version_at_path(shell, &fallback).await {
                    Some((version, parsed)) => (version, parsed),
                    None => ("--".to_string(), None),
                };
                path_needs_setup = tool_supports_path_fix && !shell_config_has_local_bin(&shell_config_file).await;
                (
                    "installed".to_string(),
                    display,
                    norm,
                    fallback.to_string_lossy().to_string(),
                )
            } else {
                (
                    "not_installed".to_string(),
                    "--".to_string(),
                    None,
                    "--".to_string(),
                )
            }
        } else {
            (
                "not_installed".to_string(),
                "--".to_string(),
                None,
                "--".to_string(),
            )
        }
    } else {
        (
            "not_installed".to_string(),
            "--".to_string(),
            None,
            "--".to_string(),
        )
    };

    let (latest_version, latest_norm) = {
        let state = app.state::<AppState>();
        let inner = state.inner.lock().ok()?;
        let tool = inner.tools.iter().find(|tool| tool.id == tool_id)?;
        let latest = tool.latest_version.clone();
        let (_, norm) = parse_version(&latest);
        (latest, norm)
    };

    let mut final_status = status;
    if final_status == "installed" {
        if let (Some(current), Some(latest)) = (current_norm.as_deref(), latest_norm.as_deref()) {
            if current != latest {
                final_status = "update_available".to_string();
            }
        }
    }

    let state = app.state::<AppState>();
    let mut inner = state.inner.lock().ok()?;
    let tool = inner.tools.iter_mut().find(|tool| tool.id == tool_id)?;
    tool.status = final_status;
    tool.current_version = current_version;
    tool.latest_version = latest_version;
    tool.path = path_value;
    tool.config_path = config_path;
    tool.path_needs_setup = path_needs_setup;
    tool.supports_path_fix = tool_supports_path_fix;
    tool.shell_config_file = shell_config_file;
    tool.progress = 0;
    tool.active_action = None;
    Some(tool.clone())
}

async fn refresh_tool_latest_version(
    app: &AppHandle,
    tool_id: &str,
    shell: &str,
    envs: &[(String, String)],
) -> Option<ToolState> {
    let package = npm_package_for(tool_id)?;
    let (latest_version, latest_norm) =
        match get_npm_latest_version_with_env(shell, package, envs).await {
            Some((display, norm)) => (display, norm),
            None => ("--".to_string(), None),
        };

    update_tool_state(app, tool_id, |tool| {
        tool.latest_version = latest_version;
        if matches!(
            tool.status.as_str(),
            "installing" | "updating" | "uninstalling"
        ) {
            return;
        }
        tool.status =
            reconcile_latest_status(&tool.status, &tool.current_version, latest_norm.as_deref());
    })
}

#[tauri::command]
async fn refresh_latest_versions(app: AppHandle) -> Result<Vec<ToolState>, String> {
    let shell = preferred_shell();
    let envs = build_proxy_envs(&current_settings(&app));
    let tool_ids = {
        let state = app.state::<AppState>();
        let inner = state.inner.lock().map_err(|_| "锁失败".to_string())?;
        inner
            .tools
            .iter()
            .filter(|tool| npm_package_for(&tool.id).is_some())
            .map(|tool| tool.id.clone())
            .collect::<Vec<_>>()
    };

    let mut set = tokio::task::JoinSet::new();
    for tool_id in tool_ids {
        let app_clone = app.clone();
        let envs_clone = envs.clone();
        let tool_id_clone = tool_id.clone();
        set.spawn(async move {
            refresh_tool_latest_version(&app_clone, &tool_id_clone, shell, &envs_clone).await
        });
    }

    let mut updated = Vec::new();
    while let Some(result) = set.join_next().await {
        if let Ok(Some(tool)) = result {
            updated.push(tool);
        }
    }
    Ok(updated)
}

#[tauri::command]
async fn get_settings(state: State<'_, AppState>) -> Result<SettingsState, String> {
    let settings = load_settings_from_disk().await;
    let mut inner = state.inner.lock().map_err(|_| "锁失败".to_string())?;
    inner.settings = settings.clone();
    Ok(settings)
}

#[tauri::command]
async fn save_settings(state: State<'_, AppState>, settings: SettingsState) -> Result<(), String> {
    let sanitized = sanitize_settings(settings);
    save_settings_to_disk(&sanitized).await?;
    let mut inner = state.inner.lock().map_err(|_| "锁失败".to_string())?;
    inner.settings = sanitized;
    Ok(())
}

#[tauri::command]
async fn get_tools_state(app: AppHandle) -> Result<Vec<ToolState>, String> {
    let shell = preferred_shell();
    let tool_ids = {
        let state = app.state::<AppState>();
        let inner = state.inner.lock().map_err(|_| "锁失败".to_string())?;
        inner
            .tools
            .iter()
            .map(|tool| tool.id.clone())
            .collect::<Vec<_>>()
    };

    let mut set = tokio::task::JoinSet::new();
    for tool_id in tool_ids {
        let app_clone = app.clone();
        let tool_id_clone = tool_id.clone();
        set.spawn(async move {
            let _ = refresh_tool_state(&app_clone, &tool_id_clone, shell).await;
        });
    }
    while set.join_next().await.is_some() {}

    let state = app.state::<AppState>();
    let inner = state.inner.lock().map_err(|_| "锁失败".to_string())?;
    Ok(inner.tools.clone())
}

#[tauri::command]
async fn apply_path_fix(app: AppHandle, tool_id: String) -> Result<(), String> {
    let platform = current_platform();
    if !supports_path_fix(&tool_id, platform) {
        return Err("当前平台不支持 PATH 修复。".to_string());
    }
    let config_path =
        shell_config_path(platform).ok_or_else(|| "无法识别 shell 配置文件。".to_string())?;
    let config_path_display = config_path.to_string_lossy().to_string();
    let content = fs::read_to_string(&config_path).await.unwrap_or_default();
    if content.lines().any(|line| line.contains(PATH_MARKER)) {
        emit_log(
            &app,
            "已检测到 DevHub 写入的 PATH 记录，无需重复写入。",
            "info",
        );
        if let Some(tool) = refresh_tool_state(&app, &tool_id, preferred_shell()).await {
            emit_tool_updated(&app, &tool);
        }
        return Ok(());
    }
    let has_local_bin = content.to_ascii_lowercase().contains(".local/bin");

    let mut next = content;
    if !next.is_empty() && !next.ends_with('\n') {
        next.push('\n');
    }
    if !next.is_empty() && !next.ends_with("\n\n") {
        next.push('\n');
    }
    next.push_str(PATH_MARKER);
    next.push('\n');
    next.push_str(PATH_EXPORT_LINE);
    next.push('\n');
    fs::write(&config_path, next)
        .await
        .map_err(|error| format!("写入配置文件失败：{}", error))?;
    if has_local_bin {
        emit_log(
            &app,
            "已检测到 PATH 中存在 ~/.local/bin，已追加带标记记录，便于后续清理。",
            "success",
        );
    }
    emit_log(
        &app,
        &format!(
            "已写入 {}，请在终端执行 source \"{}\" 或重启终端。",
            config_path_display, config_path_display
        ),
        "success",
    );
    if let Some(tool) = refresh_tool_state(&app, &tool_id, preferred_shell()).await {
        emit_tool_updated(&app, &tool);
    }
    Ok(())
}

#[tauri::command]
async fn apply_path_cleanup(app: AppHandle, tool_id: String) -> Result<(), String> {
    let platform = current_platform();
    if !supports_path_fix(&tool_id, platform) {
        return Err("当前平台不支持 PATH 清理。".to_string());
    }
    let config_path =
        shell_config_path(platform).ok_or_else(|| "无法识别 shell 配置文件。".to_string())?;
    let config_path_display = config_path.to_string_lossy().to_string();

    let content = match fs::read_to_string(&config_path).await {
        Ok(value) => value,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            emit_log(
                &app,
                &format!("未找到配置文件 {}，无需清理。", config_path_display),
                "info",
            );
            return Ok(());
        }
        Err(error) => return Err(format!("读取配置文件失败：{}", error)),
    };

    if !content.lines().any(|line| line.contains(PATH_MARKER)) {
        emit_log(&app, "未检测到 DevHub 写入的 PATH 记录，无需清理。", "info");
        return Ok(());
    }

    let lines: Vec<&str> = content.lines().collect();
    let mut kept = Vec::new();
    let mut idx = 0;
    while idx < lines.len() {
        let line = lines[idx];
        if line.contains(PATH_MARKER) {
            if idx + 1 < lines.len() && lines[idx + 1].trim() == PATH_EXPORT_LINE {
                idx += 2;
                continue;
            }
            idx += 1;
            continue;
        }
        kept.push(line);
        idx += 1;
    }

    let mut next = kept.join("\n");
    if content.ends_with('\n') {
        next.push('\n');
    }

    fs::write(&config_path, next)
        .await
        .map_err(|error| format!("写入配置文件失败：{}", error))?;
    emit_log(
        &app,
        &format!("已清理 {} 中的 DevHub PATH 记录。", config_path_display),
        "success",
    );
    if let Some(tool) = refresh_tool_state(&app, &tool_id, preferred_shell()).await {
        emit_tool_updated(&app, &tool);
    }
    Ok(())
}

fn start_action_inner(
    app: AppHandle,
    state: State<AppState>,
    tool_id: String,
    action: String,
) -> Result<(), String> {
    let status = status_for_action(&action).ok_or_else(|| "未知操作".to_string())?;
    let platform = current_platform();
    let _ = command_for_action(&tool_id, &action, platform)
        .ok_or_else(|| "未找到命令".to_string())?;

    let (tool_snapshot, rollback) = {
        let mut inner = state.inner.lock().map_err(|_| "锁失败".to_string())?;
        let tool = inner
            .tools
            .iter_mut()
            .find(|tool| tool.id == tool_id)
            .ok_or_else(|| "未找到工具".to_string())?;

        if matches!(
            tool.status.as_str(),
            "installing" | "updating" | "uninstalling"
        ) {
            return Ok(());
        }

        let rollback = ToolRollback {
            status: tool.status.clone(),
            current_version: tool.current_version.clone(),
            latest_version: tool.latest_version.clone(),
            path: tool.path.clone(),
            config_path: tool.config_path.clone(),
            path_needs_setup: tool.path_needs_setup,
            supports_path_fix: tool.supports_path_fix,
            shell_config_file: tool.shell_config_file.clone(),
        };

        tool.progress = 5;
        tool.active_action = Some(action.clone());
        set_tool_status(tool, &action);
        (tool.clone(), rollback)
    };

    emit_tool_updated(&app, &tool_snapshot);
    emit_progress(&app, &tool_id, tool_snapshot.progress, status);
    emit_log(
        &app,
        &format!("{} 开始{}", tool_snapshot.name, action_label(&action)),
        "info",
    );

    let task_app = app.clone();
    let event_tool_id = tool_id.clone();
    let event_action = action.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(message) = run_tool_action(task_app.clone(), tool_id, action, rollback).await {
            emit_action_result(&task_app, &event_tool_id, &event_action, false, &message);
            emit_log(&task_app, &message, "error");
        }
    });

    Ok(())
}

#[tauri::command]
fn start_action(
    window: Window,
    state: State<AppState>,
    tool_id: String,
    action: String,
) -> Result<(), String> {
    start_action_inner(window.app_handle().clone(), state, tool_id, action)
}

async fn run_tool_action(
    app: AppHandle,
    tool_id: String,
    action: String,
    rollback: ToolRollback,
) -> Result<(), String> {
    let platform = current_platform();
    let shell = preferred_shell();
    let status = status_for_action(&action).ok_or_else(|| "未知操作".to_string())?;
    let command =
        command_for_action(&tool_id, &action, platform).ok_or_else(|| "未找到命令".to_string())?;
    let proxy_envs = if matches!(action.as_str(), "install" | "update") {
        build_proxy_envs(&current_settings(&app))
    } else {
        Vec::new()
    };

    if let Err(error) = check_action_dependencies(shell, &tool_id, &action, platform).await {
        rollback_tool_state(&app, &tool_id, rollback);
        return Err(error);
    }

    if tool_id == "claude" && action == "install" && platform == PlatformKind::Unix {
        if let Err(error) = run_claude_install(&app, &tool_id, status, shell, &proxy_envs).await {
            rollback_tool_state(&app, &tool_id, rollback);
            return Err(format!("命令失败：{}", error));
        }
        finalize_success(&app, &tool_id, &action, shell).await;
        return Ok(());
    }

    emit_log(&app, &format!("执行命令：{}", command), "info");

    if let Err(error) =
        run_command_streaming(&app, &tool_id, status, shell, &command, &proxy_envs).await
    {
        rollback_tool_state(&app, &tool_id, rollback);
        return Err(format!("命令失败：{}", error));
    }

    finalize_success(&app, &tool_id, &action, shell).await;
    Ok(())
}

async fn check_action_dependencies(
    shell: &str,
    tool_id: &str,
    action: &str,
    platform: PlatformKind,
) -> Result<(), String> {
    if tool_id == "claude" {
        if action == "install" {
            if platform == PlatformKind::Windows {
                ensure_dependency(shell, "powershell").await?;
            } else {
                ensure_dependency(shell, "curl").await?;
                ensure_dependency(shell, "bash").await?;
            }
        } else if action == "update" {
            ensure_dependency(shell, "claude").await?;
        }
    }

    if tool_id != "claude" {
        ensure_dependency(shell, "npm").await?;
        ensure_dependency(shell, "node").await?;
    }

    Ok(())
}

async fn run_claude_install(
    app: &AppHandle,
    tool_id: &str,
    status: &str,
    shell: &str,
    envs: &[(String, String)],
) -> Result<(), String> {
    emit_log(app, "正在获取 Claude 安装脚本…", "info");
    let script =
        read_command_output_with_env(shell, "curl -fsSL https://claude.ai/install.sh", envs)
            .await
            .map_err(|error| format!("无法获取官方安装脚本：{}", error))?;
    let trimmed = script.trim_start();
    if trimmed.is_empty() {
        return Err("未获取到官方安装脚本，请检查网络连接。".to_string());
    }

    let lower = trimmed.to_ascii_lowercase();
    if lower.starts_with("<!doctype html") || lower.starts_with("<html") {
        if lower.contains("app unavailable in region") {
            return Err(
                "官方安装脚本不可用：当前网络/地区无法访问（App unavailable in region）。"
                    .to_string(),
            );
        }
        return Err("官方安装脚本异常：返回了 HTML 页面。请检查网络或代理。".to_string());
    }
    if !trimmed.starts_with("#!") {
        return Err("官方安装脚本异常：返回内容不是可执行脚本。".to_string());
    }

    let temp_path = std::env::temp_dir().join(format!("claude-install-{}.sh", now_timestamp()));
    fs::write(&temp_path, script)
        .await
        .map_err(|error| format!("写入安装脚本失败：{}", error))?;
    emit_log(app, "已验证安装脚本，开始执行。", "info");

    let command = format!("bash \"{}\"", temp_path.to_string_lossy());
    let result = run_command_streaming(app, tool_id, status, shell, &command, envs).await;
    let _ = fs::remove_file(&temp_path).await;
    result
}

async fn run_command_streaming(
    app: &AppHandle,
    tool_id: &str,
    status: &str,
    shell: &str,
    command: &str,
    envs: &[(String, String)],
) -> Result<(), String> {
    let (flag, wrapped) = wrap_shell_command(shell, command);
    let mut child = Command::new(shell)
        .arg(flag)
        .arg(wrapped)
        .envs(envs.iter().cloned())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("启动命令失败：{}", error))?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "无法读取标准输出".to_string())?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "无法读取错误输出".to_string())?;

    let mut stdout_lines = BufReader::new(stdout).lines();
    let mut stderr_lines = BufReader::new(stderr).lines();
    let mut progress: u8 = 10;
    update_tool_progress(app, tool_id, progress, status);

    let mut stdout_done = false;
    let mut stderr_done = false;
    while !(stdout_done && stderr_done) {
        tokio::select! {
            line = stdout_lines.next_line(), if !stdout_done => {
                match line {
                    Ok(Some(content)) => {
                        let line = strip_ansi(content.trim()).trim().to_string();
                        if !line.is_empty() {
                            emit_log(app, &line, "info");
                            progress = progress.saturating_add(8).min(90);
                            update_tool_progress(app, tool_id, progress, status);
                        }
                    }
                    Ok(None) => stdout_done = true,
                    Err(error) => {
                        emit_log(app, &format!("读取输出失败：{}", error), "warn");
                        stdout_done = true;
                    }
                }
            }
            line = stderr_lines.next_line(), if !stderr_done => {
                match line {
                    Ok(Some(content)) => {
                        let line = strip_ansi(content.trim()).trim().to_string();
                        if !line.is_empty() {
                            emit_log(app, &line, stderr_log_level(&line));
                            progress = progress.saturating_add(6).min(90);
                            update_tool_progress(app, tool_id, progress, status);
                        }
                    }
                    Ok(None) => stderr_done = true,
                    Err(error) => {
                        emit_log(app, &format!("读取错误输出失败：{}", error), "warn");
                        stderr_done = true;
                    }
                }
            }
        }
    }

    let exit = child
        .wait()
        .await
        .map_err(|error| format!("命令执行失败：{}", error))?;
    if !exit.success() {
        return Err(format!("退出码：{}", exit));
    }

    update_tool_progress(app, tool_id, 100, status);
    Ok(())
}

fn rollback_tool_state(app: &AppHandle, tool_id: &str, rollback: ToolRollback) {
    let snapshot = update_tool_state(app, tool_id, |tool| {
        tool.status = rollback.status;
        tool.current_version = rollback.current_version;
        tool.latest_version = rollback.latest_version;
        tool.path = rollback.path;
        tool.config_path = rollback.config_path;
        tool.path_needs_setup = rollback.path_needs_setup;
        tool.supports_path_fix = rollback.supports_path_fix;
        tool.shell_config_file = rollback.shell_config_file;
        tool.progress = 0;
        tool.active_action = None;
    });
    if let Some(tool) = snapshot {
        emit_tool_updated(app, &tool);
        emit_progress(app, tool_id, 0, &tool.status);
    }
}

async fn finalize_success(app: &AppHandle, tool_id: &str, action: &str, shell: &str) {
    let snapshot = refresh_tool_state(app, tool_id, shell).await;
    if let Some(tool) = snapshot {
        emit_tool_updated(app, &tool);
    }

    let message = format!(
        "{} {}完成",
        tool_name_for_log(tool_id),
        action_label(action)
    );
    emit_log(app, &message, "success");
    emit_action_result(app, tool_id, action, true, &message);
}

fn tool_name_for_log(tool_id: &str) -> String {
    tool_spec(tool_id)
        .map(|tool| tool.name.to_string())
        .unwrap_or_else(|| tool_id.to_string())
}

#[tauri::command]
fn batch_update(window: Window, state: State<AppState>) -> Result<BatchUpdateResult, String> {
    let app = window.app_handle();
    let tool_ids = {
        let inner = state.inner.lock().map_err(|_| "锁失败".to_string())?;
        inner
            .tools
            .iter()
            .filter(|tool| tool.status == "update_available")
            .map(|tool| tool.id.clone())
            .collect::<Vec<_>>()
    };

    if tool_ids.is_empty() {
        emit_log(&app, "暂无可更新的工具", "warn");
        return Ok(BatchUpdateResult {
            started: Vec::new(),
            failed: Vec::new(),
        });
    }

    let mut started: Vec<String> = Vec::new();
    let mut failed: Vec<BatchUpdateFailure> = Vec::new();
    for tool_id in tool_ids {
        if let Err(error) =
            start_action_inner(app.clone(), state.clone(), tool_id.clone(), "update".into())
        {
            emit_log(
                &app,
                &format!("{} 启动更新失败：{}", tool_name_for_log(&tool_id), error),
                "error",
            );
            failed.push(BatchUpdateFailure {
                tool_id,
                reason: error,
            });
            continue;
        }
        started.push(tool_id);
    }

    Ok(BatchUpdateResult { started, failed })
}

#[cfg(test)]
mod tests {
    use super::{
        action_label, build_action_commands_map, current_platform, path_lookup_command,
        reconcile_latest_status, resolve_action_command, stderr_log_level, supports_path_fix,
        wrap_shell_command, PATH_EXPORT_LINE, PATH_MARKER,
    };

    #[test]
    fn stderr_log_level_should_downgrade_npm_notice_and_warn() {
        assert_eq!(stderr_log_level("npm WARN deprecated foo"), "warn");
        assert_eq!(stderr_log_level("npm notice New major version"), "info");
        assert_eq!(stderr_log_level("Error: network timeout"), "error");
    }

    #[test]
    fn build_action_commands_map_should_include_claude_fix_path_on_supported_platform() {
        let commands = build_action_commands_map();
        let claude = commands
            .get("claude")
            .expect("expected claude action commands");

        if supports_path_fix("claude", current_platform()) {
            let fix_path = claude
                .get("fix_path")
                .expect("expected claude fix_path command");
            assert!(fix_path.contains(PATH_MARKER));
            assert!(fix_path.contains(PATH_EXPORT_LINE));
        } else {
            assert!(claude.get("fix_path").is_none());
        }

        assert!(commands
            .get("codex")
            .and_then(|item| item.get("install"))
            .is_some());
    }

    #[test]
    fn action_label_should_match_action_name() {
        assert_eq!(action_label("install"), "安装");
        assert_eq!(action_label("update"), "更新");
        assert_eq!(action_label("uninstall"), "卸载");
        assert_eq!(action_label("unknown"), "操作");
    }

    #[test]
    fn path_lookup_command_should_switch_by_shell() {
        assert_eq!(path_lookup_command("/bin/zsh", "npm"), "command -v npm");
        assert_eq!(path_lookup_command("cmd.exe", "npm"), "where npm");
    }

    #[test]
    fn resolve_action_command_should_switch_claude_install_by_platform() {
        let unix_install = resolve_action_command("claude", "install", super::PlatformKind::Unix);
        let windows_install =
            resolve_action_command("claude", "install", super::PlatformKind::Windows);

        assert!(unix_install.contains("install.sh"));
        assert!(windows_install.contains("install.ps1"));
    }

    #[test]
    fn resolve_action_command_should_use_npm_for_claude_windows_uninstall() {
        let windows_uninstall =
            resolve_action_command("claude", "uninstall", super::PlatformKind::Windows);
        assert_eq!(windows_uninstall, "npm uninstall -g @anthropic-ai/claude-code");
    }

    #[test]
    fn wrap_shell_command_should_switch_by_shell() {
        let (flag, wrapped) = wrap_shell_command("/bin/zsh", "npm --version");
        assert_eq!(flag, "-lc");
        assert!(wrapped.contains("set -o pipefail;"));

        let (flag, wrapped) = wrap_shell_command("cmd.exe", "npm --version");
        assert_eq!(flag, "/C");
        assert_eq!(wrapped, "npm --version");
    }

    #[test]
    fn reconcile_latest_status_should_keep_update_available_when_latest_unknown() {
        assert_eq!(
            reconcile_latest_status("update_available", "v1.0.0", None),
            "update_available"
        );
    }

    #[test]
    fn reconcile_latest_status_should_keep_current_when_current_version_unparseable() {
        assert_eq!(
            reconcile_latest_status("update_available", "--", Some("1.0.1")),
            "update_available"
        );
    }

    #[test]
    fn reconcile_latest_status_should_compare_versions() {
        assert_eq!(
            reconcile_latest_status("installed", "v1.0.0", Some("1.0.1")),
            "update_available"
        );
        assert_eq!(
            reconcile_latest_status("update_available", "v1.0.1", Some("1.0.1")),
            "installed"
        );
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(AppState::new())
        .plugin(tauri_plugin_os::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            get_settings,
            save_settings,
            clear_logs,
            get_logs_dir,
            open_logs_dir,
            check_sources,
            refresh_latest_versions,
            get_tools_state,
            get_action_commands,
            start_action,
            batch_update,
            apply_path_fix,
            apply_path_cleanup
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
