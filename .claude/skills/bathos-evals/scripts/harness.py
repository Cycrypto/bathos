#!/usr/bin/env python3
"""
bathos-evals — 로컬 evals 스켈레톤 하니스 (골든셋 SHA 고정 → 파일럿 → judge → JSON).

[Source: story-c3-c4-evals-skeleton-kr.md, service-layer-design-kr.md §3,
          surface-formats-kr.md §c, adr/D-0003]

**범위 상한(LD-3):** 스켈레톤 + 레퍼런스 1~2 케이스만. 그 이상 골든셋 확장은
범위 밖(Matthias W6). LangSmith 등 외부 SDK/네트워크 종속 없음(카피 금지).

파이프라인: 골든 로드(SHA 검증) → 파일럿 실행(`bathos inspect`류 재사용, 바이너리
부재 시 스텁+명시) → judge 채점(`judge.py`, 기본은 결정론적 로컬 stub — 프로덕션은
호출 에이전트 자신이 Claude judge 역할을 겸함, SKILL.md 참고) → 로컬 JSON 결과.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import shutil
import subprocess
import sys
import time
from datetime import datetime, timezone
from pathlib import Path
from typing import Dict, Optional

sys.path.insert(0, str(Path(__file__).resolve().parent))
import judge as judge_mod  # noqa: E402

CASE_FILENAME = "case.json"
SHA_SUFFIX = ".sha256"


# ---------------------------------------------------------------------------
# 1. 골든셋 로드 + SHA 드리프트 가드
# ---------------------------------------------------------------------------
def sha256_of_file(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def golden_case_path(case_dir: Path) -> Path:
    return case_dir / CASE_FILENAME


def golden_sha_path(case_dir: Path) -> Path:
    return case_dir / f"{CASE_FILENAME}{SHA_SUFFIX}"


def write_golden_case(case_dir: Path, data: Dict) -> str:
    """골든 케이스를 생성하고 SHA를 고정한다(초안 작성 도구, 테스트/시딩용)."""
    case_dir.mkdir(parents=True, exist_ok=True)
    case_file = golden_case_path(case_dir)
    case_file.write_text(json.dumps(data, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    digest = sha256_of_file(case_file)
    golden_sha_path(case_dir).write_text(f"{digest}  {CASE_FILENAME}\n", encoding="utf-8")
    return digest


def load_and_verify_golden(case_dir: Path) -> Dict:
    """골든 케이스를 로드하고 SHA 드리프트를 검사한다.
    반환에 `_drift`(bool)·`_expected_sha`·`_actual_sha`를 포함한다(하니스가 소비)."""
    case_file = golden_case_path(case_dir)
    sha_file = golden_sha_path(case_dir)

    if not case_file.exists():
        raise FileNotFoundError(f"골든 케이스 없음: {case_file}")

    data = json.loads(case_file.read_text(encoding="utf-8"))
    actual = sha256_of_file(case_file)

    drift = False
    expected = None
    if sha_file.exists():
        line = sha_file.read_text(encoding="utf-8").strip()
        expected = line.split()[0] if line else None
        drift = expected is not None and expected != actual
    else:
        drift = True  # SHA 파일 자체가 없으면 고정되지 않은 것 — 드리프트로 간주(보수적).

    data["_drift"] = drift
    data["_expected_sha"] = expected
    data["_actual_sha"] = actual
    return data


# ---------------------------------------------------------------------------
# 2. 파일럿 실행 (bathos-inspect 재사용 — 바이너리 부재 시 스텁, fail-safe)
# ---------------------------------------------------------------------------
def _resolve_bathos_bin() -> Optional[str]:
    env_bin = os.environ.get("BATHOS_BIN")
    if env_bin and Path(env_bin).exists():
        return env_bin
    which = shutil.which("bathos")
    return which


def run_pilot_gate_fp(case: Dict) -> Dict:
    """CF-C3 예시 태스크: 게이트 오탐률 파일럿. `bathos inspect doctor --json`을
    재사용 시도하고, 바이너리 부재 시 스텁 아티팩트로 정직하게 강등한다."""
    started = time.perf_counter()
    bin_path = _resolve_bathos_bin()
    target = case.get("input", {}).get("agent_team_path")

    if bin_path is None:
        wall_ms = (time.perf_counter() - started) * 1000
        return {
            "artifact": "STUB: bathos 바이너리 없음(BATHOS_BIN/PATH 미탐지) — 파일럿 미실행",
            "tokens": None,
            "wall_ms": round(wall_ms, 3),
            "stub": True,
        }

    cmd = [bin_path, "inspect", "doctor", "--json"]
    if target:
        cmd += ["--path", target]
    try:
        proc = subprocess.run(cmd, capture_output=True, text=True, timeout=30)
        artifact = proc.stdout if proc.returncode == 0 else f"ERROR(exit={proc.returncode}): {proc.stderr[:300]}"
    except (OSError, subprocess.TimeoutExpired) as exc:
        artifact = f"ERROR: {exc}"
    wall_ms = (time.perf_counter() - started) * 1000
    return {"artifact": artifact, "tokens": None, "wall_ms": round(wall_ms, 3), "stub": False}


def run_pilot_ctx_loss(case: Dict, root: Path) -> Dict:
    """CF-C3 예시 태스크: 스토리 컨텍스트 무손실 여부. 스토리 프론트매터에서
    지정 필드를 결정론적으로 재추출해 골든 기대값과 비교할 후보를 만든다
    (LLM 없이도 재현 가능한 '컨텍스트 복원' 프록시)."""
    started = time.perf_counter()
    story_rel = case.get("input", {}).get("story_path")
    fields = case.get("input", {}).get("required_fields", [])
    recovered: Dict[str, Optional[str]] = {}

    if story_rel:
        story_path = root / story_rel
        if story_path.exists():
            text = story_path.read_text(encoding="utf-8")
            frontmatter = _extract_frontmatter(text)
            for field_name in fields:
                recovered[field_name] = frontmatter.get(field_name)
        else:
            recovered = {f: None for f in fields}
    wall_ms = (time.perf_counter() - started) * 1000
    return {
        "artifact": json.dumps(recovered, ensure_ascii=False, sort_keys=True),
        "tokens": None,
        "wall_ms": round(wall_ms, 3),
        "stub": story_rel is None,
    }


def _extract_frontmatter(text: str) -> Dict[str, str]:
    """`---\\nkey: value\\n---` 블록의 최상위 스칼라 필드만 파싱(경량, YAML 라이브러리
    비의존 — 이 저장소 스토리파일 프론트매터는 단순 스칼라/한줄객체이므로 충분)."""
    if not text.startswith("---"):
        return {}
    end = text.find("\n---", 3)
    if end == -1:
        return {}
    block = text[3:end]
    result: Dict[str, str] = {}
    for line in block.splitlines():
        line = line.strip()
        if not line or line.startswith("#") or ":" not in line:
            continue
        key, _, value = line.partition(":")
        result[key.strip()] = value.strip()
    return result


TASK_PILOTS = {
    "gate-fp": lambda case, root: run_pilot_gate_fp(case),
    "ctx-loss": run_pilot_ctx_loss,
}


# ---------------------------------------------------------------------------
# 3. 오케스트레이션 (골든 → 파일럿 → judge → 결과 스키마)
# ---------------------------------------------------------------------------
def run_case(case_dir: Path, root: Path, judge_mode: str = "stub") -> Dict:
    golden = load_and_verify_golden(case_dir)
    task = golden.get("task", "unknown")
    pilot_fn = TASK_PILOTS.get(task)
    if pilot_fn is None:
        candidate = {"artifact": f"ERROR: 미지원 task '{task}'", "tokens": None, "wall_ms": 0.0, "stub": True}
    else:
        candidate = pilot_fn(golden, root)

    verdict = judge_mod.stub_judge(golden, candidate)

    result = {
        "eval_id": golden.get("eval_id", case_dir.name),
        "golden_sha": golden.get("_actual_sha"),
        "golden_drift": golden.get("_drift", False),
        "task": task,
        "candidate_run": {
            "artifact": candidate.get("artifact"),
            "tokens": candidate.get("tokens"),
            "wall_ms": candidate.get("wall_ms"),
        },
        "judge": verdict,
        "created": datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
    }
    return result


def write_result(results_dir: Path, result: Dict) -> Path:
    results_dir.mkdir(parents=True, exist_ok=True)
    ts = datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%S%fZ")
    out_path = results_dir / f"{result['eval_id']}-{ts}.json"
    out_path.write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    return out_path


def run_all(golden_dir: Path, root: Path, results_dir: Optional[Path] = None) -> list:
    results = []
    if not golden_dir.exists():
        return results
    for case_dir in sorted(p for p in golden_dir.iterdir() if p.is_dir()):
        if not golden_case_path(case_dir).exists():
            continue
        result = run_case(case_dir, root)
        results.append(result)
        if results_dir is not None:
            write_result(results_dir, result)
    return results


# ---------------------------------------------------------------------------
# CLI
# ---------------------------------------------------------------------------
def main(argv: Optional[list] = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="cmd", required=True)

    p_run = sub.add_parser("run", help="단일 골든 케이스 실행")
    p_run.add_argument("--case-dir", type=Path, required=True)
    p_run.add_argument("--root", type=Path, default=Path("."))
    p_run.add_argument("--results-dir", type=Path, default=Path(".agent-team/11-qa/evals/results"))

    p_all = sub.add_parser("run-all", help="골든셋 전체 실행")
    p_all.add_argument("--golden-dir", type=Path, default=Path(".agent-team/11-qa/evals/golden"))
    p_all.add_argument("--root", type=Path, default=Path("."))
    p_all.add_argument("--results-dir", type=Path, default=Path(".agent-team/11-qa/evals/results"))

    p_verify = sub.add_parser("verify", help="골든셋 SHA 드리프트만 검사(실행 없음)")
    p_verify.add_argument("--golden-dir", type=Path, default=Path(".agent-team/11-qa/evals/golden"))

    args = parser.parse_args(argv)

    if args.cmd == "run":
        result = run_case(args.case_dir, args.root)
        out = write_result(args.results_dir, result)
        if result["golden_drift"]:
            print(f"[bathos evals] ! 골든 드리프트 감지: {args.case_dir} (SHA 불일치)", file=sys.stderr)
        print(f"[bathos evals] ✓ 결과 저장: {out}")
        print(json.dumps(result, ensure_ascii=False, indent=2))
        return 2 if result["golden_drift"] else 0

    if args.cmd == "run-all":
        results = run_all(args.golden_dir, args.root, args.results_dir)
        drifted = [r for r in results if r["golden_drift"]]
        if not results:
            print("[bathos evals] · 데이터 없음(스켈레톤) — golden/ 비어 있음")
            return 0
        print(f"[bathos evals] ✓ {len(results)}건 실행 완료" + (f" · 드리프트 {len(drifted)}건" if drifted else ""))
        return 2 if drifted else 0

    if args.cmd == "verify":
        if not args.golden_dir.exists():
            print("[bathos evals] · 데이터 없음(스켈레톤) — golden/ 없음")
            return 0
        bad = []
        for case_dir in sorted(p for p in args.golden_dir.iterdir() if p.is_dir()):
            if not golden_case_path(case_dir).exists():
                continue
            golden = load_and_verify_golden(case_dir)
            if golden["_drift"]:
                bad.append(case_dir.name)
        if bad:
            print(f"[bathos evals] ! 드리프트 감지: {', '.join(bad)}", file=sys.stderr)
            return 2
        print("[bathos evals] ✓ 드리프트 없음")
        return 0

    return 1


if __name__ == "__main__":
    raise SystemExit(main())
