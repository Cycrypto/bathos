# BATHOS × GLM — GLM 백엔드로 구동하기

> **성격: 이식이 아니라 "모델 스왑".** Claude Code 런타임을 그대로 두고 백엔드 모델만 GLM(Zhipu/Z.ai)으로 바꾼다. Agent Teams·훅·슬래시 커맨드·MCP·`bathos` 엔진이 **100% 그대로** 동작한다.
> 전제: Z.ai의 **Anthropic 호환 엔드포인트**. (공식 Anthropic이 아니라 Z.ai 호환 레이어 — 내부에서 Claude 모델명을 GLM으로 매핑.)
> ⚠️ 엔드포인트·모델 매핑·플랜 정책은 버전에 따라 변함 → 실사용 전 `docs.z.ai/scenario-example/develop-tools/claude` 재확인.

---

## 1. 3분 셋업

1. **Z.ai API 키 발급** — Z.ai(z.ai) GLM Coding Plan 가입 후 API 키 생성.
2. **환경변수 설정** (아래 둘 필수):
   ```bash
   export ANTHROPIC_BASE_URL="https://api.z.ai/api/anthropic"
   export ANTHROPIC_AUTH_TOKEN="<발급받은 Z.ai 키>"
   export API_TIMEOUT_MS="3000000"   # 선택: 긴 에이전트 턴 대비
   ```
   또는 저장소 헬퍼: `source scripts/glm-env.sh`(키는 `Z_AI_API_KEY` 환경변수로 주입 — 파일에 키를 넣지 말 것).
3. **Claude Code 실행** — 평소처럼 `claude` 실행. 이제 모델 호출이 GLM으로 나간다.
4. **확인** — 아무 프롬프트나 던져 응답이 오면 연결 성공. `/status`(또는 `/config`)에서 엔드포인트 확인.

---

## 2. BATHOS 특이점 — 모델 필드 매핑 (유일한 적응 포인트)

BATHOS 에이전트 정의(`.claude/agents/_base/*.md`)의 `model` 필드는 **`claude-fable-5`·`claude-sonnet-5`**(비표준 ID)를 씁니다. Z.ai 호환 레이어는 표준 Claude **별칭**(opus/sonnet/haiku)을 GLM으로 매핑하므로, 다음 중 하나를 권장:

- **(권장·무변경) `/config`의 Default teammate model** 을 지정 → 팀원 스폰 시 그 모델 사용. 저장소 파일 수정 불필요.
- **(대안) 별칭으로 매핑** — teammate 모델을 `opus`/`sonnet`/`haiku` 별칭으로 두면 Z.ai가 GLM으로 매핑.
- Z.ai 기본 매핑 예시(문서 기준, 변동 가능): **Opus/Sonnet → GLM-4.7**, **Haiku → GLM-4.5-Air**. 최신 코딩 모델은 **GLM-5.2(1M 컨텍스트)**. → 기본 매핑을 유지하면 플랜이 최신 모델로 자동 갱신됨.

> 원칙: 저장소의 정본 `model` 필드(`claude-*`)는 **Claude 실행용 기본값으로 보존**하고, GLM 구동은 **환경변수 + `/config`** 로만 전환하는 것을 권장(코드 무변경·가역).

---

## 3. 무엇이 되고 무엇을 주의하나

| 항목 | GLM 백엔드에서 |
|------|----------------|
| Agent Teams(웨이브 팀 스폰) | ✅ 그대로 |
| 훅(careful/freeze/gate/SessionStart/SessionEnd) | ✅ 그대로(`bathos` 바이너리 로컬 존재 전제) |
| 슬래시 커맨드 40+ | ✅ 그대로 |
| MCP(pencil 등) | ✅ 그대로 |
| `bathos` 엔진(게이트·감사·상태) | ✅ 그대로 |
| 도구 호출/긴 컨텍스트 | ◐ GLM-4.6/5.2 실사용 가능 수준(멀티턴 실무에서 Sonnet과 사실상 대등하다는 자체보고). 정밀 head-to-head는 근거 제한 |
| 품질/판정 일관성 | ⚠️ 모델이 다르므로 게이트 판정·리뷰 톤이 Claude와 다를 수 있음 → 중요한 게이트는 결과 재검토 권장 |
| 비용/할당량 | ⚠️ Z.ai 티어(Lite/Pro/Max)별 할당량 |

---

## 4. 되돌리기

환경변수만 제거(또는 새 셸)하면 즉시 Claude(Anthropic)로 복귀. 저장소 변경이 없으므로 완전 가역.
```bash
unset ANTHROPIC_BASE_URL ANTHROPIC_AUTH_TOKEN API_TIMEOUT_MS
```

## 출처
- Z.ai × Claude Code 공식 가이드: https://docs.z.ai/scenario-example/develop-tools/claude
- GLM Coding Plan × Claude Code: https://codingplan.run/guides/claude-code-with-glm
- GLM-5.2(1M ctx): https://www.marktechpost.com/2026/06/14/z-ai-launches-glm-5-2-... · GLM-4.6(200K): https://docs.z.ai/guides/llm/glm-4.6
