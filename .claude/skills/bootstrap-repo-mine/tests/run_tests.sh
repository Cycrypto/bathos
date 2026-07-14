#!/usr/bin/env bash
# bootstrap-repo-mine 폴백 경로 테스트 러너. git 필요(로컬 임시 저장소 생성), 네트워크 미사용.
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"
python3 -m unittest test_mine -v
