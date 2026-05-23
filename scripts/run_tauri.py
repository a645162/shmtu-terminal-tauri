#!/usr/bin/env python3
"""Run the main Tauri application (shmtu-terminal-tauri)."""

import subprocess
import sys
from pathlib import Path

script_dir = Path(__file__).parent
tauri_dir = script_dir.parent / "src-tauri"

if not tauri_dir.exists():
    print(f"Error: Tauri project not found at {tauri_dir}", file=sys.stderr)
    sys.exit(1)

sys.exit(subprocess.call(["cargo", "run"] + sys.argv[1:], cwd=tauri_dir))
