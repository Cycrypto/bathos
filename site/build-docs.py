#!/usr/bin/env python3
"""Build the self-contained BATHOS docs site from bathos/docs/*.md.

Runtime output has ZERO external references. pandoc is used at BUILD time only
to convert markdown -> html fragments; everything else (template, TOC, nav,
link rewriting, table wrapping) is done here.
"""
import os, re, subprocess, html, sys

# This script lives at bathos/site/build-docs.py. Sources are bathos/docs/*.md;
# output is bathos/site/docs/. Run:  python3 bathos/site/build-docs.py  (needs pandoc)
HERE = os.path.dirname(os.path.abspath(__file__))          # bathos/site
BATHOS = os.path.dirname(HERE)                             # bathos
DOCS_SRC = os.path.join(BATHOS, "docs")
OUT = os.path.join(HERE, "docs")

LANGS = ["en", "kr", "es"]
LANG_ATTR = {"en": "en", "kr": "ko", "es": "es"}
FLAG = {"en": "🇺🇸", "kr": "🇰🇷", "es": "🇪🇸"}
LANG_LABEL = {"en": "EN", "kr": "KO", "es": "ES"}

# ---- doc metadata (order matters: sidebar + prev/next) ----
# key = md basename stem (UPPER); slug = output slug (lower)
DOCS = [
    {"src": "USAGE",        "slug": "usage",        "group": "start"},
    {"src": "USECASE",      "slug": "usecase",      "group": "start"},
    {"src": "FEATURES",     "slug": "features",     "group": "concepts"},
    {"src": "ARCHITECTURE", "slug": "architecture", "group": "concepts"},
    {"src": "ROLE-GUIDE",   "slug": "role-guide",   "group": "customize"},
    {"src": "MODULE-GUIDE", "slug": "module-guide", "group": "customize"},
    {"src": "QUOTA",        "slug": "quota",        "group": "operate"},
    {"src": "FAQ",          "slug": "faq",          "group": "operate"},
]

# Single-language repo-root documents (no translations). Rendered once, in their
# native language; the language toggle on these pages falls back to the docs home
# of the chosen language. `file` is relative to the bathos/ package root.
SINGLES = [
    {"file": "README.md", "slug": "readme", "group": "project", "native": "en"},
    {"file": "CLAUDE.md", "slug": "claude", "group": "project", "native": "kr"},
    {"file": "ETHOS.md",  "slug": "ethos",  "group": "project", "native": "kr"},
]

GROUP_LABEL = {
    "start":     {"en": "Start",       "kr": "시작",       "es": "Empezar"},
    "concepts":  {"en": "Concepts",    "kr": "개념",       "es": "Conceptos"},
    "customize": {"en": "Customize",   "kr": "커스터마이즈", "es": "Personalizar"},
    "operate":   {"en": "Operate",     "kr": "운영",       "es": "Operar"},
    "project":   {"en": "Project",     "kr": "프로젝트",    "es": "Proyecto"},
}

NATIVE_TAG = {"en": "EN", "kr": "KR", "es": "ES"}
NATIVE_WORD = {"en": {"en": "English only", "kr": "영어 원문", "es": "solo inglés"},
               "kr": {"en": "Korean only", "kr": "한국어 원문", "es": "solo coreano"},
               "es": {"en": "Spanish only", "kr": "스페인어 원문", "es": "solo español"}}

