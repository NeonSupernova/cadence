#!/bin/zsh
set -e
cd "${0:A:h}"
echo 'Building Cadence for macOS…'
bun install
bun run launch
