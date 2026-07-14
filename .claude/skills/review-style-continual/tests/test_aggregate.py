#!/usr/bin/env python3
"""review-style-continual 결정성 테스트 (합성 픽스처만 사용, 외부 의존 없음).

실행: python3 -m unittest discover -s .claude/skills/review-style-continual/tests
[Source: story-c1-review-continual-kr.md §5]
"""
import json
import sys
import tempfile
import unittest
from pathlib import Path

SKILL_DIR = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(SKILL_DIR / "scripts"))

import aggregate  # noqa: E402


def write_jsonl(path: Path, records):
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("w", encoding="utf-8") as fh:
        for i, rec in enumerate(records, start=1):
            rec.setdefault("seq", i)
            rec.setdefault("project_id", "test")
            rec.setdefault("ts", f"2026-07-08T00:00:{i:02d}Z")
            rec.setdefault("hash_prev", "genesis" if i == 1 else "x")
            rec.setdefault("hash_self", "x")
            fh.write(json.dumps(rec) + "\n")


def resolved_event(finding_id, resolution, seq=None):
    rec = {"actor": "Thomas", "action": "finding.resolved", "target": f"{finding_id}:{resolution}"}
    if seq is not None:
        rec["seq"] = seq
    return rec


class TestAggregate(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.root = Path(self.tmp.name)
        self.audit_log = self.root / "audit-log.jsonl"
        self.index = self.root / "findings-index.jsonl"
        self.style = self.root / "style.md"
        self.history = self.root / "history"

    def tearDown(self):
        self.tmp.cleanup()

    def _index(self, mapping):
        write_jsonl(
            self.index,
            [{"finding_id": fid, "category": cat} for fid, cat in mapping.items()],
        )

    # ① fixed 다수 category → 승격 제안
    def test_promote_on_high_fix_rate(self):
        events = [resolved_event(f"F-{i:04d}", "fixed") for i in range(8)]
        events += [resolved_event("F-9000", "dismissed")]
        write_jsonl(self.audit_log, events)
        self._index({f"F-{i:04d}": "security" for i in range(8)} | {"F-9000": "security"})

        result = aggregate.run(self.audit_log, self.index, self.style)
        decision = next(d for d in result.decisions if d.category == "security")
        self.assertEqual(decision.status, "promote")
        self.assertIn("승격", result.after)

    # ② dismissed 다수 → 억제 제안
    def test_suppress_on_high_dismiss_rate(self):
        events = [resolved_event(f"S-{i:04d}", "dismissed") for i in range(8)]
        events += [resolved_event("S-9000", "fixed")]
        write_jsonl(self.audit_log, events)
        self._index({f"S-{i:04d}": "style-nit" for i in range(8)} | {"S-9000": "style-nit"})

        result = aggregate.run(self.audit_log, self.index, self.style)
        decision = next(d for d in result.decisions if d.category == "style-nit")
        self.assertEqual(decision.status, "suppress")
        self.assertIn("억제", result.after)

    # ③ 표본 4건(<5, 추정 임계) → 보류(관측 부족)
    def test_insufficient_sample_holds(self):
        events = [resolved_event(f"P-{i:04d}", "fixed") for i in range(4)]
        write_jsonl(self.audit_log, events)
        self._index({f"P-{i:04d}": "perf" for i in range(4)})

        result = aggregate.run(self.audit_log, self.index, self.style, threshold=5)
        decision = next(d for d in result.decisions if d.category == "perf")
        self.assertEqual(decision.status, "insufficient")
        self.assertIn("관측 부족", decision.reason)

    # ④ 상충(fixed·dismissed 동수) → 판단 유보
    def test_conflict_defers_judgement(self):
        events = [resolved_event(f"C-{i:04d}", "fixed") for i in range(5)]
        events += [resolved_event(f"C-{i:04d}", "dismissed") for i in range(5, 10)]
        write_jsonl(self.audit_log, events)
        self._index({f"C-{i:04d}": "correctness" for i in range(10)})

        result = aggregate.run(self.audit_log, self.index, self.style)
        decision = next(d for d in result.decisions if d.category == "correctness")
        self.assertEqual(decision.status, "conflict")
        self.assertIn("유보", decision.reason)

    # ⑤ 재실행 → 편집(기존 규칙 보존 + diff) 확인, 통째 재작성 아님
    def test_rerun_preserves_manual_section_and_edits_only_marker_block(self):
        events = [resolved_event(f"F-{i:04d}", "fixed") for i in range(8)]
        write_jsonl(self.audit_log, events)
        self._index({f"F-{i:04d}": "security" for i in range(8)})

        first = aggregate.run(self.audit_log, self.index, self.style)
        aggregate.write_apply(self.style, self.history, first)
        after_first = self.style.read_text(encoding="utf-8")
        self.assertIn("## 기본 원칙 (수동 편집 영역)", after_first)  # 수동 영역 보존
        self.assertEqual(len(list(self.history.glob("style-*.md"))), 1)

        # 두 번째 실행: 새 데이터 추가(억제 카테고리 신설)
        more_events = events + [resolved_event(f"D-{i:04d}", "dismissed") for i in range(8)]
        write_jsonl(self.audit_log, more_events)
        self._index(
            {f"F-{i:04d}": "security" for i in range(8)}
            | {f"D-{i:04d}": "style-nit" for i in range(8)}
        )
        second = aggregate.run(self.audit_log, self.index, self.style)
        self.assertTrue(second.changed)
        self.assertIn("## 기본 원칙 (수동 편집 영역)", second.after)
        # 버전이 증가해야 함(마커의 vN)
        self.assertGreater(aggregate.read_version(second.after), aggregate.read_version(after_first))

    # AC4: 라벨 이벤트 전무 → 콜드스타트 안내(에러 아님), 편집 보류
    def test_cold_start_when_no_labels_at_all(self):
        write_jsonl(self.audit_log, [])
        result = aggregate.run(self.audit_log, self.index, self.style)
        self.assertTrue(result.cold_start)
        self.assertIn("콜드스타트", result.after if result.changed else result.before)

    # findings-index 부재 → category 조인 불가, uncategorized로 정직하게 표시(크래시 없음)
    def test_missing_findings_index_falls_back_to_uncategorized(self):
        events = [resolved_event(f"U-{i:04d}", "fixed") for i in range(6)]
        write_jsonl(self.audit_log, events)
        # self.index 파일을 만들지 않음(부재)
        result = aggregate.run(self.audit_log, self.index, self.style)
        self.assertFalse(result.cold_start)
        self.assertTrue(any(d.category == "uncategorized" for d in result.decisions))

    # 손상된 라인은 스킵(관용) — 크래시 없이 정상 라인만 반영
    def test_malformed_line_is_skipped_gracefully(self):
        self.audit_log.parent.mkdir(parents=True, exist_ok=True)
        with self.audit_log.open("w", encoding="utf-8") as fh:
            fh.write("{not valid json\n")
            fh.write(json.dumps({"seq": 1, "action": "finding.resolved", "target": "F-0001:fixed"}) + "\n")
        result = aggregate.run(self.audit_log, self.index, self.style)
        self.assertFalse(result.cold_start)

    def test_apply_write_creates_history_snapshot_before_overwrite(self):
        events = [resolved_event(f"F-{i:04d}", "fixed") for i in range(6)]
        write_jsonl(self.audit_log, events)
        self._index({f"F-{i:04d}": "security" for i in range(6)})
        result = aggregate.run(self.audit_log, self.index, self.style)
        aggregate.write_apply(self.style, self.history, result)
        self.assertTrue(self.style.exists())
        self.assertEqual(len(list(self.history.glob("style-*.md"))), 1)


if __name__ == "__main__":
    unittest.main()
