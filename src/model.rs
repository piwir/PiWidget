/* PiWidget · 数据模型（schema v5）
 *
 * 磁盘格式：
 *   { version: 5, pinned, ghost, notes[],
 *     timetables[{id,name,cells{day:{period:课程名}}}], activeTimetableId,
 *     windowPos{x,y}, windowSize{w,h} }
 *
 * windowPos / windowSize 自 v5 起统一为**逻辑像素**（与 GPUI / 系统坐标一致）。
 * v4 及以前存的是物理像素，恢复时会被高 DPI 放大，因此 migrate() 丢弃旧的
 * windowSize（位置保留并按屏幕夹取），课表折叠字段 timetableCollapsed 也一并废弃
 * —— 课表不再占用右侧栏，改为便笺列表里的固定课表卡 + 独立窗口。
 *
 * v1（note 字符串）/ v2（单一 timetable）/ v3（多课表）/ v4（物理像素窗口状态）
 * 均在 migrate() 自动升到 v5。
 * 改 schema 必须同步：SCHEMA_VERSION、migrate()、validate()、tests/regression.rs。
 */

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

pub const SCHEMA_VERSION: u32 = 5;

/// 窗口左上角（逻辑像素，v5 起）
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct WindowPos {
    pub x: f64,
    pub y: f64,
}

/// 窗口尺寸（逻辑像素，v5 起）
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct WindowSize {
    pub w: f64,
    pub h: f64,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Note {
    pub id: String,
    pub title: String,
    pub content: String,
}

/// cells: day(1..=6) → period(1..=8) → 课程名
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Timetable {
    pub id: String,
    pub name: String,
    pub cells: BTreeMap<String, BTreeMap<String, String>>,
}

impl Timetable {
    pub fn empty(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            cells: BTreeMap::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct State {
    pub version: u32,
    /// 兼容保留：置顶按钮已随 GPUI 迁移移除，字段仍读写以免破坏旧文件。
    pub pinned: bool,
    pub ghost: bool,
    pub notes: Vec<Note>,
    pub timetables: Vec<Timetable>,
    pub active_timetable_id: String,
    pub window_pos: Option<WindowPos>,
    pub window_size: Option<WindowSize>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            version: SCHEMA_VERSION,
            pinned: false,
            ghost: false,
            notes: Vec::new(),
            timetables: vec![Timetable::empty("default", "默认")],
            active_timetable_id: "default".into(),
            window_pos: None,
            window_size: None,
        }
    }
}

impl State {
    pub fn active_timetable(&self) -> &Timetable {
        self.timetables
            .iter()
            .find(|t| t.id == self.active_timetable_id)
            .unwrap_or(&self.timetables[0])
    }

    pub fn active_timetable_mut(&mut self) -> &mut Timetable {
        let idx = self
            .timetables
            .iter()
            .position(|t| t.id == self.active_timetable_id)
            .unwrap_or(0);
        &mut self.timetables[idx]
    }

    pub fn cell(&self, day: u8, period: u8) -> Option<&String> {
        self.active_timetable()
            .cells
            .get(&day.to_string())
            .and_then(|d| d.get(&period.to_string()))
    }

    /// 今天的课节数（周日为 0，与既有 Hero 逻辑一致）
    pub fn today_period_count(&self, weekday_sun0: u8) -> usize {
        if weekday_sun0 == 0 {
            return 0;
        }
        self.active_timetable()
            .cells
            .get(&weekday_sun0.to_string())
            .map(|d| d.values().filter(|v| !v.is_empty()).count())
            .unwrap_or(0)
    }

    pub fn set_cell(&mut self, day: u8, period: u8, text: &str) {
        let text = text.trim();
        let tt = self.active_timetable_mut();
        let day_key = day.to_string();
        let period_key = period.to_string();
        if text.is_empty() {
            if let Some(day_cells) = tt.cells.get_mut(&day_key) {
                day_cells.remove(&period_key);
                if day_cells.is_empty() {
                    tt.cells.remove(&day_key);
                }
            }
            return;
        }
        tt.cells
            .entry(day_key)
            .or_default()
            .insert(period_key, text.to_string());
    }

    pub fn add_note(&mut self) -> String {
        let id = new_id();
        self.notes.push(Note {
            id: id.clone(),
            title: String::new(),
            content: String::new(),
        });
        id
    }

    pub fn delete_note(&mut self, id: &str) {
        self.notes.retain(|n| n.id != id);
    }

    pub fn move_note(&mut self, from: usize, to: usize) {
        if from == to || from >= self.notes.len() || to >= self.notes.len() {
            return;
        }
        let note = self.notes.remove(from);
        self.notes.insert(to, note);
    }

    pub fn set_active_timetable(&mut self, id: &str) {
        if self.timetables.iter().any(|t| t.id == id) {
            self.active_timetable_id = id.to_string();
        }
    }

    pub fn add_timetable(&mut self) -> &Timetable {
        let name = format!("课表 {}", self.timetables.len() + 1);
        let tt = Timetable::empty(new_id(), name);
        self.active_timetable_id = tt.id.clone();
        self.timetables.push(tt);
        self.timetables.last().expect("just pushed")
    }

    /// 返回 Ok(()) 或错误文案（空名拒绝，与既有交互一致）
    pub fn rename_timetable(&mut self, id: &str, name: &str) -> Result<(), &'static str> {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            return Err("名称不能为空");
        }
        if let Some(tt) = self.timetables.iter_mut().find(|t| t.id == id) {
            tt.name = trimmed.to_string();
        }
        Ok(())
    }

    pub fn delete_timetable(&mut self, id: &str) -> Result<(), &'static str> {
        if self.timetables.len() <= 1 {
            return Err("至少保留一个课表");
        }
        let Some(idx) = self.timetables.iter().position(|t| t.id == id) else {
            return Ok(());
        };
        self.timetables.remove(idx);
        if self.active_timetable_id == id {
            self.active_timetable_id = self.timetables[0].id.clone();
        }
        Ok(())
    }
}

