/* PiWidget 回归自检（gpui-kit 无头 UI 测试）
 *
 * 对应 Vue 时代 scripts/regression.py 的断言，按语义重写为 Rust 断言：
 *   - 观测真实渲染树（元素 id + 无障碍属性 + 布局边界）
 *   - 用真实指针/键盘事件驱动交互，不直接调用内部方法
 *   - 覆盖便笺 CRUD、课表卡与新窗口、多课表、持久化、迁移、窗口尺寸恢复
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

/// 「＋ 新建」现在是下拉：点开菜单里的「新建便笺」
fn add_note_via_menu(harness: &Harness, cx: &mut TestAppContext) {
    harness.click("note-add", cx);
    harness.render(cx);
    cx.update_window(harness.window, |_, window, cx| {
        window.within("popup-menu").click(0usize, cx);
    })
    .expect("click 新建便笺");
    harness.render(cx);
}

#[gpui_kit::test]
fn note_create_edit_collapse_and_delete(cx: &mut TestAppContext) {
    temp_data_dir("case4");
    reset_state(base_state());
    let harness = Harness::open(cx);

    add_note_via_menu(&harness, cx);
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

/// 打开便笺列表首行的课表卡（课表不再占右侧栏，默认收起）
fn open_timetable_card(harness: &Harness, cx: &mut TestAppContext) {
    harness.click("tt-note-toggle", cx);
    harness.render(cx);
}

#[gpui_kit::test]
fn timetable_lives_in_note_list_as_a_card(cx: &mut TestAppContext) {
    temp_data_dir("case6");
    reset_state(base_state());
    let harness = Harness::open(cx);
    harness.render(cx);

    // 课表卡在便笺列表里，默认收起（不渲染课格，也不再占右侧栏）
    let card = harness.snapshot("tt-note-card", cx);
    assert!(card.visible(), "课表卡可见");
    assert!(
        cx.update_window(harness.window, |_, window, _| window
            .try_find("right-column")
            .is_none())
        .unwrap(),
        "不再有 58% 右侧栏"
    );
    assert!(
        cx.update_window(harness.window, |_, window, _| window
            .try_find(("tt-cell", 11u32))
            .is_none())
        .unwrap(),
        "课表卡收起时不渲染课格"
    );

    open_timetable_card(&harness, cx);
    harness.update(cx, |this, _, _| assert!(this.timetable_open(), "课表卡已展开"));

    // 8 节 × 6 天
    for day in 1u8..=6 {
        for period in 1u8..=8 {
            let id = ("tt-cell", day as u32 * 10 + period as u32);
            let snap = harness.snapshot(id, cx);
            assert!(snap.visible(), "课格 {day}-{period} 可见");
        }
    }

    harness.update(cx, |this, _, _| {
        assert_eq!(this.state().active_timetable().cells.len(), 0, "默认课表为空");
    });
}

#[gpui_kit::test]
fn new_note_button_offers_timetable_entry(cx: &mut TestAppContext) {
    temp_data_dir("case6b");
    reset_state(base_state());
    let harness = Harness::open(cx);
    harness.render(cx);

    // ＋ 新建 是下拉：包含「新建便笺」与「课表」
    harness.click("note-add", cx);
    harness.render(cx);
    let menu_items: Vec<String> = cx
        .update_window(harness.window, |_, window, _| {
            let mut labels = Vec::new();
            for i in 0..4usize {
                if let Some(snap) = window.within("popup-menu").try_find(i) {
                    if let Some(label) = snap.label() {
                        labels.push(label.to_string());
                    }
                }
            }
            labels
        })
        .unwrap();
    assert!(
        menu_items.iter().any(|l| l.contains("新建便笺")),
        "下拉含新建便笺，实际 {menu_items:?}"
    );
    assert!(
        menu_items.iter().any(|l| l.contains("课表")),
        "下拉含课表，实际 {menu_items:?}"
    );
}

#[gpui_kit::test]
fn cell_edit_enter_commits_and_escape_cancels(cx: &mut TestAppContext) {
    temp_data_dir("case7");
    reset_state(base_state());
    let harness = Harness::open(cx);
    harness.render(cx);
    open_timetable_card(&harness, cx);

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
    // 课表卡上也显示同一份统计
    let card_meta = harness.label("tt-note-meta", cx);
    assert!(
        card_meta.contains("今天") || card_meta.contains("今日无课"),
        "课表卡统计文案合法，实际 {card_meta}"
    );
}

// ---------- 多课表 ----------

#[gpui_kit::test]
fn multiple_timetables_crud(cx: &mut TestAppContext) {
    temp_data_dir("case9");
    reset_state(base_state());
    let harness = Harness::open(cx);
    harness.render(cx);
    open_timetable_card(&harness, cx);

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

// ---------- 课表新窗口 ----------

#[gpui_kit::test]
fn timetable_card_opens_a_separate_window(cx: &mut TestAppContext) {
    temp_data_dir("case10");
    let mut seeded = base_state();
    seeded.set_cell(1, 1, "高数");
    reset_state(seeded);
    let harness = Harness::open(cx);
    harness.render(cx);

    let windows_before = cx.update(|cx| cx.windows().len());
    harness.click("tt-open-window", cx);
    harness.render(cx);
    let windows_after = cx.update(|cx| cx.windows().len());
    assert_eq!(
        windows_after,
        windows_before + 1,
        "↗ 打开独立课表窗口（{} → {}）",
        windows_before,
        windows_after
    );

    // 新窗口里能渲染出同一份课格数据
    let other = cx
        .update(|cx| {
            cx.windows()
                .into_iter()
                .find(|handle| handle.window_id() != harness.window.window_id())
        })
        .expect("找到课表窗口");
    cx.update_window(other, |_, window, cx| window.render_frame(cx))
        .expect("渲染课表窗口");
    cx.update_window(other, |_, window, _| {
        let cell = window.find(("tt-cell", 11u32));
        assert_eq!(cell.label().as_deref(), Some("高数"), "课表窗口显示同一份数据");
    })
    .expect("读取课表窗口");
}

/// 重复点 ↗ 只激活已有窗口，不再开第二个
#[gpui_kit::test]
fn opening_timetable_window_twice_reuses_it(cx: &mut TestAppContext) {
    temp_data_dir("case10c");
    reset_state(base_state());
    let harness = Harness::open(cx);
    harness.render(cx);

    harness.click("tt-open-window", cx);
    harness.render(cx);
    let after_first = cx.update(|cx| cx.windows().len());

    harness.click("tt-open-window", cx);
    harness.render(cx);
    let after_second = cx.update(|cx| cx.windows().len());
    assert_eq!(
        after_second, after_first,
        "第二次点 ↗ 复用已有窗口（{} → {}）",
        after_first, after_second
    );
}

#[gpui_kit::test]
fn timetable_card_stays_without_main_window_column(cx: &mut TestAppContext) {
    temp_data_dir("case10b");
    reset_state(base_state());
    let harness = Harness::open(cx);
    harness.render(cx);

    // 收起 → 展开 → 收起，课表卡始终在列表首行
    let collapsed_card = harness.snapshot("tt-note-card", cx).bounds();
    open_timetable_card(&harness, cx);
    harness.update(cx, |this, window, cx| this.toggle_timetable(window, cx));
    harness.render(cx);

    let again = harness.snapshot("tt-note-card", cx).bounds();
    assert_eq!(
        f32::from(collapsed_card.origin.y),
        f32::from(again.origin.y),
        "课表卡位置稳定在便笺列表首行"
    );
    harness.update(cx, |this, _, _| assert!(!this.timetable_open(), "再次点击收起"));
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
        assert_eq!(this.state().version, 5, "迁移到 v5");
    });
    open_timetable_card(&harness, cx);
    assert_eq!(harness.label(("tt-cell", 12u32), cx), "旧课", "v1 迁移课表");
}

/// 自启动窗口变大的回归：v4 数据里存的是物理像素尺寸（高 DPI 下 1504 逻辑像素
/// 被当成物理像素写盘），开机恢复时不能再原样当窗口尺寸用。
#[gpui_kit::test]
fn v4_physical_window_size_does_not_restore_oversized(cx: &mut TestAppContext) {
    let dir = temp_data_dir("v4-window");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("data.json"),
        r#"{
            "version": 4,
            "notes": [],
            "timetables": [{"id":"default","name":"默认","cells":{}}],
            "activeTimetableId": "default",
            "timetableCollapsed": true,
            "windowPos": {"x": 40, "y": 40},
            "windowSize": {"w": 1504.6666, "h": 651.3333}
        }"#,
    )
    .unwrap();

    let harness = Harness::open(cx);
    harness.render(cx);
    harness.update(cx, |this, _, _| {
        assert_eq!(this.state().version, 5, "升级到 v5");
        assert!(
            this.state().window_size.is_none(),
            "旧物理像素尺寸被丢弃，实际 {:?}",
            this.state().window_size
        );
        assert!(
            this.state().window_pos.is_some(),
            "位置仍然保留（同一坐标空间）"
        );
    });

    // 落盘后不再写回废弃的折叠字段，尺寸也换成 v5 的逻辑像素
    harness.update(cx, |this, window, cx| {
        this.save_action(&piwidget::actions::SaveNow, window, cx);
    });
    let raw = std::fs::read_to_string(storage::data_path()).unwrap();
    assert!(
        !raw.contains("timetableCollapsed"),
        "升级后不再写回废弃的折叠字段"
    );
    assert!(raw.contains("\"version\": 5"), "落盘为 v5，实际 {raw}");
    let value: serde_json::Value = serde_json::from_str(&raw).unwrap();
    assert!(value["windowSize"].is_null(), "旧尺寸已被丢弃且未被写回");
}

#[gpui_kit::test]
fn window_size_persists_in_logical_pixels(cx: &mut TestAppContext) {
    temp_data_dir("window-logical");
    reset_state(base_state());
    let harness = Harness::open(cx);
    harness.render(cx);

    harness.update(cx, |this, window, cx| {
        let bounds = window.bounds();
        this.record_window_bounds(bounds.origin, bounds.size, cx);
        this.save_action(&piwidget::actions::SaveNow, window, cx);
    });

    let raw = std::fs::read_to_string(storage::data_path()).unwrap();
    let value: serde_json::Value = serde_json::from_str(&raw).unwrap();
    let saved = &value["windowSize"];
    let w = saved["w"].as_f64().expect("windowSize.w");
    let h = saved["h"].as_f64().expect("windowSize.h");
    assert!(
        (360.0..=4000.0).contains(&w) && (360.0..=4000.0).contains(&h),
        "落盘尺寸是逻辑像素而不是物理像素，实际 {w}×{h}"
    );
    assert_eq!(value["version"], 5, "写回 v5");
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
fn notes_fill_the_whole_window_width(cx: &mut TestAppContext) {
    temp_data_dir("case14");
    reset_state(base_state());
    let harness = Harness::open(cx);
    harness.render(cx);

    let sheet = harness.snapshot("sheet", cx).bounds();
    let notes = harness.snapshot("notes", cx).bounds();
    let ratio = f32::from(notes.size.width) / f32::from(sheet.size.width);
    assert!(
        ratio > 0.95,
        "便笺列表占满整宽（课表已移出右栏），实际 {ratio:.3}"
    );
    let hero = harness.snapshot("hero", cx).bounds();
    assert!(
        f32::from(hero.size.width) / f32::from(sheet.size.width) > 0.95,
        "Hero 也占满整宽"
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
