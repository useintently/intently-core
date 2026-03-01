#!/usr/bin/env python3
"""Block direct edits to generated and build artifact files."""
import json
import sys
import re


BLOCKED_PATTERNS = [
    r"/target/",
    r"/dist/",
    r"/node_modules/",
    r"_bindings\.ts$",
    r"/bindings\.ts$",
    r"\.\w+\.generated\.\w+$",
]


def main() -> None:
    data = json.loads(sys.stdin.read())
    tool = data.get("tool_name", "")
    if tool not in ("Edit", "Write"):
        return
    fp = data.get("tool_input", {}).get("file_path", "")
    for pattern in BLOCKED_PATTERNS:
        if re.search(pattern, fp):
            print(
                "BLOCKED: Generated files must not be edited manually. "
                f"Matched pattern: {pattern}"
            )
            sys.exit(2)


if __name__ == "__main__":
    main()
