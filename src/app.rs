/* PiWidget · 根视图
 *
 * 墨色卡片 + 单列：Hero + 便笺列表；列表首行是固定的「课表」便笺卡，
 * 展开即内嵌课格，↗ 可把课表开成独立窗口（见 timetable_window.rs）。
 * 状态变更走 800ms 防抖落盘，Ctrl/⌘+S 立即落盘。
 *
 * 窗口尺寸一律用**逻辑像素**保存与恢复：v5 之前的物理像素尺寸在高 DPI 屏上
 * 会让窗口按缩放倍数放大（开机自启动窗口变大），因此 migrate() 直接丢弃旧尺寸，
 * 这里再按屏幕可视区夹一次兜底。
 */

use std::time::Duration;

use gpui_kit::component::input::InputEvent as InputEventKind;
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
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
use crate::grid::CourseGrid;
use crate::model::{State, WindowPos, WindowSize};
use crate::storage;
use crate::theme as tokens;
use crate::timetable_window::TimetableView;

const SAVE_DEBOUNCE: Duration = Duration::from_millis(800);
/// 恢复窗口时距离屏幕边缘的最小留白（逻辑像素）
const SCREEN_MARGIN: f32 = 24.0;

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
    /// 便笺列表里的课表卡是否展开（与 `expanded` 互斥）
    timetable_open: bool,
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
    focus_handle: FocusHandle,
    /// 已打开的课表窗口（再次点 ↗ 时激活而不是重复开窗）
    timetable_window: Option<AnyWindowHandle>,
    /// 内联编辑的事件订阅（Enter / 失焦提交）
    edit_subscription: Option<Subscription>,
    /// 防抖落盘任务（保留句柄以便取消）
    save_task: Option<Task<()>>,
}

