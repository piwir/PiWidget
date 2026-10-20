/* PiWidget · 根视图
 *
 * 墨色卡片 + 双栏（便笺 42% / 课表 58%）；折叠时课表条跳到底部、左栏占满整行。
 * 状态变更走 800ms 防抖落盘，Ctrl/⌘+S 立即落盘。
 */

use std::time::Duration;

use gpui_kit::component::input::InputEvent as InputEventKind;
use gpui_kit::component::{
    Disableable as _, Sizable as _, Theme, ThemeMode,
    button::{Button, ButtonVariants as _},
    h_flex,
    input::{Input, InputState, Textarea, TextareaState},
    v_flex,
};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::actions::{CancelEdit, SaveNow};
use crate::model::{State, WindowPos, WindowSize};
use crate::storage;
use crate::theme as tokens;

const SAVE_DEBOUNCE: Duration = Duration::from_millis(800);
/// 折叠后窗口宽 = 展开宽 × 0.42（与双栏比例一致）
const COLLAPSED_RATIO: f32 = 0.42;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Editing {
    None,
    Title(usize),
    Body(usize),
    Cell { day: u8, period: u8 },
    TimetableName,
}

pub struct PiWidget {
    state: State,
    expanded: Option<usize>,
    editing: Editing,
    title_input: Option<Entity<InputState>>,
    body_input: Option<Entity<TextareaState>>,
    cell_input: Option<Entity<InputState>>,
    name_input: Option<Entity<InputState>>,
    toast: Option<(String, bool)>,
    toast_seq: u64,
    dirty: bool,
    autostart_on: bool,
    autostart_available: bool,
    expanded_width: Option<f32>,
    focus_handle: FocusHandle,
    /// 内联编辑的事件订阅（Enter / 失焦提交）
    edit_subscription: Option<Subscription>,
    /// 防抖落盘任务（保留句柄以便取消）
    save_task: Option<Task<()>>,
}

impl PiWidget {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let loaded = storage::load();
        let mut this = Self {
            expanded_width: loaded.state.window_size.map(|s| s.w as f32),
            state: loaded.state,
            expanded: None,
            editing: Editing::None,
            title_input: None,
            body_input: None,
            cell_input: None,
            name_input: None,
            toast: None,
            toast_seq: 0,
            dirty: false,
            autostart_on: crate::autostart::is_enabled(),
            autostart_available: crate::autostart::is_supported(),
            focus_handle: cx.focus_handle(),
            edit_subscription: None,
            save_task: None,
        };
        this.focus_handle.focus(window, cx);

