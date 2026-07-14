#!/usr/bin/env python3
"""
bootstrap-repo-mine — 콜드스타트 저장소 컨벤션/버그 taxonomy 채굴(초안 생성까지).

[Source: story-c2-bootstrap-mine-kr.md, service-layer-design-kr.md §2,
          _recon/open-swe-analysis.md §2(bootstrap-repo-analysis)]

**범위(risk-log A-5 확정):** 이번 구현은 "저장소 컨벤션 초안 텍스트 생성까지"다.
`.agent-team/01-reverse/`는 John(#1 Reverse Specialist) 소유이므로 이 스크립트는
**그 경로에 절대 쓰지 않는다** — 실제 리버스 산출물 반영은 범위 밖 후속 단계다.

설계 원칙(정직성):
  - "버그 taxonomy·컨벤션을 의미론적으로 요약"하는 것은 본질적으로 생성(LLM) 작업이다.
    이 스크립트는 **결정론적 수집·필터링·빈도 통계**(테스트 가능한 부분)만 책임지고,
    수집된 원자료(raw evidence)를 근거 인용과 함께 초안에 담는다. 최종 서술 품질을
    끌어올리는 다듬기는 이 스킬을 호출하는 에이전트(Claude)가 원자료를 검토해
    수행할 것을 SKILL.md가 권고한다(휴리스틱을 LLM 성능으로 과장하지 않는다).
"""
from __future__ import annotations

import argparse
import json
import re
import shutil
import subprocess
import sys
from collections import Counter
from dataclasses import dataclass, field
from pathlib import Path
from typing import Dict, List, Optional

MIN_MERGED_PR_THRESHOLD = 8  # (추정, open-swe 관례 — 조정 가능) US12 AC4
DEFAULT_BOT_PATTERNS = [
    r"\[bot\]$",
    r"^dependabot",
    r"^renovate",
    r"^github-actions",
    r"^snyk-bot",
]
STOPWORDS = {
    "the", "a", "an", "and", "or", "to", "of", "in", "on", "for", "is", "are",
    "this", "that", "with", "it", "be", "as", "at", "by", "from", "was", "were",
    "please", "should", "would", "could", "이", "그", "저", "것", "수", "등", "및",
    "을", "를", "이런", "그런", "위해", "하는", "합니다", "해야", "있습니다",
}
DEFAULT_TARGET_WORDS = (400, 1200)  # US12 AC2 — (사실: story 명시 범위)


def is_bot(login: str, extra_patterns: Optional[List[str]] = None) -> bool:
    patterns = list(DEFAULT_BOT_PATTERNS) + list(extra_patterns or [])
    return any(re.search(p, login, flags=re.IGNORECASE) for p in patterns)


def is_gh_available() -> bool:
    return shutil.which("gh") is not None


def is_git_repo(path: Path) -> bool:
    """`.git` 존재 + 최소 1개 커밋(HEAD 해석 가능)까지 확인한다. 갓 `git init`한
    저장소(커밋 0개)는 "이력 없음"으로 취급해 구조 폴백으로 보낸다."""
    if not (path / ".git").exists():
        return False
    out = _run(["git", "rev-parse", "--verify", "HEAD"], cwd=path, timeout=5)
    return out is not None


def _run(cmd: List[str], cwd: Path, timeout: int = 20) -> Optional[str]:
    try:
        proc = subprocess.run(
            cmd, cwd=str(cwd), capture_output=True, text=True, timeout=timeout
        )
    except (OSError, subprocess.TimeoutExpired):
        return None
    if proc.returncode != 0:
        return None
    return proc.stdout


# ---------------------------------------------------------------------------
# 1. 수집 (gh PR 우선 → git log 폴백 → 구조 폴백)
# ---------------------------------------------------------------------------
@dataclass
class GhPr:
    number: int
    author: str
    title: str
    comments: List[str] = field(default_factory=list)


def fetch_gh_prs(repo_path: Path, limit: int = 200) -> Optional[List[GhPr]]:
    """`gh pr list --state merged --json ...` 결과. gh 미설치/미인증/네트워크
    실패 시 None(폴백 신호) — 예외를 던지지 않는다(fail-safe)."""
    if not is_gh_available():
        return None
    out = _run(
        [
            "gh", "pr", "list", "--state", "merged", "--limit", str(limit),
            "--json", "number,author,title,comments",
        ],
        cwd=repo_path,
    )
    if out is None:
        return None
    try:
        raw = json.loads(out)
    except json.JSONDecodeError:
        return None
    prs: List[GhPr] = []
    for item in raw:
        author = (item.get("author") or {}).get("login", "unknown")
        comments = [c.get("body", "") for c in item.get("comments", []) if c.get("body")]
        prs.append(GhPr(number=item.get("number", 0), author=author, title=item.get("title", ""), comments=comments))
    return prs


