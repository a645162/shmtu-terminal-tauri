#!/usr/bin/env python3

from __future__ import annotations

import shutil
import subprocess
import sys
from pathlib import Path


def find_manifest_dirs(root_dir: Path) -> list[Path]:
    return sorted(manifest.parent for manifest in root_dir.rglob("Cargo.toml"))


def main() -> int:
    root_dir = Path(sys.argv[1]).resolve() if len(sys.argv) > 1 else Path.cwd()

    if shutil.which("cargo") is None:
        print("cargo is not installed or not in PATH", file=sys.stderr)
        return 1

    manifest_dirs = find_manifest_dirs(root_dir)
    if not manifest_dirs:
        print(f"No Cargo.toml found under {root_dir}", flush=True)
        return 0

    exit_code = 0

    for manifest_dir in manifest_dirs:
        print(f"==> cargo update: {manifest_dir}", flush=True)
        result = subprocess.run(["cargo", "update"], cwd=manifest_dir, check=False)
        if result.returncode != 0:
            print(f"Failed: {manifest_dir}", file=sys.stderr, flush=True)
            exit_code = 1

    return exit_code


if __name__ == "__main__":
    raise SystemExit(main())
