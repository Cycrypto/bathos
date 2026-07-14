# SESSION SNAPSHOT — 재개 핸드오프 (BATHOS)

> 저장: {{actor}}(리드) · 갱신: {{timestamp}} · 목적: 세션 종료 전 전체 상태 보존, 다음 세션에서 즉시 재개.
> 한 줄: {{one_line_summary}}

---

## ★ 현재 상태 (콜드스타트는 여기부터)

{{current_state_paragraph}}

- 제품/작업 상태: {{status_bullets}}
- 현재 레벨(Lv): {{current_level}}  ·  진행 중 웨이브: {{active_wave}}

**미완 / 다음 작업 (우선순위):**
1. {{next_action_1}}
2. {{next_action_2}}
3. {{next_action_3}}

**▶ 다음에 실행할 커맨드:** `{{next_command}}`

---

## 1. 확정 사항 (Decisions — 락인됨)
{{decisions}}
> User Sovereignty 기록(사용자가 직접 결정/확정한 항목): {{user_decisions}}

## 2. 완료된 단계 (Done)
{{done_steps}}

## 3. 진행 중 (In-progress) — ⚠️ 팀원은 재개되지 않음
- 진행 중 웨이브: {{active_wave}} / 활성이었던 팀원: {{active_roles}}
- 재개 시 해당 웨이브 커맨드를 **새로 실행**해 팀원을 재스폰해야 함(인수인계는 디스크 산출물로 무손실).

## 4. 리스크 / 블로커
{{risks}}

## 5. 엔진 상태 다이제스트 (`bathos … show` 기준)
- 라우팅/레벨: {{route_digest}}
- 웨이브 상태: {{wave_digest}}
- 최신 게이트 판정: {{gate_digest}}

## 6. 제품 코드 git 상태
- 브랜치: {{git_branch}} · 변경: {{git_changes}} · 마지막 커밋: {{git_last_commit}}

## 7. 핵심 산출물 경로 인덱스
- 상태: `.agent-team/_state/{manifest.json, wave-log.md, signoff.md, SESSION-SNAPSHOT.md}`
- 산출물: {{artifact_index}}
- 제품 소스: {{source_paths}}
