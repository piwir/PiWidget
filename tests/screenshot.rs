/* README 截图生成
 *
 * 渲染演示数据（两条便笺 + 两套课表）并输出 docs/screenshot.png。
 *
 * 平台说明：GPUI 的 headless 渲染器只在 macOS（Metal）与 Linux（wgpu）提供；
 * Windows 上 `render_to_image` 返回 "not implemented for this platform"。
 * 因此本测试在 Windows 上跳过并打印说明，其余平台正常生成截图。
 *
 * 运行：cargo test --test screenshot -- --test-threads=1
 */

use gpui_kit::{
    AppContext, Bounds, HeadlessAppContext, Point, WindowBounds, WindowOptions, px, size,
};

use piwidget::app::PiWidget;
use piwidget::model::{Note, State, Timetable};
use piwidget::storage;

/// 中性桌面色，展示透明圆角（与 Vue 版回归脚本一致）
const DEMO_BG: (u8, u8, u8) = (0x23, 0x26, 0x2e);

fn demo_state() -> State {
    let mut cells_a = std::collections::BTreeMap::new();
    let mut day1 = std::collections::BTreeMap::new();
    day1.insert("1".to_string(), "高数".to_string());
    day1.insert("2".to_string(), "高数".to_string());
    day1.insert("4".to_string(), "英语".to_string());
    cells_a.insert("1".to_string(), day1);
    let mut day2 = std::collections::BTreeMap::new();
    day2.insert("1".to_string(), "物理".to_string());
    day2.insert("3".to_string(), "物理".to_string());
    cells_a.insert("2".to_string(), day2);
    let mut day3 = std::collections::BTreeMap::new();
    day3.insert("2".to_string(), "化学".to_string());
    day3.insert("5".to_string(), "化学".to_string());
    cells_a.insert("3".to_string(), day3);
    let mut day4 = std::collections::BTreeMap::new();
    day4.insert("1".to_string(), "高数".to_string());
    day4.insert("6".to_string(), "体育".to_string());
    cells_a.insert("4".to_string(), day4);
    let mut day5 = std::collections::BTreeMap::new();
    day5.insert("3".to_string(), "编程".to_string());
    cells_a.insert("5".to_string(), day5);

    let mut cells_b = std::collections::BTreeMap::new();
    let mut b1 = std::collections::BTreeMap::new();
    b1.insert("2".to_string(), "线性代数".to_string());
    cells_b.insert("1".to_string(), b1);
    let mut b3 = std::collections::BTreeMap::new();
    b3.insert("4".to_string(), "概率论".to_string());
    cells_b.insert("3".to_string(), b3);

    State {
        notes: vec![
            Note {
                id: "a1".into(),
                title: "本周待办".into(),
                content: "交周报\n整理实验数据\n预约牙医".into(),
            },
            Note {
                id: "a2".into(),
                title: "购物清单".into(),
                content: "燕麦拿铁 x2\n咖啡豆 1kg".into(),
            },
        ],
        timetables: vec![
            Timetable {
                id: "default".into(),
                name: "默认".into(),
                cells: cells_a,
            },
            Timetable {
                id: "t2".into(),
                name: "这学期".into(),
                cells: cells_b,
            },
        ],
        active_timetable_id: "default".into(),
        ..State::default()
    }
}

#[test]
fn renders_readme_screenshot() {
    let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("screenshot-data")
        .join(uuid::Uuid::new_v4().to_string());
    std::fs::create_dir_all(&dir).expect("create screenshot data dir");
    storage::set_data_dir_override(&dir);
    storage::save(&demo_state()).expect("seed demo data");

    // 真 GPUI 上下文 + 平台 headless 渲染器：Linux wgpu / macOS Metal。
    // Windows 未实现 headless renderer，render_to_image 会失败 → 跳过。
    let mut cx = HeadlessAppContext::with_platform(
        gpui_kit::platform::current_platform(true).text_system(),
        std::sync::Arc::new(gpui_kit::assets::Assets),
        gpui_kit::platform::current_headless_renderer,
    );
    cx.update(|cx| {
        gpui_kit::init(cx);
        piwidget::app::install_theme(cx);
    });

    let (handle, view) = cx
        .update(|cx| {
            gpui_kit::open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds {
                        origin: Point::default(),
                        size: size(px(760.0), px(540.0)),
                    })),
                    ..Default::default()
                },
                cx,
                |window, cx| cx.new(|cx| PiWidget::new(window, cx)),
            )
        })
        .expect("open screenshot window");
    let handle: gpui_kit::AnyWindowHandle = handle.into();

    // 触发一帧绘制，让布局与绘制列表就绪
    cx.update_window(handle, |_, window, _cx| window.refresh())
        .expect("refresh frame");

    assert_eq!(
        cx.update(|cx| view.read(cx).state().notes.len()),
        2,
        "演示数据含两条便笺"
    );

    let image = match cx.capture_screenshot(handle) {
        Ok(image) => image,
        Err(err) => {
            println!("screenshot skipped: headless renderer unavailable ({err})");
            return;
        }
    };

    // 把桌面底色铺到透明像素，便于 README 展示圆角
    let mut composed = image.clone();
    let bg = [DEMO_BG.0, DEMO_BG.1, DEMO_BG.2];
    for pixel in composed.pixels_mut() {
        let alpha = f32::from(pixel[3]) / 255.0;
        for (channel, bg_channel) in bg.iter().enumerate() {
            let fg = f32::from(pixel[channel]);
            let mixed = fg * alpha + f32::from(*bg_channel) * (1.0 - alpha);
            pixel[channel] = mixed.round() as u8;
        }
        pixel[3] = 255;
    }

    let out = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("docs")
        .join("screenshot.png");
    std::fs::create_dir_all(out.parent().unwrap()).expect("create docs dir");
    composed.save(&out).expect("save screenshot");

    let size = std::fs::metadata(&out).expect("screenshot file").len();
    println!("saved {} ({} bytes)", out.display(), size);
    assert!(size > 4096, "截图非空（{} bytes）", size);
}
