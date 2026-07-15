# =============================================================================
# BATHOS M6 — artifact-verify.ps1   (Windows PowerShell 포트)
# 원본: .claude/hooks/artifact-verify.sh
# TaskCompleted / SubagentStop: 산출물 존재 및 게이트 기준 충족 검증
# =============================================================================
# 계약:  api-contracts.md §D (TaskCompleted artifact-verify), §A-1 StoryFile 계약
# 예외:  exceptions.md §2 E-CTX-LOSS (9섹션 누락/developer_context 공란)
# DoD:   산출물 미충족 → exit 2 + 구체 피드백; 충족 또는 비해당 → exit 0
# 검사 대상:
#   - StoryFile: status=ready-for-dev, 6개 필수 섹션 존재, developer_context 비어있음 금지
#   - QA task: qa-summary.md 존재
#   - W3 task: readiness-report-kr.md 존재, verdict 포함
#   - SubagentStop(#15 Matthew 종료): 03-story-engineering/story-*-kr.md 전수 검증 + [Source:] 표기
# 이식 메모: jq 유무 분기 제거 — ConvertFrom-Json/정규식 네이티브 사용.
# =============================================================================

. (Join-Path $PSScriptRoot '_common.ps1')

$ctx = Get-BathosContext
$artDir = Join-Path $ctx.ProjectDir '.agent-team'
$ic = [System.Text.RegularExpressions.RegexOptions]::IgnoreCase

# --------------------------------------------------------------------------
# 2. stdin JSON 파싱: 태스크 제목 추출
# --------------------------------------------------------------------------
$parsed = Read-HookInput
$taskTitle = ''
$taskDesc = ''
if ($null -ne $parsed) {
    $tt = Get-JsonField $parsed @('task','title'); if ($null -eq $tt) { $tt = Get-JsonField $parsed @('title') }
    if ($null -ne $tt) { $taskTitle = [string]$tt }
    $td = Get-JsonField $parsed @('task','description'); if ($null -eq $td) { $td = Get-JsonField $parsed @('description') }
    if ($null -ne $td) { $taskDesc = [string]$td }
}

# --------------------------------------------------------------------------
# 2-b. SubagentStop 컨텍스트 탐지 (B-3 신규 / R-3 강건화)
# --------------------------------------------------------------------------
# 입력 스키마 구분:
#   SubagentStop  = { role|agent_type|subagent_type|..., outputs, ... } — task 필드 없음
#   TaskCompleted = { task.title, ... }                                 — task/title 필드 있음
# R-3: 페이로드의 역할 필드명이 런타임 버전마다 다를 수 있으므로 후보 필드를 폭넓게 조회한다.
$subagentRole = ''
if ($null -ne $parsed) {
    $taskField = Get-JsonField $parsed @('task')
    if ($null -eq $taskField) {
        $roleField = Get-JsonField $parsed @('role')
        if ($null -eq $roleField) { $roleField = Get-JsonField $parsed @('agent_type') }
        if ($null -eq $roleField) { $roleField = Get-JsonField $parsed @('subagent_type') }
        if ($null -eq $roleField) { $roleField = Get-JsonField $parsed @('agentType') }
        if ($null -eq $roleField) { $roleField = Get-JsonField $parsed @('agent','type') }
        if ($null -eq $roleField) { $roleField = Get-JsonField $parsed @('agent','name') }
        if ($null -eq $roleField) { $roleField = Get-JsonField $parsed @('agent_name') }
        if ($null -eq $roleField) { $roleField = Get-JsonField $parsed @('name') }
        if ($null -ne $roleField) { $subagentRole = [string]$roleField }
    }
}

# --------------------------------------------------------------------------
# 3. StoryFile 필수 섹션 검증 함수
# --------------------------------------------------------------------------
# D1 Completeness 필수 섹션 — 정본: bathos-story-engine/src/compiler.rs REQUIRED_SECTIONS (6개).
$RequiredStorySections = @(
    'story_requirements'
    'developer_context'
    'architecture_compliance'
    'library_framework_requirements'
    'file_structure_requirements'
    'testing_requirements'
)

