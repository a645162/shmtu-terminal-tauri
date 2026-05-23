#!/usr/bin/env python3
"""Run shmtu-ocr-gui (验证码OCR图形界面，egui)."""

import subprocess
import sys
from pathlib import Path

script_dir = Path(__file__).parent
project_dir = script_dir.parent / "src-tauri" / "vendor" / "shmtu-cas-rs"
gui_dir = project_dir / "ocr" / "shmtu-ocr-gui"

if not gui_dir.exists():
    print(f"Error: GUI project not found at {gui_dir}", file=sys.stderr)
    sys.exit(1)

sys.exit(subprocess.call(["cargo", "run"] + sys.argv[1:], cwd=gui_dir))