pub fn new_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

fn str_field<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(Value::as_str)
}

/// 单元格清洗：day 仅 1..=6、period 仅 1..=8、值为字符串，空值剔除
fn clean_cells(raw: &Value) -> BTreeMap<String, BTreeMap<String, String>> {
    let mut out = BTreeMap::new();
    let Some(map) = raw.as_object() else {
        return out;
    };
    for (day, day_val) in map {
        if !matches!(day.as_str(), "1" | "2" | "3" | "4" | "5" | "6") {
            continue;
        }
        let Some(day_obj) = day_val.as_object() else {
            continue;
        };
        let mut periods = BTreeMap::new();
        for (period, cell) in day_obj {
            if !matches!(
                period.as_str(),
                "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8"
            ) {
                continue;
            }
            if let Some(text) = cell.as_str() {
                periods.insert(period.clone(), text.to_string());
            }
        }
        if !periods.is_empty() {
            out.insert(day.clone(), periods);
        }
    }
    out
}

fn clean_timetables(raw: &Value) -> Vec<Timetable> {
    let list: Vec<Timetable> = raw
        .as_array()
        .map(|items| {
            items
                .iter()
                .enumerate()
                .filter_map(|(i, item)| {
                    if !item.is_object() {
                        return None;
                    }
                    let id = str_field(item, "id")
                        .filter(|s| !s.is_empty())
                        .map(str::to_string)
                        .unwrap_or_else(|| format!("tt-{i}"));
                    let name = str_field(item, "name")
                        .filter(|s| !s.is_empty())
                        .map(str::to_string)
                        .unwrap_or_else(|| format!("课表 {}", i + 1));
                    let cells = item.get("cells").map(clean_cells).unwrap_or_default();
                    Some(Timetable { id, name, cells })
                })
                .collect()
        })
        .unwrap_or_default();
    if list.is_empty() {
        vec![Timetable::empty("default", "默认")]
    } else {
        list
    }
}

fn clean_notes(raw: &Value) -> Vec<Note> {
    raw.as_array()
        .map(|items| {
            items
                .iter()
                .filter(|n| n.is_object())
                .map(|n| Note {
                    id: str_field(n, "id")
                        .filter(|s| !s.is_empty())
                        .map(str::to_string)
                        .unwrap_or_else(new_id),
                    title: str_field(n, "title").unwrap_or_default().to_string(),
                    content: str_field(n, "content").unwrap_or_default().to_string(),
                })
                .collect()
        })
        .unwrap_or_default()
}

