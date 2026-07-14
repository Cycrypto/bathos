#!/usr/bin/env python3
"""bathos-evals 스켈레톤 하니스 테스트 — 레퍼런스 케이스 e2e 스모크 + 드리프트 가드
+ 빈/부분실패 리포트 상태(Matthias M-7). [Source: story-c3-c4-evals-skeleton-kr.md §5]
"""
import json
import os
import sys
import tempfile
import unittest
from pathlib import Path

SKILL_DIR = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(SKILL_DIR / "scripts"))

import harness  # noqa: E402
import judge as judge_mod  # noqa: E402
import report  # noqa: E402


class TestHarnessCtxLoss(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.root = Path(self.tmp.name)
        self.golden_dir = self.root / "golden"
        self.results_dir = self.root / "results"

        # 후보가 읽을 "스토리파일" 픽스처(프론트매터 포함).
        self.story_path = self.root / "story-fixture-kr.md"
        self.story_path.write_text(
            "---\n"
            "story: X1\n"
            "owner: Stephen\n"
            "priority: Should\n"
            "status: ready-for-dev\n"
            "---\n\n# 본문\n",
            encoding="utf-8",
        )

    def tearDown(self):
        self.tmp.cleanup()

    def _make_ctx_loss_case(self, case_id="E-TEST-1", required_fields=None):
        required_fields = required_fields or ["story", "owner", "priority", "status"]
        case_dir = self.golden_dir / case_id
        data = {
            "eval_id": case_id,
            "task": "ctx-loss",
            "input": {"story_path": "story-fixture-kr.md", "required_fields": required_fields},
            "expected": {
                "story": "X1",
                "owner": "Stephen",
                "priority": "Should",
                "status": "ready-for-dev",
            },
        }
        harness.write_golden_case(case_dir, data)
        return case_dir

    # 골든 로드 + SHA 일치 → 드리프트 없음
    def test_golden_load_no_drift(self):
        case_dir = self._make_ctx_loss_case()
        golden = harness.load_and_verify_golden(case_dir)
        self.assertFalse(golden["_drift"])

    # 골든 1바이트 변조 → 드리프트 경고(AC1)
    def test_golden_drift_detected_on_mutation(self):
        case_dir = self._make_ctx_loss_case()
        case_file = harness.golden_case_path(case_dir)
        text = case_file.read_text(encoding="utf-8")
        mutated = text.replace("Stephen", "Stephan")  # 1개 문자 변경
        case_file.write_text(mutated, encoding="utf-8")

        golden = harness.load_and_verify_golden(case_dir)
        self.assertTrue(golden["_drift"])

        # CLI 레벨: run 커맨드가 exit 2로 드리프트를 신호해야 한다.
        exit_code = harness.main(
            ["run", "--case-dir", str(case_dir), "--root", str(self.root), "--results-dir", str(self.results_dir)]
        )
        self.assertEqual(exit_code, 2)

    # e2e 스모크: golden 로드 → 파일럿(ctx-loss) → judge → JSON 결과 생성(AC2)
    def test_end_to_end_smoke_ctx_loss_full_recovery_wins(self):
        case_dir = self._make_ctx_loss_case()
        result = harness.run_case(case_dir, root=self.root)
        self.assertEqual(result["judge"]["verdict"], "win")
        self.assertFalse(result["golden_drift"])
        out_path = harness.write_result(self.results_dir, result)
        self.assertTrue(out_path.exists())
        reloaded = json.loads(out_path.read_text(encoding="utf-8"))
        self.assertEqual(reloaded["eval_id"], "E-TEST-1")
        self.assertIn("candidate_run", reloaded)
        self.assertIn("judge", reloaded)

    # 부분 손실 시 verdict가 lose/tie로 정직하게 낮아짐(날조 금지)
    def test_partial_context_loss_does_not_win(self):
        case_dir = self.golden_dir / "E-TEST-2"
        data = {
            "eval_id": "E-TEST-2",
            "task": "ctx-loss",
            "input": {
                "story_path": "story-fixture-kr.md",
                "required_fields": ["story", "owner", "priority", "status", "missing_field"],
            },
            "expected": {
                "story": "X1",
                "owner": "Stephen",
                "priority": "Should",
                "status": "ready-for-dev",
                "missing_field": "이 필드는 프론트매터에 없어야 함(의도적 결손 픽스처)",
            },
        }
        harness.write_golden_case(case_dir, data)
        result = harness.run_case(case_dir, root=self.root)
        self.assertIn(result["judge"]["verdict"], ("tie", "lose"))


class TestHarnessGateFp(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.root = Path(self.tmp.name)
        self.golden_dir = self.root / "golden"

    def tearDown(self):
        self.tmp.cleanup()

    # bathos 바이너리 부재 → 스텁 강등(에러 아님, fail-safe) + judge tie
    def test_gate_fp_stub_when_binary_absent(self):
        case_dir = self.golden_dir / "E-TEST-GATE"
        data = {
            "eval_id": "E-TEST-GATE",
            "task": "gate-fp",
            "input": {"agent_team_path": str(self.root)},
            "expected": {"verdict_contains": "PASS"},
        }
        harness.write_golden_case(case_dir, data)

        old_env = os.environ.pop("BATHOS_BIN", None)
        original_resolver = harness._resolve_bathos_bin
        harness._resolve_bathos_bin = lambda: None  # 바이너리 탐지 자체를 결정론적으로 봉쇄
        try:
            result = harness.run_case(case_dir, root=self.root)
        finally:
            harness._resolve_bathos_bin = original_resolver
            if old_env is not None:
                os.environ["BATHOS_BIN"] = old_env

        self.assertTrue(result["candidate_run"]["artifact"].startswith("STUB"))
        self.assertEqual(result["judge"]["verdict"], "tie")


class TestJudgeStub(unittest.TestCase):
    def test_stub_judge_is_pure_and_deterministic(self):
        golden = {"task": "ctx-loss", "expected": {"a": "1", "b": "2"}}
        candidate = {"artifact": json.dumps({"a": "1", "b": "2"})}
        r1 = judge_mod.stub_judge(golden, candidate)
        r2 = judge_mod.stub_judge(golden, candidate)
        self.assertEqual(r1, r2)
        self.assertEqual(r1["verdict"], "win")

    def test_format_judge_prompt_contains_reproducibility_caveat(self):
        prompt = judge_mod.format_judge_prompt({"eval_id": "E-1", "task": "ctx-loss", "expected": {}}, {"artifact": "x"})
        self.assertIn("재현성", prompt)


class TestReport(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.results_dir = Path(self.tmp.name) / "results"

    def tearDown(self):
        self.tmp.cleanup()

    # Matthias M-7: 빈 결과 → "데이터 없음(스켈레톤)" 배너, 표를 0으로 채우지 않음
    def test_empty_results_shows_no_data_banner(self):
        text = report.render([], run_id="test-run")
        self.assertIn("데이터 없음(스켈레톤)", text)
        self.assertIn("## §1", text)
        self.assertIn("## §4", text)

    # N < 3(임계) → (추정) 배지 강제(surface-formats §c-4 규칙1)
    def test_small_n_forces_estimate_badge(self):
        results = [
            {
                "eval_id": "E-1", "golden_sha": "abc", "task": "ctx-loss",
                "candidate_run": {"artifact": "{}", "tokens": None, "wall_ms": 1.2},
                "judge": {"verdict": "win", "reason": "ok", "judged_by": "stub-heuristic-v1"},
                "created": "2026-07-08T00:00:00Z",
            }
        ]
        text = report.render(results, run_id="test-run")
        self.assertIn("(추정) — 표본 부족", text)

    # Matthias M-7: 부분 실패(judge 에러 k건) → "부분 리포트(k건 실패)" 상태 표기
    def test_partial_failure_marks_report_partial(self):
        results = [
            {
                "eval_id": "E-1", "golden_sha": "abc", "task": "gate-fp",
                "candidate_run": {"artifact": "ERROR: boom", "tokens": None, "wall_ms": 1.0},
                "judge": {"verdict": "lose", "reason": "실행 실패", "judged_by": "stub-heuristic-v1"},
                "created": "2026-07-08T00:00:00Z",
            },
            {
                "eval_id": "E-2", "golden_sha": "def", "task": "gate-fp",
                "candidate_run": {"artifact": "PASS ok", "tokens": None, "wall_ms": 2.0},
                "judge": {"verdict": "win", "reason": "ok", "judged_by": "stub-heuristic-v1"},
                "created": "2026-07-08T00:00:00Z",
            },
        ]
        text = report.render(results, run_id="test-run")
        self.assertIn("부분 리포트(1건 실패)", text)
        self.assertIn("[FAIL: 실행 실패]", text)

    # 베이스라인 미실측 → 델타를 날조하지 않고 "—"로 표시
    def test_baseline_not_measured_shown_as_dash_not_zero(self):
        results = [
            {
                "eval_id": "E-1", "golden_sha": "abc", "task": "ctx-loss",
                "candidate_run": {"artifact": "{}", "tokens": None, "wall_ms": 5.0},
                "judge": {"verdict": "win", "reason": "ok", "judged_by": "stub-heuristic-v1"},
                "created": "2026-07-08T00:00:00Z",
            },
            {
                "eval_id": "E-2", "golden_sha": "def", "task": "ctx-loss",
                "candidate_run": {"artifact": "{}", "tokens": None, "wall_ms": 5.0},
                "judge": {"verdict": "win", "reason": "ok", "judged_by": "stub-heuristic-v1"},
                "created": "2026-07-08T00:00:00Z",
            },
            {
                "eval_id": "E-3", "golden_sha": "def", "task": "ctx-loss",
                "candidate_run": {"artifact": "{}", "tokens": None, "wall_ms": 5.0},
                "judge": {"verdict": "win", "reason": "ok", "judged_by": "stub-heuristic-v1"},
                "created": "2026-07-08T00:00:00Z",
            },
        ]
        text = report.render(results, run_id="test-run")
        self.assertNotIn("(추정) — 표본 부족", text)  # N=3 >= 임계
        self.assertIn("미실측", text)


if __name__ == "__main__":
    unittest.main()
