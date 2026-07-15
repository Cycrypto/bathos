# =============================================================================
# BATHOS Dynamis A2 — scope-inject.ps1   (Windows PowerShell 포트)
# 원본: .claude/hooks/scope-inject.sh
# PreToolUse(Read/Edit/Write/MultiEdit): 상위 디렉터리 스코프 규칙 자동 부착(advisory)
# =============================================================================
# 계약:  story-a2-scope-inject-kr.md, hook-and-gate-design-kr.md §2
# 역할 분리(핵심): scope-inject = 가시화(advisory, 차단 안 함) / freeze-guard = 강제(차단).
#   이 훅은 어떤 경우에도 차단(exit 2 상당의 deny)을 반환하지 않는다 — 항상 exit 0.
# DoD: 상위에 AGENTS.md/OWNERS/CLAUDE.md 있으면 additionalContext로 주입(AC1).
#      없으면 무출력 통과(AC2, fail-open). 탐색 깊이 상한 8(AC3).
#      8KB 초과 파일은 절단 주입(AC4).
# 이식 메모:
#   - jq 유무 분기 제거 — ConvertTo-Json 항상 사용(AC5 grep 폴백 불필요).
#   - bash dirname/PPID 세션프록시를 .NET 경로 API + 부모 PID 조회로 대응.
#   - 부모 PID는 Get-CimInstance(Win32_Process)로 조회(5.1/7 공통, Windows 한정).
#     조회 실패 시 자기 PID로 폴백(캐시 적중률만 낮아짐 — 최악의 경우도 "재주입 생략
#     실패"에 그쳐 advisory 훅 안전성엔 영향 없음).
# =============================================================================

. (Join-Path $PSScriptRoot '_common.ps1')

$ctx = Get-BathosContext

$MaxDepth      = 8
$RuleFilenames = @('AGENTS.md', 'OWNERS', 'CLAUDE.md')
$MaxBytes      = 8192   # AC4: 8KB 초과 시 절단

# --------------------------------------------------------------------------
# 2. stdin JSON 파싱: 대상 파일 경로 추출 (freeze-guard.ps1과 동일 골격)
# --------------------------------------------------------------------------
$parsed = Read-HookInput
$filePathRaw = ''
$tool = ''
if ($null -ne $parsed) {
    $t = Get-JsonField $parsed @('tool_name');            if ($null -ne $t) { $tool = [string]$t }
    $f = Get-JsonField $parsed @('tool_input','file_path'); if ($null -ne $f) { $filePathRaw = [string]$f }
}

switch ($tool) {
    'Read'      { }
    'Edit'      { }
    'Write'     { }
    'MultiEdit' { }
    ''          { }   # 도구명 미확인 → 보수적으로 계속
    default     { exit 0 }   # 그 외 도구는 통과(advisory 전용 훅이므로 조용히)
}

if ([string]::IsNullOrEmpty($filePathRaw)) { exit 0 }

# --------------------------------------------------------------------------
# 3. 대상 디렉터리 절대경로화 (파일이 아직 없어도 됨 — Write 신규 생성 케이스)
# --------------------------------------------------------------------------
try {
    if ([System.IO.Path]::IsPathRooted($filePathRaw)) {
        $fullFilePath = $filePathRaw
    } else {
        $fullFilePath = Join-Path $ctx.ProjectDir $filePathRaw
    }
} catch {
    $fullFilePath = Join-Path $ctx.ProjectDir $filePathRaw
}

$targetDir = Split-Path -Path $fullFilePath -Parent
if ([string]::IsNullOrEmpty($targetDir)) { $targetDir = $ctx.ProjectDir }
if (Test-Path -LiteralPath $targetDir -PathType Container) {
    try { $targetDir = (Resolve-Path -LiteralPath $targetDir).Path } catch { }
}

$projectDirAbs = $ctx.ProjectDir
try { $projectDirAbs = (Resolve-Path -LiteralPath $ctx.ProjectDir).Path } catch { }

# --------------------------------------------------------------------------
# 4. 상위 디렉터리를 거슬러 올라가며 규칙 파일 수집
#    - 프로젝트 루트를 넘어서지 않는다(안전 경계 — 무관한 상위 디렉터리 스캔 방지).
#    - 깊이 상한 MAX_DEPTH(8) — AC3.
#    - 근접(target 디렉터리에 가까움) → 루트 순으로 채우고, 표시 시 역순(AC1).
# --------------------------------------------------------------------------
$foundNearToRoot = New-Object System.Collections.Generic.List[string]
$cur = $targetDir
$depth = 0
while ($depth -lt $MaxDepth) {
    foreach ($fname in $RuleFilenames) {
        $candidate = Join-Path $cur $fname
        if (Test-Path -LiteralPath $candidate -PathType Leaf) { $foundNearToRoot.Add($candidate) }
    }
    # 프로젝트 루트에 도달했으면 종료(그 이상은 스캔하지 않음 — 안전 경계)
    if ((ConvertTo-SlashPath $cur) -eq (ConvertTo-SlashPath $projectDirAbs)) { break }
    $parent = Split-Path -Path $cur -Parent
    if ([string]::IsNullOrEmpty($parent) -or ($parent -eq $cur)) { break }
    $cur = $parent
    $depth++
}

