<#
=============================================================================
BATHOS Dynamis — dist/lib/host-detect.ps1  (Windows PowerShell 포트)
원본: dist/lib/host-detect.sh
CF-B5 / SS9 (Could · 스텁만) — 런타임 host-detection + 출력분기 단일 모듈.

★ 이 파일은 스텁이다. 실제 Codex/Copilot 등 분기 로직은 미구현이다(LD-2:
  Claude Code 외 full-hook 런타임 미타깃). 활성 조건: Dynamis가 Claude Code
  외 런타임을 실제로 타깃하기로 결정할 때(현재 아님).

ponytail 사상(판별 키는 (추정) — 실타깃 결정 시 해당 호스트 최신 문서로
재검증 필요, Search Before Building):
  PLUGIN_DATA 존재          -> Codex        (추정)
  COPILOT_PLUGIN_DATA 존재  -> GitHub Copilot (추정)
  그 외(기본값)              -> Claude Code

규약(US8 AC2): 호스트 판별·출력분기 로직은 이 모듈 하나에만 존재해야 한다.
다른 스크립트(uninstall.ps1, check-*.ps1 등)에서 동일 로직을 복제하지 않는다
(복제 시 CF-B5 AC2 위반).

사용법:
  . <경로>/dist/lib/host-detect.ps1
  $hostName = Get-BathosHost
  Write-BathosHookOutput -HostName $hostName -Payload '{"decision":"allow"}'

타깃: Windows PowerShell 5.1 하한 + PowerShell 7+ 호환(삼항/`??` 미사용).
=============================================================================
#>

Set-StrictMode -Version 2.0
$ErrorActionPreference = 'Stop'

# Get-BathosHost — 현재 실행 호스트를 판별해 소문자 문자열로 반환.
#   반환값: "claude" | "codex" | "copilot"
# (추정) 환경변수 키는 미검증 — 실타깃 결정 시 해당 호스트 공식 문서 확인 필요.
function Get-BathosHost {
    $forced = $env:BATHOS_FORCE_HOST
    if (-not [string]::IsNullOrEmpty($forced)) {
        # 테스트/디버깅 전용 강제 오버라이드(스모크 테스트에서 사용).
        return $forced
    }
    if (-not [string]::IsNullOrEmpty($env:PLUGIN_DATA)) {
        return 'codex'
    }
    if (-not [string]::IsNullOrEmpty($env:COPILOT_PLUGIN_DATA)) {
        return 'copilot'
    }
    return 'claude'
}

# Write-BathosHookOutput -HostName <host> -Payload <json_payload>
#   호스트별 훅 출력 형태로 분기한다.
#   Claude Code 경로만 실동작(그대로 통과). 그 외 호스트는 미구현을
#   명시적으로 알리고 실패를 신호한다 — 조용히 잘못된 형태를 출력하지 않는다
#   (날조 금지: 지원하지 않는 걸 지원하는 척하지 않는다).
#
# 이식 메모: bash판은 stdout(payload)과 함수 반환코드($?)가 서로 다른 채널이라
#   `out="$(...)"; rc=$?` 로 독립 캡처할 수 있다. PowerShell 함수는 파이프라인
#   출력과 "반환값"이 같은 채널이므로(그리고 dot-source 컨텍스트에서 exit을
#   부르면 호출측 프로세스 전체가 죽는다), 두 채널을 인위적으로 합치는 대신
#   [PSCustomObject]@{ Output; ExitCode }를 반환해 호출측이 각각 검사하게 한다.
#   Output은 host=claude일 때만 payload, 그 외에는 $null(오출력 금지, bash판과 동일).
function Write-BathosHookOutput {
    param(
        [Parameter(Mandatory = $true)] [string] $HostName,
        [Parameter(Mandatory = $true)] [string] $Payload
    )

    switch ($HostName) {
        'claude' {
            return [PSCustomObject]@{ Output = $Payload; ExitCode = 0 }
        }
        'codex' {
            [Console]::Error.WriteLine("[bathos host-detect] ! $HostName 출력 분기는 미구현(스텁, Could 범위 밖). 참조: docs/agent-portability-kr.md §2")
            return [PSCustomObject]@{ Output = $null; ExitCode = 2 }
        }
        'copilot' {
            [Console]::Error.WriteLine("[bathos host-detect] ! $HostName 출력 분기는 미구현(스텁, Could 범위 밖). 참조: docs/agent-portability-kr.md §2")
            return [PSCustomObject]@{ Output = $null; ExitCode = 2 }
        }
        default {
            [Console]::Error.WriteLine("[bathos host-detect] X 알 수 없는 host: $HostName")
            return [PSCustomObject]@{ Output = $null; ExitCode = 2 }
        }
    }
}

# 직접 실행 시(dot-source가 아니라 실행) 자가진단만 수행 — 부작용 없음.
if ($MyInvocation.InvocationName -ne '.') {
    $detected = Get-BathosHost
    Write-Host "[bathos host-detect] 감지된 host: $detected (스텁 - 실제 분기는 claude만 동작)"
}
