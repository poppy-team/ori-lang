#!/usr/bin/env bash
set -euo pipefail

repo=$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)

# The bootstrap gate builds Stage 2 with Stage 1 and Stage 3 with Stage 2.
# Stop here on failure; a Stage 0-only example suite cannot certify self-hosting.
"$repo/tools/qa/test_bootstrap_stages.sh"

echo 'Bootstrap verified. Full stage-built conformance corpus still requires a separate gate.'
