#!/usr/bin/env python3
"""Run shmtu-cas (核心CAS库示例或测试)."""

from __future__ import annotations

import subprocess
import sys
from pathlib import Path


SCRIPT_DIR = Path(__file__).resolve().parent
PROJECT_DIR = SCRIPT_DIR.parents[1] / "src-tauri" / "vendor" / "shmtu-cas-rs"
CAS_DIR = PROJECT_DIR / "Core" / "shmtu-cas"


def main() -> int:
    if not CAS_DIR.exists():
        print(f"Error: CAS project not found at {CAS_DIR}", file=sys.stderr)
        return 1

    return subprocess.call(["cargo", "run", "--"] + sys.argv[1:], cwd=CAS_DIR)


if __name__ == "__main__":
    raise SystemExit(main())
