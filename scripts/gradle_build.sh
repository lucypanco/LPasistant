#!/usr/bin/env bash
# Gradle 构建 debug APK(在 WSL 内运行)
set -euo pipefail

echo "== 同步 mobile 工程到 WSL 原生目录 =="
rm -rf "$HOME/qqbot/mobile"
cp -r /mnt/c/qq-bot/mobile "$HOME/qqbot/mobile"

cd "$HOME/qqbot/mobile"
export ANDROID_HOME="$HOME/android"
export JAVA_HOME="${JAVA_HOME:-/usr/lib/jvm/java-17-openjdk-amd64}"

echo "== local.properties =="
echo "sdk.dir=$ANDROID_HOME" > local.properties

echo "== gradle wrapper assembleRelease(首次会下载 Gradle 发行版与依赖)=="
# 绕过 gradlew shell 脚本,直接调用 wrapper 主类(等效且无引号坑)
# 2026-11-14 安全审计后改产 release 包:release 不可调试、不带 ui-tooling 调试组件
java -Dorg.gradle.appname=gradlew \
     -classpath "$PWD/gradle/wrapper/gradle-wrapper.jar" \
     org.gradle.wrapper.GradleWrapperMain assembleRelease --no-daemon 2>&1 | tail -20

echo "== 拷回 APK(release,复用 debug 密钥签名)=="
cp app/build/outputs/apk/release/app-release.apk /mnt/c/qq-bot/LPasistant-release.apk
cp app/build/outputs/apk/release/app-release.apk /mnt/c/qq-bot/release/LPasistant-v1.0-release.apk
ls -la /mnt/c/qq-bot/LPasistant-release.apk /mnt/c/qq-bot/release/LPasistant-v1.0-release.apk
echo "== APK 构建完成 =="
