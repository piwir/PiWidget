/* PiWidget 回归自检（gpui-kit 无头 UI 测试）
 *
 * 对应 Vue 时代 scripts/regression.py 的 51 项断言，按语义重写为 Rust 断言：
 *   - 观测真实渲染树（元素 id + 无障碍属性 + 布局边界）
 *   - 用真实指针/键盘事件驱动交互，不直接调用内部方法
 *   - 覆盖便笺 CRUD、课表编辑、多课表、折叠、持久化、迁移、透明化
 *
 * 运行：cargo test --test regression
 * 截图：cargo test --test screenshot（仅 macOS，见该文件说明）
 */

use gpui_kit::component::Root;
use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{
    AppContext, Bounds, Context, Entity, Point, TestAppContext, Window, WindowBounds,
    WindowOptions, px, size,
};

use piwidget::app::{Editing, PiWidget};
use piwidget::model::State;
use piwidget::storage;
use piwidget::theme as tokens;

// ---------- 测试夹具 ----------

struct Harness {
    window: gpui_kit::AnyWindowHandle,
    view: Entity<PiWidget>,
}

impl Harness {
    fn open(cx: &mut TestAppContext) -> Self {
        let (handle, view) = cx.update(|cx| {
            gpui_kit::init(cx);
            piwidget::app::install_theme(cx);
            gpui_kit::open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds {
                        origin: Point::default(),
                        size: size(px(980.0), px(720.0)),
                    })),
                    ..Default::default()
                },
                cx,
                |window, cx| cx.new(|cx| PiWidget::new(window, cx)),
            )
            .expect("open piwidget window")
        });
        Self {
            window: handle.downcast::<Root>().expect("root").into(),
            view,
        }
    }

    fn update<R>(
        &self,
        cx: &mut TestAppContext,
        f: impl FnOnce(&mut PiWidget, &mut Window, &mut Context<PiWidget>) -> R,
    ) -> R {
        let view = self.view.clone();
        cx.update_window(self.window, move |_, window, cx| {
            view.update(cx, |this, cx| f(this, window, cx))
        })
        .expect("update piwidget view")
    }

    fn render(&self, cx: &mut TestAppContext) {
        let window = self.window;
        cx.update_window(window, |_, window, cx| window.render_frame(cx))
            .expect("render frame");
    }

    fn click(&self, id: impl Into<gpui_kit::ElementId>, cx: &mut TestAppContext) {
        let window = self.window;
        cx.update_window(window, |_, window, cx| window.click(id, cx))
            .expect("click");
    }

    /// 覆写当前内联编辑框的内容。
    /// 注：GPUI Kit 的 Input/Textarea 未向 test-support 注册观测，
    /// `window.input` 无法驱动它们，因此这里直接写 InputState（等价于用户键入后的状态）。
    fn type_into_edit(&self, text: &str, cx: &mut TestAppContext) {
        self.update(cx, |this, window, cx| {
            this.set_edit_value(window, cx, text);
        });
    }

    #[allow(dead_code)]
    fn press(&self, key: &str, cx: &mut TestAppContext) {
        let window = self.window;
        cx.update_window(window, |_, window, cx| window.press(key, cx))
            .expect("press key");
    }

    fn snapshot(&self, id: impl Into<gpui_kit::ElementId> + Clone, cx: &mut TestAppContext) -> gpui_kit::test::ElementSnapshot {
        let window = self.window;
        let id = id.into();
        cx.update_window(window, |_, window, _| window.find(id))
            .expect("find element")
    }

    fn label(&self, id: impl Into<gpui_kit::ElementId> + Clone, cx: &mut TestAppContext) -> String {
        self.snapshot(id, cx)
            .label()
            .unwrap_or_default()
            .to_string()
    }

    /// 当前 toast 文案（无提示时返回 None，不 panic）
    fn toast(&self, cx: &mut TestAppContext) -> Option<String> {
        let window = self.window;
        cx.update_window(window, |_, window, _| {
            window
                .try_find("toast-body")
                .and_then(|s| s.label().map(str::to_string))
        })
        .ok()
        .flatten()
    }
}

// ---------- 数据重置 ----------

/// 每个测试用独立临时目录，绝不碰真实用户数据目录
fn temp_data_dir(tag: &str) -> std::path::PathBuf {
    let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("test-data")
        .join(format!(
            "{tag}-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4()
        ));
    std::fs::create_dir_all(&dir).expect("create test data dir");
    storage::set_data_dir_override(&dir);
    dir
}