        if let Some(warning) = loaded.warning {
            this.show_toast(warning, true, window, cx);
        }
        this
    }

    // ---------- 只读访问 ----------

    pub fn state(&self) -> &State {
        &self.state
    }

    pub fn toast_message(&self) -> Option<&str> {
        self.toast.as_ref().map(|(m, _)| m.as_str())
    }

    pub fn editing(&self) -> Editing {
        self.editing
    }

    pub fn expanded(&self) -> Option<usize> {
        self.expanded
    }

    pub fn autostart_on(&self) -> bool {
        self.autostart_on
    }

    pub fn autostart_available(&self) -> bool {
        self.autostart_available
    }

    // ---------- 保存 ----------

    fn mark_dirty(&mut self, cx: &mut Context<Self>) {
        self.dirty = true;
        cx.notify();
        let entity = cx.entity().downgrade();
        let task = cx.spawn(async move |_, cx| {
            cx.background_executor().timer(SAVE_DEBOUNCE).await;
            entity
                .update(cx, |this, cx| {
                    if this.dirty {
                        this.save_now(cx);
                    }
                })
                .ok();
        });
        self.save_task = Some(task);
    }

    pub fn save_now(&mut self, cx: &mut Context<Self>) {
        match storage::save(&self.state) {
            Ok(()) => self.dirty = false,
            Err(err) => {
                self.toast = Some((format!("保存失败：{err}"), true));
                self.toast_seq += 1;
            }
        }
        cx.notify();
    }

    pub fn save_action(&mut self, _: &SaveNow, window: &mut Window, cx: &mut Context<Self>) {
        match storage::save(&self.state) {
            Ok(()) => {
                self.dirty = false;
                self.show_toast("已保存", false, window, cx);
            }
            Err(err) => self.show_toast(format!("保存失败：{err}"), true, window, cx),
        }
        cx.notify();
    }

    pub fn cancel_action(&mut self, _: &CancelEdit, _: &mut Window, cx: &mut Context<Self>) {
        if self.editing != Editing::None {
            self.editing = Editing::None;
            self.title_input = None;
            self.body_input = None;
            self.cell_input = None;
            self.name_input = None;
            self.edit_subscription = None;
            cx.notify();
        }
    }

    // ---------- toast ----------

    pub fn show_toast(
        &mut self,
        message: impl Into<String>,
        error: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.toast = Some((message.into(), error));
        self.toast_seq += 1;
        let seq = self.toast_seq;
        cx.notify();

        let entity = cx.entity().downgrade();
        let window_handle = window.window_handle();
        cx.spawn(async move |_, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(2500))
                .await;
            cx.update_window(window_handle, |_, _, cx| {
                entity
                    .update(cx, |this, cx| {
                        if this.toast_seq == seq {
                            this.toast = None;
                            cx.notify();
                        }
                    })
                    .ok();
            })
            .ok();
        })
        .detach();
    }

    // ---------- 便笺 ----------

    pub fn add_note(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.expanded.is_some() {
            self.commit_current(window, cx);
        }
        self.state.add_note();
        let index = self.state.notes.len() - 1;
        self.expanded = Some(index);
        self.mark_dirty(cx);
        self.begin_title_edit(window, cx);
    }

    pub fn toggle_note(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if self.expanded == Some(index) {
            self.commit_current(window, cx);
            self.expanded = None;
            self.editing = Editing::None;
        } else {
            if self.expanded.is_some() {
                self.commit_current(window, cx);
            }
            self.expanded = Some(index);
            self.editing = Editing::None;
        }
        cx.notify();
    }

    pub fn delete_note(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if index >= self.state.notes.len() {
            return;
        }
        self.state.notes.remove(index);
        self.expanded = None;
        self.editing = Editing::None;
        self.title_input = None;
        self.body_input = None;
        self.edit_subscription = None;
        self.mark_dirty(cx);
        self.show_toast("已删除", false, window, cx);
    }

    pub fn move_note(&mut self, from: usize, to: usize, cx: &mut Context<Self>) {
        if let Some(current) = self.expanded {
            if from == current {
                self.expanded = Some(to);
            } else if from < current && to >= current {
                self.expanded = Some(current - 1);
            } else if from > current && to <= current {
                self.expanded = Some(current + 1);
            }
        }
        self.state.move_note(from, to);
        self.mark_dirty(cx);
    }

    pub fn commit_current(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match self.editing {
            Editing::Title(index) => {
                if let Some(input) = self.title_input.take() {
                    let value = input.read(cx).value().to_string();
                    if let Some(note) = self.state.notes.get_mut(index) {
                        note.title = value;
                    }
                    self.mark_dirty(cx);
                }
            }
            Editing::Body(index) => {
                if let Some(input) = self.body_input.take() {
                    let value = input.read(cx).value().to_string();
                    if let Some(note) = self.state.notes.get_mut(index) {
                        note.content = value;
                    }
                    self.mark_dirty(cx);
                }
            }
            _ => {}
        }
        self.editing = Editing::None;
        self.edit_subscription = None;
        let _ = window;
        cx.notify();
    }

    pub fn begin_title_edit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(index) = self.expanded else {
            return;
        };
        let Some(note) = self.state.notes.get(index) else {
            return;
        };
        let text = note.title.clone();
        let input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("标题")
                .default_value(text)
        });
        input.update(cx, |state, cx| state.focus(window, cx));
        self.edit_subscription = Some(cx.subscribe_in(
            &input,
            window,
            move |this, _, event: &InputEventKind, window, cx| match event {
                InputEventKind::PressEnter { .. } | InputEventKind::Blur => {
                    this.commit_current(window, cx);
                }
                _ => {}
            },
        ));
        self.title_input = Some(input);
        self.editing = Editing::Title(index);
        cx.notify();
    }

    pub fn begin_body_edit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(index) = self.expanded else {
            return;
        };
        let Some(note) = self.state.notes.get(index) else {
            return;
        };
        let text = note.content.clone();
        let input = cx.new(|cx| {
            TextareaState::new(window, cx)
                .placeholder("写点什么…")
                .default_value(text)
        });
        input.update(cx, |state, cx| state.focus(window, cx));
        self.edit_subscription = Some(cx.subscribe_in(
            &input,
            window,
            move |this, _, event: &InputEventKind, window, cx| {
                if matches!(event, InputEventKind::Blur) {
                    this.commit_current(window, cx);
                }
            },
        ));
        self.body_input = Some(input);
        self.editing = Editing::Body(index);
        cx.notify();
    }

    // ---------- 课表 ----------

    pub fn begin_cell_edit(
        &mut self,
        day: u8,
        period: u8,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let text = self.state.cell(day, period).cloned().unwrap_or_default();
        let input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("课程名")
                .default_value(text)
        });
        input.update(cx, |state, cx| state.focus(window, cx));
        self.edit_subscription = Some(cx.subscribe_in(
            &input,
            window,
            move |this, _, event: &InputEventKind, window, cx| match event {
                InputEventKind::PressEnter { .. } | InputEventKind::Blur => {
                    this.commit_cell_edit(day, period, window, cx);
                }
                _ => {}
            },
        ));
        self.cell_input = Some(input);
        self.editing = Editing::Cell { day, period };
        cx.notify();
    }

    pub fn commit_cell_edit(
        &mut self,
        day: u8,
        period: u8,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(input) = self.cell_input.take() {
            let value = input.read(cx).value().to_string();
            self.state.set_cell(day, period, &value);
            self.mark_dirty(cx);
        }
        self.editing = Editing::None;
        self.edit_subscription = None;
        let _ = window;
        cx.notify();
    }

    pub fn set_active_timetable(&mut self, id: &str, cx: &mut Context<Self>) {
        self.state.set_active_timetable(id);
        self.mark_dirty(cx);
    }

    pub fn add_timetable(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let name = self.state.add_timetable().name.clone();
        self.mark_dirty(cx);
        self.show_toast(format!("已新建「{name}」"), false, window, cx);
    }

    /// 多课表循环切换（无下拉组件时的键盘可达替代）
    pub fn cycle_timetable(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let ids: Vec<String> = self.state.timetables.iter().map(|t| t.id.clone()).collect();
        let Some(pos) = ids
            .iter()
            .position(|id| *id == self.state.active_timetable_id)
        else {
            return;
        };
        let next = ids[(pos + 1) % ids.len()].clone();
        self.set_active_timetable(&next, cx);
        let name = self.state.active_timetable().name.clone();
        self.show_toast(format!("已切换到「{name}」"), false, window, cx);
    }

    pub fn begin_rename(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let name = self.state.active_timetable().name.clone();
        let input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("课表名称")
                .default_value(name)
        });
        input.update(cx, |state, cx| state.focus(window, cx));
        self.edit_subscription = Some(cx.subscribe_in(
            &input,
            window,
            move |this, _, event: &InputEventKind, window, cx| match event {
                InputEventKind::PressEnter { .. } | InputEventKind::Blur => {
                    this.commit_rename(window, cx);
                }
                _ => {}
            },
        ));
        self.name_input = Some(input);
        self.editing = Editing::TimetableName;
        cx.notify();
    }

    pub fn commit_rename(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(input) = self.name_input.take() else {
            return;
        };
        let value = input.read(cx).value().to_string();
        let id = self.state.active_timetable_id.clone();
        match self.state.rename_timetable(&id, &value) {
            Ok(()) => {
                let name = self.state.active_timetable().name.clone();
                self.mark_dirty(cx);
                self.show_toast(format!("已重命名为「{name}」"), false, window, cx);
            }
            Err(err) => self.show_toast(err, true, window, cx),
        }
        self.editing = Editing::None;
        self.edit_subscription = None;
        cx.notify();
    }

    pub fn delete_timetable(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let id = self.state.active_timetable_id.clone();
        let name = self.state.active_timetable().name.clone();
        match self.state.delete_timetable(&id) {
            Ok(()) => {
                self.mark_dirty(cx);
                self.show_toast(format!("已删除「{name}」"), false, window, cx);
            }
            Err(err) => self.show_toast(err, true, window, cx),
        }
        cx.notify();
    }

    pub fn toggle_collapse(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.state.timetable_collapsed = !self.state.timetable_collapsed;
        self.mark_dirty(cx);
        self.apply_collapse_resize(window);
        cx.notify();
    }

    /// 收起：窗口宽收缩到 展开宽 × 0.42；展开：恢复已记录的展开宽度。
    fn apply_collapse_resize(&mut self, window: &mut Window) {
        let current = window.bounds().size;
        let scale = window.scale_factor();
        let current_width = f32::from(current.width) / scale;
        let height = f32::from(current.height) / scale;

        if self.state.timetable_collapsed {
            self.expanded_width = Some(current_width);
            let target = (current_width * COLLAPSED_RATIO).max(200.0);
            window.resize(size(px(target * scale), px(height * scale)));
        } else {
            let target = self
                .expanded_width
                .or(self.state.window_size.map(|s| s.w as f32 / scale))
                .unwrap_or(tokens::DESIGN_WIDTH)
                .max(tokens::MIN_EXPANDED_WIDTH);
            window.resize(size(px(target * scale), px(height * scale)));
        }
    }

    // ---------- 外观 / 自启 / 导入导出 ----------

    pub fn toggle_ghost(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.state.ghost = !self.state.ghost;
        self.mark_dirty(cx);
        let message = if self.state.ghost {
            "已透明化"
        } else {
            "已恢复显示"
        };
        self.show_toast(message, false, window, cx);
        cx.notify();
    }

    pub fn toggle_autostart(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.autostart_available {
            self.show_toast("开机自启仅桌面端可用", true, window, cx);
            return;
        }
        match crate::autostart::toggle(self.autostart_on) {
            Ok(enabled) => {
                self.autostart_on = enabled;
                let message = if enabled {
                    "已开启开机自启"
                } else {
                    "已关闭开机自启"
                };
                self.show_toast(message, false, window, cx);
            }
            Err(err) => self.show_toast(format!("自启设置失败：{err}"), true, window, cx),
        }
        cx.notify();
    }

    pub fn export_data(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let json = storage::export(&self.state);
        let dir = storage::data_dir();
        let receiver = cx.prompt_for_new_path(&dir, Some("piwidget-data.json"));
        let window_handle = window.window_handle();
        cx.spawn(async move |this, cx| {
            let selection = receiver.await.ok().and_then(Result::ok).flatten();
            let Some(path) = selection else {
                return;
            };
            let result = std::fs::write(&path, json);
            let file_name = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            cx.update_window(window_handle, |_, window, cx| {
                this.update(cx, |this, cx| match result {
                    Ok(()) => this.show_toast(format!("已导出 {file_name}"), false, window, cx),
                    Err(err) => this.show_toast(format!("导出失败：{err}"), true, window, cx),
                })
                .ok();
            })
            .ok();
        })
        .detach();
    }

    pub fn import_data(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let options = PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("导入 PiWidget 数据".into()),
        };
        let receiver = cx.prompt_for_paths(options);
        let window_handle = window.window_handle();
        cx.spawn(async move |this, cx| {
            let selection = receiver.await.ok().and_then(Result::ok).flatten();
            let Some(paths) = selection else {
                return;
            };
            let Some(path) = paths.into_iter().next() else {
                return;
            };
            let raw = std::fs::read_to_string(&path);
            cx.update_window(window_handle, |_, window, cx| {
                this.update(cx, |this, cx| match raw {
                    Ok(raw) => match storage::import(&raw) {
                        Ok(state) => {
                            this.state = state;
                            this.expanded = None;
                            this.editing = Editing::None;
                            this.title_input = None;
                            this.body_input = None;
                            this.mark_dirty(cx);
                            this.save_now(cx);
                            this.show_toast("已导入", false, window, cx);
                        }
                        Err(err) => this.show_toast(err, true, window, cx),
                    },
                    Err(err) => this.show_toast(format!("导入失败：{err}"), true, window, cx),
                })
                .ok();
            })
            .ok();
        })
        .detach();
    }

    // ---------- 窗口状态 ----------

    pub fn observe_window(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        cx.observe_window_bounds(window, |this, window, cx| {
            let bounds = window.bounds();
            let scale = window.scale_factor();
            let width = f32::from(bounds.size.width) / scale;
            let height = f32::from(bounds.size.height) / scale;
            let origin = bounds.origin;
            let scale64 = scale as f64;

            this.state.window_pos = Some(WindowPos {
                x: f64::from(origin.x) / scale64,
                y: f64::from(origin.y) / scale64,
            });
            // 收起态手动拖宽不污染展开尺寸（只更新高度）
            if this.state.timetable_collapsed {
                let saved_w = this
                    .state
                    .window_size
                    .map(|s| s.w)
                    .unwrap_or(width as f64 * scale64);
                this.state.window_size = Some(WindowSize {
                    w: saved_w,
                    h: height as f64 * scale64,
                });
            } else {
                this.state.window_size = Some(WindowSize {
                    w: width as f64 * scale64,
                    h: height as f64 * scale64,
                });
                this.expanded_width = Some(width);
            }
            this.mark_dirty(cx);
        })
        .detach();
    }

    // ---------- 测试辅助（integration test 专用） ----------

    pub fn commit_current_public(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.commit_current(window, cx);
    }

    pub fn commit_rename_for_test(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.commit_rename(window, cx);
    }

    pub fn set_note_title_for_test(&mut self, index: usize, title: &str) {
        if let Some(note) = self.state.notes.get_mut(index) {
            note.title = title.to_string();
        }
    }

    pub fn set_note_content_for_test(&mut self, index: usize, content: &str) {
        if let Some(note) = self.state.notes.get_mut(index) {
            note.content = content.to_string();
        }
    }

    pub fn set_cell_for_test(
        &mut self,
        day: u8,
        period: u8,
        text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.state.set_cell(day, period, text);
        self.mark_dirty(cx);
        let _ = window;
    }

    /// 写入当前内联编辑框。
    /// 库的 Input/Textarea 未向 test-support 注册观测，测试无法用 `window.input` 驱动，
    /// 这里直接写 InputState（等价于用户键入后的状态）。
    pub fn set_edit_value(&mut self, window: &mut Window, cx: &mut Context<Self>, value: &str) {
        match self.editing {
            Editing::Title(_) => {
                if let Some(input) = self.title_input.clone() {
                    input.update(cx, |state, cx| state.set_value(value.to_string(), window, cx));
                }
            }
            Editing::Body(_) => {
                if let Some(input) = self.body_input.clone() {
                    input.update(cx, |state, cx| state.set_value(value.to_string(), window, cx));
                }
            }
            Editing::Cell { .. } => {
                if let Some(input) = self.cell_input.clone() {
                    input.update(cx, |state, cx| state.set_value(value.to_string(), window, cx));
                }
            }
            Editing::TimetableName => {
                if let Some(input) = self.name_input.clone() {
                    input.update(cx, |state, cx| state.set_value(value.to_string(), window, cx));
                }
            }
            Editing::None => {}
        }
    }
}

