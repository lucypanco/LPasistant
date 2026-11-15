#!/usr/bin/env bash
# 诊断 ai_edge_torch 0.7.2 包结构,找 qwen2 转换脚本
SP=/home/xuzhy78/aiedge-venv/lib/python3.12/site-packages/ai_edge_torch
echo "── 包顶层 ──"
ls "$SP" | head -20
echo "── generative 目录 ──"
ls "$SP/generative" 2>/dev/null | head -15
echo "── generative/examples ──"
ls "$SP/generative/examples" 2>/dev/null | head -25
echo "── qwen 相关文件 ──"
find "$SP" -iname "*qwen*" 2>/dev/null | head -10
echo "── convert 类脚本 ──"
find "$SP" -name "convert*.py" 2>/dev/null | head -15
