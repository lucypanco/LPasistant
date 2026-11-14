#!/usr/bin/env python3
"""把 mobile/core 同步到 WSL Ubuntu 并运行 cargo test。

前提:Windows 已装 WSL 发行版(默认 Ubuntu)且其内已装 rustup。
用法:
  python wsl_test.py            # 同步 + cargo test
"""
import subprocess
import sys

DISTRO = "Ubuntu"
SYNC = "mkdir -p ~/qqbot && rm -rf ~/qqbot/core && cp -r /mnt/c/qq-bot/mobile/core ~/qqbot/core"
TEST = (
    "set -o pipefail; cd ~/qqbot/core && cargo test 2>&1 | tail -c 3000 && "
    "cargo build -q 2>&1 | tail -c 500 && "
    "cargo run -q --bin uniffi-bindgen -- generate "
    "--library target/debug/liblpasistant_core.so --language kotlin "
    "--out-dir /mnt/c/qq-bot/mobile/app/src/main/java 2>&1 | tail -c 500 && "
    "echo '== BINDINGS-OK ==' && ls /mnt/c/qq-bot/mobile/app/src/main/java/uniffi/lpasistant_core/"
)


def run(cmd: list) -> int:
    print("$ " + " ".join(cmd), flush=True)
    return subprocess.run(cmd).returncode


def main() -> None:
    if sys.platform == "win32":
        try:
            sys.stdout.reconfigure(encoding="utf-8")
        except Exception:
            pass
    print("── 同步 core 到 WSL ──")
    if run(["wsl.exe", "-d", DISTRO, "--", "bash", "-lc", SYNC]) != 0:
        sys.exit("同步失败:检查 WSL 发行版名称与 /mnt/c 挂载")
    print("── cargo test ──")
    code = run(["wsl.exe", "-d", DISTRO, "--", "bash", "-lc", TEST])
    sys.exit(code)


if __name__ == "__main__":
    main()
