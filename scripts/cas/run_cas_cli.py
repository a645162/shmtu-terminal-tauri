#!/usr/bin/env python3
"""Run shmtu-cas-cli (上海海事大学CAS登录与账单查询命令行工具)."""

from __future__ import annotations

import subprocess
import sys
from pathlib import Path


SCRIPT_DIR = Path(__file__).resolve().parent
PROJECT_DIR = SCRIPT_DIR.parents[1] / "src-tauri" / "vendor" / "shmtu-cas-rs"
CLI_DIR = PROJECT_DIR / "Core" / "shmtu-cas-cli"


def main() -> int:
    if not CLI_DIR.exists():
        print(f"Error: CLI project not found at {CLI_DIR}", file=sys.stderr)
        return 1

    return subprocess.call(["cargo", "run", "--"] + sys.argv[1:], cwd=CLI_DIR)


if __name__ == "__main__":
    raise SystemExit(main())
