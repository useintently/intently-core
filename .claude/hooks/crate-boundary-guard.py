#!/usr/bin/env python3
"""Enforce crate dependency boundaries in the Rust workspace.

Rules:
  - Intently_core must NOT import from Intently_cli
  - Intently_cli can import from Intently_core
  - apps/desktop/src-tauri/ can import from both crates
"""
import json
import sys
import re


def main() -> None:
    data = json.loads(sys.stdin.read())
    tool = data.get("tool_name", "")
    if tool not in ("Edit", "Write"):
        return

    tool_input = data.get("tool_input", {})
    fp = tool_input.get("file_path", "")

    if "crates/Intently_core/" not in fp:
        return

    content = ""
    if tool == "Edit":
        content = tool_input.get("new_string", "")
    elif tool == "Write":
        content = tool_input.get("content", "")

    if re.search(r"use\s+Intently_cli|Intently_cli::", content):
        print(
            "BLOCKED: Crate boundary violation. "
            "Intently_core must not depend on Intently_cli. "
            "The dependency direction is: Intently_cli -> Intently_core, not the reverse."
        )
        sys.exit(2)


if __name__ == "__main__":
    main()
