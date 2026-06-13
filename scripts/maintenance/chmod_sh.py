#!/usr/bin/env python3
"""遍历 scripts 目录及子目录下的所有 .sh 脚本并执行 chmod +x."""

from __future__ import annotations

import os
import stat
import sys
from pathlib import Path


def make_sh_executable(root: str | Path | None = None) -> int:
    if root is None:
        root = Path(__file__).resolve().parents[1]
    else:
        root = Path(root).resolve()

    if not root.is_dir():
        print(f"[chmod_sh] {root} 不是目录", file=sys.stderr)
        return 1

    sh_files: list[Path] = []
    for path in root.rglob("*.sh"):
        if path.is_file():
            sh_files.append(path)

    if not sh_files:
        print(f"[chmod_sh] 在 {root} 下没有找到 *.sh 文件")
        return 0

    for path in sorted(sh_files):
        try:
            current = path.stat().st_mode
            new_mode = current | stat.S_IXUSR | stat.S_IXGRP | stat.S_IXOTH
            os.chmod(path, new_mode)
            print(f"  +x {path.relative_to(root)}")
        except OSError:
            print(f"  [警告] 无法设置执行权限: {path}", file=sys.stderr)

    print(f"[chmod_sh] 已处理 {len(sh_files)} 个脚本")
    return 0


if __name__ == "__main__":
    sys.exit(make_sh_executable())
