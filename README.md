<div align="center">

![PiWidget](assets/icons/icon.png)

# PiWidget

桌面便笺 + 周课表小组件 · Rust + GPUI Kit · 数据全部本地

</div>

![截图](docs/screenshot.png)

## 功能

- **便笺** — 「＋ 新建」下拉可新建便笺或打开课表；展开编辑 / 删除 / 拖拽排序，
  收起时显示标题 + 首行摘要
- **课表** — 便笺列表首行是一张固定课表卡：点开即内嵌 8 节 × 6 天课格，
  「↗」可把课表开成独立窗口（两边数据实时同步），今日列荧光高亮
- **多套课表** — 自由新建 / 切换 / 重命名 / 删除，点格即改
- **窗口** — 无边框透明圆角卡片，可拖动缩放；透明化一键切换；
  窗口位置与尺寸按逻辑像素记住，换 DPI / 换屏幕后仍按原样恢复
- **开机自启** — 标题栏开关，随系统登录启动
- **数据** — 编辑防抖自动保存，`Ctrl+S` 立即落盘，一键导入 / 导出 JSON
- **主题** — 墨色 `#16181D` + 荧光笔黄 `#FFD84D`，深色护眼

## 快速开始

```bash
cargo run                 # 开发运行
cargo test --lib          # 数据层单元测试
./scripts/test.sh         # 全量测试（含无头 UI 回归，串行执行）
cargo build --release     # 打包可执行文件
```

Linux 构建需要 `libxkbcommon-x11-dev`（缺失时可用工作区 `.dev-lib/` 内的本地链接兜底，见 `scripts/test.sh`）。

## 数据与隐私

- 数据仅存本地：`%APPDATA%\com.piwidget.app\data.json`（Windows）、
  `~/Library/Application Support/com.piwidget.app/data.json`（macOS）、
  `~/.local/share/com.piwidget.app/data.json`（Linux），**无任何网络请求**
- 从 Tauri 版本升级无需迁移：路径与 schema 完全一致
- v1 / v2 / v3 / v4 旧格式自动迁移到 **schema v5**；文件损坏自动备份 `.bak` 并恢复默认
- v5 起窗口尺寸改存逻辑像素：v4 存在高 DPI 屏上的物理像素尺寸会让开机自启的窗口异常放大，
  升级时自动丢弃旧尺寸并重新按屏幕夹取

## 开发

- Windows 资源：`build.rs` + `piwidget.rc` 把 `assets/icons/icon.ico`（资源 ID 1）嵌进 exe，
  CI 打包前用 `scripts/check_windows_icon.py` 校验图标确实在 PE 里；
  换图标改 `assets/icons/icon.png`，再跑 `python3 scripts/build_icon.py` 重新生成 `.ico`
- 架构：`src/app.rs` 根视图与交互，`src/grid.rs` 课格渲染单元，
  `src/timetable_window.rs` 独立课表窗口，`src/model.rs` schema v5 与迁移，
  `src/storage.rs` 原子落盘，`src/theme.rs` 六色 token，`src/autostart.rs` 开机自启
- 回归：`tests/regression.rs` 用 GPUI Kit 无头窗口驱动真实指针事件并断言渲染树；
  `tests/screenshot.rs` 重新生成 `docs/screenshot.png`
