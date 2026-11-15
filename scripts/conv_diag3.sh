#!/usr/bin/env bash
P=/home/xuzhy78/aiedge-venv/lib/python3.12/site-packages/litert_torch/generative/examples/qwen
echo "── convert_to_tflite.py 头部与参数 ──"
sed -n '1,80p' "$P/convert_to_tflite.py"
echo ""
echo "── flags/main ──"
grep -n "flags\|DEFINE\|argparse\|__main__\|def main\|tokenizer" "$P/convert_to_tflite.py" | head -20
