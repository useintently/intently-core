#!/usr/bin/env python3
"""Force full reads (no partial offset/limit) for critical configuration files."""
import json
import sys
import re


CRITICAL_PATTERNS = [
    r"\.claude/agents/.*\.md$",
    r"\.claude/rules/.*\.md$",
    r"(^|/)Cargo\.toml$",
]


def main() -> None:
    data = json.loads(sys.stdin.read())
    tool = data.get("tool_name", "")
    if tool != "Read":
        return

    tool_input = data.get("tool_input", {})
    fp = tool_input.get("file_path", "")

    has_offset = "offset" in tool_input and tool_input["offset"] is not None
    has_limit = "limit" in tool_input and tool_input["limit"] is not None

    if not (has_offset or has_limit):
        return

    for pattern in CRITICAL_PATTERNS:
        if re.search(pattern, fp):
            print(
                "BLOCKED: Critical files must be read in full. "
                "Remove offset/limit parameters. "
                f"File: {fp}"
            )
            sys.exit(2)


if __name__ == "__main__":
    main()