/// 计算启动窗口边界：恢复上次位置与尺寸，位置带屏外守卫。
pub fn restored_bounds(cx: &App) -> WindowBounds {
    let loaded = storage::load();
    let size_w = loaded
        .state
        .window_size
        .map(|s| s.w as f32)
        .unwrap_or(tokens::DESIGN_WIDTH);
    let size_h = loaded
        .state
        .window_size
        .map(|s| s.h as f32)
        .unwrap_or(tokens::DESIGN_HEIGHT);

    let displays: Vec<Bounds<Pixels>> = cx.displays().iter().map(|d| d.bounds()).collect();
    let origin = loaded
        .state
        .window_pos
        .filter(|pos| on_screen(pos, size_w, size_h, &displays))
        .map(|pos| point(px(pos.x as f32), px(pos.y as f32)));

    let bounds = Bounds {
        origin: origin.unwrap_or_else(|| point(px(120.0), px(120.0))),
        size: size(px(size_w), px(size_h)),
    };
    WindowBounds::Windowed(bounds)
}

/// 位置与任一屏幕相交才恢复（防拔显示器 / 分辨率变化后落到屏外）
fn on_screen(pos: &WindowPos, w: f32, h: f32, displays: &[Bounds<Pixels>]) -> bool {
    let x = pos.x as f32;
    let y = pos.y as f32;
    displays.iter().any(|bounds| {
        let left = f32::from(bounds.origin.x);
        let top = f32::from(bounds.origin.y);
        let right = left + f32::from(bounds.size.width);
        let bottom = top + f32::from(bounds.size.height);
        x < right && x + w > left && y < bottom && y + h > top
    })
}

