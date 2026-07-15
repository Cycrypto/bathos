# =============================================================================
# BATHOS M6 — freeze-guard.ps1   (Windows PowerShell 포트)
# 원본: .claude/hooks/freeze-guard.sh  [Dynamis A1 — 위험 경로 fingerprint 게이트]
# PreToolUse(Write/Edit/MultiEdit): owned_paths 밖 편집 + 위험경로 미승인 변경 차단
# =============================================================================
# 계약:  api-contracts.md §D (PreToolUse freeze), B-1 (owned_paths 불변식)
# 예외:  exceptions.md §5 E-FREEZE-VIOLATION, §4 E-PATH-COLLISION, E-FINGERPRINT-MISS
# DoD:   owned_paths 밖 편집 → exit 2(차단); owned_paths 내 또는 미설정 → exit 0
# 설정:  BATHOS_OWNED_PATHS  콜론(:) 구분 허용 경로 패턴 목록
#        BATHOS_ROLE         현재 에이전트 역할명(감사 actor)
# 안전:  거짓양성(과차단) > 거짓음성(놓침). 위험경로는 판정 불가 시 "차단"(안전 측).
# 이식 메모:
#   - jq 대신 네이티브 JSON. 원본의 "jq 없으면 차단" 분기는 "stdin 파싱 실패 시 차단"으로 대응.
#   - 위험경로 게이트는 Write/Edit/MultiEdit 별 pseudo-diff를 재구성해 엔진
#     `fingerprint check`로 승인 캐시와 대조한다(원본 §5 동일 구조).
#   - 한 프로젝트는 설치 시 bash 훅 또는 ps1 훅 중 하나만 배선되므로(install-time
#     dispatch), 지문 해시의 .sh↔.ps1 바이트 일치는 요구되지 않는다(각자 self-consistent).
# =============================================================================

. (Join-Path $PSScriptRoot '_common.ps1')

$ctx = Get-BathosContext
$parsed = Read-HookInput

# --------------------------------------------------------------------------
# 2. tool_name + file_path 추출
# --------------------------------------------------------------------------
$tool = ''
$filePath = ''
if ($null -ne $parsed) {
    $t = Get-JsonField $parsed @('tool_name');            if ($null -ne $t) { $tool = [string]$t }
    $f = Get-JsonField $parsed @('tool_input','file_path'); if ($null -ne $f) { $filePath = [string]$f }
}

# 파일 경로를 수반하지 않는 도구(Bash 등)는 통과 / Write·Edit·MultiEdit 만 검사
switch ($tool) {
    'Write'     { }
    'Edit'      { }
    'MultiEdit' { }
    ''          { }   # 도구명 미확인 → 보수적으로 계속
    default     { exit 0 }
}
if ([string]::IsNullOrEmpty($filePath)) { exit 0 }

# --------------------------------------------------------------------------
# 3. 경로 정규화: PROJECT_DIR(또는 BATHOS_ROOT) 기준 상대 경로. 슬래시로 정규화.
# --------------------------------------------------------------------------
$projSlash = ConvertTo-SlashPath $ctx.ProjectDir
$rootSlash = ConvertTo-SlashPath $ctx.BathosRoot
$fileSlash = ConvertTo-SlashPath $filePath
if ($fileSlash.StartsWith($projSlash + '/')) {
    $relPath = $fileSlash.Substring($projSlash.Length + 1)
} elseif ($fileSlash.StartsWith($rootSlash + '/')) {
    $relPath = $fileSlash.Substring($rootSlash.Length + 1)
} else {
    $relPath = $fileSlash
}

# --------------------------------------------------------------------------
# 4. 감사 append 헬퍼(로컬 클로저)
# --------------------------------------------------------------------------
$actor = $env:BATHOS_ROLE; if ([string]::IsNullOrEmpty($actor)) { $actor = 'hook:freeze' }
function script:_AppendAudit([string]$action) {
    Add-BathosAudit -Context $ctx -Action $action -Target $relPath -Actor $actor
}

# ============================================================================
# 5. [Dynamis A1] 위험 경로 fingerprint 게이트 — owned_paths 검사보다 먼저
# ============================================================================
$dangerousPatterns = @(
    '*.claude/settings.json'
    '*.claude/hooks/*'
    '*.github/workflows/*'
    '*_state/manifest.json'
)
$isDangerous = $false
foreach ($pat in $dangerousPatterns) {
    if (Test-GlobMatch -Path $relPath -Pattern $pat) { $isDangerous = $true; break }
}

