#!/usr/bin/env python3
"""Run shmtu-cas-cli (上海海事大学CAS登录与账单查询命令行工具)."""

import subprocess
import sys
from pathlib import Path

script_dir = Path(__file__).parent
project_dir = script_dir.parent / "src-tauri" / "vendor" / "shmtu-cas-rs"
cli_dir = project_dir / "Core" / "shmtu-cas-cli"

if not cli_dir.exists():
    print(f"Error: CLI project not found at {cli_dir}", file=sys.stderr)
    sys.exit(1)

sys.exit(subprocess.call(["cargo", "run", "--"] + sys.argv[1:], cwd=cli_dir))
