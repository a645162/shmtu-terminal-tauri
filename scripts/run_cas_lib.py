#!/usr/bin/env python3
"""Run shmtu-cas (核心CAS库示例或测试)."""

import subprocess
import sys
from pathlib import Path

script_dir = Path(__file__).parent
project_dir = script_dir.parent / "src-tauri" / "vendor" / "shmtu-cas-rs"
cas_dir = project_dir / "Core" / "shmtu-cas"

if not cas_dir.exists():
    print(f"Error: CAS project not found at {cas_dir}", file=sys.stderr)
    sys.exit(1)

sys.exit(subprocess.call(["cargo", "run", "--"] + sys.argv[1:], cwd=cas_dir))
