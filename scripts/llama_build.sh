#!/usr/bin/env bash
# llama.cpp → Android arm64 静态库(供 JNI 桥链接)
set -o pipefail
cd ~/llama.cpp
cmake -S . -B build-android \
  -DCMAKE_TOOLCHAIN_FILE=$HOME/android/ndk/26.3.11579264/build/cmake/android.toolchain.cmake \
  -DANDROID_ABI=arm64-v8a \
  -DANDROID_PLATFORM=android-26 \
  -DCMAKE_BUILD_TYPE=Release \
  -DGGML_OPENMP=OFF \
  -DGGML_LLAMAFILE=OFF \
  -DLLAMA_CURL=OFF \
  -DLLAMA_BUILD_TESTS=OFF \
  -DLLAMA_BUILD_EXAMPLES=OFF \
  -DLLAMA_BUILD_SERVER=OFF \
  -DGGML_BUILD_TESTS=OFF \
  -DBUILD_SHARED_LIBS=OFF
cmake --build build-android --config Release -j$(nproc) --target llama
echo "BUILD-EXIT=$?"
find build-android -name "*.a" | head -8
