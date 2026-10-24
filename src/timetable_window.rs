/* PiWidget · 独立课表窗口
 *
 * 课表不再占用主窗口右栏：从便笺列表的课表卡「↗」打开这个窗口。
 * 这里的增删改都写回 `PiWidget` 持有的同一份 `State`（单一事实源），
 * 主窗口与课表窗口通过 entity 观察互相刷新，不需要额外的全局 store。
 */

use std::collections::BTreeMap;
use std::time::Duration;

use gpui_kit::component::input::InputEvent as InputEventKind;
use gpui_kit::component::{
    Disableable as _, Sizable as _, button::{Button, ButtonVariants as _}, h_flex,
    input::{Input, InputState}, v_flex,
};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::app::{PiWidget, now};
use crate::grid::CourseGrid;
use crate::theme as tokens;

/// 课表窗口渲染所需的只读快照（避免跨 window 长期借用主视图状态）
#[derive(Clone, Debug)]
pub struct TimetableSnapshot {
    pub id: String,
    pub name: String,
    pub cells: BTreeMap<String, BTreeMap<String, String>>,
    pub today_count: usize,
    pub timetable_count: usize,
    pub can_delete: bool,
}

pub struct TimetableView {
    /// 数据事实源：主窗口视图
    pi: WeakEntity<PiWidget>,
    renaming: bool,
    editing: Option<(u8, u8)>,
    name_input: Option<Entity<InputState>>,
    cell_input: Option<Entity<InputState>>,
    edit_subscription: Option<Subscription>,
    toast: Option<(String, bool)>,
    toast_seq: u64,
    focus_handle: FocusHandle,
}

impl TimetableView {
    pub fn new(pi: &Entity<PiWidget>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        // 主窗口改动（改名 / 新建 / 切课表）时本窗口跟着刷新
        cx.observe(pi, |_, _, cx| cx.notify()).detach();
        let this = Self {
            pi: pi.downgrade(),
            renaming: false,
            editing: None,
            name_input: None,
            cell_input: None,
            edit_subscription: None,
            toast: None,
            toast_seq: 0,
            focus_handle: cx.focus_handle(),
        };
        this.focus_handle.focus(window, cx);
        this
    }

    /// 测试辅助：判断当前是否在编辑某格
    pub fn editing(&self) -> Option<(u8, u8)> {
        self.editing
    }

    fn snapshot(&self, cx: &App) -> Option<TimetableSnapshot> {
        let pi = self.pi.upgrade()?;
        Some(pi.read_with(cx, |view, _| {
            let state = view.state();
            TimetableSnapshot {
                id: state.active_timetable_id.clone(),
                name: state.active_timetable().name.clone(),
                cells: state.active_timetable().cells.clone(),
                today_count: state.today_period_count(now().weekday_sun0),
                timetable_count: state.timetables.len(),
                can_delete: state.timetables.len() > 1,
            }
        }))
    }

    fn with_state(
        &mut self,
        cx: &mut Context<Self>,
        f: impl FnOnce(&mut crate::model::State),
    ) {
        if let Some(pi) = self.pi.upgrade() {
            pi.update(cx, |view, cx| view.edit_state(f, cx));
        }
    }

