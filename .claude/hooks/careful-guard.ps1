# =============================================================================
# BATHOS M6 — careful-guard.ps1   (Windows PowerShell 포트)
# 원본: .claude/hooks/careful-guard.sh  [Dynamis A1 — 위험 경로 Bash 우회 하드닝]
# PreToolUse(Bash): 파괴적 명령을 실행 전에 하드 차단 + 사용자 확인 요구
# =============================================================================
# 계약:  api-contracts.md §D (PreToolUse careful-guard)
# 예외:  exceptions.md §5 E-DESTRUCTIVE
# DoD:   파괴명령 → exit 2(차단) + stderr 피드백; 정상 명령 → exit 0(통과)
# 안전:  거짓양성(과차단) > 거짓음성(놓침) — ETHOS careful 원칙
# 이식 메모:
#   - ERE(grep -Ei) 패턴을 .NET 정규식(대소문자 무시)으로 옮김:
#       [[:space:]]→\s, [[:alpha:]]→[a-zA-Z], [[:alnum:]_"`.]→[\w"`.]
#   - .NET 정규식은 부정 전방탐색을 지원하나, 원본과의 동작 동일성을 위해
#     DELETE-without-WHERE 는 원본처럼 2단계로 유지한다.
#   - Windows에서 이 훅은 Bash 도구(Git Bash 유무 무관)의 명령 문자열을 검사한다.
# =============================================================================

. (Join-Path $PSScriptRoot '_common.ps1')

$ctx = Get-BathosContext
$parsed = Read-HookInput

$cmd  = ''
$tool = ''
if ($null -ne $parsed) {
    $c = Get-JsonField $parsed @('tool_input','command'); if ($null -ne $c) { $cmd  = [string]$c }
    $t = Get-JsonField $parsed @('tool_name');            if ($null -ne $t) { $tool = [string]$t }
}

# Bash 도구가 아니면 통과 (도구명 확인 가능한 경우만)
if (($tool -ne '') -and ($tool -ne 'Bash')) { exit 0 }
# 명령이 비어 있으면 통과
if ([string]::IsNullOrEmpty($cmd)) { exit 0 }

# --------------------------------------------------------------------------
# 3. 위험 패턴 목록 (.NET 정규식, 대소문자 무시)
# --------------------------------------------------------------------------
$dangerPatterns = @(
    # --- 파일시스템 파괴 ---
    'rm\s+-[a-zA-Z]*r[a-zA-Z]*f(\s|$)'        # rm -rf, rm -fr, rm -Rf 등
    'rm\s+-[a-zA-Z]*f[a-zA-Z]*r(\s|$)'        # rm -fr 변형
    'rm\s+-rf\s*/([^/]|$)'                     # rm -rf / (루트)
    'rm\s+-rf\s+~'                             # rm -rf ~ (홈)
    'rm\s+-rf\s+\.'                            # rm -rf . (현재 디렉터리)
    'rm\s+-rf\s+\*'                            # rm -rf * (와일드카드)

    # --- 디스크 초기화 / 덮어쓰기 ---
    'mkfs\.'                                   # mkfs.ext4 등
    'dd\s+if='                                 # dd if=... (디스크 덮어쓰기)
    ':>\s*/[^\s]'                              # :> /path (파일 덮어쓰기)
    'shred\s'                                  # shred (복구불가 삭제)

    # --- SQL 파괴적 DML/DDL ---
    'DROP\s+(TABLE|DATABASE|SCHEMA|INDEX)'     # DROP 계열
    'TRUNCATE\s+TABLE'                         # TRUNCATE (전체 삭제, 롤백 불가)

    # --- git 파괴적 명령 ---
    'git\s+push\s+.*--force'                   # git push --force
    'git\s+push\s+.*-f(\s|$)'                  # git push -f
    'git\s+push\s+.*-f\s'                      # git push -f <remote>
    'git\s+reset\s+--hard'                     # git reset --hard
    'git\s+checkout\s+-[Bf]'                   # git checkout -B/-f
    'git\s+clean\s+-f'                         # git clean -f (미추적 파일 삭제)
    'git\s+branch\s+-D'                        # git branch -D (강제 삭제)

    # --- 권한/소유권 위험 ---
    'chmod\s+-R\s+777'                         # chmod -R 777
    'chown\s+-R\s+(root|0)'                    # chown -R root

    # --- sudo 위험 조합 ---
    'sudo\s+(rm|dd|mkfs|shred|chmod|chown)'    # sudo + 파괴 명령

    # --- [Dynamis A1] 위험 경로 Bash 우회 변경 (risk-log A-3, 완전성 미주장) ---
    '>\s*[^&|;]*\.claude/settings\.json'                                              # settings.json 리다이렉트 덮어쓰기
    '>>?\s*[^&|;]*\.claude/hooks/'                                                    # hooks/ 아래 파일 리다이렉트
    'sed\s+-i[^|;]*\.claude/(settings\.json|hooks/)'                                  # sed -i로 훅/설정 직접 수정
    '(curl|wget)\s[^|;]*(-o|-O|--output)\s*[^&|;]*\.claude/(settings\.json|hooks/)'   # curl/wget으로 훅/설정 덮어쓰기
    '(printf|echo)[^|;]*>\s*[^&|;]*_state/manifest\.json'                             # manifest.json 직접 리다이렉트 쓰기
    'jq[^|;]*>\s*[^&|;]*_state/manifest\.json'                                        # jq 파이프로 manifest.json 우회 편집
    '(cp|mv)\s+[^|;]+\s+[^&|;]*\.claude/(settings\.json|hooks/)'                      # cp/mv로 훅/설정 교체
    '>>?\s*[^&|;]*\.github/workflows/'                                                # CI 워크플로 리다이렉트 쓰기
)