function script:Test-StoryFile {
    param([string]$StoryFile)
    $errors = New-Object System.Collections.Generic.List[string]

    if (-not (Test-Path -LiteralPath $StoryFile -PathType Leaf)) {
        $errors.Add('StoryFile 없음: ' + $StoryFile)
        return $errors
    }

    $lines = @(Get-Content -LiteralPath $StoryFile -ErrorAction SilentlyContinue)
    $content = ($lines -join "`n")

    # status 검사: frontmatter에 status: ready-for-dev 필요
    if ($content -notmatch 'status:\s*"?ready-for-dev"?') {
        $errors.Add('StoryFile status가 ready-for-dev 아님: ' + $StoryFile)
    }

    # 필수 섹션 존재 검사 (헤딩 유무와 무관하게 본문에 해당 키워드 존재 확인 — 원본 3중 alt 대응)
    $lowerContent = $content.ToLowerInvariant()
    foreach ($section in $RequiredStorySections) {
        if (-not $lowerContent.Contains($section.ToLowerInvariant())) {
            $errors.Add('필수 섹션 누락: ' + $section)
        }
    }

    # developer_context 비어있음 금지 (섹션 헤딩 다음에 내용이 있어야 함)
    # 간단한 휴리스틱: developer_context 이후 10줄 내에 비어있지 않은 라인 확인
    $dcLineIdx = -1
    for ($i = 0; $i -lt $lines.Count; $i++) {
        if ($lines[$i] -match '(?i)developer_context') { $dcLineIdx = $i; break }
    }
    if ($dcLineIdx -ge 0) {
        $afterCount = 0
        $endIdx = [Math]::Min($dcLineIdx + 10, $lines.Count - 1)
        for ($j = $dcLineIdx + 1; $j -le $endIdx; $j++) {
            if ($lines[$j] -match '\S') { $afterCount++ }
        }
        if ($afterCount -lt 1) {
            $errors.Add('developer_context 섹션이 비어 있음 (E-CTX-LOSS)')
        }
    }

    return $errors
}

# --------------------------------------------------------------------------
# 3-b. SubagentStop 분기 — #15 Matthew 종료 시 StoryFile 완전성 검증 (B-3)
# --------------------------------------------------------------------------
# api-contracts §D: SubagentStop(artifact-verify) → StoryFile 스키마 검증
# D1 완전성: 6섹션 + [Source:] 출처 표기(§A-1 계약 불변식 ②)
# 미충족 시 exit 2(재컴파일 유도), 충족 시 exit 0. fail-safe: 다른 역할은 통과.
if (-not [string]::IsNullOrEmpty($subagentRole)) {
    if ([regex]::IsMatch($subagentRole, '#15|matthew|story[-_]engineer', $ic)) {
        $saErrors = New-Object System.Collections.Generic.List[string]
        $storyFilesFound = 0
        $storyDir = Join-Path $artDir '03-story-engineering'

        $storyFiles = @()
        if (Test-Path -LiteralPath $storyDir -PathType Container) {
            $storyFiles = @(Get-ChildItem -LiteralPath $storyDir -Filter 'story-*-kr.md' -File -ErrorAction SilentlyContinue)
        }

        foreach ($sf in $storyFiles) {
            $storyFilesFound++

            $errs = Test-StoryFile -StoryFile $sf.FullName
            foreach ($e in $errs) { $saErrors.Add($e) }

            # [Source:] 출처 표기 검사 — api-contracts §A-1 계약 불변식 ②
            $sfContent = ''
            try { $sfContent = Get-Content -LiteralPath $sf.FullName -Raw -ErrorAction Stop } catch { $sfContent = '' }
            if ([string]::IsNullOrEmpty($sfContent) -or (-not $sfContent.Contains('[Source:'))) {
                $saErrors.Add($sf.Name + ': [Source:] 출처 표기 없음 (api-contracts §A-1 ② 위반)')
            }
        }

        # StoryFile 자체가 없는 경우
        if ($storyFilesFound -eq 0) {
            $saErrors.Add('StoryFile 없음: ' + (Join-Path $storyDir 'story-*-kr.md'))
        }

        if ($saErrors.Count -gt 0) {
            Write-HookError ''
            Write-HookError '[BATHOS artifact-verify] ❌ StoryFile 완전성 미충족 — #15 재컴파일 필요'
            foreach ($e in $saErrors) { Write-HookError ('[BATHOS artifact-verify] • ' + $e) }
            Write-HookError '[BATHOS artifact-verify] #15 Matthew: StoryFile 보완 후 재제출하세요 (E-CTX-LOSS 방지)'
            Write-HookError '[BATHOS artifact-verify] 참고: api-contracts.md §A-1, exceptions.md §2 E-CTX-LOSS'
            exit 2
        }

        Write-HookError '[BATHOS artifact-verify] ✅ StoryFile 완전성 검증 통과 (#15 종료)'
        exit 0
    }

    # 다른 역할 종료: fail-safe 통과 (훅은 절대 차단 금지)
    exit 0
}

# --------------------------------------------------------------------------
# 4. 태스크 컨텍스트 기반 검증 라우팅
# --------------------------------------------------------------------------
$errorsList = New-Object System.Collections.Generic.List[string]
$verified = $false
$combinedTD = $taskTitle + ' ' + $taskDesc

