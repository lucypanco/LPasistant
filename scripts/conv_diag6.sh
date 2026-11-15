#!/usr/bin/env bash
SP=/home/xuzhy78/aiedge-venv/lib/python3.12/site-packages/litert_torch
Q=$SP/generative/examples/qwen
T=$SP/generative/tools
echo "── tools 目录 ──"
ls "$T" 2>/dev/null | head -20
echo "── qwen example 目录 ──"
ls "$Q" | head
echo "── verify_qwen2.py 头部 ──"
sed -n '1,55p' "$Q/verify_qwen2.py"
