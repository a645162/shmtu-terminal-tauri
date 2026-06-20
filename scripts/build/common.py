#!/usr/bin/env python3
"""Shared helpers for building the Tauri desktop app on each platform."""

from __future__ import annotations

import os
import shutil
import subprocess
import sys
from pathlib import Path


PROJECT_ROOT = Path(__file__).resolve().parents[2]


def _run(command: list[str], env: dict[str, str] | None = None) -> int:
    print(f"[build] $ {' '.join(command)}")
    return subprocess.call(command, cwd=PROJECT_ROOT, env=env)


def _ensure_command(name: str, hint: str | None = None) -> None:
    if shutil.which(name) is None:
        message = f"[build] Missing required command: {name}"
        if hint:
            message += f" ({hint})"
        print(message, file=sys.stderr)
        raise SystemExit(1)


def _warn_linux_packaging_dependencies() -> None:
    if shutil.which("pkg-config") is None:
        return

    result = subprocess.run(
        ["pkg-config", "--modversion", "librsvg-2.0"],
        cwd=PROJECT_ROOT,
        check=False,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        print(
            "[build] Warning: `librsvg-2.0.pc` not found. "
            "AppImage bundling may fail; install `librsvg2-dev`.",
            file=sys.stderr,
        )


def build_for_platform(platform_name: str, bundle_target: str | None = None) -> int:
    _ensure_command("bun")
    _ensure_command("cargo")

    env = os.environ.copy()

    if platform_name == "linux":
        env.setdefault("APPIMAGE_EXTRACT_AND_RUN", "1")
        _warn_linux_packaging_dependencies()

    commands: list[list[str]] = [
        ["bun", "run", "build"],
        ["bun", "run", "tauri", "build"],
    ]

    if bundle_target:
        commands[-1].extend(["--bundles", bundle_target])

    for command in commands:
        code = _run(command, env=env)
        if code != 0:
            return code

    return 0


def main(argv: list[str] | None = None) -> int:
    args = list(sys.argv[1:] if argv is None else argv)
    if not args:
        print("Usage: common.py <linux|macos|windows> [bundle-target]", file=sys.stderr)
        return 1

    platform_name = args[0]
    bundle_target = args[1] if len(args) > 1 else None
    return build_for_platform(platform_name, bundle_target)


if __name__ == "__main__":
    raise SystemExit(main())
