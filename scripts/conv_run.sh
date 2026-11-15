#!/usr/bin/env bash
# Qwen2.5-0.5B-Instruct → tflite (dynamic_int8) 转换
cd /mnt/c/qq-bot/release/models
~/aiedge-venv/bin/python -m litert_torch.generative.examples.qwen.convert_to_tflite \
  --model_size=0.5b \
  --checkpoint_path=/mnt/c/qq-bot/release/models/hf-qwen25-05b \
  --output_path=/mnt/c/qq-bot/release/models/ \
  --output_name_prefix=qwen25-05b \
  --quantize=dynamic_int8 \
  --kv_cache_max_len=1408 \
  --prefill_seq_lens=64 \
  --prefill_seq_lens=256 \
  --prefill_seq_lens=1024 \
  2>&1
echo "CONVERT-EXIT=$?"
ls -la /mnt/c/qq-bot/release/models/ | grep -v hf-qwen