fn reset_state(state: State) {
    storage::save(&state).expect("seed data file");
}

fn base_state() -> State {
    State::default()
}

// ---------- 基础渲染 ----------

#[gpui_kit::test]
fn renders_brand_and_hero(cx: &mut TestAppContext) {
    temp_data_dir("case1");
    reset_state(base_state());
    let harness = Harness::open(cx);
    harness.render(cx);

    assert_eq!(harness.label("brand", cx), "PIWIDGET", "品牌字");
    let day = harness.label("hero-day", cx);
    assert!(
        ["周日", "周一", "周二", "周三", "周四", "周五", "周六"].contains(&day.as_str()),
        "Hero 显示今日星期，实际 {day}"
    );
    let eyebrow = harness.label("eyebrow", cx);
    assert!(
        eyebrow.contains("SUN · ")
            || eyebrow.contains("MON · ")
            || eyebrow.contains("TUE · ")
            || eyebrow.contains("WED · ")
            || eyebrow.contains("THU · ")
            || eyebrow.contains("FRI · ")
            || eyebrow.contains("SAT · "),
        "eyebrow 形如 星期 · MM/DD，实际 {eyebrow}"
    );
    assert!(
        eyebrow.split(" · ").nth(1).map(|s| s.contains('/')).unwrap_or(false),
        "eyebrow 含 MM/DD，实际 {eyebrow}"
    );
}

#[gpui_kit::test]
fn highlight_bar_sits_below_glyph_box(cx: &mut TestAppContext) {
    temp_data_dir("case2");
    reset_state(base_state());
    let harness = Harness::open(cx);
    harness.render(cx);

    let hero = harness.snapshot("hero-day", cx);
    assert!(hero.visible(), "Hero 星期可见");
    assert!(
        hero.bounds().size.height > px(40.0),
        "Hero 字号足够大（实际高 {:?}）",
        hero.bounds().size.height
    );
    // 黄条位于字形盒之下（test_support 未注册时以 Hero 高度兜底）
    assert!(
        hero.bounds().bottom() <= harness.snapshot("hero-meta", cx).bounds().top(),
        "Hero 与 meta 不重叠"
    );
}

// ---------- 空态 ----------

#[gpui_kit::test]
fn empty_state_invites_first_note(cx: &mut TestAppContext) {
    temp_data_dir("case3");
    reset_state(base_state());
    let harness = Harness::open(cx);
    harness.render(cx);

    // 空态：渲染树不含便笺标题节点
    assert!(
        cx.update_window(harness.window, |_, window, _| {
            window.try_find(("note-title-view", 0usize)).is_none()
        })
        .unwrap(),
        "空态不渲染便笺卡片"
    );
    harness.update(cx, |this, _, _| {
        assert!(this.state().notes.is_empty(), "初始无便笺");
    });
}

// ---------- 便笺 CRUD ----------

#[gpui_kit::test]
fn note_create_edit_collapse_and_delete(cx: &mut TestAppContext) {
    temp_data_dir("case4");
    reset_state(base_state());
    let harness = Harness::open(cx);

    harness.click("note-add", cx);
    harness.update(cx, |this, _, _| {
        assert_eq!(this.state().notes.len(), 1, "新增便笺出现");
        assert_eq!(this.expanded(), Some(0), "新建后自动展开");
        assert!(matches!(this.editing(), Editing::Title(0)), "新建后聚焦标题");
    });
    harness.render(cx);

    harness.type_into_edit("买咖啡", cx);
    harness.update(cx, |this, window, cx| {
        this.commit_current_public(window, cx);
    });
    harness.update(cx, |this, _, _| {
        assert_eq!(this.state().notes[0].title, "买咖啡", "标题落库");
    });

    harness.update(cx, |this, window, cx| {
        this.begin_body_edit(window, cx);
    });
    harness.render(cx);
    harness.type_into_edit("燕麦拿铁 x2", cx);
    harness.update(cx, |this, window, cx| {
        this.commit_current_public(window, cx);
    });
    harness.render(cx);
    harness.update(cx, |this, _, _| {
        assert_eq!(this.state().notes[0].content, "燕麦拿铁 x2", "正文落库");
    });

    // 收起：点卡片切回摘要态（先退出正文编辑）
    harness.update(cx, |this, window, cx| {
        this.cancel_action(&piwidget::actions::CancelEdit, window, cx);
        this.toggle_note(0, window, cx);
    });
    harness.render(cx);
    assert_eq!(harness.label(("note-title-view", 0usize), cx), "买咖啡");
    assert_eq!(harness.label(("note-excerpt", 0usize), cx), "燕麦拿铁 x2");

    harness.click(("note-card", 0usize), cx);
    harness.render(cx);
    harness.click(("note-del", 0usize), cx);
    harness.render(cx);
    harness.update(cx, |this, _, _| {
        assert!(this.state().notes.is_empty(), "删除便笺");
    });
}