# short sidebar label + card kicker + card blurb, per lang
META = {
    "usage": {
        "label": {"en": "Usage guide", "kr": "사용 가이드", "es": "Guía de uso"},
        "blurb": {"en": "Install, wire the engine into a project, and drive the 7-wave pipeline. The reference for day-to-day operation.",
                  "kr": "설치, 엔진을 프로젝트에 연결, 7웨이브 파이프라인 진행. 일상 운영의 레퍼런스.",
                  "es": "Instala, conecta el motor a un proyecto y dirige el pipeline de 7 waves. La referencia del día a día."}},
    "usecase": {
        "label": {"en": "Use case", "kr": "사용 사례", "es": "Caso de uso"},
        "blurb": {"en": "A full worked example — building a new service from scratch, step by step, one wave at a time.",
                  "kr": "완전한 실전 예제 — 새 서비스를 처음부터, 한 웨이브씩 단계별로 구축.",
                  "es": "Un ejemplo completo — construir un servicio nuevo desde cero, paso a paso, wave a wave."}},
    "features": {
        "label": {"en": "Features & mechanics", "kr": "기능·작동 원리", "es": "Funciones y mecánica"},
        "blurb": {"en": "What sets BATHOS apart and the actual mechanics beneath it — state, router, waves, gates, story engine, plugs.",
                  "kr": "BATHOS의 차별점과 그 아래의 실제 메커니즘 — 상태·라우터·웨이브·게이트·스토리 엔진·플러그.",
                  "es": "Qué distingue a BATHOS y la mecánica real que hay debajo — estado, router, waves, gates, story engine, plugs."}},
    "architecture": {
        "label": {"en": "Architecture", "kr": "아키텍처", "es": "Arquitectura"},
        "blurb": {"en": "For contributors: the two planes, the Rust workspace, invariants, exit codes, router scoring, hooks, and key ADRs.",
                  "kr": "기여자용: 두 실행면, Rust 워크스페이스, 불변식, exit 코드, 라우터 스코어링, 훅, 핵심 ADR.",
                  "es": "Para contribuidores: los dos planos, el workspace de Rust, invariantes, exit codes, scoring del router, hooks y ADRs clave."}},
    "role-guide": {
        "label": {"en": "Role customization", "kr": "역할 커스터마이즈", "es": "Personalizar roles"},
        "blurb": {"en": "The 3-layer override model — base / team / user — for tuning role identity, ownership, and facilitation.",
                  "kr": "3계층 오버라이드(base/team/user)로 역할 정체성·소유 경로·facilitation 수위 조정.",
                  "es": "El modelo de override en 3 capas — base / team / user — para ajustar identidad, propiedad y facilitación de roles."}},
    "module-guide": {
        "label": {"en": "Writing modules", "kr": "모듈 작성", "es": "Escribir módulos"},
        "blurb": {"en": "Build a custom plug-in that hangs off Wave 4 — module.yaml contract, the trigger DSL, and a full worked pack.",
                  "kr": "W4에 붙는 커스텀 플러그 제작 — module.yaml 계약, 트리거 DSL, 완성 예제 팩까지.",
                  "es": "Crea un plug-in propio que cuelga de la Wave 4 — el contrato module.yaml, el DSL de triggers y un pack completo."}},
    "quota": {
        "label": {"en": "Token & quota", "kr": "토큰·쿼터", "es": "Tokens y cuota"},
        "blurb": {"en": "What drives cost and the three levers to control it, plus how to recognize and recover from a usage limit.",
                  "kr": "비용을 결정하는 요인과 통제 레버 3종, 사용량 한도 인식·복구법.",
                  "es": "Qué determina el coste y las tres palancas para controlarlo, más cómo reconocer y recuperarse de un límite de uso."}},
    "faq": {
        "label": {"en": "FAQ", "kr": "FAQ", "es": "FAQ"},
        "blurb": {"en": "Quick answers to the first questions — what it is, what it needs, what it costs, and where to go for depth.",
                  "kr": "먼저 나오는 질문에 대한 빠른 답 — 무엇인지, 무엇이 필요한지, 비용, 더 깊은 문서로의 안내.",
                  "es": "Respuestas rápidas a las primeras preguntas — qué es, qué necesita, cuánto cuesta y dónde profundizar."}},
    "readme": {
        "label": {"en": "README (overview)", "kr": "README (개요)", "es": "README (visión general)"},
        "blurb": {"en": "The repository README — project overview, quick start, and the map to everything else.",
                  "kr": "레포지토리 README — 프로젝트 개요, 빠른 시작, 나머지 전체로의 지도.",
                  "es": "El README del repositorio — visión general del proyecto, inicio rápido y el mapa hacia todo lo demás."}},
    "claude": {
        "label": {"en": "Operating rules (CLAUDE.md)", "kr": "운영 규칙 (CLAUDE.md)", "es": "Reglas operativas (CLAUDE.md)"},
        "blurb": {"en": "The package operating rules read by the lead and every teammate — roles, waves, gates, directory conventions.",
                  "kr": "리드와 모든 팀원이 읽는 패키지 운영 규칙 — 역할·웨이브·게이트·디렉터리 규약.",
                  "es": "Las reglas operativas del paquete que leen el líder y cada compañero — roles, waves, gates y convenciones de directorio."}},
    "ethos": {
        "label": {"en": "ETHOS (principles)", "kr": "ETHOS (원칙)", "es": "ETHOS (principios)"},
        "blurb": {"en": "The builder ETHOS behind every decision — Boil the Ocean, Search Before Building, User Sovereignty.",
                  "kr": "모든 결정 뒤의 빌더 ETHOS — Boil the Ocean, Search Before Building, User Sovereignty.",
                  "es": "El ETHOS del constructor tras cada decisión — Boil the Ocean, Search Before Building, User Sovereignty."}},
}