# 아무 규칙 파일도 없으면 무해 통과 (AC2, fail-open)
if ($foundNearToRoot.Count -eq 0) { exit 0 }

# --------------------------------------------------------------------------
# 5. 동일 세션 내 반복 주입 억제(캐시) — CONCERNS 대응, 구현 재량
#    부모 프로세스(Claude Code 본체) PID를 세션 프록시로 사용해 동일 세션 동안은
#    동일 규칙 파일 집합을 1회만 주입한다(컨텍스트 비대 방지).
#    한계(문서화): 조회 실패/PID 재사용 시 오탐 가능 — 비차단 advisory 훅이므로
#    최악의 경우도 "정보 재주입 생략"에 그친다(안전).
# --------------------------------------------------------------------------
function script:Get-BathosParentPid {
    try {
        $p = Get-CimInstance -ClassName Win32_Process -Filter ("ProcessId=" + $PID) -ErrorAction Stop
        if ($null -ne $p -and $null -ne $p.ParentProcessId) { return [string]$p.ParentProcessId }
    } catch { }
    try {
        $proc = Get-Process -Id $PID -ErrorAction Stop
        $parentProp = $proc.PSObject.Properties['Parent']
        if ($null -ne $parentProp -and $null -ne $parentProp.Value) { return [string]$parentProp.Value.Id }
    } catch { }
    return [string]$PID
}

$parentPid = Get-BathosParentPid
$tempDir = $env:TEMP; if ([string]::IsNullOrEmpty($tempDir)) { $tempDir = [System.IO.Path]::GetTempPath() }
$cacheFile = Join-Path $tempDir ('bathos-scope-inject-seen-' + $parentPid)
$signature = ($foundNearToRoot -join '|')

$alreadySeen = $false
if (Test-Path -LiteralPath $cacheFile) {
    try {
        $cacheContent = Get-Content -LiteralPath $cacheFile -Raw -ErrorAction Stop
        if ($null -ne $cacheContent -and $cacheContent.Contains($signature)) { $alreadySeen = $true }
    } catch { }
}
if ($alreadySeen) { exit 0 }  # 이미 이번 세션에서 동일 규칙 집합을 주입함 — 재주입 생략

try { Add-Content -LiteralPath $cacheFile -Value $signature -ErrorAction Stop } catch { }

# --------------------------------------------------------------------------
# 6. additionalContext 텍스트 조립 (루트 → 근접 순, AC1 — 근접 우선 명시)
# --------------------------------------------------------------------------
$contextLines = New-Object System.Collections.Generic.List[string]
$contextLines.Add('[스코프 규칙] 아래 상위 디렉터리에 적용 규칙 파일이 있습니다(루트→근접 순, 근접 파일이 우선):')

for ($idx = $foundNearToRoot.Count - 1; $idx -ge 0; $idx--) {
    $rulePath = $foundNearToRoot[$idx]
    $relPath = $rulePath
    $projSlash = ConvertTo-SlashPath $projectDirAbs
    $ruleSlash = ConvertTo-SlashPath $rulePath
    if ($ruleSlash.StartsWith($projSlash + '/')) {
        $relPath = $ruleSlash.Substring($projSlash.Length + 1)
    }

    $size = 0
    try { $size = (Get-Item -LiteralPath $rulePath).Length } catch { $size = 0 }

    $contextLines.Add('--- ' + $relPath + ' ---')
    if ($size -gt $MaxBytes) {
        # AC4: 8KB 초과 → 절단 주입(사실 명시)
        try {
            $bytes = [System.IO.File]::ReadAllBytes($rulePath)
            $excerptBytes = New-Object byte[] $MaxBytes
            [System.Array]::Copy($bytes, $excerptBytes, $MaxBytes)
            $excerpt = [System.Text.Encoding]::UTF8.GetString($excerptBytes)
        } catch { $excerpt = '' }
        $contextLines.Add($excerpt)
        $contextLines.Add('...(이하 생략 — 원본 ' + $size + '바이트 중 ' + $MaxBytes + '바이트만 표시. 전문은 ' + $relPath + ' 참고)')
    } else {
        $content = ''
        try { $content = [System.IO.File]::ReadAllText($rulePath) } catch { $content = '' }
        $contextLines.Add($content)
    }
}

$additionalContext = ($contextLines -join "`n")

# --------------------------------------------------------------------------
# 7. stdout JSON 출력 (advisory — 차단 없음, hookSpecificOutput.additionalContext)
# --------------------------------------------------------------------------
$outObj = [PSCustomObject]@{
    hookSpecificOutput = [PSCustomObject]@{
        hookEventName     = 'PreToolUse'
        additionalContext = $additionalContext
    }
}
$outObj | ConvertTo-Json -Depth 10 -Compress

exit 0