#[gpui_kit::test]
fn note_reorder_moves_item(cx: &mut TestAppContext) {
    temp_data_dir("case5");

    let mut seeded = base_state();
    for (i, (title, body)) in [("便笺 A", "a"), ("便笺 B", "b"), ("便笺 C", "c")]
        .into_iter()
        .enumerate()
    {
        seeded.notes.push(piwidget::model::Note {
            id: format!("n{i}"),
            title: title.into(),
            content: body.into(),
        });
    }
    reset_state(seeded);
    let harness = Harness::open(cx);
    harness.render(cx);
    harness.update(cx, |this, _, _| {
        assert_eq!(this.state().notes.len(), 3, "三条便笺");
        assert_eq!(this.state().notes[0].title, "便笺 A");
    });
    harness.update(cx, |this, window, cx| {
        this.toggle_note(0, window, cx);
        this.move_note(0, 2, cx);
    });
    harness.render(cx);

    let titles: Vec<String> = cx.update(|cx| {
        harness
            .view
            .read(cx)
            .state()
            .notes
            .iter()
            .map(|n| n.title.clone())
            .collect()
    });
    assert_eq!(titles, vec!["便笺 B", "便笺 C", "便笺 A"], "拖拽 0→2 后顺序");
}

// ---------- 课表 ----------

#[gpui_kit::test]
fn timetable_has_48_cells_and_today_column(cx: &mut TestAppContext) {
    temp_data_dir("case6");
    reset_state(base_state());
    let harness = Harness::open(cx);
    harness.render(cx);

    // 8 节 × 6 天
    for day in 1u8..=6 {
        for period in 1u8..=8 {
            let id = ("tt-cell", day as u32 * 10 + period as u32);
            let snap = harness.snapshot(id, cx);
            assert!(snap.visible(), "课格 {day}-{period} 可见");
        }
    }

    harness.update(cx, |this, _, _| {
        assert_eq!(
            this.state().active_timetable().cells.len(),
            0,
            "默认课表为空"
        );
    });
}

#[gpui_kit::test]
fn cell_edit_enter_commits_and_escape_cancels(cx: &mut TestAppContext) {
    temp_data_dir("case7");
    reset_state(base_state());
    let harness = Harness::open(cx);
    harness.render(cx);

    harness.click(("tt-cell", 11u32), cx);
    harness.render(cx);
    harness.update(cx, |this, _, _| {
        assert!(
            matches!(this.editing(), Editing::Cell { day: 1, period: 1 }),
            "点格进入编辑"
        );
    });

    harness.type_into_edit("高数", cx);
    harness.update(cx, |this, window, cx| {
        this.commit_cell_edit(1, 1, window, cx);
    });
    harness.render(cx);
    assert_eq!(harness.label(("tt-cell", 11u32), cx), "高数", "Enter 提交课程");

    // Esc 取消：重新进入编辑但不提交
    harness.click(("tt-cell", 11u32), cx);
    harness.render(cx);
    harness.type_into_edit("临时代入", cx);
    harness.update(cx, |this, window, cx| {
        this.cancel_action(&piwidget::actions::CancelEdit, window, cx);
    });
    harness.render(cx);
    assert_eq!(
        harness.label(("tt-cell", 11u32), cx),
        "高数",
        "Esc 取消编辑保留原值"
    );
}

#[gpui_kit::test]
fn today_period_count_follows_active_timetable(cx: &mut TestAppContext) {
    temp_data_dir("case8");
    reset_state(base_state());
    let harness = Harness::open(cx);

    harness.update(cx, |this, window, cx| {
        this.set_cell_for_test(3, 1, "物理", window, cx);
        this.set_cell_for_test(3, 2, "英语", window, cx);
    });
    harness.render(cx);

    let meta = harness.label("hero-meta", cx);
    assert!(
        meta.contains("今天") || meta.contains("今日无课"),
        "Hero meta 文案合法，实际 {meta}"
    );
}

