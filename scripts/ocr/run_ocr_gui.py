#!/usr/bin/env python3
"""Run shmtu-ocr-gui (验证码OCR图形界面，egui)."""

from __future__ import annotations

import subprocess
import sys
from pathlib import Path


SCRIPT_DIR = Path(__file__).resolve().parent
PROJECT_DIR = SCRIPT_DIR.parents[1] / "src-tauri" / "vendor" / "shmtu-cas-rs"
GUI_DIR = PROJECT_DIR / "ocr" / "shmtu-ocr-gui"


def main() -> int:
    if not GUI_DIR.exists():
        print(f"Error: GUI project not found at {GUI_DIR}", file=sys.stderr)
        return 1

    return subprocess.call(["cargo", "run"] + sys.argv[1:], cwd=GUI_DIR)


if __name__ == "__main__":
    raise SystemExit(main())
