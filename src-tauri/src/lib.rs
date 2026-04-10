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
use tokio::io::{AsyncReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::Mutex as AsyncMutex;
use tokio::time::{Duration, Instant};
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine;
use encoding_rs::GB18030;

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
const CLAUDE_INSTALL_PS1_URL: &str = "https://claude.ai/install.ps1";
const CLAUDE_INSTALL_SH_URL: &str = "https://claude.ai/install.sh";
const CLAUDE_INSTALL_SH_CMD: &str = "curl -fsSL https://claude.ai/install.sh | bash";
const CLAUDE_INSTALL_PS1_COMMAND: &str = "powershell -Command \"irm https://claude.ai/install.ps1 | iex\"";

const TOOL_SPECS: [ToolSpec; 3] = [
    ToolSpec {
        id: "claude",
        name: "Claude CLI",
        vendor: "Anthropic",
        vendor_icon: "/assets/anthropic.svg",
        bin: "claude",
        config_dir: ".claude",
        install_cmd: CLAUDE_INSTALL_SH_CMD,
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
    if platform == PlatformKind::Windows {
        return "用户 PATH（环境变量）".to_string();
    }
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

fn action_label_from_status(status: &str) -> &'static str {
    match status {
        "installing" => "安装",
        "updating" => "更新",
        "uninstalling" => "卸载",
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
            supports_path_fix: true,
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
                CLAUDE_INSTALL_PS1_COMMAND.to_string()
            }
            (PlatformKind::Windows, "uninstall") => {
                "powershell -Command \"Remove-Item -Path (Join-Path $env:USERPROFILE '.local\\bin\\claude.exe') -Force; Remove-Item -Path (Join-Path $env:USERPROFILE '.local\\share\\claude') -Recurse -Force\"".to_string()
            }
            (_, "install") => CLAUDE_INSTALL_SH_CMD.to_string(),
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
            let fix_path_preview = if platform == PlatformKind::Windows {
                "更新用户 PATH：%USERPROFILE%\\.local\\bin".to_string()
            } else {
                format!("{}\n{}", PATH_MARKER, PATH_EXPORT_LINE)
            };
            commands.insert(
                "fix_path".to_string(),
                fix_path_preview,
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

async fn write_utf8_bom(path: &Path, content: &str) -> Result<(), String> {
    let mut bytes = Vec::with_capacity(3 + content.len());
    bytes.extend_from_slice(&[0xEF, 0xBB, 0xBF]);
    bytes.extend_from_slice(content.as_bytes());
    fs::write(path, bytes)
        .await
        .map_err(|error| format!("写入文件失败：{}", error))?;
    Ok(())
}

fn parse_version_candidate(raw: &str, strict: bool) -> Option<(String, Option<String>)> {
    let token = extract_version_token(raw)?;
    if strict && !token.contains('.') {
        return None;
    }
    Some((format_version_display(&token), Some(token)))
}

fn parse_version_from_text(text: &str, strict: bool) -> Option<(String, Option<String>)> {
    for line in text.lines() {
        let trimmed = strip_ansi(line).trim().to_string();
        if trimmed.is_empty() {
            continue;
        }
        if let Some(parsed) = parse_version_candidate(&trimmed, strict) {
            return Some(parsed);
        }
        if strict && trimmed.to_ascii_lowercase().contains("version") {
            if let Some(parsed) = parse_version_candidate(&trimmed, false) {
                return Some(parsed);
            }
        }
    }
    None
}

fn extract_json_payload(text: &str) -> Option<String> {
    let start = text.find('{')?;
    let end = text.rfind('}')?;
    if end <= start {
        return None;
    }
    Some(text[start..=end].to_string())
}

fn decode_output(bytes: &[u8]) -> String {
    if bytes.is_empty() {
        return String::new();
    }
    if let Ok(text) = std::str::from_utf8(bytes) {
        return text.to_string();
    }
    if cfg!(target_os = "windows") {
        let (cow, _, _) = GB18030.decode(bytes);
        return cow.into_owned();
    }
    String::from_utf8_lossy(bytes).into_owned()
}

fn validate_claude_install_script(script: &str) -> Result<(), String> {
    let trimmed = script.trim_start();
    if trimmed.is_empty() {
        return Err("未获取到官方安装脚本，请检查网络连接。".to_string());
    }
    let lower = trimmed.to_ascii_lowercase();
    if lower.starts_with("<!doctype html") || lower.starts_with("<html") {
        if lower.contains("app unavailable in region") {
            return Err("官方安装脚本不可用：当前网络/地区无法访问（App unavailable in region）。".to_string());
        }
        return Err("官方安装脚本异常：返回了 HTML 页面。请检查网络或代理。".to_string());
    }
    Ok(())
}

fn has_shell_shebang(script: &str) -> bool {
    script.trim_start().starts_with("#!")
}

async fn fetch_claude_install_script_windows(
    app: &AppHandle,
    shell: &str,
    envs: &[(String, String)],
    powershell: Option<&PowerShellInfo>,
    proxy: Option<&str>,
    source_label: &str,
) -> Result<String, String> {
    let fetch_start = Instant::now();
    emit_claude_script_fetch_start(app);

    if resolve_command_path(shell, "curl").await.is_some() {
        match read_command_output_with_env(
            shell,
            &format!("curl -fsSL --max-time 8 {}", CLAUDE_INSTALL_PS1_URL),
            envs,
        )
        .await
        {
            Ok(output) => {
                emit_claude_script_fetch_done(app, fetch_start.elapsed());
                validate_claude_install_script(&output)?;
                return Ok(output);
            }
            Err(error) => {
                if powershell.is_none() {
                    let message = format!("无法获取官方安装脚本：{}", error);
                    emit_script_fetch_failed(app, "claude", &message);
                    return Err(message);
                }
                emit_log(
                    app,
                    &format!("curl 获取安装脚本失败：{}，尝试 PowerShell 获取…", error),
                    "warn",
                );
            }
        }
    }

    let info = powershell.ok_or_else(|| "缺少依赖：curl 或 powershell".to_string())?;
    if let Some(proxy_value) = proxy {
        emit_log(
            app,
            &format!(
                "PowerShell 将使用代理（来源：{}）：{}",
                source_label,
                redact_proxy_url(proxy_value)
            ),
            "info",
        );
    } else {
        emit_log(
            app,
            "PowerShell 未检测到代理（DevHub 设置/环境变量），将直连访问。",
            "warn",
        );
    }

    let fetch_command = build_powershell_fetch_command(info, proxy);
    let output = read_command_output_with_env(shell, &fetch_command, envs)
        .await
        .map_err(|error| {
            let message = format!("无法获取官方安装脚本：{}", error);
            emit_script_fetch_failed(app, "claude", &message);
            message
        })?;
    emit_claude_script_fetch_done(app, fetch_start.elapsed());
    validate_claude_install_script(&output)?;
    Ok(output)
}

fn windows_local_bin_path() -> Option<PathBuf> {
    resolve_home_dir().map(|home| home.join(".local").join("bin"))
}

fn normalize_windows_path_segment(value: &str) -> String {
    value
        .trim()
        .trim_matches('"')
        .replace('/', "\\")
        .trim_end_matches('\\')
        .to_ascii_lowercase()
}

fn windows_userprofile_local_bin_literal() -> String {
    "%USERPROFILE%\\.local\\bin".to_string()
}

fn windows_path_contains(path_value: &str, target: &str) -> bool {
    let normalized_target = normalize_windows_path_segment(target);
    let normalized_env = normalize_windows_path_segment(&windows_userprofile_local_bin_literal());
    path_value
        .split(';')
        .map(normalize_windows_path_segment)
        .any(|segment| segment == normalized_target || segment == normalized_env)
}

async fn windows_local_bin_needs_path_setup(shell: &str) -> bool {
    if let Some(target) = windows_local_bin_path() {
        if let Some(user_path) = windows_user_path(shell).await {
            return !windows_path_contains(&user_path, &target.to_string_lossy());
        }
        return true;
    }
    true
}

fn split_windows_path(value: &str) -> Vec<String> {
    value
        .split(';')
        .map(|item| item.trim())
        .filter(|item| !item.is_empty())
        .map(|item| item.to_string())
        .collect()
}

fn escape_powershell_single_quotes(value: &str) -> String {
    value.replace('\'', "''")
}

fn encode_powershell_command(command: &str) -> String {
    let mut bytes: Vec<u8> = Vec::new();
    for unit in command.encode_utf16() {
        bytes.extend_from_slice(&unit.to_le_bytes());
    }
    BASE64_STANDARD.encode(bytes)
}

fn build_powershell_encoded_command(info: &PowerShellInfo, command: &str) -> String {
    let encoded = encode_powershell_command(command);
    format!(
        "{} -NoProfile -NonInteractive -EncodedCommand {}",
        quote_cmd_arg(&info.bin),
        encoded
    )
}

async fn windows_user_path(shell: &str) -> Option<String> {
    let info = resolve_windows_powershell(shell).await?;
    let command = build_powershell_encoded_command(
        &info,
        "[Environment]::GetEnvironmentVariable('Path','User')",
    );
    read_command_output(shell, &command).await.ok()
}

async fn set_windows_user_path(shell: &str, value: &str) -> Result<(), String> {
    let info = resolve_windows_powershell(shell)
        .await
        .ok_or_else(|| "缺少依赖：powershell 或 pwsh".to_string())?;
    let escaped = escape_powershell_single_quotes(value);
    let command = build_powershell_encoded_command(
        &info,
        &format!(
            "[Environment]::SetEnvironmentVariable('Path','{}','User')",
            escaped
        ),
    );
    read_command_output(shell, &command).await.map(|_| ())
}

fn proxy_url_from_envs(envs: &[(String, String)]) -> Option<String> {
    for key in [
        "HTTPS_PROXY",
        "HTTP_PROXY",
        "https_proxy",
        "http_proxy",
        "ALL_PROXY",
        "all_proxy",
    ] {
        if let Some((_, value)) = envs.iter().find(|(name, _)| name == key) {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }
    None
}

fn proxy_url_from_process_env() -> Option<String> {
    for key in [
        "HTTPS_PROXY",
        "HTTP_PROXY",
        "https_proxy",
        "http_proxy",
        "ALL_PROXY",
        "all_proxy",
    ] {
        if let Ok(value) = std::env::var(key) {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }
    None
}

fn normalize_proxy_url(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    if trimmed.contains("://") {
        return trimmed.to_string();
    }
    format!("http://{}", trimmed)
}

fn build_proxy_envs_from_url(url: &str) -> Vec<(String, String)> {
    let normalized = normalize_proxy_url(url);
    if normalized.is_empty() {
        return Vec::new();
    }
    vec![
        ("http_proxy".to_string(), normalized.clone()),
        ("https_proxy".to_string(), normalized.clone()),
        ("all_proxy".to_string(), normalized.clone()),
        ("HTTP_PROXY".to_string(), normalized.clone()),
        ("HTTPS_PROXY".to_string(), normalized.clone()),
        ("ALL_PROXY".to_string(), normalized.clone()),
        ("npm_config_proxy".to_string(), normalized.clone()),
        ("npm_config_https_proxy".to_string(), normalized),
    ]
}

#[derive(Clone)]
struct PowerShellInfo {
    bin: String,
}

fn quote_cmd_arg(value: &str) -> String {
    if value.contains(' ') || value.contains('"') {
        format!("\"{}\"", value.replace('"', "\\\""))
    } else {
        value.to_string()
    }
}

fn build_powershell_fetch_command(info: &PowerShellInfo, proxy_url: Option<&str>) -> String {
    let command = if let Some(proxy) = proxy_url {
        format!(
            "{}; irm {} -TimeoutSec 8 -ErrorAction Stop",
            build_powershell_proxy_prelude(proxy),
            CLAUDE_INSTALL_PS1_URL
        )
    } else {
        format!("irm {} -TimeoutSec 8 -ErrorAction Stop", CLAUDE_INSTALL_PS1_URL)
    };
    build_powershell_encoded_command(info, &command)
}

fn build_powershell_proxy_prelude(proxy: &str) -> String {
    let escaped = escape_powershell_single_quotes(proxy);
    format!(
        "$proxy='{}'; $env:HTTP_PROXY=$proxy; $env:HTTPS_PROXY=$proxy; $env:ALL_PROXY=$proxy; \
        try {{ $wp = New-Object System.Net.WebProxy($proxy, $true); \
        [System.Net.WebRequest]::DefaultWebProxy = $wp; \
        [System.Net.WebRequest]::DefaultWebProxy.Credentials = [System.Net.CredentialCache]::DefaultNetworkCredentials; }} catch {{}}",
        escaped
    )
}

// Windows 安装走“macOS 同款”流程：下载脚本后本地执行（避免 cmd/pwsh 管道差异）

fn windows_claude_uninstall_script() -> &'static str {
    "Remove-Item -Path (Join-Path $env:USERPROFILE '.local\\bin\\claude.exe') -Force; Remove-Item -Path (Join-Path $env:USERPROFILE '.local\\share\\claude') -Recurse -Force"
}

fn build_windows_claude_uninstall_encoded_command(info: &PowerShellInfo) -> String {
    build_powershell_encoded_command(info, windows_claude_uninstall_script())
}

async fn resolve_windows_powershell(shell: &str) -> Option<PowerShellInfo> {
    if resolve_command_path(shell, "pwsh").await.is_some() {
        return Some(PowerShellInfo {
            bin: "pwsh".to_string(),
        });
    }
    if resolve_command_path(shell, "powershell").await.is_some() {
        return Some(PowerShellInfo {
            bin: "powershell".to_string(),
        });
    }
    None
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
    let stdout = decode_output(&output.stdout).trim().to_string();
    let stderr = decode_output(&output.stderr).trim().to_string();
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

async fn capture_command_output_with_env(
    shell: &str,
    command: &str,
    envs: &[(String, String)],
) -> Result<(String, String, bool), String> {
    let (flag, wrapped) = wrap_shell_command(shell, command);
    let output = Command::new(shell)
        .arg(flag)
        .arg(wrapped)
        .envs(envs.iter().cloned())
        .output()
        .await
        .map_err(|error| format!("命令执行失败: {}", error))?;
    let stdout = decode_output(&output.stdout).trim().to_string();
    let stderr = decode_output(&output.stderr).trim().to_string();
    Ok((stdout, stderr, output.status.success()))
}

async fn capture_command_output(
    shell: &str,
    command: &str,
) -> Result<(String, String, bool), String> {
    capture_command_output_with_env(shell, command, &[]).await
}

async fn capture_command_output_direct(
    path: &Path,
    args: &[&str],
) -> Result<(String, String, bool), String> {
    let output = Command::new(path)
        .args(args)
        .output()
        .await
        .map_err(|error| format!("命令执行失败: {}", error))?;
    let stdout = decode_output(&output.stdout).trim().to_string();
    let stderr = decode_output(&output.stderr).trim().to_string();
    Ok((stdout, stderr, output.status.success()))
}

fn is_windows_script_path(path: &Path) -> bool {
    if !cfg!(target_os = "windows") {
        return false;
    }
    match path.extension().and_then(|value| value.to_str()) {
        Some(ext) => matches!(ext.to_ascii_lowercase().as_str(), "cmd" | "bat"),
        None => false,
    }
}

async fn get_tool_version_at_path(shell: &str, path: &Path) -> Option<(String, Option<String>)> {
    if !is_windows_script_path(path) {
        if let Ok((stdout, stderr, success)) =
            capture_command_output_direct(path, &["--version"]).await
        {
            let strict = !success;
            if let Some(parsed) = parse_version_from_text(&stdout, strict)
                .or_else(|| parse_version_from_text(&stderr, strict))
            {
                return Some(parsed);
            }
        }
    }

    let command = format!("\"{}\" --version", path.to_string_lossy());
    match capture_command_output(shell, &command).await {
        Ok((stdout, stderr, success)) => {
            let strict = !success;
            parse_version_from_text(&stdout, strict).or_else(|| parse_version_from_text(&stderr, strict))
        }
        Err(error) => parse_version_from_text(&error, true),
    }
}

async fn resolve_tool_version(shell: &str, tool_id: &str, path: &Path) -> (String, Option<String>) {
    if let Some((display, norm)) = get_tool_version_at_path(shell, path).await {
        return (display, norm);
    }
    if let Some(package) = npm_package_for(tool_id) {
        if let Some((display, norm)) = get_npm_current_version(shell, package).await {
            return (display, norm);
        }
    }
    ("--".to_string(), None)
}

async fn npm_global_bin_path(shell: &str) -> Option<PathBuf> {
    if resolve_command_path(shell, "npm").await.is_none() {
        return None;
    }
    let output = read_command_output(shell, "npm bin -g").await.ok()?;
    let trimmed = output.lines().next().unwrap_or("").trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(PathBuf::from(trimmed))
    }
}

async fn get_npm_current_version(shell: &str, package: &str) -> Option<(String, Option<String>)> {
    if resolve_command_path(shell, "npm").await.is_none() {
        return None;
    }
    let command = format!("npm list -g {} --depth=0 --json", package);
    let (stdout, stderr, _) = capture_command_output(shell, &command).await.ok()?;
    let combined = if stdout.is_empty() {
        stderr.clone()
    } else if stderr.is_empty() {
        stdout.clone()
    } else {
        format!("{}\n{}", stdout, stderr)
    };
    let payload = extract_json_payload(&combined)?;
    let value: serde_json::Value = serde_json::from_str(&payload).ok()?;
    let version = value
        .get("dependencies")?
        .get(package)?
        .get("version")?
        .as_str()?;
    parse_version_candidate(version, false)
}

fn windows_local_bin_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(home) = resolve_home_dir() {
        candidates.push(home.join(".claude").join("bin").join("claude.cmd"));
        candidates.push(home.join(".claude").join("bin").join("claude.exe"));
        candidates.push(home.join(".claude").join("bin").join("claude"));
        candidates.push(home.join(".local").join("bin").join("claude.cmd"));
        candidates.push(home.join(".local").join("bin").join("claude.exe"));
        candidates.push(home.join(".local").join("bin").join("claude"));
    }
    if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
        let base = PathBuf::from(local_app_data);
        candidates.push(base.join("Programs").join("Claude").join("claude.exe"));
        candidates.push(base.join("Programs").join("Claude").join("bin").join("claude.exe"));
        candidates.push(base.join("Claude").join("bin").join("claude.exe"));
        candidates.push(base.join("claude").join("bin").join("claude.exe"));
    }
    if let Ok(program_files) = std::env::var("ProgramFiles") {
        let base = PathBuf::from(program_files);
        candidates.push(base.join("Claude").join("claude.exe"));
        candidates.push(base.join("Claude").join("bin").join("claude.exe"));
    }
    candidates
}

async fn find_windows_claude_fallback(shell: &str) -> Option<PathBuf> {
    if let Some(bin_dir) = npm_global_bin_path(shell).await {
        let cmd_path = bin_dir.join("claude.cmd");
        let exe_path = bin_dir.join("claude.exe");
        if fs::metadata(&cmd_path).await.is_ok() {
            return Some(cmd_path);
        }
        if fs::metadata(&exe_path).await.is_ok() {
            return Some(exe_path);
        }
    }
    for path in windows_local_bin_candidates() {
        if fs::metadata(&path).await.is_ok() {
            return Some(path);
        }
    }
    None
}

async fn windows_claude_detected(shell: &str) -> bool {
    if resolve_command_path(shell, "claude").await.is_some() {
        return true;
    }
    find_windows_claude_fallback(shell).await.is_some()
}

async fn claude_windows_debug_paths(shell: &str) -> Vec<String> {
    let mut items: Vec<String> = Vec::new();
    if let Some(bin_dir) = npm_global_bin_path(shell).await {
        items.push(format!("npm bin -g: {}", bin_dir.to_string_lossy()));
        for name in ["claude.cmd", "claude.exe", "claude"] {
            let path = bin_dir.join(name);
            let exists = fs::metadata(&path).await.is_ok();
            let status = if exists { "存在" } else { "不存在" };
            items.push(format!("check: {} ({})", path.to_string_lossy(), status));
        }
    } else {
        items.push("npm bin -g: 未获取".to_string());
    }

    for path in windows_local_bin_candidates() {
        let exists = fs::metadata(&path).await.is_ok();
        let status = if exists { "存在" } else { "不存在" };
        items.push(format!("check: {} ({})", path.to_string_lossy(), status));
    }

    items
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

#[derive(Clone, Copy, PartialEq, Eq)]
enum ProxySource {
    Settings,
    Environment,
    None,
}

struct ProxyContext {
    envs: Vec<(String, String)>,
    url: Option<String>,
    source: ProxySource,
}

impl ProxyContext {
    fn none() -> Self {
        Self {
            envs: Vec::new(),
            url: None,
            source: ProxySource::None,
        }
    }
}

fn proxy_source_label(source: ProxySource) -> &'static str {
    match source {
        ProxySource::Settings => "DevHub 设置",
        ProxySource::Environment => "环境变量",
        ProxySource::None => "未检测到",
    }
}

fn redact_proxy_url(value: &str) -> String {
    let trimmed = value.trim();
    if let Some(scheme_pos) = trimmed.find("://") {
        let after_scheme = &trimmed[scheme_pos + 3..];
        if let Some(at_pos) = after_scheme.find('@') {
            let prefix = &trimmed[..scheme_pos + 3];
            let host = &after_scheme[at_pos + 1..];
            return format!("{}***@{}", prefix, host);
        }
    }
    trimmed.to_string()
}

fn proxy_settings_snapshot(settings: &SettingsState) -> String {
    let url = settings.proxy_url.trim();
    let display = if url.is_empty() {
        "<empty>".to_string()
    } else {
        redact_proxy_url(url)
    };
    format!("enabled={} url={}", settings.proxy_enabled, display)
}

fn resolve_proxy_context(settings: &SettingsState) -> ProxyContext {
    if settings.proxy_enabled && !settings.proxy_url.trim().is_empty() {
        let normalized = normalize_proxy_url(&settings.proxy_url);
        return ProxyContext {
            envs: build_proxy_envs_from_url(&normalized),
            url: Some(normalized),
            source: ProxySource::Settings,
        };
    }

    if let Some(proxy) = proxy_url_from_process_env() {
        let normalized = normalize_proxy_url(&proxy);
        if !normalized.is_empty() {
            return ProxyContext {
                envs: build_proxy_envs_from_url(&normalized),
                url: Some(normalized),
                source: ProxySource::Environment,
            };
        }
    }

    ProxyContext::none()
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

async fn check_claude_script(
    app: &AppHandle,
    shell: &str,
    envs: &[(String, String)],
    platform: PlatformKind,
    powershell: Option<&PowerShellInfo>,
) -> String {
    let command = if platform == PlatformKind::Windows {
        let Some(info) = powershell else {
            return "fail".to_string();
        };
        let proxy = proxy_url_from_envs(envs);
        if let Some(proxy_value) = proxy.as_deref() {
            emit_log(
                app,
                &format!("Claude PowerShell fetch 使用代理：{}", redact_proxy_url(proxy_value)),
                "info",
            );
        }
        build_powershell_fetch_command(info, proxy.as_deref())
    } else {
        format!("curl -fsSL --max-time 8 {}", CLAUDE_INSTALL_SH_URL)
    };
    match read_command_output_with_env(shell, &command, envs).await {
        Ok(output) => {
            let trimmed = output.trim_start();
            if validate_claude_install_script(trimmed).is_err() {
                return "fail".to_string();
            }
            if platform != PlatformKind::Windows && !has_shell_shebang(trimmed) {
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
    let proxy_context = resolve_proxy_context(&current_settings(&app));
    let envs = proxy_context.envs;
    let powershell_info = if platform == PlatformKind::Windows && check_claude {
        resolve_windows_powershell(shell).await
    } else {
        None
    };
    let has_curl = if check_claude {
        resolve_command_path(shell, "curl").await.is_some()
    } else {
        false
    };
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
            powershell_info.is_some()
        } else {
            has_curl
        };
        if !fetch_ready {
            "fail".to_string()
        } else {
            check_claude_script(&app, shell, &envs, platform, powershell_info.as_ref())
                .await
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

fn emit_command_log_with_action(app: &AppHandle, tool_id: &str, status: &str, command: &str) {
    emit_log(
        app,
        &format!(
            "执行命令（{} {}）：{}",
            tool_name_for_log(tool_id),
            action_label_from_status(status),
            command
        ),
        "info",
    );
}

fn emit_script_fetch_failed(app: &AppHandle, tool_id: &str, error: &str) {
    emit_log(
        app,
        &format!("{} 安装脚本获取失败：{}", tool_name_for_log(tool_id), error),
        "error",
    );
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

fn emit_tool_install_attempt(
    app: &AppHandle,
    tool_id: &str,
    channel: &str,
    detail: Option<&str>,
) {
    let suffix = detail.unwrap_or("");
    emit_log(
        app,
        &format!("尝试使用 {} 安装 {}{}…", channel, tool_name_for_log(tool_id), suffix),
        "info",
    );
}

fn emit_tool_uninstall_attempt(
    app: &AppHandle,
    tool_id: &str,
    channel: &str,
    detail: Option<&str>,
) {
    let suffix = detail.unwrap_or("");
    emit_log(
        app,
        &format!("尝试使用 {} 卸载 {}{}…", channel, tool_name_for_log(tool_id), suffix),
        "info",
    );
}

fn emit_windows_path_read_failed(app: &AppHandle, context: &str, level: &str) {
    emit_log(app, &format!("读取用户 PATH 失败，{}。", context), level);
}

fn emit_path_cleanup_noop(app: &AppHandle, scope: &str) {
    let trimmed = scope.trim();
    if trimmed.is_empty() {
        emit_log(app, "未检测到 DevHub 写入的 PATH 记录，无需清理。", "info");
    } else {
        emit_log(
            app,
            &format!("未检测到 DevHub 写入的 {} PATH 记录，无需清理。", trimmed),
            "info",
        );
    }
}

fn emit_batch_update_empty(app: &AppHandle) {
    emit_log(app, "暂无可更新的工具", "warn");
}

fn emit_update_start_failed(app: &AppHandle, tool_id: &str, error: &str) {
    emit_log(
        app,
        &format!("{} 启动更新失败：{}", tool_name_for_log(tool_id), error),
        "error",
    );
}

fn emit_claude_script_fetch_start(app: &AppHandle) {
    emit_log(app, "正在获取 Claude 安装脚本…", "info");
}

fn emit_claude_script_fetch_done(app: &AppHandle, elapsed: Duration) {
    emit_log(
        app,
        &format!("官方安装脚本获取完成，耗时 {:.1}s", elapsed.as_secs_f32()),
        "info",
    );
}

fn emit_claude_script_execute_start(app: &AppHandle, detail: &str) {
    emit_log(
        app,
        &format!("已验证安装脚本，开始执行（{}）。", detail),
        "info",
    );
}

fn emit_claude_script_execute_done(app: &AppHandle, elapsed: Duration) {
    emit_log(
        app,
        &format!("安装脚本执行结束，耗时 {:.1}s", elapsed.as_secs_f32()),
        "info",
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
        let path_buf = PathBuf::from(&path);
        let (display, norm) = resolve_tool_version(shell, tool_id, &path_buf).await;
        ("installed".to_string(), display, norm, path)
    } else if tool_id == "claude" {
        if platform == PlatformKind::Windows {
            if let Some(fallback) = find_windows_claude_fallback(shell).await {
                let (display, norm) = resolve_tool_version(shell, tool_id, &fallback).await;
                path_needs_setup = windows_local_bin_needs_path_setup(shell).await;
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
        } else if let Some(fallback) = claude_fallback_path() {
            if fs::metadata(&fallback).await.is_ok() {
                let (display, norm) = resolve_tool_version(shell, tool_id, &fallback).await;
                path_needs_setup =
                    tool_supports_path_fix && !shell_config_has_local_bin(&shell_config_file).await;
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
    let proxy_context = resolve_proxy_context(&current_settings(&app));
    let envs = proxy_context.envs;
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
    if platform == PlatformKind::Windows {
        return apply_windows_path_fix(app, tool_id).await;
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
    if platform == PlatformKind::Windows {
        return apply_windows_path_cleanup(app, tool_id).await;
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
        emit_path_cleanup_noop(&app, "");
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

async fn apply_windows_path_fix(app: AppHandle, tool_id: String) -> Result<(), String> {
    let shell = preferred_shell();
    let target = windows_local_bin_path()
        .ok_or_else(|| "无法解析用户目录。".to_string())?
        .to_string_lossy()
        .to_string();
    let current = windows_user_path(shell).await.unwrap_or_default();
    if current.trim().is_empty() {
        emit_windows_path_read_failed(&app, "已中止写入", "error");
        return Err("读取用户 PATH 失败".to_string());
    }
    if windows_path_contains(&current, &target) {
        emit_log(&app, "已检测到用户 PATH 中存在 Claude 目录，无需重复写入。", "info");
        if let Some(tool) = refresh_tool_state(&app, &tool_id, shell).await {
            emit_tool_updated(&app, &tool);
        }
        return Ok(());
    }

    let mut segments = split_windows_path(&current);
    segments.push(target.clone());
    let next = segments.join(";");
    set_windows_user_path(shell, &next)
        .await
        .map_err(|error| format!("写入用户 PATH 失败：{}", error))?;
    if let Some(updated) = windows_user_path(shell).await {
        if !windows_path_contains(&updated, &target) {
            emit_log(
                &app,
                "已尝试写入用户 PATH，但未检测到目标目录，请检查权限或重试。",
                "warn",
            );
        }
    }
    emit_log(
        &app,
        &format!("已写入用户 PATH：{}", target),
        "success",
    );
    emit_log(&app, "请重启终端或 DevHub 以使 PATH 生效。", "success");
    if let Some(tool) = refresh_tool_state(&app, &tool_id, shell).await {
        emit_tool_updated(&app, &tool);
    }
    Ok(())
}

async fn apply_windows_path_cleanup(app: AppHandle, tool_id: String) -> Result<(), String> {
    let shell = preferred_shell();
    let target = windows_local_bin_path()
        .ok_or_else(|| "无法解析用户目录。".to_string())?
        .to_string_lossy()
        .to_string();
    let current = windows_user_path(shell).await.unwrap_or_default();
    if current.is_empty() {
        emit_windows_path_read_failed(&app, "无法清理", "warn");
        return Err("读取用户 PATH 失败".to_string());
    }
    let mut removed = false;
    let mut segments: Vec<String> = Vec::new();
    for item in split_windows_path(&current) {
        if windows_path_contains(&item, &target) {
            removed = true;
            continue;
        }
        segments.push(item);
    }
    if !removed {
        emit_path_cleanup_noop(&app, "Claude ");
        return Ok(());
    }
    let next = segments.join(";");
    set_windows_user_path(shell, &next)
        .await
        .map_err(|error| format!("写入用户 PATH 失败：{}", error))?;
    emit_log(&app, "已从用户 PATH 中移除 Claude 目录。", "success");
    if let Some(tool) = refresh_tool_state(&app, &tool_id, shell).await {
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
    let proxy_context = if matches!(action.as_str(), "install" | "update") {
        let settings = current_settings(&app);
        emit_log(
            &app,
            &format!("代理设置快照：{}", proxy_settings_snapshot(&settings)),
            "info",
        );
        resolve_proxy_context(&settings)
    } else {
        ProxyContext::none()
    };
    let proxy_envs = proxy_context.envs;
    let proxy_url = proxy_context.url.clone();

    if let Err(error) = check_action_dependencies(shell, &tool_id, &action, platform).await {
        rollback_tool_state(&app, &tool_id, rollback);
        return Err(format_action_error(&tool_id, &action, &error));
    }

    let result = if tool_id == "claude" && action == "install" {
        run_claude_install(
            &app,
            &tool_id,
            status,
            shell,
            platform,
            &proxy_envs,
            proxy_url.clone(),
            proxy_context.source,
        )
        .await
    } else if tool_id == "claude" && action == "uninstall" && platform == PlatformKind::Windows {
        run_claude_uninstall_windows(&app, &tool_id, status, shell, &proxy_envs).await
    } else {
        run_action_command(&app, &tool_id, status, shell, &command, &proxy_envs).await
    };

    finalize_action_result(&app, &tool_id, &action, shell, rollback, result).await
}

async fn run_action_command(
    app: &AppHandle,
    tool_id: &str,
    status: &str,
    shell: &str,
    command: &str,
    envs: &[(String, String)],
) -> Result<(), String> {
    emit_command_log_with_action(app, tool_id, status, command);
    run_command_streaming(app, tool_id, status, shell, command, envs).await
}

async fn finalize_action_result(
    app: &AppHandle,
    tool_id: &str,
    action: &str,
    shell: &str,
    rollback: ToolRollback,
    result: Result<(), String>,
) -> Result<(), String> {
    if let Err(error) = result {
        rollback_tool_state(app, tool_id, rollback);
        return Err(format_action_error(tool_id, action, &error));
    }
    finalize_success(app, tool_id, action, shell).await;
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
                let has_powershell = resolve_windows_powershell(shell).await.is_some();
                let has_curl = resolve_command_path(shell, "curl").await.is_some();
                if !has_powershell && !has_curl {
                    return Err("缺少依赖：powershell/pwsh 或 curl（至少需要一个）".to_string());
                }
            } else {
                ensure_dependency(shell, "curl").await?;
                ensure_dependency(shell, "bash").await?;
            }
        } else if action == "update" {
            ensure_dependency(shell, "claude").await?;
        } else if action == "uninstall" && platform == PlatformKind::Windows {
            if resolve_windows_powershell(shell).await.is_none() {
                return Err("缺少依赖：powershell 或 pwsh".to_string());
            }
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
    platform: PlatformKind,
    envs: &[(String, String)],
    proxy_url: Option<String>,
    proxy_source: ProxySource,
) -> Result<(), String> {
    if platform == PlatformKind::Windows {
        return run_claude_install_windows(
            app,
            tool_id,
            status,
            shell,
            envs,
            proxy_url,
            proxy_source,
        )
        .await;
    }

    let fetch_start = Instant::now();
    emit_claude_script_fetch_start(app);
    let script = read_command_output_with_env(
        shell,
        &format!("curl -fsSL {}", CLAUDE_INSTALL_SH_URL),
        envs,
    )
    .await
    .map_err(|error| {
        let message = format!("无法获取官方安装脚本：{}", error);
        emit_script_fetch_failed(app, tool_id, &message);
        message
    })?;
    emit_claude_script_fetch_done(app, fetch_start.elapsed());
    let trimmed = script.trim_start();
    if trimmed.is_empty() {
        return Err("未获取到官方安装脚本，请检查网络连接。".to_string());
    }

    validate_claude_install_script(trimmed)?;
    if !has_shell_shebang(trimmed) {
        return Err("官方安装脚本异常：返回内容不是可执行脚本。".to_string());
    }

    let temp_path = std::env::temp_dir().join(format!("claude-install-{}.sh", now_timestamp()));
    fs::write(&temp_path, script)
        .await
        .map_err(|error| format!("写入安装脚本失败：{}", error))?;
    let exec_start = Instant::now();
    emit_claude_script_execute_start(app, "bash");

    let command = format!("bash \"{}\"", temp_path.to_string_lossy());
    let result = run_command_streaming(app, tool_id, status, shell, &command, envs).await;
    emit_claude_script_execute_done(app, exec_start.elapsed());
    let _ = fs::remove_file(&temp_path).await;
    result
}

async fn run_claude_install_windows(
    app: &AppHandle,
    tool_id: &str,
    status: &str,
    shell: &str,
    envs: &[(String, String)],
    proxy_url: Option<String>,
    proxy_source: ProxySource,
) -> Result<(), String> {
    let powershell = resolve_windows_powershell(shell).await;
    let mut errors: Vec<String> = Vec::new();
    let proxy = proxy_url.or_else(|| proxy_url_from_envs(envs));
    let source_label = proxy_source_label(proxy_source);

    if let Some(info) = powershell.as_ref() {
        let script = fetch_claude_install_script_windows(
            app,
            shell,
            envs,
            powershell.as_ref(),
            proxy.as_deref(),
            source_label,
        )
        .await?;
        match execute_claude_powershell_install(
            app, tool_id, status, shell, envs, info, &script,
        )
        .await
        {
            Ok(()) => return Ok(()),
            Err(error) => errors.push(error),
        }
    } else {
        errors.push("缺少依赖：powershell 或 pwsh".to_string());
    }

    Err(errors.join("；"))
}

async fn run_powershell_script_streaming(
    app: &AppHandle,
    tool_id: &str,
    status: &str,
    powershell: &PowerShellInfo,
    script_path: &Path,
    envs: &[(String, String)],
) -> Result<(), String> {
    emit_log(
        app,
        "Windows 运行 PowerShell 脚本改用直连通道，避免 cmd.exe 带来的额外延迟。",
        "info",
    );
    let mut child = Command::new(&powershell.bin)
        .arg("-NoProfile")
        .arg("-NonInteractive")
        .arg("-ExecutionPolicy")
        .arg("Bypass")
        .arg("-File")
        .arg(script_path)
        .envs(envs.iter().cloned())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("启动命令失败：{}", error))?;
    run_child_streaming(app, tool_id, status, &mut child).await
}

async fn execute_claude_powershell_install(
    app: &AppHandle,
    tool_id: &str,
    status: &str,
    shell: &str,
    envs: &[(String, String)],
    powershell: &PowerShellInfo,
    script: &str,
) -> Result<(), String> {
    let exec_start = Instant::now();
    let temp_path = std::env::temp_dir().join(format!("claude-install-{}.ps1", now_timestamp()));
    write_utf8_bom(&temp_path, script).await?;
    emit_tool_install_attempt(app, tool_id, "PowerShell 本地脚本", Some("（macOS 同款流程）"));
    emit_command_log_with_action(
        app,
        tool_id,
        status,
        &format!("{} -File {}", powershell.bin, temp_path.to_string_lossy()),
    );
    emit_claude_script_execute_start(app, "PowerShell 本地脚本");

    let result =
        run_powershell_script_streaming(app, tool_id, status, powershell, &temp_path, envs).await;
    emit_claude_script_execute_done(app, exec_start.elapsed());
    tokio::time::sleep(Duration::from_millis(500)).await;
    let _ = fs::remove_file(&temp_path).await;
    match result {
        Ok(()) => {
            if windows_claude_detected(shell).await {
                Ok(())
            } else {
                Err("PowerShell：安装完成但未检测到可执行文件".to_string())
            }
        }
        Err(error) => Err(format!("PowerShell：{}", error)),
    }
}

async fn run_claude_uninstall_windows(
    app: &AppHandle,
    tool_id: &str,
    status: &str,
    shell: &str,
    envs: &[(String, String)],
) -> Result<(), String> {
    let powershell = resolve_windows_powershell(shell)
        .await
        .ok_or_else(|| "缺少依赖：powershell 或 pwsh".to_string())?;
    let command = build_windows_claude_uninstall_encoded_command(&powershell);
    emit_tool_uninstall_attempt(app, tool_id, "PowerShell 官方命令", None);
    emit_log(
        app,
        &format!("卸载命令（官方原文）：{}", windows_claude_uninstall_script()),
        "info",
    );
    emit_command_log_with_action(app, tool_id, status, "powershell -EncodedCommand <base64>");
    run_command_streaming(app, tool_id, status, shell, &command, envs).await
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
    run_child_streaming(app, tool_id, status, &mut child).await
}

async fn run_child_streaming(
    app: &AppHandle,
    tool_id: &str,
    status: &str,
    child: &mut tokio::process::Child,
) -> Result<(), String> {

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "无法读取标准输出".to_string())?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "无法读取错误输出".to_string())?;

    let mut stdout_reader = BufReader::new(stdout);
    let mut stderr_reader = BufReader::new(stderr);
    let mut stdout_partial: Vec<u8> = Vec::new();
    let mut stderr_partial: Vec<u8> = Vec::new();
    let mut stdout_chunk = [0u8; 1024];
    let mut stderr_chunk = [0u8; 1024];
    let mut progress: u8 = 10;
    update_tool_progress(app, tool_id, progress, status);

    let mut stdout_done = false;
    let mut stderr_done = false;
    let mut last_output = Instant::now();
    let mut heartbeat = tokio::time::interval(Duration::from_secs(20));
    heartbeat.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let drain_lines = |partial: &mut Vec<u8>, chunk: &[u8]| -> Vec<String> {
        partial.extend_from_slice(chunk);
        let mut lines: Vec<String> = Vec::new();
        let mut start = 0usize;
        let mut idx = 0usize;
        while idx < partial.len() {
            let byte = partial[idx];
            if byte == b'\n' || byte == b'\r' {
                if start < idx {
                    let slice = &partial[start..idx];
                    let text = strip_ansi(decode_output(slice).trim()).trim().to_string();
                    if !text.is_empty() {
                        lines.push(text);
                    }
                }
                idx += 1;
                start = idx;
                continue;
            }
            idx += 1;
        }
        if start > 0 {
            partial.drain(0..start);
        }
        lines
    };

    while !(stdout_done && stderr_done) {
        tokio::select! {
            read = stdout_reader.read(&mut stdout_chunk), if !stdout_done => {
                match read {
                    Ok(0) => {
                        stdout_done = true;
                        let lines = drain_lines(&mut stdout_partial, &[]);
                        for line in lines {
                            emit_log(app, &line, "info");
                            progress = progress.saturating_add(8).min(90);
                            update_tool_progress(app, tool_id, progress, status);
                            last_output = Instant::now();
                        }
                    }
                    Ok(n) => {
                        let lines = drain_lines(&mut stdout_partial, &stdout_chunk[..n]);
                        for line in lines {
                            emit_log(app, &line, "info");
                            progress = progress.saturating_add(8).min(90);
                            update_tool_progress(app, tool_id, progress, status);
                            last_output = Instant::now();
                        }
                    }
                    Err(error) => {
                        emit_log(app, &format!("读取输出失败：{}", error), "warn");
                        stdout_done = true;
                    }
                }
            }
            read = stderr_reader.read(&mut stderr_chunk), if !stderr_done => {
                match read {
                    Ok(0) => {
                        stderr_done = true;
                        let lines = drain_lines(&mut stderr_partial, &[]);
                        for line in lines {
                            emit_log(app, &line, stderr_log_level(&line));
                            progress = progress.saturating_add(6).min(90);
                            update_tool_progress(app, tool_id, progress, status);
                            last_output = Instant::now();
                        }
                    }
                    Ok(n) => {
                        let lines = drain_lines(&mut stderr_partial, &stderr_chunk[..n]);
                        for line in lines {
                            emit_log(app, &line, stderr_log_level(&line));
                            progress = progress.saturating_add(6).min(90);
                            update_tool_progress(app, tool_id, progress, status);
                            last_output = Instant::now();
                        }
                    }
                    Err(error) => {
                        emit_log(app, &format!("读取错误输出失败：{}", error), "warn");
                        stderr_done = true;
                    }
                }
            }
            _ = heartbeat.tick() => {
                if last_output.elapsed() >= Duration::from_secs(20) && progress < 90 {
                    emit_log(app, "命令仍在执行，暂未输出日志，请稍候…", "info");
                    progress = progress.saturating_add(2).min(90);
                    update_tool_progress(app, tool_id, progress, status);
                    last_output = Instant::now();
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
    let mut snapshot = refresh_tool_state(app, tool_id, shell).await;
    if action == "install" && tool_id == "claude" && current_platform() == PlatformKind::Windows {
        if snapshot.as_ref().map(|tool| tool.status.as_str()) == Some("not_installed") {
            for _ in 0..3 {
                tokio::time::sleep(std::time::Duration::from_millis(800)).await;
                snapshot = refresh_tool_state(app, tool_id, shell).await;
                if snapshot.as_ref().map(|tool| tool.status.as_str()) != Some("not_installed") {
                    break;
                }
            }
        }
    }

    if let Some(tool) = snapshot.clone() {
        emit_tool_updated(app, &tool);
    }

    if action == "install" {
        if let Some(ref tool) = snapshot {
            if tool.status == "not_installed" {
                if tool_id == "claude" && current_platform() == PlatformKind::Windows {
                    let details = claude_windows_debug_paths(shell).await.join(" | ");
                    if !details.is_empty() {
                        emit_log(
                            app,
                            &format!("Claude Windows 路径检查：{}", details),
                            "warn",
                        );
                    }
                }
                let message = format!(
                    "{} 安装完成但未检测到可执行文件，请检查安装日志或重试。",
                    tool_name_for_log(tool_id)
                );
                emit_log(app, &message, "error");
                emit_action_result(app, tool_id, action, false, &message);
                return;
            }
        }
    }

    if action == "install"
        && tool_id == "claude"
        && current_platform() == PlatformKind::Windows
        && snapshot
            .as_ref()
            .is_some_and(|tool| tool.supports_path_fix && tool.path_needs_setup)
    {
        emit_log(app, "检测到 Claude PATH 未配置，开始自动写入用户 PATH…", "info");
        if let Err(error) = apply_windows_path_fix(app.clone(), tool_id.to_string()).await {
            emit_log(
                app,
                &format!("自动写入用户 PATH 失败：{}", error),
                "warn",
            );
        }
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

fn format_action_error(tool_id: &str, action: &str, error: &str) -> String {
    format!(
        "{} {}失败：{}",
        tool_name_for_log(tool_id),
        action_label(action),
        error
    )
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
        emit_batch_update_empty(&app);
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
            emit_update_start_failed(&app, &tool_id, &error);
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
    fn resolve_action_command_should_use_powershell_for_claude_windows_uninstall() {
        let windows_uninstall =
            resolve_action_command("claude", "uninstall", super::PlatformKind::Windows);
        assert!(windows_uninstall.contains("Remove-Item -Path"));
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