HOME_STR = {
    "eyebrow": {"en": "βάθος · documentation", "kr": "βάθος · 문서", "es": "βάθος · documentación"},
    "title":   {"en": "BATHOS documentation", "kr": "BATHOS 문서", "es": "Documentación de BATHOS"},
    "lede": {"en": "Everything to install BATHOS, run the 7-wave pipeline, customize roles and modules, manage quota, and contribute — in English, 한국어, and Español.",
             "kr": "BATHOS 설치부터 7웨이브 파이프라인 진행, 역할·모듈 커스터마이즈, 쿼터 관리, 기여까지 — English · 한국어 · Español.",
             "es": "Todo para instalar BATHOS, ejecutar el pipeline de 7 waves, personalizar roles y módulos, gestionar la cuota y contribuir — en English, 한국어 y Español."},
    "home": {"en": "Home", "kr": "홈", "es": "Inicio"},
    "landing": {"en": "← Landing", "kr": "← 랜딩", "es": "← Inicio"},
    "onpage": {"en": "On this page", "kr": "이 문서에서", "es": "En esta página"},
    "docs": {"en": "Docs", "kr": "문서", "es": "Docs"},
    "prev": {"en": "Previous", "kr": "이전", "es": "Anterior"},
    "next": {"en": "Next", "kr": "다음", "es": "Siguiente"},
    "menu": {"en": "Open navigation", "kr": "내비게이션 열기", "es": "Abrir navegación"},
    "foot": {"en": "depth over surface · MIT License · runs on Claude Code v2.1.32+",
             "kr": "표층이 아닌 깊이 · MIT License · Claude Code v2.1.32+ 에서 동작",
             "es": "la profundidad sobre la superficie · MIT License · funciona sobre Claude Code v2.1.32+"},
}


def home_href(lang):
    return "index.html" if lang == "kr" else "index-%s.html" % lang


def doc_href(slug, lang):
    return "%s-%s.html" % (slug, lang)


def run_pandoc(path):
    r = subprocess.run(
        ["pandoc", path, "-f", "gfm", "-t", "html5", "--syntax-highlighting=none"],
        capture_output=True, text=True, check=True)
    return r.stdout


DOCS_PREFIX_RE = re.compile(r'href="docs/([A-Za-z][A-Za-z-]*)-(en|kr|es)\.md(#[^"]*)?"')
LINK_RE = re.compile(r'href="([A-Za-z][A-Za-z-]*)-(en|kr|es)\.md(#[^"]*)?"')
# CLAUDE.md / ETHOS.md / README.md are now rendered as on-site pages (SINGLES).
SINGLE_LINK_RE = re.compile(r'href="(?:\.\./)?(CLAUDE|ETHOS|README)\.md(#[^"]*)?"')
# CREDITS.md / LICENSE have no on-site page → the landing carries the notice.
CREDITS_RE = re.compile(r'href="(?:\.\./)?CREDITS\.md(?:#[^"]*)?"')
LICENSE_RE = re.compile(r'href="(?:\.\./)?LICENSE"')
IMG_RE = re.compile(r"<img\b[^>]*>")


