/* PiWidget · Windows 资源
 *
 * 把 assets/icons/icon.ico 作为图标资源（ID 1）写进 exe。
 *
 * 为什么必须是 ID 1：GPUI 的 Windows 后端创建窗口时用
 *   LoadImageW(GetModuleHandleW(None), PCWSTR(1), IMAGE_ICON, ...)
 * 取窗口 / 任务栏图标；资源 ID 不是 1 就取不到，窗口会退回系统默认图标。
 *
 * 非 Windows 目标直接跳过（embed-resource 在非 Windows 下本来就什么都不做）。
 * Windows 目标下图标是硬要求：编不出来就让构建失败，避免再发出一个没有图标的包。
 */

fn main() {
    println!("cargo:rerun-if-changed=assets/icons/icon.ico");
    println!("cargo:rerun-if-changed=piwidget.rc");
    println!("cargo:rerun-if-changed=build.rs");

    // .rc 里是相对路径，rc 由 cargo 在 crate 根目录执行；但保险起见显式给绝对路径，
    // 避免某些 CI 环境把 build script 的 CWD 设在别处
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".into());
    let icon = std::path::Path::new(&manifest_dir).join("assets/icons/icon.ico");
    if !icon.exists() {
        panic!("缺少图标文件：{}", icon.display());
    }

    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os != "windows" {
        // 非 Windows 目标不编资源；但仍要证明 .ico 可读，避免把坏图标带进发布
        assert_readable_ico(&icon);
        return;
    }

    embed_resource::compile("piwidget.rc", embed_resource::NONE)
        .manifest_required()
        .expect("嵌入 Windows 图标资源失败：检查 .ico 与 rc.exe/llvm-rc/windres 是否可用");
}

/// 用最少的解析验证 .ico 结构（目录项与每帧长度自洽）。
/// 交叉编译时 windres/rc 往往不在本机，坏图标只有到 Windows 上才会暴露，这里提前拦住。
fn assert_readable_ico(path: &std::path::Path) {
    let data = std::fs::read(path).expect("读取 .ico 失败");
    assert!(data.len() > 22, ".ico 太小");
    assert_eq!(&data[..2], &[0, 0], ".ico 缺少保留字段");
    let kind = u16::from_le_bytes([data[2], data[3]]);
    assert_eq!(kind, 1, ".ico 类型不是 1（图标）");
    let count = u16::from_le_bytes([data[4], data[5]]) as usize;
    assert!(count > 0, ".ico 不含任何尺寸");

    let mut seen = Vec::new();
    for i in 0..count {
        let entry = 6 + i * 16;
        let size = u32::from_le_bytes([
            data[entry + 8],
            data[entry + 9],
            data[entry + 10],
            data[entry + 11],
        ]) as usize;
        let offset = u32::from_le_bytes([
            data[entry + 12],
            data[entry + 13],
            data[entry + 14],
            data[entry + 15],
        ]) as usize;
        assert!(
            offset + size <= data.len(),
            ".ico 第 {i} 帧越界（offset={offset}, size={size}, total={}）",
            data.len()
        );
        seen.push(data[entry] as u16);
    }
    assert!(seen.len() >= 4, ".ico 只有 {} 个尺寸，至少 4 个", seen.len());
}
