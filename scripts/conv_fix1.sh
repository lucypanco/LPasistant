#!/usr/bin/env bash
~/aiedge-venv/bin/pip show ai-edge-torch | grep -iE "requires|version"
echo "── 装 torchao 匹配版本 ──"
~/aiedge-venv/bin/pip install -q "torchao==0.16.0" -i https://mirrors.aliyun.com/pypi/simple/ 2>&1 | tail -1
~/aiedge-venv/bin/python -c "import torchao; print('torchao', torchao.__version__)" 2>&1 | tail -1
~/aiedge-venv/bin/python -c "from litert_torch.generative.examples.qwen import convert_to_tflite; print('converter import OK')" 2>&1 | tail -3
