#!/usr/bin/env bash
# PiWidget 测试入口
# GPUI Kit 的主题/键位/数据目录覆盖是进程级全局状态，
# 回归测试按线程并行会互相干扰，因此统一用 --test-threads=1 串行执行。
set -euo pipefail
cd "$(dirname "$0")/.."

# Linux 缺少 libxkbcommon-x11 开发包时，用工作区内的本地链接兜底（CI 请装 libxkbcommon-x11-dev）
if [ -f .dev-lib/libxkbcommon-x11.so ]; then
  export RUSTFLAGS="${RUSTFLAGS:-} -L $PWD/.dev-lib"
fi
export RUST_MIN_STACK="${RUST_MIN_STACK:-33554432}"

cargo test --lib "$@"
cargo test --test regression -- --test-threads=1 "$@"
