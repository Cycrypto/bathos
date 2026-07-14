#!/usr/bin/env python3
# =============================================================================
# BATHOS Dynamis — dist/tests/test_mcp_server.py
# Story B6 §5 테스트 접근 구현.
#
#   1) 스냅샷 테스트(핵심, AC1): load_instruction_text()가 canonical 파일과
#      바이트 동일한지 확인(포크 방지). mcp 패키지 설치 여부와 무관하게
#      실행 가능(순수 로직만 임포트).
#   2) 바이너리 부재 시 정직한 에러(AC2): get_wave_state()가 존재하지 않는
#      바이너리에 대해 날조 없이 에러 dict를 반환하는지 확인.
#   3) 알 수 없는 kind/name에 대한 명시적 예외.
#
# 실행: python3 dist/tests/test_mcp_server.py
# =============================================================================
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent / "mcp"))

import server  # noqa: E402  (dist/mcp/server.py — 순수 로직만 사용, mcp 패키지 불요)


class TestCanonicalSnapshot(unittest.TestCase):
    """AC1: tool이 반환할 텍스트가 canonical 소스와 바이트 동일해야 한다."""

    def test_role_instruction_byte_identical(self):
        canon = server.resolve_canon_root()
        path = canon / "agents" / "_base" / "09-andrew-frontend-engineer.md"
        if not path.is_file():
            self.skipTest(f"고정 픽스처 없음(환경 차이): {path}")
        expected = path.read_text(encoding="utf-8")
        actual = server.load_instruction_text("role", "09-andrew-frontend-engineer")
        self.assertEqual(expected, actual, "canonical 소스와 바이트 단위로 달라짐(포크 의심)")

    def test_unknown_kind_raises(self):
        with self.assertRaises(ValueError):
            server.load_instruction_text("unknown-kind", "x")

    def test_unknown_name_raises_with_available_list(self):
        with self.assertRaises(FileNotFoundError) as ctx:
            server.load_instruction_text("role", "this-role-does-not-exist")
        self.assertIn("사용 가능", str(ctx.exception))

    def test_list_available_roles_nonempty(self):
        roles = server.list_available("role")
        self.assertGreater(len(roles), 0, "역할 목록이 비어있음 — canonical 경로 해석 실패 의심")


class TestWaveStateHonesty(unittest.TestCase):
    """AC2: 바이너리 부재 시 날조된 상태를 반환하지 않고 정직한 에러를 낸다."""

    def test_missing_binary_returns_explicit_error(self):
        result = server.get_wave_state(bathos_bin="bathos-binary-that-does-not-exist")
        self.assertIn("error", result)
        self.assertEqual(result["error"], "bathos_binary_not_found")
        self.assertIn("message", result)


if __name__ == "__main__":
    unittest.main()
