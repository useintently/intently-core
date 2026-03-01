#!/usr/bin/env bash
# Validates Definition of Done after task completion
echo "Task completed. Verify:"
echo "  - All tests pass (cargo test, vitest)"
echo "  - No clippy warnings (cargo clippy)"
echo "  - Code formatted (cargo fmt, prettier)"
echo "  - CHANGELOG.md updated if user-visible change"
echo "  - Schemas valid if artifacts changed"
