/* PiWidget · 开机自启
 *
 * OS 注册表 / LaunchAgent / desktop entry 是唯一事实源：状态不写入 data.json，
 * 每次启动都向系统查询，与 Tauri 时代行为一致。
 */

use auto_launch::AutoLaunchBuilder;

const APP_NAME: &str = "PiWidget";

fn launcher() -> Result<auto_launch::AutoLaunch, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let exe = exe.to_string_lossy();
    AutoLaunchBuilder::new()
        .set_app_name(APP_NAME)
        .set_app_path(&exe)
        .build()
        .map_err(|e| e.to_string())
}

/// 平台是否支持自启注册
pub fn is_supported() -> bool {
    auto_launch::AutoLaunch::is_support()
}

pub fn is_enabled() -> bool {
    launcher()
        .ok()
        .and_then(|launcher| launcher.is_enabled().ok())
        .unwrap_or(false)
}

/// 切换自启状态，返回切换后的真实状态
pub fn toggle(current: bool) -> Result<bool, String> {
    let launcher = launcher()?;
    if current {
        launcher.disable().map_err(|e| e.to_string())?;
    } else {
        launcher.enable().map_err(|e| e.to_string())?;
    }
    launcher.is_enabled().map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposes_platform_support_flag() {
        // 只验证可调用；真实注册表 / launchd 状态不写入数据文件
        let _ = is_supported();
    }

    #[test]
    fn app_name_is_stable() {
        assert_eq!(APP_NAME, "PiWidget");
    }
}
