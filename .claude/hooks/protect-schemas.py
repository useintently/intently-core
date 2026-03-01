#!/usr/bin/env python3
"""Block direct edits to JSON Schema files without ADR process."""
import json
import sys
import re


def main() -> None:
    data = json.loads(sys.stdin.read())
    tool = data.get("tool_name", "")
    if tool not in ("Edit", "Write"):
        return
    fp = data.get("tool_input", {}).get("file_path", "")
    if re.search(r"schemas/.*\.schema\.json$", fp):
        print(
            "BLOCKED: Schema files are governed. "
            "Create an ADR in docs/adr/ before modifying schemas."
        )
        sys.exit(2)


if __name__ == "__main__":
    main()