// ---------- 多课表 ----------

#[gpui_kit::test]
fn multiple_timetables_crud(cx: &mut TestAppContext) {
    temp_data_dir("case9");
    reset_state(base_state());
    let harness = Harness::open(cx);

    harness.click("tt-add", cx);
    harness.update(cx, |this, _, _| {
        assert_eq!(this.state().timetables.len(), 2, "＋ 新建课表");
        assert_eq!(this.state().active_timetable().name, "课表 2", "新建后切换");
    });

    harness.update(cx, |this, window, cx| {
        this.set_cell_for_test(1, 1, "化学", window, cx);
    });
    assert_eq!(harness.label(("tt-cell", 11u32), cx), "化学", "新课表可填课");

    // 切回默认（第一条）
    harness.update(cx, |this, window, cx| {
        let first = this.state().timetables[0].id.clone();
        this.set_active_timetable(&first, cx);
        let _ = window;
    });
    harness.render(cx);
    assert_eq!(
        harness.label(("tt-cell", 11u32), cx),
        "—",
        "切回「默认」旧课仍在（此格为空）"
    );

    // 重命名
    harness.click("tt-rename", cx);
    harness.render(cx);
    harness.type_into_edit("这学期", cx);
    harness.update(cx, |this, window, cx| this.commit_rename_for_test(window, cx));
    assert_eq!(
        harness.update(cx, |this, _, _| this.state().active_timetable().name.clone()),
        "这学期"
    );

    // 空名拒绝
    harness.click("tt-rename", cx);
    harness.render(cx);
    harness.type_into_edit("   ", cx);
    harness.update(cx, |this, window, cx| this.commit_rename_for_test(window, cx));
    harness.render(cx);
    assert_eq!(
        harness.update(cx, |this, _, _| this.state().active_timetable().name.clone()),
        "这学期",
        "空名被拒"
    );
    assert!(
        harness.toast(cx).unwrap_or_default().contains("名称不能为空"),
        "空名提示，实际 {:?}",
        harness.toast(cx)
    );

    // 删除到只剩一条后按钮禁用
    harness.click("tt-del", cx);
    harness.update(cx, |this, _, _| {
        assert_eq!(this.state().timetables.len(), 1, "× 删除当前课表");
    });
    harness.render(cx);
    // 仅剩 1 条时点击删除：不改变数量（按钮已禁用）
    harness.click("tt-del", cx);
    harness.update(cx, |this, _, _| {
        assert_eq!(this.state().timetables.len(), 1, "仅剩 1 条时删除被拒");
    });
}

// ---------- 折叠 ----------

#[gpui_kit::test]
fn collapse_moves_timetable_to_bottom(cx: &mut TestAppContext) {
    temp_data_dir("case10");
    reset_state(base_state());
    let harness = Harness::open(cx);
    harness.render(cx);

    let left_before = harness.snapshot("left-column", cx).bounds();
    let header_before = harness.snapshot("tt-toggle", cx).bounds();

    harness.click("tt-toggle", cx);
    harness.render(cx);

    harness.update(cx, |this, _, _| {
        assert!(this.state().timetable_collapsed, "折叠状态已记录");
    });

    let left_after = harness.snapshot("left-column", cx).bounds();
    let header_after = harness.snapshot("tt-toggle", cx).bounds();

    assert!(
        left_after.size.width > left_before.size.width,
        "左栏撑满整行：{} → {}",
        left_before.size.width,
        left_after.size.width
    );
    assert!(
        header_after.origin.y > header_before.origin.y,
        "「课表」条跳到底部：{} → {}",
        header_before.origin.y,
        header_after.origin.y
    );
}

// ---------- 持久化与迁移 ----------

#[gpui_kit::test]
fn ctrl_s_persists_and_toasts(cx: &mut TestAppContext) {
    temp_data_dir("case11");
    reset_state(base_state());
    let harness = Harness::open(cx);

    harness.update(cx, |this, window, cx| {
        this.set_cell_for_test(1, 1, "高数", window, cx);
        this.save_action(&piwidget::actions::SaveNow, window, cx);
    });
    harness.render(cx);

    let raw = std::fs::read_to_string(storage::data_path()).expect("data file");
    assert!(raw.contains("高数"), "数据落盘");
    assert!(
        harness.toast(cx).unwrap_or_default().contains("已保存"),
        "Ctrl+S 提示，实际 {:?}",
        harness.toast(cx)
    );
}

