#!/usr/bin/env bash
# WSL 复现:one-shot batch > n_batch 时 GGML_ASSERT(n_tokens_all <= cparams.n_batch)
set -u
cd ~/llama.cpp
python3 - <<'EOF' > /tmp/long_prompt.txt
text = ("你是个人助手路由器。今天是2026-11-18(周三) 23:15。对照表:今天=0天,明天=1天,后天=2天,"
        "大后天=3天;每天 recur=daily;每周X recur=weekly:X;每月X号 recur=monthly:X。判断这句话用哪个功能。") * 6
print(text)
EOF
P=$(cat /tmp/long_prompt.txt)
~/llama.cpp/build/bin/llama-cli -m /mnt/c/qq-bot/release/models/model.gguf \
  -c 2048 -b 256 -ub 256 -t 4 --no-warmup -n 8 --single-turn -p "$P" > /tmp/repro.log 2>&1
echo "CLI-EXIT=$?"
grep -E "GGML_ASSERT|Assertion|abort|prompt tokens|error" /tmp/repro.log | head -6
