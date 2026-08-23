use tauri::Manager;
use tauri_plugin_autostart::MacosLauncher;

/// 返回数据文件完整路径：app_data_dir()/data.json
/// （目录由 identifier com.piwidget.app 派生，如 Windows 下 %APPDATA%/com.piwidget.app）
#[tauri::command]
fn data_path(app: tauri::AppHandle) -> Result<String, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?;
    // app_data_dir() 只返回路径不建目录；首次运行（新 identifier）需显式创建
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let p = dir.join("data.json");
    Ok(p.to_string_lossy().into_owned())
}

/// 导入：读用户对话框选定的文件（绕开 fs 插件 scope，路径来自 dialog 用户确认）
#[tauri::command]
fn read_file(path: String) -> Result<String, String> {
    std::fs::read_to_string(&path).map_err(|e| e.to_string())
}

/// 导出：写用户对话框选定的文件（同上）
#[tauri::command]
fn write_file(path: String, content: String) -> Result<(), String> {
    std::fs::write(&path, content).map_err(|e| e.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, None))
        .invoke_handler(tauri::generate_handler![data_path, read_file, write_file])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