impl PiWidget {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let loaded = storage::load();
        let mut this = Self {
            state: loaded.state,
            expanded: None,
            timetable_open: false,
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
            timetable_window: None,
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

    /// 便笺列表里的课表卡是否展开
    pub fn timetable_open(&self) -> bool {
        self.timetable_open
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

    /// 供课表窗口等外部视图改写数据（单一事实源仍是本实体）
    pub fn edit_state(&mut self, f: impl FnOnce(&mut State), cx: &mut Context<Self>) {
        f(&mut self.state);
        self.mark_dirty(cx);
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
        self.timetable_open = false;
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
            self.timetable_open = false;
            self.editing = Editing::None;
        }
        cx.notify();
    }

    /// 展开 / 收起便笺列表里的课表卡
    pub fn toggle_timetable(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.expanded.is_some() {
            self.commit_current(window, cx);
            self.expanded = None;
        }
        self.editing = Editing::None;
        self.cell_input = None;
        self.name_input = None;
        self.edit_subscription = None;
        self.timetable_open = !self.timetable_open;
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

    /// 把课表开成独立窗口（延迟到本轮更新结束，避免在窗口更新中途开窗）。
    /// 已经开过且还活着就激活它，不重复开。
    pub fn open_timetable_window(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(existing) = self.timetable_window {
            if cx.windows().contains(&existing) {
                existing
                    .update(cx, |_, window, _| window.activate_window())
                    .ok();
                return;
            }
            self.timetable_window = None;
        }

        let pi = cx.entity();
        let displays: Vec<Bounds<Pixels>> = cx
            .displays()
            .iter()
            .map(|display| display.visible_bounds())
            .collect();
        let bounds = timetable_window_bounds(window.bounds(), &displays);
        let main_window = window.window_handle();
        let this = cx.entity().downgrade();

        cx.defer(move |cx| {
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                window_background: WindowBackgroundAppearance::Transparent,
                window_decorations: Some(WindowDecorations::Client),
                window_min_size: Some(size(px(320.0), px(360.0))),
                titlebar: Some(TitlebarOptions {
                    title: Some("课表".into()),
                    appears_transparent: true,
                    traffic_light_position: None,
                }),
                is_resizable: true,
                ..Default::default()
            };
            let pi_for_view = pi.clone();
            let result = gpui_kit::open_window(options, cx, move |window, cx| {
                let pi = pi_for_view.clone();
                cx.new(move |cx| TimetableView::new(&pi, window, cx))
            });
            match result {
                Ok((handle, _)) => {
                    this.update(cx, |view, _| view.timetable_window = Some(handle))
                        .ok();
                }
                Err(err) => {
                    let message = format!("打开课表窗口失败：{err}");
                    cx.update_window(main_window, |_, window, cx| {
                        this.update(cx, |this, cx| this.show_toast(message, true, window, cx))
                            .ok();
                    })
                    .ok();
                }
            }
        });
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
                            this.timetable_open = false;
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

    /// 记录窗口位置与尺寸（逻辑像素）。只有主窗口调用，课表窗口不落盘。
    pub fn observe_window(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        cx.observe_window_bounds(window, |this, window, cx| {
            this.record_window_bounds(window.bounds().origin, window.bounds().size, cx);
        })
        .detach();
    }

    /// 把窗口几何写入状态（`window.bounds()` 已是逻辑像素，不再乘缩放因子）。
    /// 过小的尺寸（最小化 / 收起动画中间态）不覆盖已保存值。
    pub fn record_window_bounds(
        &mut self,
        origin: Point<Pixels>,
        size: Size<Pixels>,
        cx: &mut Context<Self>,
    ) {
        let width = f32::from(size.width);
        let height = f32::from(size.height);
        if width < tokens::MIN_WIDTH || height < tokens::MIN_HEIGHT {
            return;
        }
        self.state.window_pos = Some(WindowPos {
            x: f64::from(origin.x),
            y: f64::from(origin.y),
        });
        self.state.window_size = Some(WindowSize {
            w: width as f64,
            h: height as f64,
        });
        self.mark_dirty(cx);
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

// ---------- 窗口几何 ----------

/// 计算启动窗口边界：恢复上次位置与尺寸，并夹进屏幕可视区域。
pub fn restored_bounds(cx: &App) -> WindowBounds {
    let loaded = storage::load();
    let displays: Vec<Bounds<Pixels>> = cx
        .displays()
        .iter()
        .map(|display| display.visible_bounds())
        .collect();
    let bounds = fit_window_bounds(loaded.state.window_pos, loaded.state.window_size, &displays);
    WindowBounds::Windowed(bounds)
}

/// 课表独立窗口的初始边界：相对主窗口错开一点，并夹进所在屏幕。
pub fn timetable_window_bounds(
    parent: Bounds<Pixels>,
    displays: &[Bounds<Pixels>],
) -> Bounds<Pixels> {
    let center_x = f32::from(parent.center().x);
    let center_y = f32::from(parent.center().y);
    let display = displays
        .iter()
        .find(|d| contains_point(d, center_x, center_y))
        .or_else(|| displays.first());

    let (width, height) = match display {
        Some(d) => (
            tokens::TIMETABLE_WIDTH
                .min(f32::from(d.size.width) - SCREEN_MARGIN * 2.0)
                .max(320.0),
            tokens::TIMETABLE_HEIGHT
                .min(f32::from(d.size.height) - SCREEN_MARGIN * 2.0)
                .max(tokens::MIN_HEIGHT),
        ),
        None => (tokens::TIMETABLE_WIDTH, tokens::TIMETABLE_HEIGHT),
    };
    let size = size(px(width), px(height));

    let mut x = f32::from(parent.origin.x) + 48.0;
    let mut y = f32::from(parent.origin.y) + 24.0;
    if let Some(d) = display {
        let left = f32::from(d.origin.x);
        let top = f32::from(d.origin.y);
        x = x.clamp(left, left + f32::from(d.size.width) - width);
        y = y.clamp(top, top + f32::from(d.size.height) - height);
    }
    Bounds {
        origin: point(px(x), px(y)),
        size,
    }
}

/// 纯函数：把保存的窗口状态夹成屏幕内可用的边界。
///
/// - 尺寸先夹到 `MIN_WIDTH..=屏幕宽-2*留白`，没有记录时用设计尺寸；
/// - 位置若与任一屏幕相交则贴边夹取，否则在目标屏幕居中。
pub fn fit_window_bounds(
    saved_pos: Option<WindowPos>,
    saved_size: Option<WindowSize>,
    displays: &[Bounds<Pixels>],
) -> Bounds<Pixels> {
    let fallback_size = size(px(tokens::DESIGN_WIDTH), px(tokens::DESIGN_HEIGHT));
    let fallback = Bounds {
        origin: point(px(120.0), px(120.0)),
        size: fallback_size,
    };
    if displays.is_empty() {
        return fallback;
    }

    // 目标屏幕：保存位置落在哪块屏就用哪块，否则用第一块（主屏）
    let home_display = saved_pos.and_then(|pos| {
        displays
            .iter()
            .find(|display| contains_point(display, pos.x as f32, pos.y as f32))
            .copied()
    });
    let target = home_display.unwrap_or(displays[0]);

    let max_w = (f32::from(target.size.width) - SCREEN_MARGIN * 2.0).max(tokens::MIN_WIDTH);
    let max_h = (f32::from(target.size.height) - SCREEN_MARGIN * 2.0).max(tokens::MIN_HEIGHT);

    let width = saved_size
        .map(|s| s.w as f32)
        .unwrap_or(tokens::DESIGN_WIDTH)
        .clamp(tokens::MIN_WIDTH, max_w);
    let height = saved_size
        .map(|s| s.h as f32)
        .unwrap_or(tokens::DESIGN_HEIGHT)
        .clamp(tokens::MIN_HEIGHT, max_h);
    let size = size(px(width), px(height));

    let left = f32::from(target.origin.x);
    let top = f32::from(target.origin.y);
    let origin = match saved_pos {
        // 位置在某块屏上：夹到该屏可视区内（防止贴边后恢复时越界）
        Some(pos) if home_display.is_some() => {
            let x = (pos.x as f32).clamp(left, left + f32::from(target.size.width) - width);
            let y = (pos.y as f32).clamp(top, top + f32::from(target.size.height) - height);
            point(px(x), px(y))
        }
        // 没记录过位置 / 位置已在屏外（拔掉显示器）：在目标屏居中
        _ => point(
            px(left + (f32::from(target.size.width) - width) / 2.0),
            px(top + (f32::from(target.size.height) - height) / 2.0),
        ),
    };

    Bounds { origin, size }
}

fn contains_point(display: &Bounds<Pixels>, x: f32, y: f32) -> bool {
    let left = f32::from(display.origin.x);
    let top = f32::from(display.origin.y);
    x >= left
        && x < left + f32::from(display.size.width)
        && y >= top
        && y < top + f32::from(display.size.height)
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
            .child(self.render_hero(cx))
            .child(self.render_notes(window, cx))
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
            .test_support()
            .flex_none()
            .items_center()
            .pt(px(18.0))
            .pb(px(12.0))
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
                    .mb(px(4.0))
                    .child(
                        div()
                            .id("hero-day")
                            .test_support()
                            .aria_label(day_cn.clone())
                            .text_size(px(60.0))
                            .line_height(px(0.95 * 60.0))
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
            .test_support()
            .flex_1()
            .min_h_0()
            .p_3()
            .gap_2()
            .items_start()
            .overflow_y_scroll()
            .child(self.render_timetable_card(window, cx));

        if self.state.notes.is_empty() {
            list = list.child(
                v_flex()
                    .id("notes-empty")
                    .test_support()
                    .aria_label("还没有便笺")
                    .flex_none()
                    .w_full()
                    .py(px(24.0))
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
                            .child("点击下方 ＋ 新建"),
                    ),
            );
        }

        for index in 0..self.state.notes.len() {
            list = list.child(self.render_note_card(index, window, cx));
        }

        // 「＋ 新建」下拉：便笺 / 课表
        let pi = cx.entity().downgrade();
        let for_note = pi.clone();
        let for_timetable = pi;
        list.child(
            Button::new("note-add")
                .label("＋ 新建")
                .ghost()
                .w_full()
                .dropdown_menu(move |menu, _window, _cx| {
                    let note_entity = for_note.clone();
                    let timetable_entity = for_timetable.clone();
                    menu.item(
                        PopupMenuItem::new("新建便笺").on_click(move |_, window, cx| {
                            note_entity
                                .update(cx, |this, cx| this.add_note(window, cx))
                                .ok();
                        }),
                    )
                    .item(
                        PopupMenuItem::new("课表").on_click(move |_, window, cx| {
                            timetable_entity
                                .update(cx, |this, cx| {
                                    if !this.timetable_open {
                                        this.toggle_timetable(window, cx);
                                    }
                                })
                                .ok();
                        }),
                    )
                }),
        )
    }

    /// 便笺列表首行：固定的课表卡
    fn render_timetable_card(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let open = self.timetable_open;
        let name = self.state.active_timetable().name.clone();
        let today_count = self.state.today_period_count(now().weekday_sun0);
        let meta = if today_count > 0 {
            format!("今天 {today_count} 节")
        } else {
            "今日无课".to_string()
        };
        let can_delete = self.state.timetables.len() > 1;
        let renaming = self.editing == Editing::TimetableName;

        let mut card = v_flex()
            .id("tt-note-card")
            .test_support()
            .aria_label(name.clone())
            .flex_none()
            .w_full()
            .rounded(px(6.0))
            .border_1()
            .border_color(tokens::hsla(if open {
                tokens::alpha(tokens::HIGHLIGHT, 0.55)
            } else {
                tokens::LINE
            }))
            .bg(tokens::hsla(if open { tokens::CARD } else { tokens::INK }))
            .px(px(12.0))
            .py(px(8.0))
            .gap_1();

        // 窄窗口下分两行：标题行（名称 + 今日节数）/ 操作行（切换 ✎ ＋ × ↗）
        let title_row = h_flex()
            .id("tt-note-title-row")
            .w_full()
            .items_center()
            .gap_2()
            .child(
                div()
                    .id("tt-note-toggle")
                    .test_support()
                    .aria_label(name.clone())
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_size(px(14.0))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(tokens::hsla(tokens::TEXT))
                    .child(if open {
                        format!("课表 · {name} ▾")
                    } else {
                        format!("课表 · {name} ▸")
                    })
                    .cursor(CursorStyle::PointingHand)
                    .on_click(cx.listener(|this, _, window, cx| this.toggle_timetable(window, cx))),
            )
            .child(
                div()
                    .id("tt-note-meta")
                    .test_support()
                    .aria_label(meta.clone())
                    .flex_none()
                    .whitespace_nowrap()
                    .text_size(px(12.0))
                    .text_color(tokens::hsla(tokens::MUTED))
                    .child(meta),
            );

        let mut actions = h_flex()
            .id("tt-note-header")
            .w_full()
            .items_center()
            .justify_end()
            .gap_1();

        if !renaming {
            actions = actions
                .child(
                    Button::new("tt-active")
                        .label("切换")
                        .ghost()
                        .compact()
                        .on_click(
                            cx.listener(|this, _, window, cx| this.cycle_timetable(window, cx)),
                        ),
                )
                .child(
                    Button::new("tt-rename")
                        .label("✎")
                        .ghost()
                        .compact()
                        .on_click(cx.listener(|this, _, window, cx| this.begin_rename(window, cx))),
                );
        }
        let header = actions
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
                    .on_click(cx.listener(|this, _, window, cx| this.delete_timetable(window, cx))),
            )
            .child(
                // ↗：课表开成独立窗口
                Button::new("tt-open-window")
                    .label("↗")
                    .ghost()
                    .compact()
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.open_timetable_window(window, cx)
                    })),
            );

