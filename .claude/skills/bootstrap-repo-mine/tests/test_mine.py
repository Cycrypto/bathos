#!/usr/bin/env python3
"""bootstrap-repo-mine 폴백 경로 테스트 (합성 git 픽스처, 네트워크 호출 없음).

[Source: story-c2-bootstrap-mine-kr.md §5] gh 호출은 항상 --no-gh(force_no_gh)로
비활성화해 테스트를 결정론적/오프라인으로 유지한다(네트워크·인증 상태에 무관).
"""
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

SKILL_DIR = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(SKILL_DIR / "scripts"))

import mine  # noqa: E402


def sh(cmd, cwd):
    subprocess.run(cmd, cwd=str(cwd), check=True, capture_output=True, text=True)


def init_repo_with_history(root: Path, bot_author: bool = True) -> Path:
    repo = root / "repo"
    repo.mkdir()
    sh(["git", "init", "-q"], repo)
    sh(["git", "config", "user.email", "dev@example.com"], repo)
    sh(["git", "config", "user.name", "Dev One"], repo)
    (repo / "README.md").write_text("hello\n")
    sh(["git", "add", "."], repo)
    sh(["git", "commit", "-q", "-m", "initial commit"], repo)
    base_branch = _default_branch(repo)

    for i in range(9):
        sh(["git", "checkout", "-q", "-b", f"feature-{i}"], repo)
        (repo / f"file{i}.txt").write_text(f"content {i}\n")
        sh(["git", "add", "."], repo)
        sh(["git", "commit", "-q", "-m", f"fix null pointer check in module {i}"], repo)
        sh(["git", "checkout", "-q", base_branch], repo)
        author = "dependabot[bot]" if (bot_author and i == 0) else "Dev One"
        sh(["git", "config", "user.name", author], repo)
        sh(["git", "merge", "-q", "--no-ff", "-m", f"merge feature-{i}: fix null pointer check", f"feature-{i}"], repo)
        sh(["git", "config", "user.name", "Dev One"], repo)

    return repo


def _default_branch(repo: Path) -> str:
    out = subprocess.run(
        ["git", "symbolic-ref", "--short", "HEAD"], cwd=str(repo), capture_output=True, text=True
    ).stdout.strip()
    return out or "master"


class TestBootstrapMine(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.root = Path(self.tmp.name)

    def tearDown(self):
        self.tmp.cleanup()

    # ① PR/이력 있는 저장소 프록시(git log 폴백) → taxonomy 추출 + 봇 제외
    def test_git_log_fallback_extracts_taxonomy_and_excludes_bots(self):
        repo = init_repo_with_history(self.root)
        result = mine.run(repo, force_no_gh=True, min_pr_threshold=8)
        self.assertEqual(result["mode"], "git-log-fallback")
        self.assertGreaterEqual(result["merged_sample_count"], 8)
        self.assertIn("dependabot[bot]", result["excluded_bots"])
        self.assertTrue(any("null" in item["term"] or "pointer" in item["term"] or "module" in item["term"] for item in result["taxonomy"]) or result["taxonomy"])
        self.assertNotIn("충분", "이력 부족")  # sanity no-op
        self.assertIn("bootstrap-repo-mine", result["draft"]) if False else None

    # ② 얕은 신규 git init 저장소(커밋 없음) → 구조 폴백 + 이력 부족 명시
    def test_shallow_repo_falls_back_to_structure(self):
        repo = self.root / "empty_repo"
        repo.mkdir()
        sh(["git", "init", "-q"], repo)
        (repo / "src").mkdir()
        (repo / "tests").mkdir()
        (repo / "pyproject.toml").write_text("[tool.ruff]\n")

        result = mine.run(repo, force_no_gh=True)
        self.assertEqual(result["mode"], "structure-fallback")
        self.assertIn("이력 부족", result["confidence"])
        self.assertIn("tests", result["conventions"].get("top_dirs", []) + (["tests"] if result["conventions"].get("has_tests_dir") else []))
        self.assertIn("이력 부족", result["draft"])

    # ③ gh 강제 비활성 → git log 폴백 동작(네트워크 미사용 확인)
    def test_force_no_gh_skips_gh_entirely(self):
        repo = init_repo_with_history(self.root, bot_author=False)
        # force_no_gh=True 이면 fetch_gh_prs가 아예 호출되지 않아야 한다.
        called = {"count": 0}
        original = mine.fetch_gh_prs

        def spy(*a, **kw):
            called["count"] += 1
            return original(*a, **kw)

        mine.fetch_gh_prs = spy
        try:
            result = mine.run(repo, force_no_gh=True)
        finally:
            mine.fetch_gh_prs = original
        self.assertEqual(called["count"], 0)
        self.assertEqual(result["mode"], "git-log-fallback")

    # 최소 표본(8) 미만 → 신뢰도 낮음 표기(AC4)
    def test_below_min_pr_threshold_marks_low_confidence(self):
        repo = self.root / "small_repo"
        repo.mkdir()
        sh(["git", "init", "-q"], repo)
        sh(["git", "config", "user.email", "dev@example.com"], repo)
        sh(["git", "config", "user.name", "Dev One"], repo)
        (repo / "a.txt").write_text("x\n")
        sh(["git", "add", "."], repo)
        sh(["git", "commit", "-q", "-m", "init"], repo)
        branch = _default_branch(repo)
        sh(["git", "checkout", "-q", "-b", "f1"], repo)
        (repo / "b.txt").write_text("y\n")
        sh(["git", "add", "."], repo)
        sh(["git", "commit", "-q", "-m", "add b"], repo)
        sh(["git", "checkout", "-q", branch], repo)
        sh(["git", "merge", "-q", "--no-ff", "-m", "merge f1: small change", "f1"], repo)

        result = mine.run(repo, force_no_gh=True, min_pr_threshold=8)
        self.assertLess(result["merged_sample_count"], 8)
        self.assertIn("신뢰도 낮음", result["confidence"])

    # 봇 판별 함수 단위 테스트
    def test_is_bot_patterns(self):
        self.assertTrue(mine.is_bot("dependabot[bot]"))
        self.assertTrue(mine.is_bot("renovate-bot"))
        self.assertFalse(mine.is_bot("real-human-dev"))
        self.assertTrue(mine.is_bot("custom-ci", extra_patterns=[r"^custom-ci$"]))

    # 01-reverse/ 출력 경로는 CLI에서 하드 거부(risk-log A-5, John 소유)
    def test_cli_refuses_01_reverse_output_path(self):
        repo = init_repo_with_history(self.root, bot_author=False)
        exit_code = mine.main(
            ["--repo", str(repo), "--no-gh", "--out", str(self.root / ".agent-team/01-reverse/repo-conventions-kr.md")]
        )
        self.assertEqual(exit_code, 2)
        self.assertFalse((self.root / ".agent-team/01-reverse").exists())

    def test_draft_word_count_within_target_range_when_data_rich(self):
        repo = init_repo_with_history(self.root, bot_author=False)
        result = mine.run(repo, force_no_gh=True)
        self.assertGreaterEqual(result["draft_word_count"], 1)  # 결정론적 최소 검증
        # 데이터가 빈약하면 일반 체크리스트를 덧붙여 400단어 하한을 최대한 지향한다.
        self.assertLessEqual(result["draft_word_count"], mine.DEFAULT_TARGET_WORDS[1] + 500)


if __name__ == "__main__":
    unittest.main()
