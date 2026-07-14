#!/usr/bin/env python3
"""
judge — evals 판정 인터페이스 (기본: 결정론적 로컬 stub, 프로덕션: 에이전트 위임).

[Source: story-c3-c4-evals-skeleton-kr.md §2, service-layer-design-kr.md §3,
          risk-log C-6(judge 재현성)]

**정직성 고지(R-C2):** "Claude judge 페어와이즈 채점"은 이 저장소가 이미 Claude Code
에이전트 안에서 실행되고 있음을 활용해, **하니스를 호출하는 에이전트 자신이 judge
프롬프트를 읽고 판정**하는 것을 프로덕션 경로로 삼는다(별도 API 키/과금 불필요).
그러나 이는 **seed/온도를 코드에서 제어할 수 없다** — 대화형 에이전트 판정은
실행마다 달라질 수 있음(비결정성, 추정 미확정). 그래서:
  - **자동화/CI/테스트에는 `stub_judge`(순수 함수, 100% 재현 가능)를 기본값으로 둔다.**
  - **agent 모드**(`format_judge_prompt` + `--verdict-file`)는 실제 Claude 판정을
    파일로 주입받는 경로이며, 재현성은 "(추정) 미보장"으로 리포트 §4에 명시한다.
"""
from __future__ import annotations

import difflib
import json
from typing import Dict, Optional

JUDGED_BY_STUB = "stub-heuristic-v1"


def stub_judge(golden: Dict, candidate: Dict) -> Dict:
    """태스크별 결정론적 채점. 외부 API 호출 없음 — 순수 함수라 100% 재현 가능하다.
    (LangSmith·실 LLM 호출을 대체하는 것이 아니라, 스켈레톤 배관을 오프라인으로
    검증하기 위한 레퍼런스 구현이다. SKILL.md는 프로덕션에서 agent 모드를 권고한다.)"""
    task = golden.get("task", "unknown")
    if candidate.get("stub"):
        return {
            "verdict": "tie",
            "reason": "후보 아티팩트가 스텁(바이너리/입력 부재)이라 완전한 판정 불가 — (추정) 판정 보류에 가까움.",
            "judged_by": JUDGED_BY_STUB,
        }

    if task == "ctx-loss":
        return _judge_ctx_loss(golden, candidate)
    if task == "gate-fp":
        return _judge_gate_fp(golden, candidate)
    return {
        "verdict": "tie",
        "reason": f"미지원 task '{task}' — stub judge가 채점 규칙을 정의하지 않음.",
        "judged_by": JUDGED_BY_STUB,
    }


def _judge_ctx_loss(golden: Dict, candidate: Dict) -> Dict:
    expected = golden.get("expected", {})
    try:
        recovered = json.loads(candidate.get("artifact", "{}"))
    except json.JSONDecodeError:
        recovered = {}

    fields = list(expected.keys())
    if not fields:
        return {"verdict": "tie", "reason": "골든에 expected 필드 없음 — 채점 불가.", "judged_by": JUDGED_BY_STUB}

    matched = [f for f in fields if str(recovered.get(f, "")).strip() == str(expected.get(f, "")).strip()]
    ratio = len(matched) / len(fields)
    missing = sorted(set(fields) - set(matched))

    if ratio == 1.0:
        verdict = "win"
        reason = f"필드 {len(fields)}/{len(fields)} 완전 복원 — 컨텍스트 손실 없음."
    elif ratio >= 0.5:
        verdict = "tie"
        reason = f"필드 {len(matched)}/{len(fields)} 복원 — 부분 손실(누락: {', '.join(missing)})."
    else:
        verdict = "lose"
        reason = f"필드 {len(matched)}/{len(fields)}만 복원 — 컨텍스트 손실 의심(누락: {', '.join(missing)})."
    return {"verdict": verdict, "reason": reason, "judged_by": JUDGED_BY_STUB}


def _judge_gate_fp(golden: Dict, candidate: Dict) -> Dict:
    expected_verdict = golden.get("expected", {}).get("verdict_contains")
    artifact = candidate.get("artifact", "")
    if artifact.startswith("ERROR"):
        return {"verdict": "lose", "reason": f"파일럿 실행 실패: {artifact[:200]}", "judged_by": JUDGED_BY_STUB}
    if expected_verdict and expected_verdict in artifact:
        return {"verdict": "win", "reason": f"기대 문자열 '{expected_verdict}' 파일럿 출력에서 확인.", "judged_by": JUDGED_BY_STUB}
    similarity = difflib.SequenceMatcher(None, expected_verdict or "", artifact).ratio()
    return {
        "verdict": "tie" if similarity > 0.3 else "lose",
        "reason": f"기대 문자열 미확인(유사도 {similarity:.2f}) — 수동 검토 권장.",
        "judged_by": JUDGED_BY_STUB,
    }


def format_judge_prompt(golden: Dict, candidate: Dict) -> str:
    """프로덕션(agent 모드)용 — 하니스를 호출한 Claude 에이전트가 이 텍스트를
    읽고 verdict json({"verdict","reason","judged_by"})을 `--verdict-file`로
    돌려주는 것을 전제로 한다. 재현성 미보장(R-C2)을 프롬프트 자체에도 고지한다."""
    lines = [
        "# Evals 페어와이즈 판정 요청",
        "",
        f"- eval_id: {golden.get('eval_id')}",
        f"- task: {golden.get('task')}",
        "",
        "## 골든(기대)",
        "```json",
        json.dumps(golden.get("expected", {}), ensure_ascii=False, indent=2),
        "```",
        "",
        "## 후보(candidate_run.artifact)",
        "```",
        str(candidate.get("artifact", "")),
        "```",
        "",
        "위 골든과 후보를 비교해 win|lose|tie 중 하나로 판정하고 근거를 1~2문장으로 작성하세요.",
        "(주의: 이 판정은 대화형 에이전트가 수행하므로 seed/온도를 제어할 수 없어 재현성이 "
        "(추정) 미보장입니다 — 리포트 방법론 절에 이 한계를 명시하세요.)",
    ]
    return "\n".join(lines)


def load_verdict_file(path: str) -> Optional[Dict]:
    try:
        with open(path, "r", encoding="utf-8") as fh:
            return json.load(fh)
    except (OSError, json.JSONDecodeError):
        return None