if ($isDangerous) {
    # 5-0. stdin 파싱 실패 → diff 재구성 불가 → 판정 불가 → 차단(안전 측)
    if ($null -eq $parsed) {
        _AppendAudit 'fingerprint-miss-no-input'
        Write-HookError ''
        Write-HookError '[bathos freeze] x 위험 경로 변경 차단 — 입력 JSON을 파싱할 수 없어 승인 여부를 확인할 수 없습니다.'
        Write-HookError ('[bathos freeze] 대상: ' + $relPath)
        exit 2
    }

    # 5-a. pseudo-diff 재구성 (Write/Edit/MultiEdit)
    $diffLines = New-Object System.Collections.Generic.List[string]
    switch ($tool) {
        'Write' {
            $content = Get-JsonField $parsed @('tool_input','content'); if ($null -eq $content) { $content = '' }
            $diffLines.Add('--- /dev/null')
            $diffLines.Add('+++ b/' + $relPath)
            $diffLines.Add('@@ new file @@')
            foreach ($ln in ([string]$content -split "`n")) { $diffLines.Add('+' + ($ln -replace "`r$", '')) }
        }
        'Edit' {
            $oldS = Get-JsonField $parsed @('tool_input','old_string'); if ($null -eq $oldS) { $oldS = '' }
            $newS = Get-JsonField $parsed @('tool_input','new_string'); if ($null -eq $newS) { $newS = '' }
            $diffLines.Add('--- a/' + $relPath)
            $diffLines.Add('+++ b/' + $relPath)
            $diffLines.Add('@@ edit @@')
            foreach ($ln in ([string]$oldS -split "`n")) { $diffLines.Add('-' + ($ln -replace "`r$", '')) }
            foreach ($ln in ([string]$newS -split "`n")) { $diffLines.Add('+' + ($ln -replace "`r$", '')) }
        }
        'MultiEdit' {
            $edits = Get-JsonField $parsed @('tool_input','edits')
            $diffLines.Add('--- a/' + $relPath)
            $diffLines.Add('+++ b/' + $relPath)
            if ($null -ne $edits) {
                $i = 0
                foreach ($e in $edits) {
                    $oldS = $null; $newS = $null
                    if ($null -ne $e) {
                        $po = $e.PSObject.Properties['old_string']; if ($null -ne $po) { $oldS = $po.Value }
                        $pn = $e.PSObject.Properties['new_string']; if ($null -ne $pn) { $newS = $pn.Value }
                    }
                    if ($null -eq $oldS) { $oldS = '' }
                    if ($null -eq $newS) { $newS = '' }
                    $diffLines.Add('@@ edit ' + $i + ' @@')
                    foreach ($ln in ([string]$oldS -split "`n")) { $diffLines.Add('-' + ($ln -replace "`r$", '')) }
                    foreach ($ln in ([string]$newS -split "`n")) { $diffLines.Add('+' + ($ln -replace "`r$", '')) }
                    $i++
                }
            }
        }
    }

    # 5-b. 엔진 바이너리 부재 → 캐시 조회 불가 → 판정 불가 → 차단(안전 측)
    if (-not (Test-Path -LiteralPath $ctx.Bin)) {
        _AppendAudit 'fingerprint-miss-no-bin'
        Write-HookError ''
        Write-HookError '[bathos freeze] x 위험 경로 변경 차단 — 엔진 바이너리가 없어 승인 캐시를 확인할 수 없습니다.'
        Write-HookError ('[bathos freeze] 대상: ' + $relPath + ' (BATHOS_BIN=' + $ctx.Bin + ')')
        Write-HookError '[bathos freeze] cargo build -p bathos-cli 로 엔진을 빌드한 뒤 다시 시도하세요.'
        exit 2
    }

    # 5-c. 임시 diff 파일 기록 → fingerprint check
    $tmpDiff = [System.IO.Path]::GetTempFileName()
    $fpStatus = ''
    $fpHash = ''
    try {
        # UTF-8(BOM 없음)로 기록 — 엔진 normalize_diff 입력
        $enc = New-Object System.Text.UTF8Encoding($false)
        [System.IO.File]::WriteAllText($tmpDiff, ($diffLines -join "`n"), $enc)

        $fpOut = ''
        try { $fpOut = (& $ctx.Bin --state-dir $ctx.StateDir fingerprint check --diff-file $tmpDiff 2>$null | Out-String) } catch { $fpOut = '' }
        if (-not [string]::IsNullOrWhiteSpace($fpOut)) {
            try {
                $fpJson = $fpOut | ConvertFrom-Json
                $ps = $fpJson.PSObject.Properties['status']; if ($null -ne $ps) { $fpStatus = [string]$ps.Value }
                $ph = $fpJson.PSObject.Properties['hash'];   if ($null -ne $ph) { $fpHash   = [string]$ph.Value }
            } catch { }
        }
    } finally {
        try { Remove-Item -LiteralPath $tmpDiff -Force -ErrorAction SilentlyContinue } catch { }
    }

    if ($fpStatus -eq 'approved') {
        _AppendAudit 'fingerprint-hit'
        Write-HookError ('[bathos freeze] . 위험 경로 변경 — 기승인 지문 확인(재승인 생략): ' + $relPath)
        # 승인됐어도 owned_paths 검사는 계속(§6) — 소유권과 위험승인은 별개 축.
    } else {
        _AppendAudit 'fingerprint-miss'
        $hashDisp = $fpHash; if ([string]::IsNullOrEmpty($hashDisp)) { $hashDisp = '(계산 실패)' }
        Write-HookError ''
        Write-HookError '[bathos freeze] x 위험 경로 미승인 변경 — 차단되었습니다.'
        Write-HookError ('[bathos freeze] 대상: ' + $relPath)
        Write-HookError ('[bathos freeze] 지문(hash): ' + $hashDisp)
        Write-HookError '[bathos freeze] 승인 후 재시도하세요:'
        Write-HookError ('[bathos freeze]   & $BATHOS_BIN --state-dir "' + $ctx.StateDir + '" fingerprint approve --hash ' + $hashDisp + ' --actor <역할> --scope <hooks|ci|w5-entry 등>')
        Write-HookError '[bathos freeze] 동일 변경(공백/줄바꿈 차이 포함)은 승인 후 재승인 없이 통과합니다.'
        Write-HookError '[bathos freeze] 참고: exceptions.md E-FINGERPRINT-MISS, story-a1-guard-hardening-kr.md AC2/AC3'
        exit 2
    }
}

