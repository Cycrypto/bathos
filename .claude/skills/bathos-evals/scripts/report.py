#!/usr/bin/env python3
"""
report — evals 결과(JSON)를 surface-formats-kr.md §c 4섹션 IA 리포트(마크다운)로 렌더링.

[Source: story-c3-c4-evals-skeleton-kr.md AC3, surface-formats-kr.md §c,
          project-context-kr.md §9(무컬러이모지·날조 금지)]

정직성 강제(§c-4, 하드 규칙 — 테스트로 고정):
  1. N(케이스 수) < 임계(기본 3)면 헤더에 `(추정) — 표본 부족` 배지를 강제 표시.
  2. 결과 0건이면 §2/§3을 "데이터 없음(스켈레톤)"으로 명확히(빈 표를 0으로 채우지 않음).
  3. 델타는 항상 "vs 무파이프라인 baseline" 문구와 함께(이번 범위엔 베이스라인 미실측).
  4. §4에 정정 이력 절을 둔다(비어 있어도 구조는 항상 존재).
  5. 배지는 텍스트 라벨 병기(`[PASS]` 등), 방향 기호는 흑백 `▼▲`.
"""
from __future__ import annotations

import argparse
import json
from datetime import datetime, timezone
from pathlib import Path
from typing import Dict, List, Optional

N_ESTIMATE_THRESHOLD = 3  # (추정, surface-formats §c-4 예시값) — 미만이면 (추정) 배지 강제


def load_results(results_dir: Path) -> List[Dict]:
    if not results_dir.exists():
        return []
    results = []
    for f in sorted(results_dir.glob("*.json")):
        try:
            results.append(json.loads(f.read_text(encoding="utf-8")))
        except json.JSONDecodeError:
            continue  # 손상 파일은 관용적으로 스킵(리포트 전체를 죽이지 않음)
    return results


def is_execution_failure(result: Dict) -> bool:
    artifact = (result.get("candidate_run") or {}).get("artifact") or ""
    return isinstance(artifact, str) and artifact.startswith("ERROR")


def verdict_badge(verdicts: List[str]) -> str:
    """다수 케이스 verdict를 하나의 정성 배지로 축약(스켈레톤 규모라 단순 규칙)."""
    if not verdicts:
        return "[FAIL]"
    if all(v == "win" for v in verdicts):
        return "[PASS]"
    if any(v == "lose" for v in verdicts):
        return "[CONCERNS]" if any(v == "win" for v in verdicts) else "[FAIL]"
    return "[CONCERNS]"


