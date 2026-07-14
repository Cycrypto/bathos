#!/usr/bin/env python3
"""
review-style-continual — audit-log.jsonl 기반 리뷰 스타일 자기개선 집계 엔진.

[Source: story-c1-review-continual-kr.md, service-layer-design-kr.md §1,
          schema-extensions-kr.md §2, adr/D-0003(편집이지 재작성 아님)]

책임(결정론적·테스트 가능):
  1. `audit-log.jsonl`에서 `action == "finding.resolved"` 이벤트를 읽는다
     (target 형식 "F-0007:fixed" — finding_id:resolution).
  2. (선택) `findings-index.jsonl`로 finding_id -> category 를 조인한다.
     이 인덱스는 Phillip A1 스키마(schema-extensions §2a)가 정의하는
     `10-review/*` finding 산출물의 경량 조인 뷰이며, **실 데이터는 W5
     Thomas 리뷰가 채운다(R3/C-3, 이번 구현은 합성 픽스처로 로직만 검증)**.
     인덱스가 없으면 category 조인이 불가능함을 정직하게 표시한다(날조 금지).
  3. category별 확정(fixed)/기각(dismissed)률을 계산해 승격/억제/유보/
     관측부족 4상태로 분류한다.
  4. `style.md`의 마커 블록만 **편집**(재작성 아님)하고 변경 전 스냅샷을
     `history/`에 남긴다. 실제 파일 쓰기는 `--apply` 시에만 수행하고,
     기본은 `--dry-run`(계획만 출력) — UX 규약(디자인 정본, 파괴적 흐름은
     dry-run 기본 + 명시적 승인) 준수.

카피 금지: LangSmith 등 외부 SDK/네트워크 의존 없음(로컬 파일만, ETHOS·
project-context-kr.md §7).
"""
from __future__ import annotations

import argparse
import json
import re
import sys
from dataclasses import dataclass, field
from datetime import datetime, timezone
from pathlib import Path
from typing import Dict, Iterable, List, Optional

# ---------------------------------------------------------------------------
# 상수 (모두 "(추정, 구현 시 조정)" — Thomas M-2 정정 반영, 날조 금지)
# ---------------------------------------------------------------------------
DEFAULT_SAMPLE_THRESHOLD = 5  # (추정, 구현 시 조정) — 표본 부족 판정 임계
PROMOTE_RATE = 0.7            # (추정, 구현 시 조정) — 확정률 이상이면 승격 후보
SUPPRESS_RATE = 0.7           # (추정, 구현 시 조정) — 기각률 이상이면 억제 후보
CONFLICT_MARGIN = 0.2         # (추정, 구현 시 조정) — 확정률/기각률 차이가 이 폭 미만이면 상충

VALID_RESOLUTIONS = {"fixed", "dismissed", "deferred", "open"}
DECISIVE_RESOLUTIONS = {"fixed", "dismissed"}  # deferred/open은 표본에서 제외(미확정)

MARK_BEGIN_RE = re.compile(r"<!--\s*BATHOS:REVIEW-STYLE:BEGIN.*?-->")
MARK_END_RE = re.compile(r"<!--\s*BATHOS:REVIEW-STYLE:END\s*-->")


# ---------------------------------------------------------------------------
# 1. audit-log.jsonl 로더 (관용 — 파싱 실패 라인은 스킵, Rust 로더와 동일 철학)
#    [Source: bathos-inspect/src/loader/audit.rs 관용 로딩 원칙]
# ---------------------------------------------------------------------------
@dataclass
class ResolvedFinding:
    finding_id: str
    resolution: str
    seq: int
    ts: str


def load_resolved_findings(audit_log_path: Path) -> List[ResolvedFinding]:
    """finding.resolved 이벤트만 추출한다. 동일 finding_id 재기록은 seq가 큰
    것(최신)이 우선한다 — 기각 후 재오픈·재확정 같은 이력 정정을 반영."""
    if not audit_log_path.exists():
        return []

    latest: Dict[str, ResolvedFinding] = {}
    with audit_log_path.open("r", encoding="utf-8") as fh:
        for line in fh:
            line = line.strip()
            if not line:
                continue
            try:
                entry = json.loads(line)
            except json.JSONDecodeError:
                # 관용: 손상된 라인은 스킵(크래시 금지) — 헌법 §2 훅 계약 fail-safe 사상 계승.
                continue
            if entry.get("action") != "finding.resolved":
                continue
            target = entry.get("target", "")
            if ":" not in target:
                continue
            finding_id, _, resolution = target.partition(":")
            finding_id = finding_id.strip()
            resolution = resolution.strip()
            if not finding_id or resolution not in VALID_RESOLUTIONS:
                continue
            seq = entry.get("seq", 0)
            prev = latest.get(finding_id)
            if prev is None or seq >= prev.seq:
                latest[finding_id] = ResolvedFinding(
                    finding_id=finding_id,
                    resolution=resolution,
                    seq=seq,
                    ts=entry.get("ts", ""),
                )
    return list(latest.values())