        let header = v_flex()
            .id("tt-note-header-block")
            .w_full()
            .gap_1()
            .child(title_row)
            .child(header);

        card = card.child(header);

        if renaming {
            if let Some(input) = self.name_input.as_ref() {
                card = card.child(
                    h_flex()
                        .id("tt-rename-row")
                        .w_full()
                        .items_center()
                        .gap_2()
                        .child(Input::new(input).small().w(px(180.0)))
                        .child(
                            Button::new("tt-rename-ok")
                                .label("✓")
                                .ghost()
                                .compact()
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.commit_rename(window, cx)
                                })),
                        ),
                );
            }
            return card;
        }

        if !open {
            return card;
        }

        let editing = match self.editing {
            Editing::Cell { day, period } => Some((day, period)),
            _ => None,
        };
        let grid = CourseGrid::new(self.state.active_timetable().cells.clone())
            .row_height(Some(46.0))
            .today(now().weekday_sun0)
            .editing(editing, self.cell_input.clone())
            .on_pick({
                let view = cx.entity().downgrade();
                move |day, period, window, cx| {
                    view.update(cx, |this, cx| this.begin_cell_edit(day, period, window, cx))
                        .ok();
                }
            })
            .on_commit({
                let view = cx.entity().downgrade();
                move |day, period, window, cx| {
                    view.update(cx, |this, cx| this.commit_cell_edit(day, period, window, cx))
                        .ok();
                }
            });

        // 窄窗口里 7 列会挤在一起：给课格一个最小宽度并允许横向滚动
        card.child(
            div()
                .id("tt-card-grid")
                .w_full()
                .overflow_x_scroll()
                .child(div().w_full().min_w(px(420.0)).child(grid)),
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

    /// 单屏 1920×1080：保存的尺寸超过屏幕时按留白夹取
    #[::core::prelude::v1::test]
    fn oversized_saved_size_is_clamped_to_screen() {
        let displays = vec![Bounds {
            origin: point(px(0.0), px(0.0)),
            size: size(px(1920.0), px(1080.0)),
        }];
        // 旧版把物理像素写进 windowSize：1.5× 屏幕上 1504 会被当成逻辑像素
        let bounds = fit_window_bounds(
            Some(WindowPos { x: 0.0, y: 0.0 }),
            Some(WindowSize { w: 1504.0, h: 651.0 }),
            &displays,
        );
        assert!(f32::from(bounds.size.width) <= 1920.0 - SCREEN_MARGIN * 2.0);
        assert!(f32::from(bounds.size.height) <= 1080.0 - SCREEN_MARGIN * 2.0);
        assert_eq!(f32::from(bounds.size.height), 651.0, "合法高度不被改写");
    }

    /// 保存尺寸比屏幕还大（换小显示器）时也要能放下
    #[::core::prelude::v1::test]
    fn size_larger_than_display_fits_inside() {
        let displays = vec![Bounds {
            origin: point(px(0.0), px(0.0)),
            size: size(px(1280.0), px(800.0)),
        }];
        let bounds = fit_window_bounds(
            None,
            Some(WindowSize {
                w: 3000.0,
                h: 2000.0,
            }),
            &displays,
        );
        assert!(f32::from(bounds.size.width) <= 1280.0);
        assert!(f32::from(bounds.size.height) <= 800.0);
        assert!(f32::from(bounds.size.width) >= tokens::MIN_WIDTH);
    }

    /// 保存位置跑到屏外：回到该屏居中，而不是留在看不见的地方
    #[::core::prelude::v1::test]
    fn offscreen_position_centers_on_display() {
        let displays = vec![Bounds {
            origin: point(px(0.0), px(0.0)),
            size: size(px(1920.0), px(1080.0)),
        }];
        let bounds = fit_window_bounds(
            Some(WindowPos {
                x: 9000.0,
                y: 9000.0,
            }),
            Some(WindowSize { w: 760.0, h: 540.0 }),
            &displays,
        );
        assert_eq!(f32::from(bounds.origin.x), (1920.0 - 760.0) / 2.0);
        assert_eq!(f32::from(bounds.origin.y), (1080.0 - 540.0) / 2.0);
    }

    /// 记住的位置在屏内时原样恢复（只有贴边才夹）
    #[::core::prelude::v1::test]
    fn on_screen_position_is_preserved() {
        let displays = vec![Bounds {
            origin: point(px(0.0), px(0.0)),
            size: size(px(1920.0), px(1080.0)),
        }];
        let bounds = fit_window_bounds(
            Some(WindowPos { x: 100.0, y: 80.0 }),
            Some(WindowSize { w: 760.0, h: 540.0 }),
            &displays,
        );
        assert_eq!(f32::from(bounds.origin.x), 100.0);
        assert_eq!(f32::from(bounds.origin.y), 80.0);
        assert_eq!(f32::from(bounds.size.width), 760.0);
        assert_eq!(f32::from(bounds.size.height), 540.0);
    }

    /// 第二块屏上的窗口要留在第二块屏
    #[::core::prelude::v1::test]
    fn second_display_position_keeps_its_screen() {
        let displays = vec![
            Bounds {
                origin: point(px(0.0), px(0.0)),
                size: size(px(1920.0), px(1080.0)),
            },
            Bounds {
                origin: point(px(1920.0), px(0.0)),
                size: size(px(2560.0), px(1440.0)),
            },
        ];
        let bounds = fit_window_bounds(
            Some(WindowPos {
                x: 2000.0,
                y: 100.0,
            }),
            Some(WindowSize { w: 760.0, h: 540.0 }),
            &displays,
        );
        assert_eq!(f32::from(bounds.origin.x), 2000.0);
        assert_eq!(f32::from(bounds.size.width), 760.0);
    }

    /// 课表窗口默认错开主窗口，靠右时会夹回屏幕内
    #[::core::prelude::v1::test]
    fn timetable_window_stays_on_screen() {
        let displays = vec![Bounds {
            origin: point(px(0.0), px(0.0)),
            size: size(px(1920.0), px(1080.0)),
        }];
        let parent = Bounds {
            origin: point(px(1500.0), px(900.0)),
            size: size(px(760.0), px(540.0)),
        };
        let bounds = timetable_window_bounds(parent, &displays);
        assert!(f32::from(bounds.origin.x) >= 0.0);
        assert!(f32::from(bounds.origin.x) + f32::from(bounds.size.width) <= 1920.0);
        assert!(f32::from(bounds.origin.y) + f32::from(bounds.size.height) <= 1080.0);
    }

    /// 屏幕比课表窗口还小时按屏幕缩小
    #[::core::prelude::v1::test]
    fn timetable_window_shrinks_on_small_display() {
        let displays = vec![Bounds {
            origin: point(px(0.0), px(0.0)),
            size: size(px(800.0), px(600.0)),
        }];
        let parent = Bounds {
            origin: point(px(0.0), px(0.0)),
            size: size(px(760.0), px(540.0)),
        };
        let bounds = timetable_window_bounds(parent, &displays);
        assert!(f32::from(bounds.size.width) <= 800.0);
        assert!(f32::from(bounds.size.height) <= 600.0);
    }

    /// 没有任何屏幕信息时不 panic，给出设计尺寸
    #[::core::prelude::v1::test]
    fn no_display_falls_back_to_design_size() {
        let bounds = fit_window_bounds(None, None, &[]);
        assert_eq!(f32::from(bounds.size.width), tokens::DESIGN_WIDTH);
        assert_eq!(f32::from(bounds.size.height), tokens::DESIGN_HEIGHT);
    }
}
