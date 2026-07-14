// BATHOS inspect — dashboard(report/serve) 인라인 스크립트.
// 무CDN: build_report()가 include_str!로 <script> 안에 그대로 삽입한다.
// [Source: design-system-kr.md §5.6 LangToggle, design-handoff-kr.md §5/§6]

(function () {
  "use strict";

  // ── 1. 언어 토글 — data-lang 속성 방식(선례 site/index.html 계승) ───────
  var wrap = document.getElementById("bathos-inspect-wrap");
  if (wrap) {
    var btns = wrap.querySelectorAll(".langtog button[data-setlang]");
    btns.forEach(function (b) {
      b.addEventListener("click", function () {
        var lang = b.getAttribute("data-setlang");
        wrap.setAttribute("data-lang", lang);
        btns.forEach(function (x) {
          x.setAttribute("aria-pressed", x === b ? "true" : "false");
        });
        try {
          localStorage.setItem("bathos-inspect-lang", lang);
        } catch (e) {
          /* localStorage 불가 환경(예: file:// 일부 브라우저) — 조용히 무시, 크래시 금지 */
        }
      });
    });
    try {
      var saved = localStorage.getItem("bathos-inspect-lang");
      if (saved === "en" || saved === "kr") {
        wrap.setAttribute("data-lang", saved);
        btns.forEach(function (x) {
          x.setAttribute("aria-pressed", x.getAttribute("data-setlang") === saved ? "true" : "false");
        });
      }
    } catch (e) {
      /* 무시 */
    }
  }

  // ── 2. RFC3339 → 브라우저 로컬 시각 표기 ────────────────────────────────
  // <time datetime="…iso…" data-iso="…iso…">iso 원문</time> 를 뷰어의 로컬
  // 표기로 치환한다. JS 비활성/실패 시에도 ISO 원문이 남아있어 정보 손실이 없다
  // (점진적 향상 — 크래시 금지 원칙과 동일한 정신).
  document.querySelectorAll("time[data-iso]").forEach(function (el) {
    var iso = el.getAttribute("data-iso");
    if (!iso) return;
    var d = new Date(iso);
    if (isNaN(d.getTime())) return;
    try {
      el.textContent = d.toLocaleString();
    } catch (e) {
      /* Intl 미지원 환경 — ISO 원문 유지 */
    }
  });

  // ── 3. 경고 배너 닫기(aria-hidden 토글, DOM 제거는 하지 않음) ───────────
  document.querySelectorAll(".banner .x").forEach(function (btn) {
    btn.addEventListener("click", function () {
      var banner = btn.closest(".banner");
      if (banner) banner.setAttribute("hidden", "");
    });
  });
})();
