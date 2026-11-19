#!/bin/sh
# One-time setup after clone: pre-commit runs fmt --check and clippy (-D warnings).
set -e
cd "$(dirname "$0")/.."
git config core.hooksPath .githooks
echo "git hooks: .githooks/pre-commit (cargo fmt --check, cargo clippy)"