# --------------------------------------------------------------------------
# 6. BATHOS_OWNED_PATHS 미설정 시 freeze 비활성 (위험경로 게이트와 별개)
# --------------------------------------------------------------------------
$ownedRaw = $env:BATHOS_OWNED_PATHS
if ([string]::IsNullOrEmpty($ownedRaw)) { exit 0 }

# --------------------------------------------------------------------------
# 7~8. owned_paths 패턴 매칭 (콜론 구분) → 소유 경로 검사
#   "prefix/**" → prefix 이하 전체 / 그 외 → glob 매칭. brace 확장 없음(원본 제약).
# --------------------------------------------------------------------------
$ownedPatterns = $ownedRaw -split ':'
$isOwned = $false
foreach ($pattern in $ownedPatterns) {
    if ([string]::IsNullOrEmpty($pattern)) { continue }
    if (Test-GlobMatch -Path $relPath -Pattern $pattern) { $isOwned = $true; break }
}

if ($isOwned) { exit 0 }

# 소유 경로 밖 → 차단
_AppendAudit 'freeze-violation'
Write-HookError ''
Write-HookError '[bathos freeze] x 소유 경로 밖 편집이 차단되었습니다.'
Write-HookError ('[bathos freeze] 대상 파일: ' + $relPath)
Write-HookError '[bathos freeze] 허용 경로 (BATHOS_OWNED_PATHS):'
foreach ($p in $ownedPatterns) {
    if (-not [string]::IsNullOrEmpty($p)) { Write-HookError ('[bathos freeze]   - ' + $p) }
}
Write-HookError '[bathos freeze] 소유 경로 밖을 수정해야 한다면:'
Write-HookError '[bathos freeze]   1. 해당 경로 소유자와 메시지로 합의하거나'
Write-HookError '[bathos freeze]   2. Paul(리드)에게 보고하십시오.'
Write-HookError '[bathos freeze] 참고: exceptions.md §5 E-FREEZE-VIOLATION, §4 E-PATH-COLLISION'
exit 2  # ← 하드 차단
