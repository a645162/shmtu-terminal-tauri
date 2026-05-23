#!/usr/bin/env python3
"""Run shmtu-ocr-cli (验证码OCR命令行测试工具)."""

import subprocess
import sys
from pathlib import Path

script_dir = Path(__file__).parent
project_dir = script_dir.parent / "src-tauri" / "vendor" / "shmtu-cas-rs"
cli_dir = project_dir / "ocr" / "shmtu-ocr-cli"

if not cli_dir.exists():
    print(f"Error: CLI project not found at {cli_dir}", file=sys.stderr)
    sys.exit(1)

sys.exit(subprocess.call(["cargo", "run", "--"] + sys.argv[1:], cwd=cli_dir))