def _doclink(m):
    name, lang, anchor = m.group(1), m.group(2), (m.group(3) or "")
    return 'href="%s-%s.html%s"' % (name.lower(), lang, anchor)


def rewrite_links(frag):
    frag = DOCS_PREFIX_RE.sub(_doclink, frag)          # docs/USAGE-en.md → usage-en.html
    frag = LINK_RE.sub(_doclink, frag)                 # USAGE-en.md → usage-en.html
    frag = SINGLE_LINK_RE.sub(
        lambda m: 'href="%s.html%s"' % (m.group(1).lower(), m.group(2) or ""), frag)
    frag = CREDITS_RE.sub('href="../index.html"', frag)
    frag = LICENSE_RE.sub('href="../index.html"', frag)
    return frag


def strip_badges(frag):
    """Remove <img> (e.g. README's external shields.io badges) — no external refs."""
    return IMG_RE.sub("", frag)


def wrap_tables(frag):
    frag = frag.replace("<table>", '<div class="table-wrap"><table>')
    frag = frag.replace("</table>", "</table></div>")
    return frag


TAG_RE = re.compile(r"<[^>]+>")
HEAD_RE = re.compile(r'<h([1-4])\s+id="([^"]*)"[^>]*>(.*?)</h\1>', re.DOTALL)


def strip_tags(s):
    s = TAG_RE.sub("", s)
    s = html.unescape(s)
    return re.sub(r"\s+", " ", s).strip()


def extract(frag):
    """Return (title, toc) where toc = list of (level, id, text) for h2/h3."""
    title = None
    toc = []
    for m in HEAD_RE.finditer(frag):
        lvl = int(m.group(1)); hid = m.group(2); text = strip_tags(m.group(3))
        if lvl == 1 and title is None:
            title = text
        if lvl in (2, 3):
            toc.append((lvl, hid, text))
    if toc:
        base = min(l for l, _, _ in toc)
        toc = [(("lv3" if l > base else "lv2"), hid, text) for (l, hid, text) in toc]
    return title or "BATHOS", toc


def esc(s):
    return html.escape(s, quote=True)


# DOCS (translated, per-lang) + SINGLES (single-language repo-root pages), in
# sidebar / home-card order. `"native" in d` distinguishes a single from a doc.
ALL = DOCS + SINGLES


def group_order():
    seen = []
    for d in ALL:
        if d["group"] not in seen:
            seen.append(d["group"])
    return seen


def single_href(slug):
    return "%s.html" % slug


def render_langtog(slug, cur, single=False):
    # slug None => home page toggle. single=True => only the native language is a
    # real page; the other two fall back to that language's docs home.
    out = ['<div class="langtog" role="group" aria-label="Language / 언어 / Idioma">']
    for l in LANGS:
        if single:
            href = single_href(slug) if l == cur else home_href(l)
        else:
            href = home_href(l) if slug is None else doc_href(slug, l)
        cura = ' aria-current="true"' if l == cur else ""
        out.append('<a href="%s"%s><span class="flag" aria-hidden="true">%s</span>%s</a>'
                   % (href, cura, FLAG[l], LANG_LABEL[l]))
    out.append("</div>")
    return "".join(out)


def render_sidebar(cur_slug, lang):
    groups = []
    for g in group_order():
        items = [d for d in ALL if d["group"] == g]
        lis = []
        for d in items:
            cur = ' aria-current="page"' if d["slug"] == cur_slug else ""
            if "native" in d:                       # single-language page
                href = single_href(d["slug"])
                label = '%s <span class="nat">· %s</span>' % (
                    esc(META[d["slug"]]["label"][lang]), NATIVE_TAG[d["native"]])
            else:
                href = doc_href(d["slug"], lang)
                label = esc(META[d["slug"]]["label"][lang])
            lis.append('<li><a href="%s"%s>%s</a></li>' % (href, cur, label))
        groups.append('<div class="grp"><div class="grp-t">%s</div><ul>%s</ul></div>'
                      % (esc(GROUP_LABEL[g][lang]), "".join(lis)))
    return "\n".join(groups)


