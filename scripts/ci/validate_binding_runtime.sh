#!/usr/bin/env bash
set -euo pipefail
python3 scripts/ci/validate_binding_runtime.py "$@"