// ---------- 主题 ----------

/// 把六色 token 灌进 GPUI Kit 主题，避免内置控件出现第二套配色。
pub fn install_theme(cx: &mut App) {
    Theme::change(ThemeMode::Dark, None, cx);
    let ink = tokens::hsla(tokens::INK);
    let card = tokens::hsla(tokens::CARD);
    let text = tokens::hsla(tokens::TEXT);
    let muted = tokens::hsla(tokens::MUTED);
    let line = tokens::hsla(tokens::LINE);
    let highlight = tokens::hsla(tokens::HIGHLIGHT);

    Theme::update(cx, |theme| {
        theme.colors.background = ink;
        theme.colors.foreground = text;
        theme.colors.border = line;
        theme.colors.muted = muted;
        theme.colors.muted_foreground = muted;
        theme.colors.primary = highlight;
        theme.colors.primary_foreground = ink;
        theme.colors.secondary = card;
        theme.colors.secondary_foreground = text;
        theme.colors.accent = highlight;
        theme.colors.accent_foreground = ink;
        theme.colors.input = card;
        theme.colors.popover = card;
        theme.colors.popover_foreground = text;
        theme.colors.ring = highlight;
        theme.colors.selection = tokens::hsla(tokens::mix(tokens::INK, tokens::HIGHLIGHT, 0.35));
        theme.radius = px(tokens::RADIUS);
        theme.radius_lg = px(tokens::RADIUS);
        theme.focus_ring = true;
    });
}

