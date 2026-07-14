#!/usr/bin/env bash
# bathos-evals 스켈레톤 하니스 테스트 러너. 외부 의존 없음(python3 stdlib만),
# bathos 바이너리 부재를 정상 스텁 경로로 간주(fail-safe 회귀).
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"
python3 -m unittest test_harness -v
