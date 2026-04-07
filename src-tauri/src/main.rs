// Tauri 应用二进制入口：仅转发到库入口 run()。
// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    devhub_lib::run()
}