// ---------- 渲染 ----------

impl Focusable for PiWidget {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for PiWidget {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let collapsed = self.state.timetable_collapsed;
        let opacity = if self.state.ghost {
            tokens::FADED
        } else {
            1.0
        };

        v_flex()
            .id("sheet")
            .test_support()
            .size_full()
            .relative()
            .rounded(px(tokens::RADIUS))
            .bg(tokens::hsla(tokens::INK))
            .border_1()
            .border_color(tokens::hsla(tokens::sheet_border()))
            .overflow_hidden()
            .opacity(opacity)
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(Self::save_action))
            .on_action(cx.listener(Self::cancel_action))
            .child(self.render_titlebar(window, cx))
            .child(self.render_body(window, cx, collapsed))
            .when_some(self.toast.clone(), |this, (message, error)| {
                this.child(self.render_toast(message, error))
            })
    }
}

impl PiWidget {
    fn render_titlebar(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let ghost = self.state.ghost;
        let autostart = self.autostart_on;

        let actions = h_flex()
            .gap_1()
            .child(icon_button(
                "ghost",
                ghost,
                "透明化",
                IconShape::Ghost,
                cx.listener(|this, _, window, cx| this.toggle_ghost(window, cx)),
            ))
            .child(icon_button(
                "autostart",
                autostart,
                "开机自启",
                IconShape::Power,
                cx.listener(|this, _, window, cx| this.toggle_autostart(window, cx)),
            ))
            .child(icon_button(
                "export",
                false,
                "导出数据",
                IconShape::Export,
                cx.listener(|this, _, window, cx| this.export_data(window, cx)),
            ))
            .child(icon_button(
                "import",
                false,
                "导入数据",
                IconShape::Import,
                cx.listener(|this, _, window, cx| this.import_data(window, cx)),
            ));

        h_flex()
            .id("titlebar")
            .flex_none()
            .h(px(tokens::TITLEBAR_HEIGHT))
            .px_4()
            .items_center()
            .justify_between()
            .border_b_1()
            .border_color(tokens::hsla(tokens::LINE))
            .child(
                // 拖拽区：品牌字 + 空白填充；按钮必须在其外部
                h_flex()
                    .id("drag-region")
                    .flex_1()
                    .h_full()
                    .items_center()
                    .window_control_area(WindowControlArea::Drag)
                    .child(
                        div()
                            .id("brand")
                            .test_support()
                            .aria_label("PIWIDGET")
                            .text_size(px(19.0))
                            .font_weight(FontWeight::BOLD)
                            .text_color(tokens::hsla(tokens::TEXT))
                            .child("PIWIDGET"),
                    ),
            )
            .child(actions)
    }

    fn render_body(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        collapsed: bool,
    ) -> impl IntoElement {
        if collapsed {
            let left = v_flex()
                .id("left-column")
                .test_support()
                .flex_1()
                .min_w_0()
                .w_full()
                .child(self.render_hero(cx))
                .child(self.render_notes(window, cx));

            v_flex()
                .id("body")
                .flex_1()
                .min_h_0()
                .child(left)
                .child(self.render_timetable(window, cx, true))
        } else {
            let left = v_flex()
                .id("left-column")
                .test_support()
                .flex_none()
                .w(relative(0.42))
                .min_w_0()
                .h_full()
                .border_r_1()
                .border_color(tokens::hsla(tokens::LINE))
                .child(self.render_hero(cx))
                .child(self.render_notes(window, cx));

            let right = div()
                .id("right-column")
                .test_support()
                .flex_none()
                .w(relative(0.58))
                .min_w_0()
                .h_full()
                .child(self.render_timetable(window, cx, false));

            h_flex()
                .id("body")
                .flex_1()
                .min_h_0()
                .child(left)
                .child(right)
        }
    }

    fn render_hero(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let now = now();
        let (eyebrow, day_cn) = hero_labels(&now);
        let day_count = self.state.today_period_count(now.weekday_sun0);
        let meta = if day_count > 0 {
            format!("今天 {day_count} 节")
        } else {
            "今日无课 · 好好休息".to_string()
        };
        let _ = cx;

        v_flex()
            .id("hero")
            .flex_none()
            .items_center()
            .pt(px(22.0))
            .pb(px(14.0))
            .border_b_1()
            .border_color(tokens::hsla(tokens::LINE))
            .child(
                div()
                    .id("eyebrow")
                    .test_support()
                    .aria_label(eyebrow.clone())
                    .text_size(px(12.0))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(tokens::hsla(tokens::MUTED))
                    .child(eyebrow),
            )
            .child(
                v_flex()
                    .items_center()
                    .mt(px(2.0))
                    .mb(px(6.0))
                    .child(
                        div()
                            .id("hero-day")
                            .test_support()
                            .aria_label(day_cn.clone())
                            .text_size(px(76.0))
                            .line_height(px(0.9 * 76.0))
                            .font_weight(FontWeight::BOLD)
                            .text_color(tokens::hsla(tokens::TEXT))
                            .child(day_cn),
                    )
                    // 签名：荧光笔下划线（静态，替代 CSS sweep 动画）
                    .child(
                        div()
                            .id("highlight-bar")
                            .w(px(96.0))
                            .h(px(4.0))
                            .mt(px(4.0))
                            .rounded(px(2.0))
                            .bg(tokens::hsla(tokens::HIGHLIGHT)),
                    ),
            )
            .child(
                div()
                    .id("hero-meta")
                    .test_support()
                    .aria_label(meta.clone())
                    .text_size(px(13.0))
                    .text_color(tokens::hsla(tokens::MUTED))
                    .child(meta),
            )
    }

