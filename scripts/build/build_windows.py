#!/usr/bin/env python3

from __future__ import annotations

from common import build_for_platform


if __name__ == "__main__":
    raise SystemExit(build_for_platform("windows", "nsis,msi"))
