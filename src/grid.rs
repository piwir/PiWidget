/* PiWidget · 课格（主窗口内嵌卡与独立课表窗口共用）
 *
 * 8 节 × 6 天，今日列荧光高亮；点格进入 inline 编辑（由宿主视图持有 InputState）。
 * 宿主通过 `CourseGrid` 传入只读快照 + 两个回调（点格 / 确认），
 * 保证两处渲染样式与交互完全一致。
 */

use std::collections::BTreeMap;
use std::rc::Rc;

use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::{
    Sizable as _,
    button::{Button, ButtonVariants as _},
    v_flex,
};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::theme as tokens;

pub const DAYS: [&str; 6] = ["一", "二", "三", "四", "五", "六"];
pub const PERIODS: u8 = 8;

/// 课格点击回调（参数：day, period）
type CellCallback = Rc<dyn Fn(u8, u8, &mut Window, &mut App)>;

/// 课表网格元素。
pub struct CourseGrid {
    cells: BTreeMap<String, BTreeMap<String, String>>,
    today: u8,
    editing: Option<(u8, u8)>,
    input: Option<Entity<InputState>>,
    on_pick: Option<CellCallback>,
    on_commit: Option<CellCallback>,
    row_height: Option<f32>,
}

impl CourseGrid {
    pub fn new(cells: BTreeMap<String, BTreeMap<String, String>>) -> Self {
        Self {
            cells,
            today: 0,
            editing: None,
            input: None,
            on_pick: None,
            on_commit: None,
            row_height: None,
        }
    }

    /// 固定行高（内嵌卡用，避免为撑满窗口留出大片空白）；
    /// 不设置时格子按剩余高度均分（独立课表窗口用）。
    pub fn row_height(mut self, height: Option<f32>) -> Self {
        self.row_height = height;
        self
    }

    /// 今日列（周日为 0 时不点亮任何列）
    pub fn today(mut self, weekday_sun0: u8) -> Self {
        self.today = weekday_sun0;
        self
    }

    pub fn editing(mut self, cell: Option<(u8, u8)>, input: Option<Entity<InputState>>) -> Self {
        self.editing = cell;
        self.input = input;
        self
    }

    pub fn on_pick(mut self, f: impl Fn(u8, u8, &mut Window, &mut App) + 'static) -> Self {
        self.on_pick = Some(Rc::new(f));
        self
    }

    pub fn on_commit(mut self, f: impl Fn(u8, u8, &mut Window, &mut App) + 'static) -> Self {
        self.on_commit = Some(Rc::new(f));
        self
    }

}

impl IntoElement for CourseGrid {
    type Element = gpui_kit::AnyElement;

    fn into_element(self) -> Self::Element {
        let today = self.today;
        let editing = self.editing;
        let input = self.input;
        let on_pick = self.on_pick;
        let on_commit = self.on_commit;
        let cells = self.cells;
        let row_height = self.row_height;

        // 9 行（表头 + 8 节）× 7 列（节次 + 6 天）
        let mut grid = div()
            .id("tt-grid-layout")
            .grid()
            .grid_cols(7)
            .gap(px(1.0))
            .w_full();
        grid = match row_height {
            Some(_) => grid.grid_rows_min_content(9).flex_none(),
            None => grid.grid_rows(9).flex_1().min_h_0(),
        };

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

        for period in 1u8..=PERIODS {
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
                let text = cells
                    .get(&day.to_string())
                    .and_then(|d| d.get(&period.to_string()))
                    .cloned()
                    .unwrap_or_default();
                grid = grid.child(render_cell(
                    day,
                    period,
                    text,
                    today,
                    editing == Some((day, period)),
                    input.clone(),
                    on_pick.clone(),
                    on_commit.clone(),
                    row_height,
                ));
            }
        }

        let wrapper = v_flex().id("tt-grid").w_full().p(px(8.0)).child(grid);
        match row_height {
            Some(_) => wrapper.flex_none().into_any_element(),
            None => wrapper.flex_1().min_h_0().into_any_element(),
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn render_cell(
    day: u8,
    period: u8,
    text: String,
    today: u8,
    editing: bool,
    input: Option<Entity<InputState>>,
    on_pick: Option<CellCallback>,
    on_commit: Option<CellCallback>,
    row_height: Option<f32>,
) -> AnyElement {
    let empty = text.is_empty();
    let is_today = today == day;

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

    let id = ("tt-cell", day as u32 * 10 + period as u32);
    let mut cell = div()
        .id(id)
        .test_support()
        .aria_label(if empty {
            "—".to_string()
        } else {
            text.clone()
        })
        .w_full()
        .min_w_0()
        .rounded(px(3.0))
        .bg(tokens::hsla(bg))
        .flex()
        .items_center()
        .justify_center()
        .px(px(4.0))
        .py(px(3.0))
        .cursor(CursorStyle::IBeam);

    // 内嵌卡：固定行高（内容撑高会把整张卡拉得很长）；独立窗口：跟随网格行高
    cell = match row_height {
        Some(height) => cell.h(px(height)),
        None => cell.h_full().min_h_0(),
    };

    if editing {
        if let Some(input) = input.as_ref() {
            cell = cell.child(Input::new(input).small());
        }
        let confirm = on_commit.clone();
        return cell
            .child(
                Button::new(("tt-cell-ok", day as u32 * 10 + period as u32))
                    .label("✓")
                    .ghost()
                    .compact()
                    .on_click(move |_, window, cx| {
                        if let Some(confirm) = confirm.as_ref() {
                            confirm(day, period, window, cx);
                        }
                    }),
            )
            .into_any_element();
    }

    let label = if empty { "—".to_string() } else { text };
    let pick = on_pick.clone();
    cell.child(
        v_flex()
            .items_center()
            .justify_center()
            .child(
                div()
                    .text_size(px(if empty { 11.0 } else { 12.0 }))
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
            ),
    )
    .on_click(move |_, window, cx| {
        if let Some(pick) = pick.as_ref() {
            pick(day, period, window, cx);
        }
    })
    .into_any_element()
}
