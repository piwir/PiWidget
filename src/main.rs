// 发布版不要额外控制台窗口（Windows）
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use gpui_kit::prelude::*;
use gpui_kit::*;
use piwidget::app::{PiWidget, install_theme, restored_bounds};
use piwidget::theme as tokens;

fn main() {
    gpui_kit::application()
        .with_assets(gpui_kit::assets::Assets)
        .run(|cx| {
            gpui_kit::init(cx);
            install_theme(cx);
            cx.bind_keys(piwidget::actions::key_bindings());

            let options = WindowOptions {
                window_bounds: Some(restored_bounds(cx)),
                window_background: WindowBackgroundAppearance::Transparent,
                window_decorations: Some(WindowDecorations::Client),
                // 与 tokens::MIN_WIDTH / MIN_HEIGHT 一致：记录尺寸的下限也用同一组值，
                // 否则拖到最小尺寸时窗口几何不会被记住
                window_min_size: Some(size(px(tokens::MIN_WIDTH), px(tokens::MIN_HEIGHT))),
                titlebar: Some(TitlebarOptions {
                    title: Some("便笺".into()),
                    appears_transparent: true,
                    traffic_light_position: None,
                }),
                is_resizable: true,
                ..Default::default()
            };

            gpui_kit::open_window(options, cx, |window, cx| {
                cx.new(|cx| {
                    let mut view = PiWidget::new(window, cx);
                    view.observe_window(window, cx);
                    view
                })
            })
            .expect("failed to open PiWidget window");
        });
}
