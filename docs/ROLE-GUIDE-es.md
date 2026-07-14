# BATHOS — Personalización de roles (overrides de 3 capas)

> Ajusta los 17 roles a tu proyecto (idioma, rutas propias, plataforma, tono) **sin tocar las definiciones base**. Lo que cambias es *cómo* un rol aplica su oficio — no *quién* es el rol.
>
> **Ver también:** [Escribir módulos (MODULE-GUIDE-es)](MODULE-GUIDE-es.md) · [Uso (USAGE-es)](USAGE-es.md) · 한국어: [`ROLE-GUIDE-kr.md`](ROLE-GUIDE-kr.md) · English: [`ROLE-GUIDE-en.md`](ROLE-GUIDE-en.md)

---

## 0. Las tres capas

La definición efectiva de un rol es la fusión ordenada de tres capas:

| Capa | Ubicación | Posee |
|------|-----------|-------|
| **base** | `.claude/agents/_base/NN-*.md` | Identidad fija — nombre · trasfondo · modelo + estándares de oficio de primer nivel / filosofía / DoD |
| **team** | Overrides del proyecto | Rutas propias · plataforma (web/iOS…) · énfasis de dominio · stack tecnológico · umbrales NFR · tono y voz |
| **user** | Overrides personales | Idioma · nivel de detalle · intensidad de facilitación |

**Regla de fusión:** los escalares se sobrescriben (user > team > base), los arrays se anexan. Así el usuario fija el idioma, el equipo fija las rutas propias, y ambos se componen sobre la identidad base fija.

## 1. Lo que no se puede sobrescribir
La capa base fija **nombre · trasfondo · modelo** más los estándares de oficio. Es inmutable a propósito — es lo que mantiene a los roles "de primer nivel" y consistentes. Sobrescribe *cómo aplican el oficio a tu proyecto*, no *quiénes son*.

## 2. Anatomía de un rol base
Cada archivo base (`.claude/agents/_base/NN-name.md`) es frontmatter YAML + cuerpo:

```yaml
---
role_number: 4
name: james
slug: james-architect          # ← el slug de agent-type usado para el spawn
model: opus                     # opus = Opus 4.8 · sonnet = Sonnet 5
wave: W2 (tras completar Joshua)
spawnable: true                 # Paul (#0) es false — el líder es la sesión principal
tools: [Read, Grep, Glob, Bash, Write, WebFetch, WebSearch]
---
```
Secciones del cuerpo (estandarizadas tras la mejora): Identidad · §0 Filosofía · Misión y entregables · Estándares de oficio (innegociables) · Antipatrones · Proceso · DoD · Nota de 3 capas.

## 3. Añadir un override de equipo (ejemplos)
Crea una nota de capa team que se compone sobre el rol base. Overrides de equipo habituales:
- **Rutas propias** — p. ej. Andrew (#9) = `web/`, Phillip (#8) = `api/` (solapamiento cero → seguro en paralelo).
- **Plataforma/stack** — "web = Next.js", "backend = Go + Postgres".
- **Umbrales NFR** — "p95 < 150ms", "bundle < 200KB".
- **Énfasis de dominio/tono** — voz de marca, foco regulatorio.

Los overrides de equipo deben limitarse a *hechos específicos del proyecto*; deja los estándares de oficio a la base.

## 4. Añadir un override de usuario
Ajustes personales que atraviesan proyectos:
- **Idioma** — salida en coreano/inglés.
- **Nivel de detalle** — conciso vs. detallado.
- **Facilitación** — cuán proactivamente los roles hacen sugerencias (User Sovereignty siempre aplica).

## 5. Generar (spawn) roles
El líder (Paul) genera compañeros por `slug` (agent-type), pasando rutas de entrada, rutas propias y el DoD. Ejemplo: "Genera `james-architect`, lee `03-service-planning/`, posee `04-architecture/`, sigue ETHOS.md." Concurrencia ≤ 3; apagar al final de la wave.

## 6. Buenos hábitos
Unos pocos hábitos mantienen la personalización limpia, segura en paralelo y fiel a la base:

- **No hagas fork de la base** para cambiar hechos del proyecto — usa las capas team/user.
- **Mantén las rutas propias disjuntas** — asigna rutas sin solapamiento a los roles paralelos (freeze-guard lo hace cumplir).
- **Mantén el listón elevado** — al añadir un rol nuevo, sigue la estructura de primer nivel de 6 secciones (filosofía · estándares de oficio · antipatrones · proceso · DoD).
- Si lo que necesitas es un *pack de dominio* y no un *rol*, usa un plug ([`MODULE-GUIDE-es.md`](MODULE-GUIDE-es.md)).

---

<div align="center">한국어: <a href="ROLE-GUIDE-kr.md">ROLE-GUIDE-kr</a> · English: <a href="ROLE-GUIDE-en.md">ROLE-GUIDE-en</a></div>
