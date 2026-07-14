#!/usr/bin/env bash
# review-style-continual 결정성 테스트 러너 (외부 의존 없음, python3 stdlib만).
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"
python3 -m unittest test_aggregate -v