def git_log_fallback(repo_path: Path, limit: int = 300) -> Dict:
    """gh 불가/미인증 시: 로컬 git log만으로 병합 신호를 근사한다.
    원격 PR 코멘트는 얻을 수 없으므로 명시적으로 "부분 결과"를 표시한다(AC3)."""
    if not is_git_repo(repo_path):
        return {"available": False, "merge_commits": [], "authors": []}

    merges_out = _run(
        ["git", "log", f"-{limit}", "--merges", "--pretty=format:%an|%s"],
        cwd=repo_path,
    )
    merge_commits = []
    if merges_out:
        for line in merges_out.splitlines():
            if "|" not in line:
                continue
            author, _, subject = line.partition("|")
            merge_commits.append({"author": author.strip(), "subject": subject.strip()})

    all_authors_out = _run(["git", "log", f"-{limit}", "--pretty=format:%an"], cwd=repo_path)
    authors = [a.strip() for a in (all_authors_out or "").splitlines() if a.strip()]

    return {"available": True, "merge_commits": merge_commits, "authors": authors}


def structure_fallback(repo_path: Path) -> Dict:
    """PR/git 이력이 전무한 저장소용: 디렉터리/테스트/린트 구성 신호만 채굴한다."""
    signals = {
        "top_dirs": sorted(
            [p.name for p in repo_path.iterdir() if p.is_dir() and not p.name.startswith(".")]
        )[:20],
        "has_tests_dir": any((repo_path / d).is_dir() for d in ("tests", "test", "__tests__", "spec")),
        "lint_configs": [
            f.name
            for f in repo_path.glob("*")
            if f.is_file()
            and f.name
            in (
                ".eslintrc", ".eslintrc.json", ".eslintrc.js", "ruff.toml", "pyproject.toml",
                ".flake8", ".rustfmt.toml", "clippy.toml", ".editorconfig",
            )
        ],
        "ci_configs": [str(p.relative_to(repo_path)) for p in (repo_path / ".github" / "workflows").glob("*.y*ml")]
        if (repo_path / ".github" / "workflows").is_dir()
        else [],
    }
    return signals


# ---------------------------------------------------------------------------
# 2. taxonomy 추출 (결정론적 빈도 기반 — LLM 아님, 명시적 휴리스틱)
# ---------------------------------------------------------------------------
def tokenize(text: str) -> List[str]:
    words = re.findall(r"[A-Za-z가-힣][A-Za-z가-힣_\-]{2,}", text.lower())
    return [w for w in words if w not in STOPWORDS]


def build_taxonomy(texts_with_source: List[tuple], top_k: int = 8) -> List[Dict]:
    """texts_with_source: [(text, source_label), ...]
    반환: [{"term","count","example"}] — 근거 인용 1건 포함(날조 금지)."""
    counter: Counter = Counter()
    examples: Dict[str, str] = {}
    for text, source in texts_with_source:
        for tok in set(tokenize(text)):
            counter[tok] += 1
            if tok not in examples:
                snippet = text.strip().replace("\n", " ")[:120]
                examples[tok] = f"{source}: \"{snippet}\""
    ranked = counter.most_common(top_k)
    return [{"term": term, "count": count, "example": examples[term]} for term, count in ranked]


def word_count(text: str) -> int:
    return len(text.split())


# ---------------------------------------------------------------------------
# 3. 초안 렌더링
# ---------------------------------------------------------------------------
GENERIC_CHECKLIST = """
## 일반 체크리스트 (표본 부족 시 보강 — 사실 아닌 일반 권고, 날조 아님)

- 커밋 메시지 컨벤션(Conventional Commits 등)이 있는지 확인하고 없다면 도입을 검토한다.
- 테스트 디렉터리/커버리지 기준이 코드베이스 전반에 일관되게 적용되는지 확인한다.
- 린트/포맷 설정 파일이 CI에서 강제되는지 확인한다.
- 리뷰 코멘트 이력이 쌓이면 이 스킬을 재실행해 실제 taxonomy로 교체한다.
""".strip()


