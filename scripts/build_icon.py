#!/usr/bin/env python3
"""从 assets/icons/icon.png 生成 assets/icons/icon.ico。

为什么不用 `PIL.save(..., format="ICO")` 一把梭：Pillow 默认把**所有**尺寸都写成
PNG 压缩帧，而 Windows 壳层对 PNG 帧的小图标支持不完整（16/24/32 这类会在部分
场景显示空白或旧缓存图标）。Windows 自己的约定是：

    16 / 24 / 32 / 48 / 64 / 128  →  BMP(DIB) 帧，带 AND 掩码
    256                           →  PNG 帧（体积小，Vista+ 支持）

这里按该约定手工拼 ICO 目录，尺寸与顺序都对得上资源管理器 / 任务栏 / Alt-Tab 的取值习惯。

用法：python3 scripts/build_icon.py [源 PNG] [输出 ICO]
"""

import struct
import sys
from pathlib import Path

# Same guard as check_windows_icon.py: legacy Windows codepages cannot encode
# non-ASCII output, and a UnicodeEncodeError here would look like a broken icon.
for _stream in (sys.stdout, sys.stderr):
    try:
        _stream.reconfigure(encoding="utf-8", errors="replace")
    except (AttributeError, ValueError, OSError):
        pass

try:
    from PIL import Image
except ImportError:  # pragma: no cover - 仅开发期需要
    sys.exit("Pillow is required: pip install pillow")

REPO = Path(__file__).resolve().parent.parent
SRC = Path(sys.argv[1]) if len(sys.argv) > 1 else REPO / "assets/icons/icon.png"
OUT = Path(sys.argv[2]) if len(sys.argv) > 2 else REPO / "assets/icons/icon.ico"

BMP_SIZES = [16, 24, 32, 48, 64, 128]
PNG_SIZE = 256


def dib_frame(image: "Image.Image") -> bytes:
    """把一张 RGBA 图编码成 ICO 用的 DIB（BITMAPINFOHEADER + BGRA 自底向上 + AND 掩码）。"""
    width, height = image.size
    rgba = image.convert("RGBA")

    # BITMAPINFOHEADER：高度写两倍（XOR 位图 + AND 掩码）
    header = struct.pack(
        "<IiiHHIIiiII",
        40,  # biSize
        width,
        height * 2,
        1,  # biPlanes
        32,  # biBitCount
        0,  # biCompression = BI_RGB
        0,  # biSizeImage
        0,
        0,
        0,
        0,
    )

    pixels = bytearray()
    # DIB 是自底向上、BGRA
    for y in range(height - 1, -1, -1):
        for x in range(width):
            r, g, b, a = rgba.getpixel((x, y))
            pixels += bytes((b, g, r, a))

    # AND 掩码：32bpp 下按 1bpp 填 0（完全由 alpha 决定），每行按 4 字节对齐
    row_bytes = ((width + 31) // 32) * 4
    mask = bytes(row_bytes * height)

    return header + bytes(pixels) + mask


def png_frame(image: "Image.Image") -> bytes:
    import io

    buffer = io.BytesIO()
    image.convert("RGBA").save(buffer, format="PNG", optimize=True)
    return buffer.getvalue()


def main() -> int:
    if not SRC.exists():
        sys.exit(f"source image not found: {SRC}")

    source = Image.open(SRC).convert("RGBA")
    if source.size[0] < PNG_SIZE or source.size[1] < PNG_SIZE:
        sys.exit(f"source image must be at least {PNG_SIZE}x{PNG_SIZE}, got {source.size}")

    frames = []
    for size in BMP_SIZES:
        frames.append((size, dib_frame(source.resize((size, size), Image.LANCZOS))))
    frames.append((PNG_SIZE, png_frame(source.resize((PNG_SIZE, PNG_SIZE), Image.LANCZOS))))

    # ICO 头 + 目录 + 数据
    offset = 6 + 16 * len(frames)
    directory = bytearray()
    payload = bytearray()
    for size, data in frames:
        directory += struct.pack(
            "<BBBBHHII",
            size if size < 256 else 0,
            size if size < 256 else 0,
            0,  # 调色板颜色数
            0,  # 保留
            1,  # 平面数
            32,  # 位深
            len(data),
            offset,
        )
        payload += data
        offset += len(data)

    OUT.parent.mkdir(parents=True, exist_ok=True)
    OUT.write_bytes(struct.pack("<HHH", 0, 1, len(frames)) + bytes(directory) + bytes(payload))

    print(f"[OK] {OUT} ({OUT.stat().st_size} bytes, {len(frames)} sizes: "
          f"{', '.join(str(s) for s, _ in frames)})")
    return 0


if __name__ == "__main__":
    sys.exit(main())