    fn show_toast(
        &mut self,
        message: impl Into<String>,
        error: bool,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.toast = Some((message.into(), error));
        self.toast_seq += 1;
        let seq = self.toast_seq;
        cx.notify();
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(2500))
                .await;
            this.update(cx, |this, cx| {
                if this.toast_seq == seq {
                    this.toast = None;
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    // ---------- 课格编辑 ----------

    fn begin_cell_edit(
        &mut self,
        day: u8,
        period: u8,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let text = self
            .snapshot(cx)
            .and_then(|s| {
                s.cells
                    .get(&day.to_string())
                    .and_then(|d| d.get(&period.to_string()))
                    .cloned()
            })
            .unwrap_or_default();
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
        self.editing = Some((day, period));
        self.renaming = false;
        cx.notify();
    }

    pub fn commit_cell_edit(
        &mut self,
        day: u8,
        period: u8,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(input) = self.cell_input.take() {
            let value = input.read(cx).value().to_string();
            self.with_state(cx, |state| state.set_cell(day, period, &value));
        }
        self.editing = None;
        self.edit_subscription = None;
        cx.notify();
    }

    fn cancel_edit(&mut self, cx: &mut Context<Self>) {
        self.editing = None;
        self.renaming = false;
        self.cell_input = None;
        self.name_input = None;
        self.edit_subscription = None;
        cx.notify();
    }

    // ---------- 多课表管理 ----------

    fn cycle_timetable(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let ids: Vec<String> = self
            .pi
            .upgrade()
            .map(|pi| {
                pi.read_with(cx, |view, _| {
                    view.state()
                        .timetables
                        .iter()
                        .map(|t| t.id.clone())
                        .collect::<Vec<_>>()
                })
            })
            .unwrap_or_default();
        let Some(snapshot) = self.snapshot(cx) else {
            return;
        };
        let Some(pos) = ids.iter().position(|id| *id == snapshot.id) else {
            return;
        };
        if ids.len() < 2 {
            self.show_toast("只有一个课表", false, window, cx);
            return;
        }
        let next = ids[(pos + 1) % ids.len()].clone();
        self.with_state(cx, |state| state.set_active_timetable(&next));
        let name = self
            .snapshot(cx)
            .map(|s| s.name)
            .unwrap_or_else(|| "课表".into());
        self.show_toast(format!("已切换到「{name}」"), false, window, cx);
    }

    fn add_timetable(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let mut name = String::new();
        self.with_state(cx, |state| {
            name = state.add_timetable().name.clone();
        });
        self.show_toast(format!("已新建「{name}」"), false, window, cx);
    }

    fn delete_timetable(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(snapshot) = self.snapshot(cx) else {
            return;
        };
        let mut result = Ok(());
        self.with_state(cx, |state| {
            result = state.delete_timetable(&snapshot.id);
        });
        match result {
            Ok(()) => self.show_toast(format!("已删除「{}」", snapshot.name), false, window, cx),
            Err(err) => self.show_toast(err, true, window, cx),
        }
    }

    fn begin_rename(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(snapshot) = self.snapshot(cx) else {
            return;
        };
        let input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("课表名称")
                .default_value(snapshot.name)
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
        self.renaming = true;
        self.editing = None;
        cx.notify();
    }

    pub fn commit_rename(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(input) = self.name_input.take() else {
            return;
        };
        let value = input.read(cx).value().to_string();
        let Some(snapshot) = self.snapshot(cx) else {
            return;
        };
        let mut result = Ok(());
        self.with_state(cx, |state| {
            result = state.rename_timetable(&snapshot.id, &value);
        });
        match result {
            Ok(()) => {
                let name = self
                    .snapshot(cx)
                    .map(|s| s.name)
                    .unwrap_or_else(|| value.trim().to_string());
                self.show_toast(format!("已重命名为「{name}」"), false, window, cx);
            }
            Err(err) => self.show_toast(err, true, window, cx),
        }
        self.renaming = false;
        self.edit_subscription = None;
        cx.notify();
    }

    /// 测试辅助：写入当前内联编辑框
    pub fn set_edit_value(&mut self, window: &mut Window, cx: &mut Context<Self>, value: &str) {
        if let Some(input) = self.cell_input.clone() {
            input.update(cx, |state, cx| state.set_value(value.to_string(), window, cx));
        } else if let Some(input) = self.name_input.clone() {
            input.update(cx, |state, cx| state.set_value(value.to_string(), window, cx));
        }
    }
}

impl Focusable for TimetableView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for TimetableView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(snapshot) = self.snapshot(cx) else {
            return v_flex()
                .id("tt-window")
                .size_full()
                .bg(tokens::hsla(tokens::INK))
                .into_any_element();
        };

        let today = now().weekday_sun0;
        let editing = self.editing;
        let mut content = v_flex().child(self.render_header(&snapshot, cx));

        if self.renaming {
            if let Some(input) = self.name_input.as_ref() {
                content = content.child(
                    h_flex()
                        .id("tt-rename-row")
                        .px_4()
                        .py_2()
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
        }

        let grid = CourseGrid::new(snapshot.cells.clone())
            .today(today)
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

        let mut sheet = v_flex()
            .id("tt-window")
            .test_support()
            .size_full()
            .relative()
            .rounded(px(tokens::RADIUS))
            .bg(tokens::hsla(tokens::INK))
            .border_1()
            .border_color(tokens::hsla(tokens::sheet_border()))
            .overflow_hidden()
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                if event.keystroke.key == "escape" {
                    this.cancel_edit(cx);
                }
            }))
            .child(self.render_titlebar(cx))
            .child(content)
            .child(grid);

        if let Some((message, error)) = self.toast.clone() {
            sheet = sheet.child(render_toast(message, error));
        }
        sheet.into_any_element()
    }
}

