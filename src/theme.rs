/* PiWidget · 设计 token（与 Vue 时代完全一致）
 *
 * 墨色 #16181D 为底，暖米白正文，荧光笔黄 #FFD84D 是唯一签名色。
 * 六色 token 之外不得新增颜色；淡色态一律从 token 混合派生。
 */

use gpui_kit::{Hsla, Rgba};

/// 卡片底：墨色
pub const INK: Rgba = Rgba {
    r: 0x16 as f32 / 255.0,
    g: 0x18 as f32 / 255.0,
    b: 0x1d as f32 / 255.0,
    a: 1.0,
};

/// 次级表面：hover / 展开态
pub const CARD: Rgba = Rgba {
    r: 0x1d as f32 / 255.0,
    g: 0x20 as f32 / 255.0,
    b: 0x27 as f32 / 255.0,
    a: 1.0,
};

/// 正文：暖米白
pub const TEXT: Rgba = Rgba {
    r: 0xe9 as f32 / 255.0,
    g: 0xe7 as f32 / 255.0,
    b: 0xdf as f32 / 255.0,
    a: 1.0,
};

/// 次要文字：暖灰
pub const MUTED: Rgba = Rgba {
    r: 0x8b as f32 / 255.0,
    g: 0x8a as f32 / 255.0,
    b: 0x80 as f32 / 255.0,
    a: 1.0,
};

/// 发丝分隔线
pub const LINE: Rgba = Rgba {
    r: 0x2b as f32 / 255.0,
    g: 0x2e as f32 / 255.0,
    b: 0x36 as f32 / 255.0,
    a: 1.0,
};

/// 签名色：荧光笔黄
pub const HIGHLIGHT: Rgba = Rgba {
    r: 0xff as f32 / 255.0,
    g: 0xd8 as f32 / 255.0,
    b: 0x4d as f32 / 255.0,
    a: 1.0,
};

/// 圆角（与 --radius 一致）
pub const RADIUS: f32 = 10.0;

/// 弱化态统一透明度（占位课格、透明化 ghost、禁用按钮）
pub const FADED: f32 = 0.35;

/// 默认窗口宽（逻辑像素）——便笺是竖长的便签形态
pub const DESIGN_WIDTH: f32 = 400.0;
/// 默认窗口高：约为宽度的两倍
pub const DESIGN_HEIGHT: f32 = 800.0;
/// 窗口宽度下限
pub const MIN_WIDTH: f32 = 360.0;
/// 最小高度
pub const MIN_HEIGHT: f32 = 360.0;
/// 课表独立窗口的默认尺寸（逻辑像素）
pub const TIMETABLE_WIDTH: f32 = 780.0;
pub const TIMETABLE_HEIGHT: f32 = 620.0;

/// token 转 Hsla（GPUI 内置组件走 Hsla）
pub fn hsla(color: Rgba) -> Hsla {
    color.into()
}

/// 从 token 派生淡色（等价 CSS color-mix）
pub fn mix(base: Rgba, other: Rgba, weight: f32) -> Rgba {
    Rgba {
        r: base.r + (other.r - base.r) * weight,
        g: base.g + (other.g - base.g) * weight,
        b: base.b + (other.b - base.b) * weight,
        a: base.a + (other.a - base.a) * weight,
    }
}

/// 带透明度的 token（等价 color-mix ... transparent）
pub fn alpha(color: Rgba, a: f32) -> Rgba {
    Rgba { a, ..color }
}

/// 卡片边框：text 18%
pub fn sheet_border() -> Rgba {
    alpha(TEXT, 0.18)
}

/// 今日列底色（highlight 8% 混入 ink）
pub fn today_bg() -> Rgba {
    mix(INK, HIGHLIGHT, 0.08)
}

/// 今日列有课格底色（highlight 14% 混入 card）
pub fn today_filled_bg() -> Rgba {
    mix(CARD, HIGHLIGHT, 0.14)
}

/// 便笺卡片 hover
pub fn note_hover_bg() -> Rgba {
    alpha(CARD, 0.6)
}

/// 字体候选链（按顺序探测系统字体，首个命中者胜出）
pub const FONT_DISPLAY_CANDIDATES: &[&str] = &[
    "Saira Condensed",
    "Bahnschrift",
    "Arial Narrow",
    "Microsoft YaHei",
    "Noto Sans CJK SC",
];

pub const FONT_BODY_CANDIDATES: &[&str] = &[
    "PingFang SC",
    "Hiragino Sans GB",
    "Microsoft YaHei",
    "Noto Sans CJK SC",
    "Segoe UI",
    "DejaVu Sans",
];

pub const FONT_MONO_CANDIDATES: &[&str] = &[
    "IBM Plex Mono",
    "Cascadia Mono",
    "SF Mono",
    "Consolas",
    "DejaVu Sans Mono",
];

/// 标题栏高度（与 Vue 版一致）
pub const TITLEBAR_HEIGHT: f32 = 44.0;

/// 从候选链里挑第一个系统已安装的字体，全部缺失时返回 None（由 GPUI 用系统默认）
pub fn pick_font(candidates: &[&str], installed: &[String]) -> Option<String> {
    let installed: Vec<String> = installed.iter().map(|n| n.to_lowercase()).collect();
    candidates
        .iter()
        .find(|name| installed.iter().any(|i| i == &name.to_lowercase()))
        .map(|name| (*name).to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_match_design_spec() {
        let hex = |c: Rgba| {
            format!(
                "#{:02X}{:02X}{:02X}",
                (c.r * 255.0).round() as u8,
                (c.g * 255.0).round() as u8,
                (c.b * 255.0).round() as u8
            )
        };
        assert_eq!(hex(INK), "#16181D");
        assert_eq!(hex(CARD), "#1D2027");
        assert_eq!(hex(TEXT), "#E9E7DF");
        assert_eq!(hex(MUTED), "#8B8A80");
        assert_eq!(hex(LINE), "#2B2E36");
        assert_eq!(hex(HIGHLIGHT), "#FFD84D");
        assert_eq!(FADED, 0.35);
        assert_eq!(RADIUS, 10.0);
    }

    #[test]
    fn mix_and_alpha_derive_without_new_colors() {
        let faded = alpha(TEXT, 0.18);
        assert!((faded.a - 0.18).abs() < f32::EPSILON);
        assert_eq!((faded.r, faded.g, faded.b), (TEXT.r, TEXT.g, TEXT.b));

        let tinted = mix(INK, HIGHLIGHT, 0.08);
        assert!(tinted.r > INK.r && tinted.r < HIGHLIGHT.r);
        assert!((tinted.a - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn picks_first_installed_font() {
        let installed = vec!["dejavu sans".to_string(), "noto sans cjk sc".to_string()];
        assert_eq!(pick_font(FONT_BODY_CANDIDATES, &installed).as_deref(), Some("Noto Sans CJK SC"));
        assert_eq!(pick_font(&["No Such Font"], &installed), None);
    }
}