# 4-1. QA / 검증 태스크
if ([regex]::IsMatch($combinedTD, 'QA|qa|테스트|검증|verify|test', $ic)) {
    $qaSummary = Join-Path $artDir '11-qa/qa-summary.md'
    if (-not (Test-Path -LiteralPath $qaSummary)) {
        $errorsList.Add('QA 태스크 완료 조건 미충족: ' + $qaSummary + ' 가 없습니다.')
        $errorsList.Add('QA 요약 파일을 먼저 생성하고 태스크를 완료하세요.')
    }
    $verified = $true
}

# 4-2. W3 / 스토리 게이트 태스크
if ([regex]::IsMatch($combinedTD, 'W3|story.gate|readiness|gate|스토리.*게이트|게이트.*판정', $ic)) {
    $readinessReport = Join-Path $artDir '03-story-engineering/readiness-report-kr.md'
    if (-not (Test-Path -LiteralPath $readinessReport)) {
        $errorsList.Add('W3 게이트 태스크 완료 조건 미충족: ' + $readinessReport + ' 가 없습니다.')
        $errorsList.Add('readiness-report-kr.md를 생성한 뒤 태스크를 완료하세요.')
    } else {
        $rrContent = ''
        try { $rrContent = Get-Content -LiteralPath $readinessReport -Raw -ErrorAction Stop } catch { $rrContent = '' }
        if ([string]::IsNullOrEmpty($rrContent) -or (-not [regex]::IsMatch($rrContent, 'verdict:\s*"*(PASS|CONCERNS|FAIL)"*', $ic))) {
            $errorsList.Add('readiness-report-kr.md에 verdict(PASS/CONCERNS/FAIL)가 없습니다.')
        }
    }
    $verified = $true
}

# 4-3. StoryFile 생성/컴파일 태스크
if ([regex]::IsMatch($combinedTD, 'story.compil|스토리.파일|story[-_].*kr|create.story', $ic)) {
    $storyDir2 = Join-Path $artDir '03-story-engineering'
    $storyFiles2 = @()
    if (Test-Path -LiteralPath $storyDir2 -PathType Container) {
        $storyFiles2 = @(Get-ChildItem -LiteralPath $storyDir2 -Filter 'story-*-kr.md' -File -ErrorAction SilentlyContinue)
    }
    if ($storyFiles2.Count -eq 0) {
        $errorsList.Add('StoryFile이 없습니다: ' + (Join-Path $storyDir2 'story-*-kr.md'))
    } else {
        foreach ($sf in $storyFiles2) {
            $errs2 = Test-StoryFile -StoryFile $sf.FullName
            if ($errs2.Count -gt 0) {
                $errorsList.Add('StoryFile 검증 실패: ' + $sf.FullName)
            }
        }
    }
    $verified = $true
}

# 4-4. W5 구현 태스크 완료 확인: 소유 경로 내 파일 생성 여부 기본 확인
# ⚠️ "구현" 단독은 오탐 多 → "W5"를 함께 요구하거나 웨이브 컨텍스트 필수
if ([regex]::IsMatch($combinedTD, 'W5|wave5|wave.5.*impl|W5.*구현|구현.*W5|[Ww]ave.*5.*구현|backend.eng|frontend.eng|ml.eng', $ic)) {
    $implNotesDir = Join-Path $artDir '08-impl-notes'
    if (-not (Test-Path -LiteralPath $implNotesDir -PathType Container)) {
        $errorsList.Add('구현 노트 디렉터리 없음: ' + $implNotesDir)
        $errorsList.Add('구현 완료 후 ' + $implNotesDir + '/*.md 를 생성하세요.')
    }
    $verified = $true
}

# --------------------------------------------------------------------------
# 5. 결과 판정
# --------------------------------------------------------------------------
if ($errorsList.Count -gt 0) {
    Write-HookError ''
    Write-HookError '[BATHOS artifact-verify] ❌ 산출물 검증 실패 — 태스크 완료 조건 미충족'
    foreach ($e in $errorsList) { Write-HookError ('[BATHOS artifact-verify] • ' + $e) }
    Write-HookError '[BATHOS artifact-verify] 산출물을 보완한 뒤 태스크를 다시 완료 처리하세요.'
    Write-HookError '[BATHOS artifact-verify] 참고: api-contracts.md §A-1, exceptions.md §2 E-CTX-LOSS'
    exit 2  # ← 차단
}

# 검증 항목 없었거나 모두 통과
if ($verified) {
    Write-HookError '[BATHOS artifact-verify] ✅ 산출물 검증 통과'
}
exit 0
