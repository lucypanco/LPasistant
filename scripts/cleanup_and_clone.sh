#!/usr/bin/env bash
# 清理方案A转换废料 + 检查 cmake/NDK + 克隆 llama.cpp
set -o pipefail
echo "── 清理转换废料 ──"
rm -rf ~/aiedge-venv ~/mpenv ~/aiedge-src ~/qw25 ~/qw25-tokenizer.model ~/qw25-tokenizer.model.spm ~/aiedge-venv 2>/dev/null
rm -f /swapfile-aiedge 2>/dev/null
rm -rf ~/.cache/pip 2>/dev/null
echo "WSL 清理完成"
df -h / | tail -1

echo "── PC 侧转换产物清理 ──"
rm -rf /mnt/c/qq-bot/release/models/hf-qwen25-05b \
       /mnt/c/qq-bot/release/models/qwen25-05b_q8_ekv1408.tflite \
       /mnt/c/qq-bot/release/models/qwen25-05b-bundled.task \
       /mnt/c/qq-bot/release/models/qwen25-05b.task \
       /mnt/c/qq-bot/release/models/ref_colab.json \
       /mnt/c/qq-bot/release/models/aiedge-v0.6.0.tar.gz 2>/dev/null
ls /mnt/c/qq-bot/release/models/ 2>/dev/null
echo "── cmake / NDK 检查 ──"
which cmake && cmake --version | head -1
ls ~/android/ndk/ 2>/dev/null
echo "── 克隆 llama.cpp(浅克隆,记录版本)──"
rm -rf ~/llama.cpp
git clone --depth 1 https://github.com/ggml-org/llama.cpp ~/llama.cpp 2>&1 | tail -2
cd ~/llama.cpp && git rev-parse --short HEAD
