/* PiWidget · 键盘快捷键与动作
 *
 * Ctrl/⌘+S 立即落盘；Esc 退出当前内联编辑。
 */

use gpui_kit::{KeyBinding, actions};

actions!(piwidget, [SaveNow, CancelEdit]);

pub fn key_bindings() -> Vec<KeyBinding> {
    vec![
        KeyBinding::new("ctrl-s", SaveNow, None),
        KeyBinding::new("cmd-s", SaveNow, None),
        KeyBinding::new("escape", CancelEdit, None),
    ]
}

/// 快捷键展示文案（README / 提示用）
pub fn save_shortcut_label() -> &'static str {
    if cfg!(target_os = "macos") {
        "⌘S"
    } else {
        "Ctrl+S"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bindings_cover_save_and_cancel() {
        let bindings = key_bindings();
        assert_eq!(bindings.len(), 3);
        let actions: Vec<&str> = bindings.iter().map(|b| b.action().name()).collect();
        assert_eq!(
            actions,
            vec![
                "piwidget::SaveNow",
                "piwidget::SaveNow",
                "piwidget::CancelEdit"
            ]
        );
        assert_eq!(bindings[0].keystrokes().len(), 1);
        assert_eq!(bindings[2].keystrokes().len(), 1);
    }

    #[test]
    fn save_label_is_platform_aware() {
        let label = save_shortcut_label();
        assert!(label == "Ctrl+S" || label == "⌘S");
    }
}
