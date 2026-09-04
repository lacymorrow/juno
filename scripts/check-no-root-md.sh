#!/usr/bin/env bash
set -euo pipefail

ROOT_MD=$(find . -maxdepth 1 -type f -name "*.md" \! -name "README.md" \! -name "CLAUDE.md" \! -name "LLMs.txt")

if [[ -n "${ROOT_MD}" ]]; then
  echo "Error: Root-level Markdown files detected (except README.md, CLAUDE.md, LLMs.txt):"
  echo "${ROOT_MD}"
  echo "All docs must live under docs/."
  exit 1
fi

echo "OK: No root-level Markdown files detected."


