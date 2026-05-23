#!/usr/bin/env python3
"""Run shmtu-ocr-server (验证码OCR HTTP服务器)."""

import subprocess
import sys
from pathlib import Path

script_dir = Path(__file__).parent
project_dir = script_dir.parent / "src-tauri" / "vendor" / "shmtu-cas-rs"
ocr_dir = project_dir / "ocr" / "shmtu-ocr-server"  # 假设的目录名

if not ocr_dir.exists():
    print(f"Error: OCR server project not found at {ocr_dir}", file=sys.stderr)
    sys.exit(1)

sys.exit(subprocess.call(["cargo", "run", "--"] + sys.argv[1:], cwd=ocr_dir))
