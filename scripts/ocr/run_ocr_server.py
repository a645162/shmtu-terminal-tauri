#!/usr/bin/env python3
"""Run shmtu-ocr-server (验证码OCR HTTP服务器)."""

from __future__ import annotations

import subprocess
import sys
from pathlib import Path


SCRIPT_DIR = Path(__file__).resolve().parent
PROJECT_DIR = SCRIPT_DIR.parents[1] / "src-tauri" / "vendor" / "shmtu-cas-rs"
OCR_DIR = PROJECT_DIR / "ocr" / "shmtu-ocr-server"


def main() -> int:
    if not OCR_DIR.exists():
        print(f"Error: OCR server project not found at {OCR_DIR}", file=sys.stderr)
        return 1

    return subprocess.call(["cargo", "run", "--"] + sys.argv[1:], cwd=OCR_DIR)


if __name__ == "__main__":
    raise SystemExit(main())
