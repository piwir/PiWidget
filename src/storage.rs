/* PiWidget · 本地持久化
 *
 * 路径与 Tauri 时代完全一致，旧数据零迁移：
 *   Windows  %APPDATA%\com.piwidget.app\data.json
 *   macOS    ~/Library/Application Support/com.piwidget.app/data.json
 *   Linux    ~/.local/share/com.piwidget.app/data.json
 *
 * Tauri 的 app_data_dir() 就是「系统 data 目录 + identifier」，因此这里用
 * BaseDirs::data_dir() 拼接同一个目录名，而不是 ProjectDirs ——
 * 后者在 Windows 会额外插入 organization 与 "data" 子目录，导致读不到旧数据。
 *
 * 写入采用「同目录临时文件 + rename」保证原子性；解析失败时把原文件备份为
 * data.json.bak 再恢复默认，与 Vue 版行为一致。
 */

use crate::model::{self, State};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// 与旧 tauri.conf.json 的 identifier 保持一致，不得修改。
pub const APP_IDENTIFIER: &str = "com.piwidget.app";
pub const DATA_FILE: &str = "data.json";
pub const BACKUP_SUFFIX: &str = ".bak";

/// 测试/便携模式下的目录覆盖（进程级，仅 tests 使用）
static DATA_DIR_OVERRIDE: std::sync::Mutex<Option<PathBuf>> = std::sync::Mutex::new(None);

/// 把数据目录指向指定位置（仅测试与便携模式使用；生产不调用）
pub fn set_data_dir_override(dir: impl Into<PathBuf>) {
    *DATA_DIR_OVERRIDE.lock().expect("data dir override lock") = Some(dir.into());
}

pub fn clear_data_dir_override() {
    *DATA_DIR_OVERRIDE.lock().expect("data dir override lock") = None;
}

/// 数据目录（不保证已存在）
pub fn data_dir() -> PathBuf {
    if let Some(dir) = DATA_DIR_OVERRIDE
        .lock()
        .expect("data dir override lock")
        .as_ref()
    {
        return dir.clone();
    }
    directories::BaseDirs::new()
        .map(|dirs| dirs.data_dir().join(APP_IDENTIFIER))
        .unwrap_or_else(|| PathBuf::from(".").join(APP_IDENTIFIER))
}

/// 数据文件完整路径
pub fn data_path() -> PathBuf {
    data_dir().join(DATA_FILE)
}

pub fn backup_path() -> PathBuf {
    let mut path = data_path().into_os_string();
    path.push(BACKUP_SUFFIX);
    PathBuf::from(path)
}

#[derive(Debug)]
pub struct Loaded {
    pub state: State,
    /// 需要以错误样式提示用户的告警（如损坏恢复）
    pub warning: Option<String>,
}

/// 读取数据；文件不存在时返回默认状态。
pub fn load() -> Loaded {
    load_from(&data_path(), &backup_path())
}

fn load_from(path: &Path, backup: &Path) -> Loaded {
    let raw = match fs::read_to_string(path) {
        Ok(raw) => raw,
        Err(err) if err.kind() == io::ErrorKind::NotFound => {
            return Loaded {
                state: State::default(),
                warning: None,
            };
        }
        Err(err) => {
            return Loaded {
                state: State::default(),
                warning: Some(format!("读取失败：{err}")),
            };
        }
    };

    if raw.trim().is_empty() {
        return Loaded {
            state: State::default(),
            warning: None,
        };
    }

    match serde_json::from_str::<serde_json::Value>(&raw) {
        Ok(value) if (1..=model::SCHEMA_VERSION as u64).contains(&version_of(&value)) => Loaded {
            state: model::migrate(&value),
            warning: None,
        },
        _ => {
            // 备份失败不影响恢复默认
            let _ = fs::copy(path, backup);
            Loaded {
                state: State::default(),
                warning: Some("数据文件损坏，已恢复默认（原数据备份为 .bak）".into()),
            }
        }
    }
}

fn version_of(value: &serde_json::Value) -> u64 {
    value.get("version").and_then(|v| v.as_u64()).unwrap_or(0)
}

/// 原子写入：先写同目录临时文件，再 rename 覆盖。
pub fn save(state: &State) -> io::Result<()> {
    save_to(&data_path(), state)
}

fn save_to(path: &Path, state: &State) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(state).map_err(io::Error::other)?;
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, json)?;
    fs::rename(&tmp, path)
}

/// 导入：校验 → 迁移 → 清空机器相关的窗口状态
pub fn import(raw: &str) -> Result<State, String> {
    let value: serde_json::Value =
        serde_json::from_str(raw).map_err(|_| "导入失败：不是有效的 JSON".to_string())?;
    model::validate(&value).map_err(|err| format!("导入失败：{err}"))?;
    let mut state = model::migrate(&value);
    // 窗口坐标与机器相关：导入他人数据不应让窗口跳位
    state.window_pos = None;
    state.window_size = None;
    Ok(state)
}

