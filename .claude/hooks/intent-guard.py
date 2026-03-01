#!/usr/bin/env python3
"""Advisory hook for intent.yaml modifications."""
import json
import sys
import re


def main() -> None:
    data = json.loads(sys.stdin.read())
    tool = data.get("tool_name", "")
    if tool not in ("Edit", "Write"):
        return
    fp = data.get("tool_input", {}).get("file_path", "")
    if re.search(r"(^|/)intent\.yaml$", fp):
        print(
            "Intent file modification detected. "
            "Ensure changes maintain valid intent.yaml schema structure."
        )


if __name__ == "__main__":
    main()
