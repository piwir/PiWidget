<div align="center">

![PiWidget](assets/icons/icon.png)

# PiWidget

桌面便笺 + 周课表小组件 · Rust + GPUI Kit · 数据全部本地

</div>

![截图](docs/screenshot.png)

## 功能

- **便笺** — 新增 / 展开编辑 / 删除 / 拖拽排序，收起时显示标题 + 首行摘要
- **多套课表** — 8 节 × 6 天，自由新建 / 切换 / 重命名 / 删除，点格即改，今日列荧光高亮
- **窗口** — 无边框透明圆角卡片，可拖动缩放；透明化一键切换
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
- v1 / v2 / v3 旧格式自动迁移；文件损坏自动备份 `.bak` 并恢复默认

## 开发

- 架构：`src/app.rs` 根视图与交互，`src/model.rs` schema v4 与迁移，`src/storage.rs` 原子落盘，
  `src/theme.rs` 六色 token，`src/autostart.rs` 开机自启
- 回归：`tests/regression.rs` 用 GPUI Kit 无头窗口驱动真实指针事件并断言渲染树；
  `tests/screenshot.rs` 重新生成 `docs/screenshot.png`
