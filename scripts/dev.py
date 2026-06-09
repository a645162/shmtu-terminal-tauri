#!/usr/bin/env python3
"""Tauri 开发服务器启动脚本 (同 dev.py)"""

import os
import sys

os.chdir(os.path.dirname(__file__) + "/..")
sys.exit(os.system("bun run tauri dev"))