    fn render_notes(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut list = v_flex()
            .id("notes")
            .flex_1()
            .min_h_0()
            .p_3()
            .gap_2()
            .items_start()
            .overflow_y_scroll();

        if self.state.notes.is_empty() {
            list = list.child(
                v_flex()
                    .id("notes-empty")
                    .test_support()
                    .aria_label("还没有便笺")
                    .flex_1()
                    .w_full()
                    .items_center()
                    .justify_center()
                    .gap_1()
                    .border_1()
                    .border_dashed()
                    .border_color(tokens::hsla(tokens::LINE))
                    .rounded(px(6.0))
                    .child(
                        div()
                            .text_size(px(13.0))
                            .text_color(tokens::hsla(tokens::alpha(tokens::TEXT, 0.75)))
                            .child("还没有便笺"),
                    )
                    .child(
                        div()
                            .text_size(px(11.0))
                            .text_color(tokens::hsla(tokens::MUTED))
                            .child("点击下方 + 新建"),
                    ),
            );
        }

        for index in 0..self.state.notes.len() {
            list = list.child(self.render_note_card(index, window, cx));
        }

        list.child(
            Button::new("note-add")
                .label("+ 新增便笺")
                .ghost()
                .w_full()
                .on_click(cx.listener(|this, _, window, cx| this.add_note(window, cx))),
        )
    }