/// 归一化：v1 / v2 / v3 / v4 / v5 均接受，返回 v5 状态
pub fn migrate(data: &Value) -> State {
    let mut state = State::default();
    if !data.is_object() {
        return state;
    }

    if let Some(v) = data.get("pinned").and_then(Value::as_bool) {
        state.pinned = v;
    }
    if let Some(v) = data.get("ghost").and_then(Value::as_bool) {
        state.ghost = v;
    }
    // v4 及以前的 timetableCollapsed 直接忽略：课表不再有折叠态

    if data.get("timetables").is_some_and(Value::is_array) {
        state.timetables = clean_timetables(&data["timetables"]);
        state.active_timetable_id = data
            .get("activeTimetableId")
            .and_then(Value::as_str)
            .filter(|id| state.timetables.iter().any(|t| t.id == *id))
            .map(str::to_string)
            .unwrap_or_else(|| state.timetables[0].id.clone());
    } else {
        // v1 / v2：单一 timetable → 包进「默认」
        let cells = data.get("timetable").map(clean_cells).unwrap_or_default();
        state.timetables = vec![Timetable {
            id: "default".into(),
            name: "默认".into(),
            cells,
        }];
        state.active_timetable_id = "default".into();
    }

    if data.get("notes").is_some_and(Value::is_array) {
        state.notes = clean_notes(&data["notes"]);
    } else if let Some(note) = data.get("note").and_then(Value::as_str) {
        if !note.is_empty() {
            state.notes = vec![Note {
                id: new_id(),
                title: "便笺".into(),
                content: note.to_string(),
            }];
        }
    }

    let source_version = data.get("version").and_then(Value::as_u64).unwrap_or(0);
    // v4 及以前 windowSize 存的是物理像素（高 DPI 屏会翻倍），恢复时直接丢弃，
    // 由 app::restore_bounds 用屏幕尺寸夹取一个合适窗口；位置仍保留（同坐标空间）。
    state.window_pos = parse_window_pos(data.get("windowPos"));
    state.window_size = if source_version >= 5 {
        parse_window_size(data.get("windowSize"))
    } else {
        None
    };
    state.version = SCHEMA_VERSION;
    state
}

fn num(value: Option<&Value>) -> Option<f64> {
    value.and_then(Value::as_f64).filter(|n| n.is_finite())
}

fn parse_window_pos(value: Option<&Value>) -> Option<WindowPos> {
    let value = value?;
    if !value.is_object() {
        return None;
    }
    Some(WindowPos {
        x: num(value.get("x"))?,
        y: num(value.get("y"))?,
    })
}

fn parse_window_size(value: Option<&Value>) -> Option<WindowSize> {
    let value = value?;
    if !value.is_object() {
        return None;
    }
    let w = num(value.get("w"))?;
    let h = num(value.get("h"))?;
    if w <= 0.0 || h <= 0.0 {
        return None;
    }
    Some(WindowSize { w, h })
}

/// 导入校验：返回错误描述或 Ok(())（文案与 Vue 版 validate() 一致）
pub fn validate(data: &Value) -> Result<(), String> {
    if !data.is_object() {
        return Err("不是有效的对象".into());
    }
    let version = data.get("version").and_then(Value::as_u64);
    match version {
        Some(1) => {
            if !data.get("note").is_some_and(Value::is_string) {
                return Err("note 不是字符串".into());
            }
            if !data.get("timetable").is_some_and(Value::is_object) {
                return Err("timetable 不是对象".into());
            }
            Ok(())
        }
        Some(2) => {
            validate_notes(data)?;
            if !data.get("timetable").is_some_and(Value::is_object) {
                return Err("timetable 不是对象".into());
            }
            Ok(())
        }
        Some(3) => {
            validate_notes(data)?;
            if !data.get("timetables").is_some_and(Value::is_array) {
                return Err("timetables 不是数组".into());
            }
            Ok(())
        }
        Some(4) | Some(5) => {
            validate_notes(data)?;
            if !data.get("timetables").is_some_and(Value::is_array) {
                return Err("timetables 不是数组".into());
            }
            if let Some(pos) = data.get("windowPos") {
                if !pos.is_null() && !(pos.is_object() && parse_window_pos(Some(pos)).is_some()) {
                    return Err("windowPos 需为 null 或 {x, y} 数字".into());
                }
            }
            if let Some(size) = data.get("windowSize") {
                if !size.is_null() && !(size.is_object() && parse_window_size(Some(size)).is_some())
                {
                    return Err("windowSize 需为 null 或 {w, h} 数字".into());
                }
            }
            Ok(())
        }
        other => Err(format!(
            "不支持的 version（当前 {}）",
            match other {
                Some(v) => v.to_string(),
                None => "null".into(),
            }
        )),
    }
}

