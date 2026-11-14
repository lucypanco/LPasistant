#!/usr/bin/env bash
# 交叉编译 arm64 jniLibs:修复工具链(android 库移入 stable)+ cargo ndk 构建
set -euo pipefail

STABLE="$HOME/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/lib/rustlib"
PARTIAL="$HOME/.rustup/toolchains/1.97.1-x86_64-unknown-linux-gnu/lib/rustlib"

if [ -d "$PARTIAL/aarch64-linux-android" ]; then
  echo "== 把 android rustlib 移入 stable 工具链 =="
  cp -r "$PARTIAL/aarch64-linux-android" "$STABLE/"
  cp -r "$PARTIAL/x86_64-linux-android" "$STABLE/"
  rm -rf "$HOME/.rustup/toolchains/1.97.1-x86_64-unknown-linux-gnu"
  echo "OK"
fi
ls "$STABLE"

echo "== cargo ndk 交叉编译 =="
cd "$HOME/qqbot/core"
export ANDROID_HOME="$HOME/android"
export ANDROID_NDK_HOME="$HOME/android/ndk/26.3.11579264"
"$HOME/.cargo/bin/cargo" ndk -t arm64-v8a -o /mnt/c/qq-bot/mobile/app/src/main/jniLibs build --release

echo "== 产物 =="
ls -la /mnt/c/qq-bot/mobile/app/src/main/jniLibs/arm64-v8a/