def render_draft(
    repo_name: str,
    mode: str,
    confidence_note: str,
    taxonomy: List[Dict],
    conventions: Dict,
    excluded_bots: List[str],
    partial_note: Optional[str] = None,
) -> str:
    lines: List[str] = []
    lines.append(f"# 저장소 컨벤션 초안 — {repo_name} (bootstrap-repo-mine)")
    lines.append("")
    lines.append(
        "> 이 초안은 `bootstrap-repo-mine` 스킬이 생성한 **텍스트 초안**이다. "
        "`.agent-team/01-reverse/`로의 실반영은 John(#1 Reverse Specialist) 소유이며 "
        "이 스킬의 책임 밖이다(risk-log A-5)."
    )
    lines.append("")
    lines.append("## 1. 개요")
    lines.append(f"- 수집 모드: `{mode}`")
    lines.append(f"- 신뢰도: {confidence_note}")
    if excluded_bots:
        lines.append(f"- 봇 계정 제외: {', '.join(sorted(set(excluded_bots)))}")
    else:
        lines.append("- 봇 계정 제외: (탐지된 봇 없음)")
    if partial_note:
        lines.append(f"- **부분 결과 안내:** {partial_note}")
    lines.append("")

    lines.append("## 2. 버그 taxonomy (반복 지적 패턴, 빈도 기반 휴리스틱)")
    if taxonomy:
        lines.append(
            "> 아래는 결정론적 빈도 집계 결과다 — 의미론적 분류는 호출 에이전트가 "
            "원자료를 검토해 다듬을 것을 권장한다(휴리스틱을 LLM 요약으로 과장하지 않음)."
        )
        for item in taxonomy:
            lines.append(f"- **{item['term']}**(관측 {item['count']}건) — 근거: {item['example']}")
    else:
        lines.append("- (추출된 반복 패턴 없음 — 표본 부족 또는 이력 부재)")
    lines.append("")

    lines.append("## 3. 팀 컨벤션 신호")
    if conventions.get("top_dirs"):
        lines.append(f"- 최상위 디렉터리: {', '.join(conventions['top_dirs'])}")
    if "has_tests_dir" in conventions:
        lines.append(f"- 테스트 디렉터리 존재: {'예' if conventions['has_tests_dir'] else '아니오'}")
    if conventions.get("lint_configs"):
        lines.append(f"- 린트/포맷 설정: {', '.join(conventions['lint_configs'])}")
    if conventions.get("ci_configs"):
        lines.append(f"- CI 워크플로: {', '.join(conventions['ci_configs'])}")
    if not any(conventions.values()):
        lines.append("- (구조 신호 없음)")
    lines.append("")

    lines.append("## 4. 한계 및 다음 단계")
    lines.append("- 이 초안은 자동 채굴 결과이며 사람의 검토 없이 정책으로 승격하지 않는다.")
    lines.append("- `01-reverse/repo-conventions-kr.md`로의 반영은 John 또는 리드 승인 경로를 거친다.")
    lines.append("- 표본이 늘어나면(병합 PR 8건 이상) 재실행해 신뢰도를 높인다.")

    draft = "\n".join(lines)
    wc = word_count(draft)
    if wc < DEFAULT_TARGET_WORDS[0] and mode != "structure-fallback-thin":
        draft = draft + "\n\n" + GENERIC_CHECKLIST
    return draft


