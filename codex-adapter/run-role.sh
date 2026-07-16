#!/usr/bin/env bash
# =============================================================================
# BATHOS  codex-adapter/run-role.sh  —  Codex 런타임 위임 러너 (ADR-D-0006, §A3.3)
#
# 사용:
#   run-role.sh <role-slug> <task-file.md> [--project <절대경로>] [--dry-run]
#
# exit 0=완료 / 1=실행 실패 / 3=E-CODEX-ABSENT(codex 미설치) / 4=E-CODEX-AUTH(미인증)
#
# 무엇을 하는가: `runtime=codex`로 배정된 역할은 Claude Code 팀원이 아니다(별도
# 프로세스) — 웨이브 커맨드가 이 스크립트로 위임한다. 역할 base md 본문 +
# ETHOS 요지 + 웨이브 커맨드가 작성한 task-file(입력 경로·소유 경로·DoD)을 하나의
# 프롬프트로 조립해 `codex exec`에 비대화로 넘기고, 완료/실패를
# `_state/panes/inbox/codex-<slug>-<ts>.txt`에 기록한다(Paul/패널이 폴링 감지).
#
# 정직한 한계(⚠️[추정] — 설계 §A3.3/R-2와 동일 공백): `codex exec`의 비대화 계약과
# `codex login status`의 서브커맨드 명·출력 형식은 이 저장소에서 라이브 인증
# 세션으로 실측되지 않았다. 따라서 인증 프리플라이트는 **차단적이지 않다** —
# 상태 확인이 모호하거나(서브커맨드 자체 오류) 실패해도 예단하여 막지 않고
# 실제 실행을 시도한 뒤, 그 결과(exit code·출력)에서 인증 실패 신호를 찾아야만
# E-CODEX-AUTH로 분류한다(과차단보다 오진단이 더 나쁘다 — 날조 금지 원칙).
#
# 이식성: bash 3.2(macOS 기본) 호환 — mapfile/연관배열(`declare -A`)/`${var,,}`/`&>`
# 금지. jq 금지 — `bathos model resolve --json`은 단일행 JSON이라 grep/sed로 충분.
# =============================================================================
set -uo pipefail

PROG="run-role.sh"

say()  { printf '[%s] %s\n' "$PROG" "$*"; }
warn() { printf '[%s] ⚠ %s\n' "$PROG" "$*" >&2; }
err()  { printf '[%s] ✗ %s\n' "$PROG" "$*" >&2; }

