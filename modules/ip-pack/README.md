<!--
출처: BATHOS 오리지널 자산
작성: Mark(BATHOS IP Specialist, 25년차) · 2026-06-30
모듈: modules/ip-pack (W4 플러그 — 코어 비의존)
라이선스: MIT (BATHOS 패키지)
-->

# IP Pack (M10) — W4 플러그 모듈

> ⚠️ **법적 한계:** 본 모듈의 산출물은 **변리사/변호사의 법적 자문이 아니라 출원 보조용 초안**이다. 정식 선행기술 검색·등록 가능성 판단·청구항 확정은 **전문가(변리사) 검토 필요**.

## 무엇인가

설계(W2 아키텍처) + 스토리(W3)에서 **신규성·진보성 발명 포인트를 식별**하고 **특허청 가이드 포맷의 출원명세서 초안**을 산출하는 **재사용 플러그 모듈**이다. 실제 한 건의 명세서가 아니라 그것을 생성하는 워크플로우·템플릿·체크리스트 패키지다.

## 구성

```
modules/ip-pack/
├── module.yaml                      # plug-manager 로더 계약(Phillip)
├── README.md                        # 이 파일
├── workflows/patent-spec-draft.md   # 출원명세서 8단계 생성 워크플로우
├── templates/patent-spec.md         # 출원명세서 섹션 골격
└── checklists/patent-quality.md     # 적대적 품질 자가검증(선택)
```

## module.yaml 계약 (plug-manager 파싱 대상)

```yaml
module_id: ip
name: IP Pack
wave: W4
trigger: "Lv>=3 OR domain=ip"   # 라우터가 이 조건에서 모듈 로드
enabled_default: false          # 코어 슬림 — 기본 비활성
provides:
  workflows: [patent-spec-draft]
  templates: [patent-spec]
outputs: ".agent-team/05-ip/"   # 산출 경로(이 밖에 쓰지 않음)
evidence_trace: true            # 청구항·서술의 근거추적 ON
```

## 코어 비의존 (design-patterns §7)

- 코어(`bathos/core/**`)는 본 모듈을 **모른다**(역의존 금지). 코어 코드 참조·수정 0.
- 모듈은 `outputs` 경로(`.agent-team/05-ip/`)에만 기록한다.
- 활성화는 라우터/plug-manager가 `trigger`로 판정(`PLUG_MODULE` 엔티티, data-model-erd §1).

## evidence_trace 설계

스토리 엔진의 Zero-Context-Loss **D2(출처추적)** 원칙을 IP 도메인에 적용:
1. 모든 청구항·기술 서술에 `[Source: .agent-team/04-architecture/<file>.md#<section>]` 태그.
2. `claims-map-kr.md`가 **청구항↔설계요소↔도면부호↔Source**를 추적.
3. 상류 `source_hash` 기록 → 변경 시 `stale` 재생성(D3).
4. 무근거 서술 금지 — 불가피한 추정은 `[근거 미확보]` 명시(수치 날조 금지).

## 산출물 (`.agent-team/05-ip/`)

| 파일 | 역할 |
|------|------|
| `invention-disclosure-kr.md` | 발명 신고서 — **먼저 확정**(Nathanael 참고) |
| `prior-art-notes-kr.md` | 선행기술 노트(검색 한계 명시) |
| `patent-spec-draft-kr.md` | 출원명세서 초안(주 산출물) |
| `claims-map-kr.md` | 청구항 추적표(evidence_trace) |