# ---------------------------------------------------------------------------
# 4. 오케스트레이션
# ---------------------------------------------------------------------------
def run(
    repo_path: Path,
    bot_patterns: Optional[List[str]] = None,
    min_pr_threshold: int = MIN_MERGED_PR_THRESHOLD,
    force_no_gh: bool = False,
) -> Dict:
    repo_name = repo_path.resolve().name
    excluded_bots: List[str] = []

    prs = None if force_no_gh else fetch_gh_prs(repo_path)

    if prs is not None:
        kept = [p for p in prs if not is_bot(p.author, bot_patterns)]
        excluded_bots = [p.author for p in prs if is_bot(p.author, bot_patterns)]
        merged_count = len(kept)
        texts = [(c, f"PR #{p.number}") for p in kept for c in p.comments]
        texts += [(p.title, f"PR #{p.number} 제목") for p in kept]
        taxonomy = build_taxonomy(texts) if texts else []
        conventions = structure_fallback(repo_path) if is_git_repo(repo_path) else {}
        confidence = "충분" if merged_count >= min_pr_threshold else f"신뢰도 낮음(병합 PR {merged_count}건 < {min_pr_threshold})"
        mode = "pr"
        partial_note = None
    else:
        gh_reason = "gh 비활성(--no-gh)" if force_no_gh else "gh 미설치/미인증/실패"
        git_data = git_log_fallback(repo_path)
        if not git_data["available"]:
            # 이력 자체가 없는 저장소 → 구조 폴백
            conventions = structure_fallback(repo_path)
            taxonomy = []
            merged_count = 0
            confidence = "이력 부족(git 저장소 아님 또는 커밋 없음)"
            mode = "structure-fallback"
            partial_note = f"{gh_reason} → git log도 사용 불가 → 코드 구조 기반 폴백."
        else:
            merge_commits = git_data["merge_commits"]
            kept_authors = [a for a in git_data["authors"] if not is_bot(a, bot_patterns)]
            excluded_bots = [a for a in git_data["authors"] if is_bot(a, bot_patterns)]
            merged_count = len(merge_commits)
            texts = [(m["subject"], f"merge by {m['author']}") for m in merge_commits if not is_bot(m["author"], bot_patterns)]
            taxonomy = build_taxonomy(texts) if texts else []
            conventions = structure_fallback(repo_path)
            if merged_count == 0:
                confidence = "이력 부족(병합 커밋 없음)"
            elif merged_count < min_pr_threshold:
                confidence = f"신뢰도 낮음(병합 커밋 {merged_count}건 < {min_pr_threshold})"
            else:
                confidence = "충분(git log 근사치, 원격 PR 코멘트 없음)"
            mode = "git-log-fallback"
            partial_note = f"{gh_reason} → 로컬 git log로 폴백. 원격 PR 코멘트는 스킵됨(부분 결과)."

    draft = render_draft(
        repo_name=repo_name,
        mode=mode,
        confidence_note=confidence,
        taxonomy=taxonomy,
        conventions=conventions,
        excluded_bots=excluded_bots,
        partial_note=partial_note,
    )

    return {
        "repo_name": repo_name,
        "mode": mode,
        "confidence": confidence,
        "merged_sample_count": merged_count,
        "excluded_bots": sorted(set(excluded_bots)),
        "taxonomy": taxonomy,
        "conventions": conventions,
        "draft": draft,
        "draft_word_count": word_count(draft),
        "partial_note": partial_note,
    }


# ---------------------------------------------------------------------------
# CLI
# ---------------------------------------------------------------------------
FORBIDDEN_OUT_MARKERS = ("01-reverse",)


def main(argv: Optional[List[str]] = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", type=Path, default=Path("."))
    parser.add_argument("--out", type=Path, default=None, help="초안 저장 경로(선택). 01-reverse/ 하위는 거부한다(John 소유).")
    parser.add_argument("--min-pr", type=int, default=MIN_MERGED_PR_THRESHOLD)
    parser.add_argument("--no-gh", action="store_true", help="gh 강제 비활성(테스트/폴백 검증용).")
    parser.add_argument("--bot-pattern", action="append", default=[], help="추가 봇 판별 정규식(반복 가능).")
    parser.add_argument("--json", action="store_true")
    args = parser.parse_args(argv)

    if args.out is not None and any(m in str(args.out) for m in FORBIDDEN_OUT_MARKERS):
        print(
            "[bathos bootstrap-mine] ✗ 01-reverse/ 경로에는 쓸 수 없습니다(John 소유, risk-log A-5). "
            "출력을 다른 경로로 지정하거나 stdout으로 받아 John/리드에게 전달하세요.",
            file=sys.stderr,
        )
        return 2

    result = run(args.repo.resolve(), bot_patterns=args.bot_pattern, min_pr_threshold=args.min_pr, force_no_gh=args.no_gh)

    if args.json:
        print(json.dumps({k: v for k, v in result.items() if k != "draft"}, ensure_ascii=False, indent=2))
        print("---DRAFT---")
        print(result["draft"])
    else:
        print(f"[bathos bootstrap-mine] · 모드={result['mode']} · 신뢰도={result['confidence']} · 단어수={result['draft_word_count']}")
        print(result["draft"])

    if args.out is not None:
        args.out.parent.mkdir(parents=True, exist_ok=True)
        args.out.write_text(result["draft"], encoding="utf-8")
        print(f"[bathos bootstrap-mine] ✓ 초안 저장: {args.out}", file=sys.stderr)

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