def render_toc(toc, lang):
    if not toc:
        return ""
    lis = []
    for cls, hid, text in toc:
        lis.append('<li><a class="%s" href="#%s">%s</a></li>' % (cls, esc(hid), esc(text)))
    return ('<aside class="toc"><span class="cap">%s</span><ul>%s</ul></aside>'
            % (esc(HOME_STR["onpage"][lang]), "".join(lis)))


def render_pager(idx, lang):
    prev_d = DOCS[idx - 1] if idx > 0 else None
    next_d = DOCS[idx + 1] if idx < len(DOCS) - 1 else None
    left = ('<a href="%s"><span class="dir">← %s</span><span class="ttl">%s</span></a>'
            % (doc_href(prev_d["slug"], lang), esc(HOME_STR["prev"][lang]), esc(META[prev_d["slug"]]["label"][lang]))
            ) if prev_d else '<span class="ph"></span>'
    right = ('<a class="next" href="%s"><span class="dir">%s →</span><span class="ttl">%s</span></a>'
             % (doc_href(next_d["slug"], lang), esc(HOME_STR["next"][lang]), esc(META[next_d["slug"]]["label"][lang]))
             ) if next_d else '<span class="ph"></span>'
    return '<nav class="pager" aria-label="%s">%s%s</nav>' % (esc(HOME_STR["docs"][lang]), left, right)


PAGE = """<!doctype html>
<html lang="{htmllang}">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<meta name="description" content="{desc}">
<title>{title} · BATHOS docs</title>
<link rel="stylesheet" href="assets/docs.css">
</head>
<body>
<a class="skip" href="#main">{skip}</a>
<header class="topbar">
  <button class="menu-btn" type="button" aria-label="{menu}" aria-expanded="false">☰</button>
  <div class="brand"><a href="{home}">βάθος · <b>BATHOS</b></a></div>
  <span class="spacer"></span>
  <a class="homelink" href="../index.html">{landing}</a>
  {langtog}
</header>
<div class="scrim" aria-hidden="true"></div>
<div class="layout">
  <nav class="sidebar" aria-label="{docs}">
{sidebar}
  </nav>
  <main id="main" class="article">
    <article>
{badge}{body}
    </article>
{pager}
  </main>
{toc}
</div>
<footer class="foot"><span class="mono">βάθος · BATHOS</span> — {foot}</footer>
<script src="assets/docs.js"></script>
</body>
</html>
"""


def build_doc(idx, d, lang):
    src = os.path.join(DOCS_SRC, "%s-%s.md" % (d["src"], lang))
    frag = run_pandoc(src)
    frag = rewrite_links(frag)
    frag = wrap_tables(frag)
    title, toc = extract(frag)
    label = META[d["slug"]]["label"][lang]
    page = PAGE.format(
        htmllang=LANG_ATTR[lang],
        desc=esc(META[d["slug"]]["blurb"][lang]),
        title=esc(label),
        skip="Skip to content",
        menu=esc(HOME_STR["menu"][lang]),
        home=home_href(lang),
        landing=esc(HOME_STR["landing"][lang]),
        docs=esc(HOME_STR["docs"][lang]),
        langtog=render_langtog(d["slug"], lang),
        sidebar=render_sidebar(d["slug"], lang),
        badge="",
        body=frag,
        pager=render_pager(idx, lang),
        toc=render_toc(toc, lang),
        foot=esc(HOME_STR["foot"][lang]),
    )
    out = os.path.join(OUT, doc_href(d["slug"], lang))
    with open(out, "w") as f:
        f.write(page)
    return out