# ---------------------------------------------------------------------------
# 2. findings-index.jsonl 로더 (category 조인 — 선택적, 없으면 조인 불가 표시)
#    형식(schema-extensions §2a): {"finding_id","category","severity",...}
# ---------------------------------------------------------------------------
def load_findings_index(index_path: Path) -> Dict[str, str]:
    """finding_id -> category 매핑. 파일 부재/개별 라인 파싱 실패는 관용 처리."""
    mapping: Dict[str, str] = {}
    if not index_path.exists():
        return mapping
    with index_path.open("r", encoding="utf-8") as fh:
        for line in fh:
            line = line.strip()
            if not line:
                continue
            try:
                rec = json.loads(line)
            except json.JSONDecodeError:
                continue
            fid = rec.get("finding_id")
            cat = rec.get("category")
            if fid and cat:
                mapping[fid] = cat
    return mapping


# ---------------------------------------------------------------------------
# 3. 집계 + 상태 분류
# ---------------------------------------------------------------------------
@dataclass
class CategoryStat:
    category: str
    fixed: int = 0
    dismissed: int = 0
    deferred: int = 0
    open_: int = 0

    @property
    def decisive_n(self) -> int:
        return self.fixed + self.dismissed

    @property
    def fixed_rate(self) -> float:
        return self.fixed / self.decisive_n if self.decisive_n else 0.0

    @property
    def dismissed_rate(self) -> float:
        return self.dismissed / self.decisive_n if self.decisive_n else 0.0


@dataclass
class Decision:
    category: str
    status: str  # promote | suppress | conflict | insufficient | neutral
    stat: CategoryStat
    reason: str


def aggregate(
    resolved: List[ResolvedFinding],
    category_of: Dict[str, str],
    threshold: int = DEFAULT_SAMPLE_THRESHOLD,
) -> Dict[str, CategoryStat]:
    stats: Dict[str, CategoryStat] = {}
    for rf in resolved:
        category = category_of.get(rf.finding_id, "uncategorized")
        stat = stats.setdefault(category, CategoryStat(category=category))
        if rf.resolution == "fixed":
            stat.fixed += 1
        elif rf.resolution == "dismissed":
            stat.dismissed += 1
        elif rf.resolution == "deferred":
            stat.deferred += 1
        elif rf.resolution == "open":
            stat.open_ += 1
    return stats


def classify(stats: Dict[str, CategoryStat], threshold: int = DEFAULT_SAMPLE_THRESHOLD) -> List[Decision]:
    decisions: List[Decision] = []
    for category, stat in sorted(stats.items()):
        n = stat.decisive_n
        if n < threshold:
            decisions.append(
                Decision(
                    category=category,
                    status="insufficient",
                    stat=stat,
                    reason=f"관측 부족(결정적 표본 {n}건 < 임계 {threshold}, 추정 임계) — 편집 보류",
                )
            )
            continue

        margin = abs(stat.fixed_rate - stat.dismissed_rate)
        if margin < CONFLICT_MARGIN:
            decisions.append(
                Decision(
                    category=category,
                    status="conflict",
                    stat=stat,
                    reason=(
                        f"상충 신호(확정 {stat.fixed}/{n}={stat.fixed_rate:.0%}, "
                        f"기각 {stat.dismissed}/{n}={stat.dismissed_rate:.0%}) — 판단 유보(억지 승격 금지)"
                    ),
                )
            )
        elif stat.fixed_rate >= PROMOTE_RATE:
            decisions.append(
                Decision(
                    category=category,
                    status="promote",
                    stat=stat,
                    reason=f"확정률 {stat.fixed_rate:.0%}(n={n}) — 고확정 패턴 승격",
                )
            )
        elif stat.dismissed_rate >= SUPPRESS_RATE:
            decisions.append(
                Decision(
                    category=category,
                    status="suppress",
                    stat=stat,
                    reason=f"기각률 {stat.dismissed_rate:.0%}(n={n}) — 반복 기각 패턴 억제",
                )
            )
        else:
            decisions.append(
                Decision(
                    category=category,
                    status="neutral",
                    stat=stat,
                    reason=f"확정 {stat.fixed}/{n}·기각 {stat.dismissed}/{n} — 임계 미달, 변경 없음",
                )
            )
    return decisions


