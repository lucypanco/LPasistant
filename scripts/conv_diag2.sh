#!/usr/bin/env bash
# 诊断 litert_torch(改名后的 ai-edge-torch)与 qwen 转换脚本
SP=/home/xuzhy78/aiedge-venv/lib/python3.12/site-packages
echo "── litert_torch 顶层 ──"
ls "$SP/litert_torch" 2>/dev/null | head -15
echo "── generative ──"
ls "$SP/litert_torch/generative" 2>/dev/null | head -10
echo "── generative/examples ──"
ls "$SP/litert_torch/generative/examples" 2>/dev/null | head -20
echo "── qwen 相关 ──"
find "$SP/litert_torch" -iname "*qwen*" 2>/dev/null | head -10
echo "── 转换脚本 ──"
find "$SP/litert_torch" -name "convert*.py" 2>/dev/null | head -10
echo "── 独立转换工具 ──"
ls "$SP/bin" 2>/dev/null | grep -iE "litert|convert|model" | head
find "$SP" -maxdepth 2 -iname "*litertlm*" -o -maxdepth 2 -iname "*genai*" 2>/dev/null | head -5