# --------------------------------------------------------------------------
# 4. 패턴 검사
# --------------------------------------------------------------------------
$detected = ''
foreach ($pat in $dangerPatterns) {
    if ([regex]::IsMatch($cmd, $pat, [System.Text.RegularExpressions.RegexOptions]::IgnoreCase)) {
        $detected = $pat
        break
    }
}

# --------------------------------------------------------------------------
# 4b. WHERE 없는 DELETE 2단계 검사 (L-4 보강)
#   1단계: DELETE FROM <식별자> 존재  →  2단계: WHERE 절 미존재 → 위험
# --------------------------------------------------------------------------
if ([string]::IsNullOrEmpty($detected)) {
    $ic = [System.Text.RegularExpressions.RegexOptions]::IgnoreCase
    if ([regex]::IsMatch($cmd, 'DELETE\s+FROM\s+[\w"`.]+', $ic)) {
        if (-not [regex]::IsMatch($cmd, '\sWHERE\s', $ic)) {
            $detected = 'DELETE FROM ... (WHERE 없는 전체 삭제 — L-4 보강)'
        }
    }
}

# --------------------------------------------------------------------------
# 5~6. 차단 또는 통과 (감사 append 동반)
# --------------------------------------------------------------------------
if (-not [string]::IsNullOrEmpty($detected)) {
    $actor = $env:BATHOS_ROLE; if ([string]::IsNullOrEmpty($actor)) { $actor = 'hook:careful' }
    Add-BathosAudit -Context $ctx -Action 'block' -Target $cmd -Actor $actor

    $head = $cmd; if ($head.Length -gt 300) { $head = $head.Substring(0,300) }
    Write-HookError ''
    Write-HookError '[bathos careful] x 파괴적/위험경로 우회 명령이 감지되어 차단합니다.'
    Write-HookError ('[bathos careful] 명령: ' + $head)
    Write-HookError ('[bathos careful] 감지 패턴: ' + $detected)
    Write-HookError '[bathos careful] 이 명령을 실행하려면:'
    Write-HookError '[bathos careful]   1. 의도를 사용자에게 설명하고'
    Write-HookError '[bathos careful]   2. 영향 범위(대상 파일/DB/저장소)를 명시하고'
    Write-HookError '[bathos careful]   3. 롤백 방법을 제시한 뒤'
    Write-HookError '[bathos careful]   4. 사용자의 명시적 승인을 받으십시오.'
    Write-HookError '[bathos careful] 위험경로(설정/훅/CI/manifest) 변경은 Write/Edit 도구 +'
    Write-HookError '[bathos careful] `bathos fingerprint approve`로 정식 승인 절차를 거치십시오.'
    Write-HookError '[bathos careful] 참고: exceptions.md §5 E-DESTRUCTIVE (Bash 경로는 완전 차단 미보장 — AC2)'
    exit 2  # ← 하드 차단
}

# 정상 통과 (감사 로그는 audit-log.ps1 PostToolUse 훅이 기록)
exit 0