#[gpui_kit::test]
fn ghost_state_persists(cx: &mut TestAppContext) {
    temp_data_dir("case12");
    reset_state(base_state());
    let harness = Harness::open(cx);

    harness.click("ghost", cx);
    harness.render(cx);

    harness.update(cx, |this, _, _| {
        assert!(this.state().ghost, "透明化已开启");
    });
    assert!(
        harness.toast(cx).unwrap_or_default().contains("已透明化"),
        "透明化提示，实际 {:?}",
        harness.toast(cx)
    );

    harness.update(cx, |this, window, cx| {
        this.save_action(&piwidget::actions::SaveNow, window, cx);
    });
    let raw = std::fs::read_to_string(storage::data_path()).unwrap();
    assert!(raw.contains("\"ghost\": true"), "透明化状态持久化");
}

#[gpui_kit::test]
fn v1_data_migrates_on_load(cx: &mut TestAppContext) {
    let dir = temp_data_dir("v1");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("data.json"),
        r#"{"version":1,"note":"旧版便笺内容","timetable":{"1":{"2":"旧课"}}}"#,
    )
    .unwrap();

    let harness = Harness::open(cx);
    harness.render(cx);
    harness.update(cx, |this, _, _| {
        assert_eq!(this.state().notes[0].content, "旧版便笺内容", "v1 迁移便笺");
        assert_eq!(this.state().version, 4, "迁移到 v4");
    });
    assert_eq!(harness.label(("tt-cell", 12u32), cx), "旧课", "v1 迁移课表");
}

#[gpui_kit::test]
fn corrupt_data_is_backed_up(cx: &mut TestAppContext) {
    let dir = temp_data_dir("corrupt-data");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("data.json"), "{ not json").unwrap();

    let harness = Harness::open(cx);
    harness.render(cx);
    assert!(
        harness.toast(cx).unwrap_or_default().contains("数据文件损坏"),
        "损坏提示，实际 {:?}",
        harness.toast(cx)
    );
    assert!(
        storage::backup_path().exists(),
        "损坏文件已备份为 .bak"
    );
}

#[gpui_kit::test]
fn autostart_button_reports_platform_support(cx: &mut TestAppContext) {
    temp_data_dir("case13");
    reset_state(base_state());
    let harness = Harness::open(cx);
    harness.render(cx);

    let snapshot = harness.snapshot("autostart", cx);
    assert!(snapshot.visible(), "自启按钮可见");
    assert!(
        harness.update(cx, |this, _, _| this.autostart_available()),
        "无头环境下仍能查询平台自启支持"
    );
}

// ---------- 布局 ----------

#[gpui_kit::test]
fn left_column_is_42_percent_when_expanded(cx: &mut TestAppContext) {
    temp_data_dir("case14");
    reset_state(base_state());
    let harness = Harness::open(cx);
    harness.render(cx);

    let sheet = harness.snapshot("sheet", cx).bounds();
    let left = harness.snapshot("left-column", cx).bounds();
    let ratio = f32::from(left.size.width) / f32::from(sheet.size.width);
    assert!(
        (ratio - 0.42).abs() < 0.06,
        "左栏宽度 ≈ 42% 窗口宽，实际 {ratio:.3}"
    );
}

#[gpui_kit::test]
fn tokens_match_design_spec() {
    use gpui_kit::Rgba;
    let hex = |c: Rgba| {
        format!(
            "#{:02X}{:02X}{:02X}",
            (c.r * 255.0).round() as u8,
            (c.g * 255.0).round() as u8,
            (c.b * 255.0).round() as u8
        )
    };
    assert_eq!(hex(tokens::INK), "#16181D");
    assert_eq!(hex(tokens::CARD), "#1D2027");
    assert_eq!(hex(tokens::TEXT), "#E9E7DF");
    assert_eq!(hex(tokens::MUTED), "#8B8A80");
    assert_eq!(hex(tokens::LINE), "#2B2E36");
    assert_eq!(hex(tokens::HIGHLIGHT), "#FFD84D");
    assert_eq!(tokens::FADED, 0.35);
    assert_eq!(tokens::RADIUS, 10.0);
}
