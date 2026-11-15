#!/usr/bin/env bash
set -o pipefail
NDK=$HOME/android/ndk/26.3.11579264
TOOLCHAIN=$NDK/build/cmake/android.toolchain.cmake
SRC=/mnt/c/qq-bot/mobile/llama-jni
OUT=/mnt/c/qq-bot/mobile/app/src/main/jniLibs/arm64-v8a
rm -rf ~/llama-jni-build
cmake -S "$SRC" -B ~/llama-jni-build \
  -DCMAKE_TOOLCHAIN_FILE=$TOOLCHAIN \
  -DANDROID_ABI=arm64-v8a \
  -DANDROID_PLATFORM=android-26 \
  -DCMAKE_BUILD_TYPE=Release \
  -DLLAMA_DIR=$HOME/llama.cpp
cmake --build ~/llama-jni-build --config Release -j$(nproc) --target llama_jni
echo "JNI-BUILD-EXIT=$?"
cp -f ~/llama-jni-build/libllama_jni.so "$OUT/libllama_jni.so"
ls -la "$OUT/libllama_jni.so"
echo "── 未定义 llama 符号数(0=已静态链入)──"
nm -D "$OUT/libllama_jni.so" | grep -c " U llama_" || echo 0