fn validate_notes(data: &Value) -> Result<(), String> {
    let Some(notes) = data.get("notes").and_then(Value::as_array) else {
        return Err("notes 不是数组".into());
    };
    for note in notes {
        let ok = note.is_object()
            && note.get("title").is_some_and(Value::is_string)
            && note.get("content").is_some_and(Value::is_string);
        if !ok {
            return Err("notes 元素需含字符串 title/content".into());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn default_state_matches_schema() {
        let state = State::default();
        assert_eq!(state.version, 5);
        assert_eq!(state.timetables.len(), 1);
        assert_eq!(state.timetables[0].id, "default");
        assert_eq!(state.active_timetable_id, "default");
        assert!(state.notes.is_empty());
        assert!(state.window_pos.is_none());
    }

    #[test]
    fn migrates_v1_note_string_and_single_timetable() {
        let raw = json!({
            "version": 1,
            "note": "旧版便笺内容",
            "timetable": { "1": { "2": "旧课" } }
        });
        assert!(validate(&raw).is_ok());
        let state = migrate(&raw);
        assert_eq!(state.version, 5);
        assert_eq!(state.notes.len(), 1);
        assert_eq!(state.notes[0].content, "旧版便笺内容");
        assert_eq!(state.notes[0].title, "便笺");
        assert_eq!(state.timetables.len(), 1);
        assert_eq!(state.timetables[0].name, "默认");
        assert_eq!(state.timetables[0].cells["1"]["2"], "旧课");
        assert_eq!(state.active_timetable_id, "default");
    }

    #[test]
    fn migrates_v2_single_timetable() {
        let raw = json!({
            "version": 2,
            "notes": [{"id": "n1", "title": "a", "content": "b"}],
            "timetable": { "2": { "3": "旧课" } }
        });
        assert!(validate(&raw).is_ok());
        let state = migrate(&raw);
        assert_eq!(state.notes.len(), 1);
        assert_eq!(state.timetables[0].cells["2"]["3"], "旧课");
        assert_eq!(state.timetables[0].name, "默认");
    }

    #[test]
    fn migrates_v3_and_fills_window_fields() {
        let raw = json!({
            "version": 3,
            "pinned": true,
            "ghost": true,
            "timetableCollapsed": true,
            "notes": [],
            "timetables": [
                {"id": "a", "name": "默认", "cells": {}},
                {"id": "b", "name": "这学期", "cells": {}}
            ],
            "activeTimetableId": "b"
        });
        assert!(validate(&raw).is_ok());
        let state = migrate(&raw);
        assert_eq!(state.version, 5);
        assert!(state.pinned && state.ghost);
        assert_eq!(state.timetables.len(), 2);
        assert_eq!(state.active_timetable_id, "b");
        assert!(state.window_pos.is_none());
        assert!(state.window_size.is_none());
    }

    #[test]
    fn falls_back_when_active_timetable_missing() {
        let raw = json!({
            "version": 3,
            "timetables": [{"id": "x", "name": "X", "cells": {}}],
            "activeTimetableId": "nope"
        });
        let state = migrate(&raw);
        assert_eq!(state.active_timetable_id, "x");
    }

    #[test]
    fn empty_timetables_falls_back_to_default() {
        let raw = json!({"version": 3, "timetables": []});
        let state = migrate(&raw);
        assert_eq!(state.timetables.len(), 1);
        assert_eq!(state.timetables[0].id, "default");
    }

    #[test]
    fn drops_invalid_cells_and_keeps_valid_ones() {
        let raw = json!({
            "version": 2,
            "notes": [],
            "timetable": {
                "1": {"1": "语文", "9": "越界", "2": 3},
                "7": {"1": "非课程日"},
                "2": {"8": "第八节"}
            }
        });
        let state = migrate(&raw);
        let cells = &state.timetables[0].cells;
        assert_eq!(cells.len(), 2);
        assert_eq!(cells["1"]["1"], "语文");
        assert!(!cells["1"].contains_key("9"));
        assert!(!cells["1"].contains_key("2"));
        assert!(!cells.contains_key("7"));
        assert_eq!(cells["2"]["8"], "第八节");
    }

    #[test]
    fn keeps_v5_window_state() {
        let raw = json!({
            "version": 5,
            "notes": [],
            "timetables": [{"id": "default", "name": "默认", "cells": {}}],
            "activeTimetableId": "default",
            "windowPos": {"x": 120.0, "y": 80.0},
            "windowSize": {"w": 760.0, "h": 540.0}
        });
        assert!(validate(&raw).is_ok());
        let state = migrate(&raw);
        assert_eq!(state.window_pos, Some(WindowPos { x: 120.0, y: 80.0 }));
        assert_eq!(state.window_size, Some(WindowSize { w: 760.0, h: 540.0 }));
    }

    /// v4 的 windowSize 是物理像素（高 DPI 会翻倍），恢复时会让窗口开得巨大，
    /// 因此升级到 v5 时直接丢弃尺寸、只保留位置。
    #[test]
    fn drops_v4_physical_window_size() {
        let raw = json!({
            "version": 4,
            "notes": [],
            "timetables": [{"id": "default", "name": "默认", "cells": {}}],
            "activeTimetableId": "default",
            "timetableCollapsed": true,
            "windowPos": {"x": 892.4, "y": 44.4},
            "windowSize": {"w": 1504.7, "h": 651.3}
        });
        assert!(validate(&raw).is_ok());
        let state = migrate(&raw);
        assert_eq!(state.version, 5);
        assert_eq!(state.window_pos, Some(WindowPos { x: 892.4, y: 44.4 }));
        assert!(state.window_size.is_none(), "旧物理像素尺寸必须丢弃");
    }

    #[test]
    fn rejects_invalid_shapes() {
        assert_eq!(validate(&json!(null)).unwrap_err(), "不是有效的对象");
        assert_eq!(
            validate(&json!({"version": 1, "note": 3, "timetable": {}})).unwrap_err(),
            "note 不是字符串"
        );
        assert_eq!(
            validate(&json!({"version": 2, "notes": [{}], "timetable": {}})).unwrap_err(),
            "notes 元素需含字符串 title/content"
        );
        assert_eq!(
            validate(&json!({"version": 3, "notes": [], "timetables": {}})).unwrap_err(),
            "timetables 不是数组"
        );
        assert!(validate(&json!({"version": 9}))
            .unwrap_err()
            .contains("不支持的 version"));
        assert_eq!(
            validate(&json!({
                "version": 4, "notes": [],
                "timetables": [],
                "windowPos": {"x": "a", "y": 1}
            }))
            .unwrap_err(),
            "windowPos 需为 null 或 {x, y} 数字"
        );
        assert_eq!(
            validate(&json!({
                "version": 4, "notes": [],
                "timetables": [],
                "windowSize": {"w": 0, "h": 10}
            }))
            .unwrap_err(),
            "windowSize 需为 null 或 {w, h} 数字"
        );
    }

    #[test]
    fn set_cell_trims_and_clears() {
        let mut state = State::default();
        state.set_cell(1, 2, "  高数  ");
        assert_eq!(state.cell(1, 2).map(String::as_str), Some("高数"));
        state.set_cell(1, 2, "   ");
        assert_eq!(state.cell(1, 2), None);
        assert!(state.timetables[0].cells.is_empty());
    }

    #[test]
    fn today_period_count_ignores_sunday() {
        let mut state = State::default();
        state.set_cell(3, 1, "物理");
        state.set_cell(3, 2, "物理");
        assert_eq!(state.today_period_count(3), 2);
        assert_eq!(state.today_period_count(0), 0);
        assert_eq!(state.today_period_count(5), 0);
    }

    #[test]
    fn note_operations_keep_order() {
        let mut state = State::default();
        let a = state.add_note();
        let b = state.add_note();
        let c = state.add_note();
        state.notes[0].title = "A".into();
        state.notes[1].title = "B".into();
        state.notes[2].title = "C".into();
        state.move_note(0, 2);
        let titles: Vec<_> = state.notes.iter().map(|n| n.title.as_str()).collect();
        assert_eq!(titles, vec!["B", "C", "A"]);
        state.delete_note(&b);
        assert_eq!(state.notes.len(), 2);
        assert!(state.notes.iter().all(|n| n.id != b));
        let _ = (a, c);
    }

    #[test]
    fn timetable_management_rules() {
        let mut state = State::default();
        let tt = state.add_timetable();
        let new_id = tt.id.clone();
        assert_eq!(state.active_timetable_id, new_id);
        assert_eq!(state.timetables.len(), 2);
        assert_eq!(
            state.rename_timetable(&new_id, "   ").unwrap_err(),
            "名称不能为空"
        );
        state.rename_timetable(&new_id, " 这学期 ").unwrap();
        assert_eq!(state.active_timetable().name, "这学期");
        state.delete_timetable(&new_id).unwrap();
        assert_eq!(state.timetables.len(), 1);
        assert_eq!(
            state.delete_timetable("default").unwrap_err(),
            "至少保留一个课表"
        );
    }

    #[test]
    fn serializes_with_camel_case_field_names() {
        let state = State::default();
        let json = serde_json::to_string(&state).unwrap();
        assert!(json.contains("\"activeTimetableId\""));
        assert!(json.contains("\"windowPos\""));
        assert!(json.contains("\"windowSize\""));
        assert!(json.contains("\"version\":5"));
        // 课表折叠态在 v5 已移除，不再写回磁盘
        assert!(!json.contains("timetableCollapsed"));
    }
}