def render(
    results: List[Dict],
    run_id: str,
    corrections: Optional[List[str]] = None,
    now: Optional[datetime] = None,
) -> str:
    now = now or datetime.now(timezone.utc)
    corrections = corrections or []
    n = len(results)
    estimate_needed = n < N_ESTIMATE_THRESHOLD
    failures = [r for r in results if is_execution_failure(r)]
    ok_results = [r for r in results if r not in failures]

    lines: List[str] = []
    header_badges = ["(추정) — 표본 부족"] if estimate_needed else []
    if failures:
        header_badges.append(f"부분 리포트({len(failures)}건 실패)")
    header_suffix = "  ·  ".join(header_badges)
    lines.append(f"# BATHOS Dynamis Benchmark · {run_id} · {now.strftime('%Y-%m-%d')}")
    if header_suffix:
        lines.append(f"[ {header_suffix} ]")
    lines.append("")

    # -------------------------------------------------------------- §1
    lines.append("## §1 요약 (Summary)")
    if n == 0:
        lines.append("")
        lines.append("**데이터 없음(스켈레톤)** — `golden/`에 케이스가 없거나 아직 실행되지 않았습니다.")
        lines.append("다음 행동: `bathos-evals` 스킬로 골든 케이스를 추가·실행하세요(Stephen W4 초안 이후 확장은 Matthias W6).")
        lines.append("")
    else:
        by_task: Dict[str, List[str]] = {}
        for r in ok_results:
            by_task.setdefault(r.get("task", "unknown"), []).append(r.get("judge", {}).get("verdict", "tie"))
        lines.append("")
        lines.append(f"N = {n}건  ·  베이스라인: 무파이프라인(이번 범위 미실측 — §2 참고)")
        lines.append("")
        for task, verdicts in sorted(by_task.items()):
            lines.append(f"- {task}  {verdict_badge(verdicts)}  (n={len(verdicts)})")
        if failures:
            lines.append(f"- 실행 실패  [FAIL]  (k={len(failures)}, §3 참고)")
        lines.append("")

    # -------------------------------------------------------------- §2
    lines.append("## §2 정량 델타 (Quantitative Delta)")
    lines.append("")
    if n == 0:
        lines.append("데이터 없음(스켈레톤) — 실측값 없음.")
    else:
        lines.append("| Metric | 파이프라인 | Baseline(무파이프라인) | Δ | 방향 |")
        lines.append("|--------|-----------:|------------------------:|---:|:----:|")
        wall_values = [
            (r.get("candidate_run") or {}).get("wall_ms")
            for r in ok_results
            if (r.get("candidate_run") or {}).get("wall_ms") is not None
        ]
        avg_wall = f"{(sum(wall_values) / len(wall_values)):.1f}ms" if wall_values else "—"
        lines.append(f"| 실행 시간(평균) | {avg_wall} | — (미실측) | — | — |")
        lines.append("| 토큰(총) | — (미측정) | — (미실측) | — | — |")
        lines.append(f"| 결함 수(실행 실패) | {len(failures)} | — (미실측) | — | — |")
        lines.append("")
        lines.append("> 모든 셀은 실측 또는 `(추정)`/`—`. **베이스라인(무파이프라인)은 이번 범위에서 미실측** — "
                      "델타(Δ)는 베이스라인 확보 전까지 산출하지 않는다(날조 금지, LD-3 상한).")

    # -------------------------------------------------------------- §3
    lines.append("")
    lines.append("## §3 케이스별 상세 (Per-case)")
    lines.append("")
    if n == 0:
        lines.append("데이터 없음(스켈레톤).")
    else:
        for r in results:
            j = r.get("judge", {})
            status = "[FAIL: 실행 실패]" if is_execution_failure(r) else f"[{j.get('verdict', 'tie').upper()}]"
            lines.append(f"### {r.get('eval_id')} — {r.get('task')} {status}")
            lines.append(f"- golden_sha: `{r.get('golden_sha')}`" + ("  ⚠ 드리프트 감지" if r.get("golden_drift") else ""))
            lines.append(f"- wall_ms: {(r.get('candidate_run') or {}).get('wall_ms', '—')}")
            lines.append(f"- judge: {j.get('judged_by', '—')} — {j.get('reason', '—')}")
            lines.append("")

    # -------------------------------------------------------------- §4
    lines.append("## §4 방법론·한계·정정 (Methodology)")
    lines.append("")
    lines.append(f"- 골든셋 SHA: {', '.join(sorted({r.get('golden_sha', '—') for r in results})) or '—'}")
    lines.append(f"- judge: {', '.join(sorted({r.get('judge', {}).get('judged_by', '—') for r in results})) or '—'}"
                  " (`stub-heuristic-v1`=결정론적 로컬 스텁, 프로덕션 권장 경로는 호출 에이전트의 agent-judge — 재현성 (추정) 미보장, R-C2)")
    lines.append("- 알려진 한계: LD-3 상한(스켈레톤+1~2 케이스), 베이스라인(무파이프라인) 미실측, "
                 "자체 태스크셋 병목(W6 Matthias 확장 예정), judge seed/온도 비제어(agent 모드).")
    lines.append("")
    lines.append("### 정정 이력")
    if corrections:
        for c in corrections:
            lines.append(f"- {c}")
    else:
        lines.append("- (아직 없음 — 이 리포트가 최초 발행분)")

    return "\n".join(lines) + "\n"


def main(argv: Optional[List[str]] = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--results-dir", type=Path, default=Path(".agent-team/11-qa/evals/results"))
    parser.add_argument("--out", type=Path, default=None)
    parser.add_argument("--run-id", default="local-run")
    args = parser.parse_args(argv)

    results = load_results(args.results_dir)
    text = render(results, run_id=args.run_id)
    if args.out:
        args.out.parent.mkdir(parents=True, exist_ok=True)
        args.out.write_text(text, encoding="utf-8")
        print(f"[bathos evals] ✓ 리포트 저장: {args.out}")
    else:
        print(text)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