# ---------------------------------------------------------------------------
# 4. style.md 마커 블록 편집 (재작성 아님 — 마커 밖 수동 영역은 절대 보존)
# ---------------------------------------------------------------------------
SEED_STYLE_MD = """\
# 리뷰 스타일 프롬프트 (프로젝트 정본)

> 이 파일은 `review-style-continual` 스킬이 `<!-- BATHOS:REVIEW-STYLE:BEGIN -->`
> ~ `<!-- BATHOS:REVIEW-STYLE:END -->` 블록만 **편집**한다. 마커 밖 내용은
> 사람이 자유롭게 수정 가능하며 스킬이 절대 건드리지 않는다(재작성 금지 불변식,
> project-context-kr.md §5-4).

<!-- BATHOS:REVIEW-STYLE:BEGIN v0 -->
(아직 데이터 없음 — `review-style-continual` 스킬을 실행하면 이 블록이 갱신됩니다.)
<!-- BATHOS:REVIEW-STYLE:END -->

## 기본 원칙 (수동 편집 영역)

- 리뷰는 근거(파일:라인)와 함께 제시한다.
- 심각도는 폐쇄 어휘(`low/med/high/critical`)만 사용한다.
"""


def render_block(decisions: List[Decision], version: int, cold_start: bool) -> str:
    lines: List[str] = [f"<!-- BATHOS:REVIEW-STYLE:BEGIN v{version} -->"]
    if cold_start:
        lines.append("## 관측 부족 (콜드스타트)")
        lines.append(
            "- `finding.resolved` 이벤트가 audit-log에 전무합니다. "
            "리뷰가 더 누적된 뒤 재실행하세요(기본 스타일 유지, 날조 금지)."
        )
        lines.append("<!-- BATHOS:REVIEW-STYLE:END -->")
        return "\n".join(lines) + "\n"

    promote = [d for d in decisions if d.status == "promote"]
    suppress = [d for d in decisions if d.status == "suppress"]
    conflict = [d for d in decisions if d.status == "conflict"]
    insufficient = [d for d in decisions if d.status == "insufficient"]

    lines.append("## 승격 규칙 (고확정 패턴)")
    if promote:
        for d in promote:
            lines.append(f"- **{d.category}** — {d.reason}: 리뷰 시 강조 반영")
    else:
        lines.append("- (해당 없음)")

    lines.append("")
    lines.append("## 억제 규칙 (반복 기각 패턴)")
    if suppress:
        for d in suppress:
            lines.append(f"- **{d.category}** — {d.reason}: 디강조/제외 권고")
    else:
        lines.append("- (해당 없음)")

    lines.append("")
    lines.append("## 판단 유보 (상충 신호)")
    if conflict:
        for d in conflict:
            lines.append(f"- **{d.category}** — {d.reason}")
    else:
        lines.append("- (해당 없음)")

    lines.append("")
    lines.append(f"## 관측 부족 (표본 < 임계 {DEFAULT_SAMPLE_THRESHOLD}, 추정 임계)")
    if insufficient:
        for d in insufficient:
            lines.append(f"- **{d.category}** — {d.reason}")
    else:
        lines.append("- (해당 없음)")

    lines.append("<!-- BATHOS:REVIEW-STYLE:END -->")
    return "\n".join(lines) + "\n"


def read_version(text: str) -> int:
    m = re.search(r"BATHOS:REVIEW-STYLE:BEGIN v(\d+)", text)
    return int(m.group(1)) if m else 0


def apply_edit(original: str, new_block: str) -> str:
    """마커가 있으면 그 사이만 치환, 없으면 파일 끝에 블록을 추가한다(재작성 아님)."""
    begin = MARK_BEGIN_RE.search(original)
    end = MARK_END_RE.search(original)
    if begin and end and begin.start() < end.start():
        return original[: begin.start()] + new_block + original[end.end():]
    # 마커가 없는 파일 — 끝에 덧붙인다(기존 수동 콘텐츠 보존).
    sep = "\n" if not original.endswith("\n") else ""
    return original + sep + "\n" + new_block


def diff_preview(before: str, after: str) -> str:
    import difflib

    return "\n".join(
        difflib.unified_diff(
            before.splitlines(),
            after.splitlines(),
            fromfile="style.md (before)",
            tofile="style.md (after)",
            lineterm="",
        )
    )


