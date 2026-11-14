#!/usr/bin/env bash
# 安装 Android rust target:从官方源下载 rust-std 包,按双层目录结构解包进工具链
set -uo pipefail

DATE="2026-07-16"
VER="1.97.1"
HOST="x86_64-unknown-linux-gnu"
TC="$HOME/.rustup/toolchains/${VER}-${HOST}/lib/rustlib"
URLBASE="https://static.rust-lang.org/dist"

install_target() {
  local t="$1"
  if [ -f "$TC/$t/lib/libstd.rlib" ] || [ -d "$TC/$t/lib" ]; then
    echo "已存在: $t"
    return 0
  fi
  local f="rust-std-${VER}-${t}.tar.xz"
  echo "下载 $f ..."
  curl -sSfL -o "$HOME/$f" "$URLBASE/$DATE/$f" || { echo "下载失败: $t"; return 1; }
  rm -rf "$HOME/_rs_$t" && mkdir -p "$HOME/_rs_$t"
  tar xJf "$HOME/$f" -C "$HOME/_rs_$t"
  local src="$HOME/_rs_$t/rust-std-${VER}-${t}/rust-std-${t}/lib/rustlib"
  echo "  结构检查:"; find "$src" -maxdepth 2 | head -5
  mkdir -p "$TC"
  cp -r "$src"/. "$TC/"
  rm -rf "$HOME/_rs_$t" "$HOME/$f"
  echo "  安装完成: $t"
}

install_target aarch64-linux-android
install_target x86_64-linux-android

echo "==== 验证 ===="
ok=0
for t in aarch64-linux-android x86_64-linux-android; do
  if [ -d "$TC/$t/lib" ]; then echo "✓ $t"; ok=$((ok+1)); else echo "✗ $t"; fi
done
echo "rustc 实测:"
rustc --target aarch64-linux-android --print file-names --crate-type bin 2>&1 | head -1
