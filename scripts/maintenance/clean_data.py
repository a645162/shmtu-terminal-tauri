#!/usr/bin/env python3
"""清空开发数据脚本."""

from __future__ import annotations

import shutil
from pathlib import Path


APP_DATA_DIR = Path.home() / ".local/share/cn.edu.shmtu.terminal.tauri"
LEGACY_DATA_DIR = Path(__file__).resolve().parents[2] / "src-tauri" / "Data"

targets = [path for path in [APP_DATA_DIR, LEGACY_DATA_DIR] if path.exists()]

if not targets:
    print("没有需要清空的数据目录")
    raise SystemExit(0)

print("即将清空：")
for path in targets:
    print(f"  - {path}")

if input("确认？(y/N) ").strip().lower() != "y":
    print("已取消")
    raise SystemExit(0)

for path in targets:
    shutil.rmtree(path)
    print(f"已清空 {path}")

print("完成")