/// 导出为格式化 JSON
pub fn export(state: &State) -> String {
    serde_json::to_string_pretty(state).unwrap_or_else(|_| "{}".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Note, Timetable};

    struct TempDir(PathBuf);

    impl TempDir {
        fn new(tag: &str) -> Self {
            let dir = std::env::temp_dir().join(format!(
                "piwidget-test-{tag}-{}-{}",
                std::process::id(),
                uuid::Uuid::new_v4()
            ));
            fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }

        fn file(&self) -> PathBuf {
            self.0.join(DATA_FILE)
        }

        fn bak(&self) -> PathBuf {
            self.0.join(format!("{DATA_FILE}{BACKUP_SUFFIX}"))
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn data_dir_matches_tauri_identifier() {
        let dir = data_dir();
        assert!(dir.ends_with(APP_IDENTIFIER), "got {dir:?}");
        assert!(!dir.to_string_lossy().contains("PiWidget/data"));
    }

    #[test]
    fn missing_file_yields_default_without_warning() {
        let temp = TempDir::new("missing");
        let loaded = load_from(&temp.file(), &temp.bak());
        assert_eq!(loaded.state, State::default());
        assert!(loaded.warning.is_none());
    }

    #[test]
    fn round_trips_state_and_preserves_fields() {
        let temp = TempDir::new("roundtrip");
        let mut state = State::default();
        state.ghost = true;
        state.notes.push(Note {
            id: "n1".into(),
            title: "买咖啡".into(),
            content: "燕麦拿铁 ×2\n记得带杯套".into(),
        });
        state.set_cell(1, 2, "高数");
        state.add_timetable();
        let active = state.active_timetable_id.clone();
        state.rename_timetable(&active, "这学期").unwrap();

        save_to(&temp.file(), &state).unwrap();
        let loaded = load_from(&temp.file(), &temp.bak());
        assert_eq!(loaded.state, state);
        assert!(loaded.warning.is_none());
    }

    #[test]
    fn corrupt_file_is_backed_up_and_reset() {
        let temp = TempDir::new("corrupt");
        fs::write(temp.file(), "{ this is not json").unwrap();
        let loaded = load_from(&temp.file(), &temp.bak());
        assert_eq!(loaded.state, State::default());
        assert_eq!(
            loaded.warning.as_deref(),
            Some("数据文件损坏，已恢复默认（原数据备份为 .bak）")
        );
        assert_eq!(
            fs::read_to_string(temp.bak()).unwrap(),
            "{ this is not json"
        );
    }

    #[test]
    fn unsupported_version_is_treated_as_corrupt() {
        let temp = TempDir::new("version");
        fs::write(temp.file(), r#"{"version":99,"notes":[]}"#).unwrap();
        let loaded = load_from(&temp.file(), &temp.bak());
        assert!(loaded.warning.is_some());
        assert_eq!(loaded.state, State::default());
    }

    #[test]
    fn empty_file_loads_default_silently() {
        let temp = TempDir::new("empty");
        fs::write(temp.file(), "   ").unwrap();
        let loaded = load_from(&temp.file(), &temp.bak());
        assert!(loaded.warning.is_none());
    }

    #[test]
    fn old_versions_are_migrated_on_load() {
        let temp = TempDir::new("migrate");
        fs::write(
            temp.file(),
            r#"{"version":1,"note":"旧版便笺内容","timetable":{"1":{"2":"旧课"}}}"#,
        )
        .unwrap();
        let loaded = load_from(&temp.file(), &temp.bak());
        assert_eq!(loaded.state.notes[0].content, "旧版便笺内容");
        assert_eq!(loaded.state.timetables[0].cells["1"]["2"], "旧课");
        assert_eq!(loaded.state.version, 5);
    }

    #[test]
    fn save_is_atomic_and_leaves_no_temp_file() {
        let temp = TempDir::new("atomic");
        save_to(&temp.file(), &State::default()).unwrap();
        save_to(&temp.file(), &State::default()).unwrap();
        assert!(temp.file().exists());
        assert!(!temp.0.join("data.json.tmp").exists());
        let entries: Vec<_> = fs::read_dir(&temp.0)
            .unwrap()
            .filter_map(Result::ok)
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(entries, vec![DATA_FILE.to_string()]);
    }

    #[test]
    fn import_rejects_bad_json_and_bad_schema() {
        assert_eq!(import("not json").unwrap_err(), "导入失败：不是有效的 JSON");
        assert_eq!(
            import(r#"{"version":2,"notes":"nope","timetable":{}}"#).unwrap_err(),
            "导入失败：notes 不是数组"
        );
    }

    #[test]
    fn import_clears_window_state() {
        let raw = r#"{
            "version": 4,
            "notes": [],
            "timetables": [{"id":"default","name":"默认","cells":{}}],
            "activeTimetableId": "default",
            "windowPos": {"x": 10, "y": 20},
            "windowSize": {"w": 800, "h": 600}
        }"#;
        let state = import(raw).unwrap();
        assert!(state.window_pos.is_none());
        assert!(state.window_size.is_none());
    }

    #[test]
    fn export_round_trips_through_import() {
        let mut state = State::default();
        state.notes.push(Note {
            id: "n1".into(),
            title: "标题".into(),
            content: "内容".into(),
        });
        state.timetables = vec![Timetable::empty("default", "默认")];
        let raw = export(&state);
        let back = import(&raw).unwrap();
        assert_eq!(back.notes, state.notes);
        assert_eq!(back.timetables, state.timetables);
    }
}
