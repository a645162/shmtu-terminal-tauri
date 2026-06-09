#!/usr/bin/env python3
"""Run the main Tauri application via bun tauri dev (Vite + Tauri dev server)."""

import subprocess
import sys
from pathlib import Path

tauri_root = Path(__file__).resolve().parent.parent

# 安装前端依赖 (如果需要)
if not (tauri_root / "node_modules").exists():
    print("[run_tauri] Installing npm dependencies...", file=sys.stderr)
    subprocess.check_call(["npm", "install"], cwd=tauri_root)

print("[run_tauri] Starting Tauri dev server...")
sys.exit(subprocess.call(["bun", "run", "tauri", "dev"], cwd=tauri_root))