    fn render_note_card(
        &mut self,
        index: usize,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let expanded = self.expanded == Some(index);
        let note = self.state.notes[index].clone();
        let editing_title = self.editing == Editing::Title(index);
        let editing_body = self.editing == Editing::Body(index);

        let excerpt = note
            .content
            .lines()
            .find(|line| !line.trim().is_empty())
            .map(|line| line.trim().to_string())
            .unwrap_or_default();

        let mut card = v_flex()
            .id(("note-card", index))
            .test_support()
            .flex_none()
            .w_full()
            .rounded(px(6.0))
            .border_1()
            .border_color(tokens::hsla(if expanded {
                tokens::alpha(tokens::TEXT, 0.2)
            } else {
                tokens::LINE
            }))
            .bg(tokens::hsla(if expanded {
                tokens::CARD
            } else {
                tokens::INK
            }))
            .px(px(12.0))
            .py(px(8.0))
            .gap_1()
            .cursor(if expanded {
                CursorStyle::Arrow
            } else {
                CursorStyle::OpenHand
            })
            .on_click(cx.listener(move |this, _, window, cx| {
                if !this.editing_matches_note(index) {
                    this.toggle_note(index, window, cx);
                }
            }));

        if expanded {
            if editing_title {
                if let Some(input) = self.title_input.as_ref() {
                    card = card.child(Input::new(input).small());
                }
            } else {
                card = card.child(
                    div()
                        .id(("note-title-edit", index))
                        .text_size(px(14.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(tokens::hsla(if note.title.is_empty() {
                            tokens::MUTED
                        } else {
                            tokens::TEXT
                        }))
                        .child(if note.title.is_empty() {
                            "未命名便笺".to_string()
                        } else {
                            note.title.clone()
                        })
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.begin_title_edit(window, cx);
                        })),
                );
            }

            if editing_body {
                if let Some(input) = self.body_input.as_ref() {
                    card = card.child(Textarea::new(input).h(px(140.0)));
                }
            } else {
                let content = if note.content.is_empty() {
                    "写点什么…".to_string()
                } else {
                    note.content.clone()
                };
                card = card.child(
                    div()
                        .id(("note-body-edit", index))
                        .text_size(px(13.0))
                        .line_height(px(22.0))
                        .text_color(tokens::hsla(tokens::TEXT))
                        .child(content)
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.begin_body_edit(window, cx);
                        })),
                );
            }
        } else {
            card = card.child(
                div()
                    .id(("note-title-view", index))
                    .test_support()
                    .aria_label(if note.title.is_empty() {
                        "未命名便笺".to_string()
                    } else {
                        note.title.clone()
                    })
                    .text_size(px(14.0))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(tokens::hsla(if note.title.is_empty() {
                        tokens::MUTED
                    } else {
                        tokens::TEXT
                    }))
                    .child(if note.title.is_empty() {
                        "未命名便笺".to_string()
                    } else {
                        note.title.clone()
                    }),
            );
            card = card.child(
                div()
                    .id(("note-excerpt", index))
                    .test_support()
                    .aria_label(if excerpt.is_empty() {
                        "暂无内容".to_string()
                    } else {
                        excerpt.clone()
                    })
                    .text_size(px(12.0))
                    .text_color(tokens::hsla(tokens::MUTED))
                    .opacity(if excerpt.is_empty() {
                        tokens::FADED
                    } else {
                        1.0
                    })
                    .child(if excerpt.is_empty() {
                        "暂无内容".to_string()
                    } else {
                        excerpt
                    }),
            );
        }

        card.child(
            h_flex().w_full().justify_end().child(
                Button::new(("note-del", index))
                    .label("×")
                    .ghost()
                    .compact()
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.delete_note(index, window, cx);
                    })),
            ),
        )
    }

    fn editing_matches_note(&self, index: usize) -> bool {
        matches!(self.editing, Editing::Title(i) | Editing::Body(i) if i == index)
    }

    fn render_timetable(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        collapsed: bool,
    ) -> impl IntoElement {
        let wrapper = |header: AnyElement, grid: Option<AnyElement>| {
            v_flex()
                .id("timetable")
                .flex_1()
                .h_full()
                .min_h_0()
                .child(header)
                .when_some(grid, |this, grid| this.child(grid))
        };

        let header = h_flex()
            .id("tt-header")
            .flex_none()
            .h(px(42.0))
            .px(px(14.0))
            .items_center()
            .gap_1()
            .when(!collapsed, |this| {
                this.border_b_1().border_color(tokens::hsla(tokens::LINE))
            })
            .child(
                Button::new("tt-toggle")
                    .label(if collapsed { "课表 ▸" } else { "课表 ▾" })
                    .ghost()
                    .on_click(cx.listener(|this, _, window, cx| this.toggle_collapse(window, cx))),
            )
            .child(self.render_timetable_switcher(window, cx));

        if collapsed {
            return wrapper(header.into_any_element(), None);
        }

        let grid = div()
            .id("tt-grid")
            .flex_1()
            .h_full()
            .min_h_0()
            .p(px(8.0))
            .child(self.render_grid(window, cx));

        wrapper(header.into_any_element(), Some(grid.into_any_element()))
    }

    fn render_timetable_switcher(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let renaming = self.editing == Editing::TimetableName;
        let active_name = self.state.active_timetable().name.clone();
        let can_delete = self.state.timetables.len() > 1;

        let mut row = h_flex().id("tt-switcher").items_center().gap_1();

        if renaming {
            if let Some(input) = self.name_input.as_ref() {
                row = row.child(Input::new(input).small().w(px(120.0)));
            }
            row = row.child(
                Button::new("tt-rename-ok")
                    .label("✓")
                    .ghost()
                    .compact()
                    .on_click(cx.listener(|this, _, window, cx| this.commit_rename(window, cx))),
            );
        } else {
            row = row
                .child(
                    Button::new("tt-active")
                        .label(format!("{active_name} ▾"))
                        .ghost()
                        .small()
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.cycle_timetable(window, cx);
                        })),
                )
                .child(
                    Button::new("tt-rename")
                        .label("✎")
                        .ghost()
                        .compact()
                        .on_click(cx.listener(|this, _, window, cx| this.begin_rename(window, cx))),
                )
                .child(
                    Button::new("tt-add")
                        .label("＋")
                        .ghost()
                        .compact()
                        .on_click(cx.listener(|this, _, window, cx| this.add_timetable(window, cx))),
                )
                .child(
                    Button::new("tt-del")
                        .label("×")
                        .ghost()
                        .compact()
                        .disabled(!can_delete)
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.delete_timetable(window, cx);
                        })),
                );
        }
        row
    }

    fn render_grid(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        const DAYS: [&str; 6] = ["一", "二", "三", "四", "五", "六"];
        let today = now().weekday_sun0;

        // 9 行（表头 + 8 节）× 7 列（节次 + 6 天）
        let mut grid = div()
            .id("tt-grid-layout")
            .grid()
            .grid_cols(7)
            .grid_rows_min_content(9)
            .gap(px(1.0))
            .size_full();

        grid = grid.child(
            div()
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(11.0))
                .text_color(tokens::hsla(tokens::MUTED))
                .child("节"),
        );
        for (i, day) in DAYS.iter().enumerate() {
            let is_today = today == (i as u8 + 1);
            grid = grid.child(
                div()
                    .id(("tt-head", i))
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_size(px(11.0))
                    .text_color(tokens::hsla(if is_today {
                        tokens::TEXT
                    } else {
                        tokens::MUTED
                    }))
                    .child(*day),
            );
        }

        for period in 1u8..=8 {
            grid = grid.child(
                div()
                    .flex()
                    .items_center()
                    .justify_end()
                    .pr(px(6.0))
                    .text_size(px(11.0))
                    .text_color(tokens::hsla(tokens::MUTED))
                    .child(format!("{period}")),
            );
            for day in 1u8..=6 {
                grid = grid.child(self.render_cell(day, period, window, cx, today));
            }
        }
        grid
    }

    fn render_cell(
        &mut self,
        day: u8,
        period: u8,
        _window: &mut Window,
        cx: &mut Context<Self>,
        today: u8,
    ) -> impl IntoElement {
        let text = self.state.cell(day, period).cloned().unwrap_or_default();
        let empty = text.is_empty();
        let is_today = today == day;
        let editing = matches!(
            self.editing,
            Editing::Cell { day: d, period: p } if d == day && p == period
        );

        let bg = if is_today {
            if empty {
                tokens::today_bg()
            } else {
                tokens::today_filled_bg()
            }
        } else if empty {
            tokens::INK
        } else {
            tokens::CARD
        };

        let mut cell = div()
            .id(("tt-cell", day as u32 * 10 + period as u32))
            .test_support()
            .aria_label(if empty {
                "—".to_string()
            } else {
                text.clone()
            })
            .size_full()
            .min_w_0()
            .rounded(px(3.0))
            .bg(tokens::hsla(bg))
            .flex()
            .items_center()
            .justify_center()
            .px(px(6.0))
            .py(px(3.0))
            .cursor(CursorStyle::IBeam);

        if editing {
            if let Some(input) = self.cell_input.as_ref() {
                cell = cell.child(Input::new(input).small());
            }
            return cell.child(
                Button::new(("tt-cell-ok", day as u32 * 10 + period as u32))
                    .label("✓")
                    .ghost()
                    .compact()
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.commit_cell_edit(day, period, window, cx);
                    })),
            );
        }

        let label = if empty { "—".to_string() } else { text };
        cell.child(
            div()
                .text_size(px(if empty { 11.0 } else { 13.0 }))
                .font_weight(if empty {
                    FontWeight::NORMAL
                } else {
                    FontWeight::SEMIBOLD
                })
                .text_color(tokens::hsla(if empty {
                    tokens::MUTED
                } else {
                    tokens::TEXT
                }))
                .opacity(if empty { tokens::FADED } else { 1.0 })
                .child(label),
        )
        .on_click(cx.listener(move |this, _, window, cx| {
            this.begin_cell_edit(day, period, window, cx);
        }))
    }

    fn render_toast(&mut self, message: String, error: bool) -> impl IntoElement {
        div()
            .id("toast")
            .absolute()
            .bottom(px(16.0))
            .left(px(0.0))
            .right(px(0.0))
            .flex()
            .justify_center()
            .child(
                div()
                    .id("toast-body")
                    .test_support()
                    .aria_label(message.clone())
                    .rounded(px(6.0))
                    .border_1()
                    .border_color(tokens::hsla(if error {
                        tokens::HIGHLIGHT
                    } else {
                        tokens::alpha(tokens::TEXT, 0.25)
                    }))
                    .bg(tokens::hsla(if error {
                        tokens::HIGHLIGHT
                    } else {
                        tokens::CARD
                    }))
                    .px(px(14.0))
                    .py(px(6.0))
                    .text_size(px(12.0))
                    .text_color(tokens::hsla(if error {
                        tokens::INK
                    } else {
                        tokens::TEXT
                    }))
                    .child(message),
            )
    }
}