def build_single(d):
    """Render a single-language repo-root document (README/CLAUDE/ETHOS) once, in
    its native language. No translations, no prev/next within the DOCS sequence."""
    lang = d["native"]
    src = os.path.join(BATHOS, d["file"])
    frag = run_pandoc(src)
    frag = strip_badges(frag)                       # drop README's external shields.io <img> badges
    frag = rewrite_links(frag)
    frag = wrap_tables(frag)
    _title, toc = extract(frag)
    label = META[d["slug"]]["label"][lang]
    badge = '<p class="langbadge">%s</p>' % esc(NATIVE_WORD[d["native"]][lang])
    page = PAGE.format(
        htmllang=LANG_ATTR[lang],
        desc=esc(META[d["slug"]]["blurb"][lang]),
        title=esc(label),
        skip="Skip to content",
        menu=esc(HOME_STR["menu"][lang]),
        home=home_href(lang),
        landing=esc(HOME_STR["landing"][lang]),
        docs=esc(HOME_STR["docs"][lang]),
        langtog=render_langtog(d["slug"], lang, single=True),
        sidebar=render_sidebar(d["slug"], lang),
        badge=badge,
        body=frag,
        pager="",                                   # singles sit outside the wave-doc prev/next chain
        toc=render_toc(toc, lang),
        foot=esc(HOME_STR["foot"][lang]),
    )
    out = os.path.join(OUT, single_href(d["slug"]))
    with open(out, "w") as f:
        f.write(page)
    return out


HOME = """<!doctype html>
<html lang="{htmllang}">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<meta name="description" content="{desc}">
<title>{title}</title>
<link rel="stylesheet" href="assets/docs.css">
</head>
<body>
<header class="topbar">
  <div class="brand"><a href="../index.html">βάθος · <b>BATHOS</b></a></div>
  <span class="spacer"></span>
  <a class="homelink" href="../index.html">{landing}</a>
  {langtog}
</header>
<main class="home">
  <div class="eyebrow">{eyebrow}</div>
  <h1>{title}</h1>
  <p class="lede">{lede}</p>
{sections}
</main>
<footer class="foot"><span class="mono">βάθος · BATHOS</span> — {foot}</footer>
</body>
</html>
"""


def build_home(lang):
    secs = []
    for g in group_order():
        items = [d for d in ALL if d["group"] == g]
        cards = []
        for d in items:
            m = META[d["slug"]]
            if "native" in d:                       # single: lang-less href, native-word kicker
                href = single_href(d["slug"])
                kicker = esc(NATIVE_WORD[d["native"]][lang])
            else:
                href = doc_href(d["slug"], lang)
                kicker = esc(GROUP_LABEL[g][lang])
            cards.append(
                '<a class="card" href="%s"><div class="k">%s</div><h3>%s</h3><p>%s</p></a>'
                % (href, kicker, esc(m["label"][lang]), esc(m["blurb"][lang])))
        secs.append('<div class="grp-t">%s</div><div class="cards">%s</div>'
                    % (esc(GROUP_LABEL[g][lang]), "".join(cards)))
    page = HOME.format(
        htmllang=LANG_ATTR[lang],
        desc=esc(HOME_STR["lede"][lang]),
        title=esc(HOME_STR["title"][lang]),
        landing=esc(HOME_STR["landing"][lang]),
        langtog=render_langtog(None, lang),
        eyebrow=esc(HOME_STR["eyebrow"][lang]),
        lede=esc(HOME_STR["lede"][lang]),
        sections="\n".join(secs),
        foot=esc(HOME_STR["foot"][lang]),
    )
    with open(os.path.join(OUT, home_href(lang)), "w") as f:
        f.write(page)


def main():
    os.makedirs(OUT, exist_ok=True)
    count = 0
    for idx, d in enumerate(DOCS):
        for lang in LANGS:
            build_doc(idx, d, lang)
            count += 1
    for lang in LANGS:
        build_home(lang)
        count += 1
    for d in SINGLES:
        build_single(d)
        count += 1
    print("built %d pages -> %s" % (count, OUT))


if __name__ == "__main__":
    main()
