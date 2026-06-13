#!/usr/bin/env python3
"""Tauri 开发服务器启动脚本."""

from __future__ import annotations

import subprocess
import sys
from pathlib import Path


TAURI_ROOT = Path(__file__).resolve().parents[2]


def main() -> int:
    if not (TAURI_ROOT / "node_modules").exists():
        print("[dev] Installing frontend dependencies...", file=sys.stderr)
        subprocess.check_call(["npm", "install"], cwd=TAURI_ROOT)

    print("[dev] Starting Tauri dev server...")
    return subprocess.call(["bun", "run", "tauri", "dev"], cwd=TAURI_ROOT)


if __name__ == "__main__":
    raise SystemExit(main())
