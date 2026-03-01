#!/usr/bin/env python3
"""Advisory hook for new modules in core architecture directories."""
import json
import sys


CORE_ARCH_DIRS = [
    "crates/Intently_core/src/ir/",
    "crates/Intently_core/src/diff/",
    "crates/Intently_core/src/policy/",
    "crates/Intently_core/src/planner/",
    "crates/Intently_core/src/evidence/",
]


def main() -> None:
    data = json.loads(sys.stdin.read())
    tool = data.get("tool_name", "")
    if tool != "Write":
        return
    fp = data.get("tool_input", {}).get("file_path", "")
    for arch_dir in CORE_ARCH_DIRS:
        if arch_dir in fp:
            print(
                "New module in core architecture directory. "
                "Consider documenting the design decision in docs/adr/."
            )
            return


if __name__ == "__main__":
    main()
