#!/usr/bin/env bash
# Validates Definition of Done after task completion
echo "Task completed. Verify:"
echo "  - All tests pass (cargo test)"
echo "  - No clippy warnings (cargo clippy)"
echo "  - Code formatted (cargo fmt)"
echo "  - CHANGELOG.md updated if user-visible change"