// ---------- 小组件 ----------

#[derive(Clone, Copy)]
enum IconShape {
    Ghost,
    Power,
    Export,
    Import,
}

fn icon_button(
    id: &'static str,
    active: bool,
    title: &'static str,
    shape: IconShape,
    listener: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    let glyph = match shape {
        IconShape::Ghost => "◐",
        IconShape::Power => "⏻",
        IconShape::Export => "↧",
        IconShape::Import => "↥",
    };

    div()
        .id(id)
        .test_support()
        .aria_label(title)
        .w(px(30.0))
        .h(px(30.0))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(6.0))
        .border_1()
        .border_color(tokens::hsla(if active {
            tokens::HIGHLIGHT
        } else {
            tokens::alpha(tokens::TEXT, 0.0)
        }))
        .bg(tokens::hsla(if active {
            tokens::HIGHLIGHT
        } else {
            tokens::alpha(tokens::INK, 0.0)
        }))
        .cursor(CursorStyle::PointingHand)
        .hover(move |style| {
            style
                .bg(tokens::hsla(tokens::HIGHLIGHT))
                .text_color(tokens::hsla(tokens::INK))
        })
        .text_color(tokens::hsla(if active {
            tokens::INK
        } else {
            tokens::TEXT
        }))
        .text_size(px(15.0))
        .child(glyph)
        .on_click(listener)
}

// ---------- 极简日期工具（不引入 chrono，避免额外依赖） ----------

pub mod chrono_lite {
    use std::time::{SystemTime, UNIX_EPOCH};

    pub struct Now {
        pub weekday_sun0: u8,
        pub month: u32,
        pub day: u32,
    }

    /// 1970-01-01 是星期四（sun0 = 4）
    pub fn now() -> Now {
        let secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let days = (secs / 86_400) as i64;
        let weekday_sun0 = ((days + 4).rem_euclid(7)) as u8;
        let (month, day) = civil_from_days(days);
        Now {
            weekday_sun0,
            month,
            day,
        }
    }

    /// Howard Hinnant 的 civil_from_days 算法
    fn civil_from_days(days: i64) -> (u32, u32) {
        let z = days + 719_468;
        let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
        let doe = (z - era * 146_097) as u64;
        let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
        let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
        (month, day)
    }
}

pub fn now() -> chrono_lite::Now {
    chrono_lite::now()
}

pub fn hero_labels(now: &chrono_lite::Now) -> (String, String) {
    const WEEK_EN: [&str; 7] = ["SUN", "MON", "TUE", "WED", "THU", "FRI", "SAT"];
    const WEEK_CN: [&str; 7] = ["周日", "周一", "周二", "周三", "周四", "周五", "周六"];
    let idx = now.weekday_sun0 as usize;
    let eyebrow = format!("{} · {:02}/{:02}", WEEK_EN[idx], now.month, now.day);
    (eyebrow, WEEK_CN[idx].to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[::core::prelude::v1::test]
    fn hero_labels_render_weekday_and_date() {
        // 2024-10-19 是星期六
        let now = chrono_lite::Now {
            weekday_sun0: 6,
            month: 10,
            day: 19,
        };
        let (eyebrow, day_cn) = hero_labels(&now);
        assert_eq!(eyebrow, "SAT · 10/19");
        assert_eq!(day_cn, "周六");
    }

    #[::core::prelude::v1::test]
    fn sunday_has_no_classes() {
        let state = State::default();
        assert_eq!(state.today_period_count(0), 0);
    }

    #[::core::prelude::v1::test]
    fn off_screen_positions_are_rejected() {
        let displays = vec![Bounds {
            origin: point(px(0.0), px(0.0)),
            size: size(px(1920.0), px(1080.0)),
        }];
        assert!(on_screen(
            &WindowPos { x: 100.0, y: 100.0 },
            760.0,
            540.0,
            &displays
        ));
        assert!(!on_screen(
            &WindowPos {
                x: 5000.0,
                y: 5000.0
            },
            760.0,
            540.0,
            &displays
        ));
    }
}
