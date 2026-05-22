#!/usr/bin/env python3
"""清空开发数据脚本"""

import shutil
from pathlib import Path

APP_DATA_DIR = Path.home() / ".local/share/cn.edu.shmtu.terminal.tauri"
LEGACY_DATA_DIR = Path(__file__).resolve().parent.parent / "src-tauri/Data"

targets = [d for d in [APP_DATA_DIR, LEGACY_DATA_DIR] if d.exists()]

if not targets:
    print("没有需要清空的数据目录")
    raise SystemExit(0)

print("即将清空：")
for d in targets:
    print(f"  - {d}")

if input("确认？(y/N) ").strip().lower() != "y":
    print("已取消")
    raise SystemExit(0)

for d in targets:
    shutil.rmtree(d)
    print(f"已清空 {d}")

print("完成")
