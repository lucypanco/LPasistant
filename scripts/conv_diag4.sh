#!/usr/bin/env bash
P=/home/xuzhy78/aiedge-venv/lib/python3.12/site-packages/litert_torch/generative/utilities
echo "── converter.py 的 flag 定义 ──"
grep -n "DEFINE\|def define_conversion_flags" "$P/converter.py" | head -30
echo ""
echo "── build_and_convert_to_tflite_from_flags 主体 ──"
sed -n "/def build_and_convert_to_tflite_from_flags/,/^def /p" "$P/converter.py" | head -60