impl TimetableView {
    fn render_titlebar(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        h_flex()
            .id("tt-titlebar")
            .flex_none()
            .h(px(tokens::TITLEBAR_HEIGHT))
            .px_4()
            .items_center()
            .justify_between()
            .border_b_1()
            .border_color(tokens::hsla(tokens::LINE))
            .child(
                h_flex()
                    .id("tt-drag-region")
                    .flex_1()
                    .h_full()
                    .items_center()
                    .window_control_area(WindowControlArea::Drag)
                    .child(
                        div()
                            .id("tt-window-title")
                            .test_support()
                            .aria_label("课表")
                            .text_size(px(15.0))
                            .font_weight(FontWeight::BOLD)
                            .text_color(tokens::hsla(tokens::TEXT))
                            .child("课表"),
                    ),
            )
            .child(
                div()
                    .id("tt-close")
                    .test_support()
                    .aria_label("关闭课表窗口")
                    .w(px(30.0))
                    .h(px(30.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(6.0))
                    .cursor(CursorStyle::PointingHand)
                    .text_color(tokens::hsla(tokens::TEXT))
                    .text_size(px(15.0))
                    .hover(|style| {
                        style
                            .bg(tokens::hsla(tokens::HIGHLIGHT))
                            .text_color(tokens::hsla(tokens::INK))
                    })
                    .child("×")
                    .on_click(cx.listener(|_, _, window, _| window.remove_window())),
            )
    }

    fn render_header(
        &mut self,
        snapshot: &TimetableSnapshot,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let meta = if snapshot.today_count > 0 {
            format!("今天 {} 节", snapshot.today_count)
        } else {
            "今日无课".to_string()
        };

        h_flex()
            .id("tt-toolbar")
            .flex_none()
            .px_4()
            .py_2()
            .items_center()
            .justify_between()
            .gap_2()
            .border_b_1()
            .border_color(tokens::hsla(tokens::LINE))
            .child(
                h_flex()
                    .items_center()
                    .gap_1()
                    .min_w_0()
                    .child(
                        Button::new("tt-active")
                            .label(format!("{} ▾", snapshot.name))
                            .ghost()
                            .small()
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.cycle_timetable(window, cx)
                            })),
                    )
                    .child(
                        Button::new("tt-rename")
                            .label("✎")
                            .ghost()
                            .compact()
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.begin_rename(window, cx)
                            })),
                    )
                    .child(
                        Button::new("tt-add")
                            .label("＋")
                            .ghost()
                            .compact()
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.add_timetable(window, cx)
                            })),
                    )
                    .child(
                        Button::new("tt-del")
                            .label("×")
                            .ghost()
                            .compact()
                            .disabled(!snapshot.can_delete)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.delete_timetable(window, cx)
                            })),
                    ),
            )
            .child(
                div()
                    .id("tt-window-meta")
                    .test_support()
                    .aria_label(meta.clone())
                    .text_size(px(12.0))
                    .text_color(tokens::hsla(tokens::MUTED))
                    .child(meta),
            )
    }
}

fn render_toast(message: String, error: bool) -> impl IntoElement {
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
