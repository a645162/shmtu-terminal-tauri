#!/usr/bin/env python3
"""Run shmtu-ocr-cli (验证码OCR命令行测试工具)."""

from __future__ import annotations

import subprocess
import sys
from pathlib import Path


SCRIPT_DIR = Path(__file__).resolve().parent
PROJECT_DIR = SCRIPT_DIR.parents[1] / "src-tauri" / "vendor" / "shmtu-cas-rs"
CLI_DIR = PROJECT_DIR / "ocr" / "shmtu-ocr-cli"


def main() -> int:
    if not CLI_DIR.exists():
        print(f"Error: CLI project not found at {CLI_DIR}", file=sys.stderr)
        return 1

    return subprocess.call(["cargo", "run", "--"] + sys.argv[1:], cwd=CLI_DIR)


if __name__ == "__main__":
    raise SystemExit(main())