# ---------------------------------------------------------------------------
# 5. 오케스트레이션
# ---------------------------------------------------------------------------
@dataclass
class RunResult:
    cold_start: bool
    decisions: List[Decision]
    before: str
    after: str
    diff: str
    changed: bool
    promoted: int = 0
    suppressed: int = 0

    def summary_line(self, version: int) -> str:
        if self.cold_start:
            return "[bathos review-learn] · 관측 부족(콜드스타트) — 편집 보류"
        if not self.changed:
            return "[bathos review-learn] · 변경 없음(임계 미달 카테고리뿐)"
        return (
            f"[bathos review-learn] ✓ 스타일 v{version} 저장: "
            f"+{self.promoted} 승격 / -{self.suppressed} 억제"
        )


def run(
    audit_log: Path,
    findings_index: Path,
    style_path: Path,
    threshold: int = DEFAULT_SAMPLE_THRESHOLD,
) -> RunResult:
    resolved = load_resolved_findings(audit_log)
    category_of = load_findings_index(findings_index)

    before = style_path.read_text(encoding="utf-8") if style_path.exists() else SEED_STYLE_MD
    version = read_version(before)

    if not resolved:
        # AC4: 라벨 이벤트 전무 → 에러 아닌 정직한 콜드스타트 안내.
        block = render_block([], version, cold_start=True)
        after = apply_edit(before, block)
        changed = after != before
        return RunResult(
            cold_start=True,
            decisions=[],
            before=before,
            after=after if changed else before,
            diff=diff_preview(before, after) if changed else "",
            changed=changed,
        )

    stats = aggregate(resolved, category_of, threshold=threshold)
    decisions = classify(stats, threshold=threshold)

    new_version = version + 1
    block = render_block(decisions, new_version, cold_start=False)
    after = apply_edit(before, block)
    changed = after != before

    promoted = sum(1 for d in decisions if d.status == "promote")
    suppressed = sum(1 for d in decisions if d.status == "suppress")

    return RunResult(
        cold_start=False,
        decisions=decisions,
        before=before,
        after=after,
        diff=diff_preview(before, after),
        changed=changed,
        promoted=promoted,
        suppressed=suppressed,
    )


def write_apply(style_path: Path, history_dir: Path, result: RunResult) -> None:
    """`after`를 저장하기 전에 `before`(수정 직전 상태, 파일 부재 시 시드 포함)를
    항상 `history/`에 스냅샷한다 — v0(시드)부터의 전체 개정 이력을 보존한다."""
    history_dir.mkdir(parents=True, exist_ok=True)
    ts = datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")
    snapshot = history_dir / f"style-{ts}.md"
    snapshot.write_text(result.before, encoding="utf-8")
    style_path.parent.mkdir(parents=True, exist_ok=True)
    style_path.write_text(result.after, encoding="utf-8")


# ---------------------------------------------------------------------------
# CLI
# ---------------------------------------------------------------------------
def main(argv: Optional[List[str]] = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--audit-log",
        type=Path,
        default=Path(".agent-team/_state/audit-log.jsonl"),
    )
    parser.add_argument(
        "--findings-index",
        type=Path,
        default=Path(".agent-team/10-review/findings-index.jsonl"),
        help="finding_id -> category 조인 뷰(Phillip A1/Thomas W5 산출 계약, 없으면 uncategorized).",
    )
    parser.add_argument(
        "--style-path",
        type=Path,
        default=Path(".agent-team/10-review/review-style/style.md"),
    )
    parser.add_argument(
        "--history-dir",
        type=Path,
        default=Path(".agent-team/10-review/review-style/history"),
    )
    parser.add_argument("--threshold", type=int, default=DEFAULT_SAMPLE_THRESHOLD)
    parser.add_argument(
        "--apply",
        action="store_true",
        help="기본은 dry-run(계획+diff만 출력). 사용자 승인 후 --apply로 실제 저장.",
    )
    parser.add_argument("--json", action="store_true", help="기계가독 JSON 출력.")
    args = parser.parse_args(argv)

    result = run(args.audit_log, args.findings_index, args.style_path, args.threshold)
    version = read_version(result.after) if result.changed else read_version(result.before)

    if args.apply:
        if result.changed:
            write_apply(args.style_path, args.history_dir, result)
        print(result.summary_line(version))
    else:
        if args.json:
            payload = {
                "cold_start": result.cold_start,
                "changed": result.changed,
                "decisions": [
                    {
                        "category": d.category,
                        "status": d.status,
                        "reason": d.reason,
                        "fixed": d.stat.fixed,
                        "dismissed": d.stat.dismissed,
                        "deferred": d.stat.deferred,
                        "open": d.stat.open_,
                    }
                    for d in result.decisions
                ],
                "diff_preview": result.diff,
            }
            print(json.dumps(payload, ensure_ascii=False, indent=2))
        else:
            print("[bathos review-learn] 계획(dry-run) — 아래 diff 승인 시 --apply로 재실행하세요.")
            print(result.diff or "(변경 없음)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
