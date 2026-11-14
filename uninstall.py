#!/usr/bin/env python3
"""LPasistant —— 卸载/清理脚本

用法:
  python uninstall.py             # 仅清理本机构建产物(cargo-ndk产物/生成绑定/build/target)
  python uninstall.py --adb       # 额外:卸载手机上的 App(需 adb 已连接授权)
  python uninstall.py --data      # 额外:删除手机端模型目录(/sdcard/Android/data/<包名>)
  python uninstall.py --adb --data

说明:
- 源代码与文档永远保留;清理后重新构建见 mobile/BUILD.md
- 卸载 App 会同时清掉其内部存储(SQLite 提醒库);--data 再清外部模型文件
"""
import argparse
import shutil
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent
MOBILE = ROOT / "mobile"
PKG = "com.lenovo.lpasistant"

# 可再生成的本机构建产物(删了不影响源码)
BUILD_PATHS = [
    MOBILE / "app" / "build",
    MOBILE / "app" / ".cxx",
    MOBILE / "build",
    MOBILE / ".gradle",
    MOBILE / "core" / "target",
    MOBILE / "app" / "src" / "main" / "jniLibs",        # cargo-ndk 产物
    MOBILE / "app" / "src" / "main" / "java" / "uniffi",  # uniffi 生成绑定
]


def run(cmd: list) -> bool:
    print("$ " + " ".join(cmd))
    try:
        r = subprocess.run(cmd, capture_output=True, text=True, timeout=120)
        out, err = (r.stdout or "").strip(), (r.stderr or "").strip()
        if out:
            print(out)
        if err:
            print(err)
        return r.returncode == 0
    except FileNotFoundError:
        print("  adb 不可用(未安装或不在 PATH),跳过此步")
        return False
    except subprocess.TimeoutExpired:
        print("  超时,跳过此步")
        return False


def main() -> None:
    if sys.platform == "win32":
        try:
            sys.stdout.reconfigure(encoding="utf-8")
        except Exception:
            pass

    ap = argparse.ArgumentParser(description="LPasistant卸载/清理")
    ap.add_argument("--adb", action="store_true", help="卸载手机上的 App(adb uninstall)")
    ap.add_argument("--data", action="store_true",
                    help="删除手机端模型目录 /sdcard/Android/data/<包名>(含 model.gguf)")
    args = ap.parse_args()

    if args.adb or args.data:
        print("── 手机端 ──")
        if args.adb:
            run(["adb", "uninstall", PKG])
        if args.data or args.adb:
            run(["adb", "shell", "rm", "-rf", f"/sdcard/Android/data/{PKG}"])

    print("── 本机构建产物 ──")
    removed = 0
    for p in BUILD_PATHS:
        if p.exists():
            shutil.rmtree(p, ignore_errors=True)
            print("已删除", p.relative_to(ROOT))
            removed += 1
    if removed == 0:
        print("(无构建产物可清理)")

    print("\n完成。源代码与文档保留;重新构建见 mobile/BUILD.md")


if __name__ == "__main__":
    main()