usage() {
  cat <<'EOF'
사용: run-role.sh <role-slug> <task-file.md> [--project <절대경로>] [--dry-run]

  role-slug     agent 정의 slug (예: thomas-code-reviewer) — .claude/agents/_base/*.md
                프론트매터의 `slug:` 값과 일치해야 한다.
  task-file.md  웨이브 커맨드가 작성한 태스크 브리프(입력 경로·소유 경로·DoD).

옵션:
  --project <DIR>   대상 프로젝트 절대경로(기본: 이 스크립트의 상위 디렉터리)
  --dry-run         codex exec를 실제로 실행하지 않고 프리플라이트+프롬프트 조립까지만
                     수행한다(조립된 프롬프트 경로를 stdout에 출력, exit 0).

exit 0=완료 / 1=실행 실패(TOML 부재 포함) / 3=E-CODEX-ABSENT(codex 미설치) /
     4=E-CODEX-AUTH(미인증, 실행 결과에서 인증 실패 신호 확인된 경우만)
EOF
}

# --------------------------------------------------------------------------
# 1. 인자 파싱
# --------------------------------------------------------------------------
ROLE_SLUG=""
TASK_FILE=""
PROJECT_DIR=""
DRY_RUN=0

while [ $# -gt 0 ]; do
  case "$1" in
    --project)
      shift
      PROJECT_DIR="${1:-}"
      ;;
    --dry-run) DRY_RUN=1 ;;
    -h|--help) usage; exit 0 ;;
    --)
      shift
      while [ $# -gt 0 ]; do
        if [ -z "$ROLE_SLUG" ]; then ROLE_SLUG="$1"
        elif [ -z "$TASK_FILE" ]; then TASK_FILE="$1"
        fi
        shift
      done
      ;;
    -*)
      err "알 수 없는 옵션: $1"
      usage >&2
      exit 1
      ;;
    *)
      if [ -z "$ROLE_SLUG" ]; then
        ROLE_SLUG="$1"
      elif [ -z "$TASK_FILE" ]; then
        TASK_FILE="$1"
      else
        err "과다 인자: $1"
        exit 1
      fi
      ;;
  esac
  shift
done

if [ -z "$ROLE_SLUG" ] || [ -z "$TASK_FILE" ]; then
  usage >&2
  exit 1
fi

# slug 형식 검증 — bathos-state의 Runtime::parse와 동일 관례(영문 소문자/숫자/-/_만).
# 형식이 나쁘면 이후 파일 경로 조합(agent md 탐색·inbox 파일명)에 안전하지 않을 수
# 있으므로 여기서 조기에 거부한다(model_plan.rs E-MODEL-ROLE-UNKNOWN과 대칭).
case "$ROLE_SLUG" in
  *[!a-zA-Z0-9_-]*)
    err "[E-CODEX-ROLE-INVALID] role-slug 형식 불량: $ROLE_SLUG (영문/숫자/-/_ 만 허용)"
    exit 1
    ;;
esac

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DEFAULT_PROJECT="$(cd "$SCRIPT_DIR/.." && pwd)"
PROJECT_DIR="${PROJECT_DIR:-$DEFAULT_PROJECT}"

if [ ! -d "$PROJECT_DIR" ]; then
  err "--project 경로 없음: $PROJECT_DIR"
  exit 1
fi
# 심볼릭 링크·상대 표기가 뒤섞이지 않도록 정규화(inbox 경로 조합 전 1회만).
PROJECT_DIR="$(cd "$PROJECT_DIR" && pwd)"

if [ ! -f "$TASK_FILE" ] || [ ! -r "$TASK_FILE" ]; then
  err "task-file 읽기 불가: $TASK_FILE"
  exit 1
fi

STATE_DIR="$PROJECT_DIR/.agent-team/_state"
INBOX_DIR="$STATE_DIR/panes/inbox"
AGENTS_DIR="$PROJECT_DIR/.claude/agents/_base"
ETHOS_FILE="$PROJECT_DIR/ETHOS.md"

# --------------------------------------------------------------------------
# 2. inbox 완료 마커 헬퍼 (§A3.3 step5 — bathos-tui inbox.rs와 동일 ts-pid·
#    atomic tmp→mv 관례. 이 함수는 실행 경로의 모든 종료 지점(성공/실패 모두)
#    에서 호출되어야 "장시간 무소식"과 "확인된 실패"를 패널이 구분할 수 있다.)
# --------------------------------------------------------------------------
write_inbox_marker() {
  local status="$1" summary="$2"
  mkdir -p "$INBOX_DIR" 2>/dev/null || { warn "inbox 디렉터리 생성 실패: $INBOX_DIR"; return 0; }
  local ts
  ts="$(date +%s)-$$"
  local f="$INBOX_DIR/codex-$ROLE_SLUG-$ts.txt"
  # 첫 줄 요약만 취한다(파일 프로토콜은 1줄 — 스크립트 소비 계약, §0.1).
  local one_line
  one_line="$(printf '%s' "$summary" | tr '\n' ' ' | head -c 500)"
  printf '%s\t%s\n' "$status" "$one_line" > "$f.tmp" || return 0
  # PANES-004: restrict to owner-only before the rename — this file previously inherited the
  # process umask (typically 0644/world-readable); it can carry a codex exec failure summary
  # that a reviewer may not intend other local accounts on a shared host to read.
  chmod 600 "$f.tmp" 2>/dev/null || true
  mv "$f.tmp" "$f"
  say "완료 마커 기록됨: $(basename "$f")"
}

# --------------------------------------------------------------------------
# 3. 프리플라이트 — codex CLI 존재 (E-CODEX-ABSENT)
# --------------------------------------------------------------------------
CODEX_BIN="${CODEX_BIN:-}"
if [ -z "$CODEX_BIN" ] && command -v codex >/dev/null 2>&1; then
  CODEX_BIN="$(command -v codex)"
fi

if [ -z "$CODEX_BIN" ]; then
  cat >&2 <<EOF
[$PROG] [E-CODEX-ABSENT] codex CLI를 찾을 수 없습니다(PATH에 'codex' 없음).
설치 후 재시도하세요: https://developers.openai.com/codex (Codex CLI)
또는 이 역할을 runtime=claude로 재배정하세요(bathos model set $ROLE_SLUG --runtime claude).
EOF
  write_inbox_marker "FAIL" "codex CLI 미설치(E-CODEX-ABSENT)"
  exit 3
fi

# --------------------------------------------------------------------------
# 4. 프리플라이트 — 인증 상태 (best-effort, 비차단 — 위 헤더 주석의 "정직한 한계" 참고)
# --------------------------------------------------------------------------
# `codex login status`의 정확한 서브커맨드 명·정상/비정상 출력 형태는 라이브
# 인증 세션으로 미실측이다(⚠️[추정]). 그래서 이 단계는 "명백히 미인증"이라고
# 읽히는 출력(패턴 매치)일 때만 선제 차단하고, 그 외(서브커맨드 자체가 없거나
# 애매한 실패)는 경고만 남기고 실제 실행으로 넘어간다 — 실행 결과에서 인증
# 실패가 다시 나타나면 §5에서 최종적으로 E-CODEX-AUTH로 분류한다.
AUTH_HINT=""
if AUTH_OUT="$("$CODEX_BIN" login status 2>&1)"; then
  : # 정상 — 통과
else
  AUTH_HINT="$AUTH_OUT"
  if printf '%s' "$AUTH_OUT" | grep -Eiq 'not[ _-]?logged[ _-]?in|unauthenticated|no.*credentials|please.*login|please.*auth'; then
    cat >&2 <<EOF
[$PROG] [E-CODEX-AUTH] codex 미인증 상태로 보입니다(codex login status):
$AUTH_OUT
'codex login'으로 인증한 뒤 재시도하세요.
EOF
    write_inbox_marker "FAIL" "codex 미인증(E-CODEX-AUTH): $(printf '%s' "$AUTH_OUT" | head -1)"
    exit 4
  fi
  warn "codex login status 확인 불가(서브커맨드 미지원 가능) — 본실행으로 진행: $AUTH_OUT"
fi

# --------------------------------------------------------------------------
# 5. 역할 정의 확보 — .codex/agents/<slug>.toml (프로젝트 로컬 우선, 자동 생성 안 함)
# --------------------------------------------------------------------------
# 탐색 순서: 프로젝트 로컬(`docs/codex-adapter-kr.md` §2 관례) → 사용자 홈(옵션 --dest
# 로 ~/.codex를 택한 경우). 둘 다 없으면 exit 1 — `to-codex.sh --write`는 홈
# 디렉터리에도 쓸 수 있는 도구라 **자동 실행하지 않는다**(사용자 승인 사안, §A3.3 step2).
TOML_PATH=""
for cand in \
  "$PROJECT_DIR/.codex/agents/$ROLE_SLUG.toml" \
  "$HOME/.codex/agents/$ROLE_SLUG.toml"
do
  if [ -f "$cand" ]; then
    TOML_PATH="$cand"
    break
  fi
done

if [ -z "$TOML_PATH" ]; then
  cat >&2 <<EOF
[$PROG] [E-CODEX-TOML-ABSENT] '$ROLE_SLUG'의 Codex 에이전트 정의(.toml)를 찾을 수
없습니다. 확인한 경로:
  - $PROJECT_DIR/.codex/agents/$ROLE_SLUG.toml
  - $HOME/.codex/agents/$ROLE_SLUG.toml
생성 안내(자동 실행하지 않음 — 검토 후 직접 실행하세요):
  bash "$PROJECT_DIR/scripts/to-codex.sh" --write --dest "$PROJECT_DIR/.codex"
위 명령은 .claude/agents/_base/*.md 전체를 .codex/agents/*.toml로 변환합니다
(model 필드는 TODO로 비워지므로 실행 전 직접 채워 넣어야 합니다 — docs/codex-adapter-kr.md §2).
EOF
  write_inbox_marker "FAIL" "codex 역할 정의(.toml) 부재: $ROLE_SLUG"
  exit 1
fi
say "역할 정의 확보: $TOML_PATH"

# --------------------------------------------------------------------------
# 6. 역할 base md 확보 (프롬프트 조립 원본 — to-codex.sh의 slug 탐색과 동일 관례)
# --------------------------------------------------------------------------
find_agent_base_md() {
  local slug="$1" f
  [ -d "$AGENTS_DIR" ] || return 1
  for f in "$AGENTS_DIR"/*.md; do
    [ -e "$f" ] || continue
    if grep -Eq "^slug:[[:space:]]*$slug([[:space:]]|#|\$)" "$f"; then
      printf '%s' "$f"
      return 0
    fi
  done
  return 1
}

AGENT_MD="$(find_agent_base_md "$ROLE_SLUG" || true)"
if [ -z "$AGENT_MD" ]; then
  err "[E-CODEX-ROLE-INVALID] '$ROLE_SLUG' 프론트매터를 가진 agent 정의를 $AGENTS_DIR 에서 찾지 못했습니다."
  write_inbox_marker "FAIL" "agent base md 부재: $ROLE_SLUG"
  exit 1
fi
say "역할 base md: $AGENT_MD"

# 프론트매터(--- ... ---) 다음 본문만 취한다(to-codex.sh body_after_fm과 동일 규칙).
body_after_fm() { awk 'p{print} /^---[[:space:]]*$/{c++; if(c==2)p=1}' "$1"; }

# --------------------------------------------------------------------------
# 7. 프롬프트 조립 — 역할 본문 + ETHOS 요지 + task-file (§A3.3 step3)
# --------------------------------------------------------------------------
PROMPT_FILE="$(mktemp "${TMPDIR:-/tmp}/bathos-codex-prompt.XXXXXX" 2>/dev/null || mktemp -t bathos-codex-prompt)"
cleanup_prompt() { rm -f "$PROMPT_FILE" 2>/dev/null || true; }
trap cleanup_prompt EXIT

{
  printf '# BATHOS Codex 위임 프롬프트 — role=%s\n\n' "$ROLE_SLUG"
  printf '## 역할 지침 (%s)\n\n' "$(basename "$AGENT_MD")"
  body_after_fm "$AGENT_MD"
  printf '\n\n## ETHOS (모든 역할의 작업 전제 — User Sovereignty·Boil the Ocean·Search Before Building)\n\n'
  if [ -f "$ETHOS_FILE" ]; then
    cat "$ETHOS_FILE"
  else
    printf '(ETHOS.md를 찾을 수 없음 — %s 확인 필요)\n' "$ETHOS_FILE"
  fi
  printf '\n\n## 이번 태스크 브리프 (%s)\n\n' "$(basename "$TASK_FILE")"
  cat "$TASK_FILE"
  printf '\n\n## 완료 계약\n\n'
  printf -- '- 산출물은 이 태스크 브리프에 명시된 소유 경로 파일로만 남긴다(디스크 SSOT).\n'
  printf -- '- 이 실행의 완료/실패는 run-role.sh가 `_state/panes/inbox/codex-%s-<ts>.txt`에\n' "$ROLE_SLUG"
  printf -- '  자동 기록한다(별도 self-report 불필요).\n'
} > "$PROMPT_FILE"

say "프롬프트 조립 완료: $PROMPT_FILE ($(wc -l < "$PROMPT_FILE" | tr -d ' ') 줄)"

if [ "$DRY_RUN" = "1" ]; then
  say "--dry-run: codex exec를 실행하지 않습니다. 조립된 프롬프트를 유지합니다."
  # PANES-005 fix: the debug copy used to live forever under ${TMPDIR:-/tmp} (no owner, no TTL,
  # no cleanup policy — only the *original* $PROMPT_FILE was covered by the EXIT trap above).
  # Relocate it into this project's own `_state/panes/processed/` — a directory this design
  # already gives a lifecycle to (Paul moves consumed inbox items there) — instead of the
  # shared OS temp root. The filename intentionally does NOT start with confirm-/feedback-/
  # answer-/codex- (the four prefixes the inbox contract, §B2, defines) so it can never be
  # mistaken for a live inbox/processed item by anything polling those prefixes.
  PROCESSED_DIR="$STATE_DIR/panes/processed"
  KEPT_PATH="$PROCESSED_DIR/dryrun-prompt-$ROLE_SLUG-$(date +%s)-$$.kept"
  if mkdir -p "$PROCESSED_DIR" 2>/dev/null && cp "$PROMPT_FILE" "$KEPT_PATH" 2>/dev/null; then
    chmod 600 "$KEPT_PATH" 2>/dev/null || true
  else
    # Fall back to the old location if the project directory isn't writable for some reason —
    # still better than silently losing the debug artifact.
    KEPT_PATH="$PROMPT_FILE.kept"
    cp "$PROMPT_FILE" "$KEPT_PATH" 2>/dev/null || true
    chmod 600 "$KEPT_PATH" 2>/dev/null || true
  fi
  say "보존된 프롬프트: $KEPT_PATH (TTL 없음 — 수동 정리 필요, _state/panes/processed/ 주기적 정리 권장)"
  write_inbox_marker "DRY-RUN" "dry-run 완료, 프롬프트=$KEPT_PATH"
  exit 0
fi

# --------------------------------------------------------------------------
# 8. model/reasoning_effort plan 오버라이드 조회 (bathos model resolve --json)
# --------------------------------------------------------------------------
# bathos가 없어도(빌드 안 됐거나 PATH 밖) 이 단계는 치명적이지 않다 — plan
# 오버라이드가 없다고 보고 codex 설치본 기본 모델로 진행한다(§A1.2 codex 행:
# "명시 지정은 사용자가 아는 유효 ID를 입력할 때만" — 조회 실패를 임의 모델로
# 채우지 않는다, 날조 금지).
find_bathos_bin() {
  local cand
  if [ -n "${BATHOS_BIN:-}" ] && [ -x "${BATHOS_BIN:-}" ]; then
    printf '%s' "$BATHOS_BIN"; return 0
  fi
  for cand in "$PROJECT_DIR/core/target/release/bathos" "$PROJECT_DIR/core/target/debug/bathos"; do
    if [ -x "$cand" ]; then printf '%s' "$cand"; return 0; fi
  done
  if command -v bathos >/dev/null 2>&1; then command -v bathos; return 0; fi
  return 1
}

MODEL_OVERRIDE=""
EFFORT_OVERRIDE=""
BATHOS_BIN_RESOLVED="$(find_bathos_bin || true)"
if [ -n "$BATHOS_BIN_RESOLVED" ]; then
  RESOLVE_JSON="$("$BATHOS_BIN_RESOLVED" -s "$STATE_DIR" model --root "$PROJECT_DIR" resolve "$ROLE_SLUG" --json 2>/dev/null || true)"
  if [ -n "$RESOLVE_JSON" ]; then
    # 단일행 compact JSON(bathos-cli가 이렇게 방출) — jq 없이 grep으로 값만 추출.
    MODEL_OVERRIDE="$(printf '%s' "$RESOLVE_JSON" | grep -o '"model"[[:space:]]*:[[:space:]]*"[^"]*"' | head -1 | sed 's/.*:"\(.*\)"$/\1/')"
    EFFORT_OVERRIDE="$(printf '%s' "$RESOLVE_JSON" | grep -o '"reasoning_effort"[[:space:]]*:[[:space:]]*"[^"]*"' | head -1 | sed 's/.*:"\(.*\)"$/\1/')"
  fi
else
  warn "bathos 바이너리를 찾지 못해 model-plan 오버라이드 조회를 건너뜁니다(codex 설치본 기본값 사용)."
fi

# --------------------------------------------------------------------------
# 9. 비대화 실행 (§A3.3 step4, ⚠️[추정 — 라이브 미실측])
# --------------------------------------------------------------------------
PROMPT_CONTENT="$(cat "$PROMPT_FILE")"
CODEX_ARGS=(exec)
[ -n "$MODEL_OVERRIDE" ] && CODEX_ARGS+=(-c "model=\"$MODEL_OVERRIDE\"")
[ -n "$EFFORT_OVERRIDE" ] && CODEX_ARGS+=(-c "model_reasoning_effort=\"$EFFORT_OVERRIDE\"")
CODEX_ARGS+=("$PROMPT_CONTENT")

say "codex exec 실행 중... (model override=${MODEL_OVERRIDE:-미지정}, effort=${EFFORT_OVERRIDE:-미지정})"
CODEX_OUT="$("$CODEX_BIN" "${CODEX_ARGS[@]}" 2>&1)"
CODEX_RC=$?

if [ "$CODEX_RC" -eq 0 ]; then
  say "codex exec 완료 (exit 0)"
  write_inbox_marker "DONE" "codex exec 완료: $(printf '%s' "$CODEX_OUT" | tail -1)"
  exit 0
fi

# 실행이 실패했을 때만 사후적으로 인증 실패 신호를 재검사한다(§4에서 선제
# 차단하지 않은 애매한 케이스의 최종 판정 지점 — 정직한 한계 문단 참고).
if printf '%s' "$CODEX_OUT" | grep -Eiq 'not[ _-]?logged[ _-]?in|unauthenticated|unauthorized|please.*login|please.*auth'; then
  err "[E-CODEX-AUTH] codex exec가 인증 실패로 보이는 오류로 종료(exit $CODEX_RC):"
  printf '%s\n' "$CODEX_OUT" >&2
  write_inbox_marker "FAIL" "codex exec 인증 실패(E-CODEX-AUTH), exit=$CODEX_RC"
  exit 4
fi

err "codex exec 실패 (exit $CODEX_RC):"
printf '%s\n' "$CODEX_OUT" >&2
write_inbox_marker "FAIL" "codex exec 실패, exit=$CODEX_RC: $(printf '%s' "$CODEX_OUT" | tail -1)"
exit 1
