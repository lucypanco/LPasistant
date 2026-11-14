#!/usr/bin/env bash
# Android SDK 安装(腾讯镜像手动解包,绕开被限速的 dl.google.com)
# 用法: bash wsl_install_sdk.sh   (在 WSL 内运行;幂等,已存在的包自动跳过)
set -uo pipefail

M="https://mirrors.cloud.tencent.com/AndroidSDK"
SDK="$HOME/android"
cd "$SDK" || { echo "无法进入 $SDK"; exit 1; }

fetch() { # fetch <相对路径/文件名> [备用名]
  local f="$1" alt="${2:-}"
  if curl -sSf -m 30 "$M/repository2-3.xml" -o repo.xml 2>/dev/null; then :; fi
  local url="$M/$f"
  if ! curl -sfI -m 15 "$url" >/dev/null 2>&1; then
    if [ -n "$alt" ] && curl -sfI -m 15 "$M/$alt" >/dev/null 2>&1; then
      url="$M/$alt"; f="$alt"
    else
      echo "SKIP(镜像无此包): $f"
      return 1
    fi
  fi
  echo "下载 $f ..."
  curl -sS --retry 2 -o "$f" "$url" || return 1
  return 0
}

# ---- 1) platform-tools (adb) ----
if [ ! -x "$SDK/platform-tools/adb" ]; then
  fetch "platform-tools-latest-linux.zip" && unzip -qo platform-tools-latest-linux.zip -d . && echo "OK platform-tools"
else echo "已存在 platform-tools"; fi

# ---- 2) build-tools 34 ----
if [ ! -d "$SDK/build-tools/34.0.0" ]; then
  fetch "build-tools_r34-linux.zip" && \
  rm -rf _bt && mkdir _bt && unzip -qo build-tools_r34-linux.zip -d _bt && \
  mkdir -p build-tools/34.0.0 && cp -r _bt/*/. build-tools/34.0.0/ && rm -rf _bt && echo "OK build-tools"
else echo "已存在 build-tools"; fi

# ---- 3) platform android-34 ----
if [ ! -f "$SDK/platforms/android-34/android.jar" ]; then
  PF=$(grep -oE 'platform-34_r[0-9]+\.zip' repo.xml 2>/dev/null | sort -V | tail -1)
  [ -z "$PF" ] && PF=$(grep -oE 'platform-34-ext[0-9]+_r[0-9]+\.zip' repo.xml 2>/dev/null | sort -V | tail -1)
  if fetch "platform-34_r02.zip" "$PF" && rm -rf _pf && mkdir _pf && unzip -qo "$PF" -d _pf 2>/dev/null; then
    d=$(find _pf -maxdepth 1 -mindepth 1 -type d | head -1)
    mkdir -p platforms && cp -r "$d"/. platforms/android-34/ && rm -rf _pf && echo "OK platform-34"
  else echo "WARN: platform-34 安装失败"; fi
else echo "已存在 platform-34"; fi

# ---- 4) NDK r26c ----
if [ ! -d "$SDK/ndk/26.3.11579264" ]; then
  fetch "android-ndk-r26c-linux.zip" && rm -rf _ndk && mkdir _ndk && \
  unzip -qo android-ndk-r26c-linux.zip -d _ndk && mkdir -p ndk && \
  cp -r _ndk/android-ndk-r26c/. ndk/26.3.11579264/ && rm -rf _ndk && echo "OK ndk"
else echo "已存在 ndk"; fi

# ---- 5) emulator ----
if [ -x "$SDK/emulator/emulator" ]; then
  echo "已存在 emulator"
else
  EMU=$(grep -oE 'emulator-linux_x64-[0-9]+\.zip' repo.xml 2>/dev/null | sort -V | tail -1)
  [ -z "$EMU" ] && EMU="emulator-linux_x64-16740729.zip"
  if fetch "$EMU"; then
    rm -rf _emu && mkdir _emu && unzip -qo "$EMU" -d _emu
    d=$(find _emu -maxdepth 1 -mindepth 1 -type d | head -1)
    mkdir -p emulator
    if [ -n "$d" ]; then cp -r "$d"/. emulator/; else cp -r _emu/. emulator/; fi
    rm -rf _emu
    echo "OK emulator"
  else
    echo "WARN: emulator 安装失败"
  fi
fi

# ---- 6) 系统镜像 android-34 google_apis x86_64 ----
IMG_DIR="$SDK/system-images/android-34/google_apis/x86_64"
if [ ! -f "$IMG_DIR/system.img" ] && [ ! -f "$IMG_DIR/system.img.qcow2" ]; then
  IMG=$(grep -oE 'sys-img/google_apis/sysimg_x86_64-34[A-Za-z0-9._-]*\.zip' repo.xml 2>/dev/null | sort -V | tail -1)
  [ -z "$IMG" ] && IMG=$(grep -oE 'sysimg_x86_64-34[A-Za-z0-9._-]*\.zip' repo.xml 2>/dev/null | sort -V | tail -1)
  if [ -n "$IMG" ] && fetch "$IMG"; then
    rm -rf _img && mkdir _img && unzip -qo "$(basename "$IMG")" -d _img
    mkdir -p "$IMG_DIR"
    d=$(find _img -maxdepth 2 -mindepth 1 -type d | tail -1)
    cp -r "$d"/. "$IMG_DIR/" && rm -rf _img && echo "OK system-image"
  else echo "WARN: 系统镜像安装失败(模拟器仍可装,无镜像不能启动)"; fi
else echo "已存在 system-image"; fi

# ---- 7) AVD(手动注册) ----
AVD_DIR="$HOME/.android/avd/api34.avd"
if [ ! -d "$AVD_DIR" ]; then
  mkdir -p "$AVD_DIR"
  cat > "$HOME/.android/avd/api34.ini" <<EOF
avd.ini.encoding=UTF-8
path=$AVD_DIR
path.rel=avd/api34.avd
target=android-34
EOF
  cat > "$AVD_DIR/config.ini" <<EOF
avd.ini.encoding=UTF-8
image.sysdir.1=system-images/android-34/google_apis/x86_64/
hw.cpu.arch=x86_64
hw.cpu.ncore=4
hw.ramSize=2048
hw.lcd.width=1080
hw.lcd.height=2340
hw.lcd.density=440
hw.keyboard=yes
disk.dataPartition.size=6G
snapshot.present=false
EOF
  echo "OK avd(api34)"
else echo "已存在 avd"; fi

# ---- 汇总 ----
echo "==== 安装结果 ===="
for p in platform-tools/adb build-tools/34.0.0/aapt2 platforms/android-34/android.jar \
         ndk/26.3.11579264/ndk-build emulator/emulator \
         system-images/android-34/google_apis/x86_64/system.img; do
  if [ -e "$SDK/$p" ] || [ -f "$SDK/$p" ]; then echo "✓ $p"; else echo "✗ $p"; fi
done
echo "==== 完成 ===="
