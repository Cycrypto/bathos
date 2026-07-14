# Caso de uso de BATHOS — construir un servicio nuevo desde cero, paso a paso

> **Objetivo de este documento:** mostrar de la forma más sencilla posible el proceso de construir *un servicio nuevo real desde cero* con BATHOS. La instalación usa el modo **"adoptar en mi proyecto"** (B):
>
> ```bash
> # B) Compilar el motor + copiar el paquete de método a mi proyecto
> ./install.sh --into /abs/path/to/your-project
> ```
>
> Seguiremos un ejemplo concreto hasta el final: **"ReadShelf"** — una pequeña webapp para registrar los libros leídos y compartir reseñas cortas con amigos. Al terminar de leer sabrás exactamente *qué escribes tú, qué hace BATHOS en cada paso y qué archivos aparecen en disco*.
>
> **Ver también:** si te interesan los principios, fija los conceptos con [`FEATURES-es.md`](FEATURES-es.md) → y el detalle de CLI · hooks está en la referencia [`USAGE-es.md`](USAGE-es.md). · 한국어: [`USECASE-kr.md`](USECASE-kr.md) · English: [`USECASE-en.md`](USECASE-en.md)

---

## 0. Modelo mental de 30 segundos

BATHOS **no es una app que se ejecuta** — es **un paquete de método que corre sobre Claude Code**. Tú:

1. **Lo instalas en tu proyecto** (copia de `.claude/` + `assets/` + `modules/`, y variable de entorno con la ruta del motor).
2. **Abres Claude Code en tu proyecto** y escribes **comandos slash** (`/team-kickoff`, `/route`, `/wave1-discovery`, …).
3. BATHOS levanta "compañeros" especialistas por wave, escribe los artefactos de trabajo en `.agent-team/`, y **el código real del producto se acumula en su lugar de siempre — `src/`**.

Eso es todo. El resto de este documento es simplemente hacer estas tres cosas, despacio, con un ejemplo real.

---

## 1. El servicio de ejemplo: "ReadShelf"

| | |
|---|---|
| **Qué** | Webapp: registrar libros leídos → escribir reseñas cortas → seguir a amigos → ver el feed de lectura de los amigos. |
| **Quién** | Un desarrollador en solitario (tú). |
| **Stack (plan)** | Next.js (front) · API Node/Express · PostgreSQL. Sin AI/ML en el MVP. |
| **Ambición** | No una mega-plataforma: primero un **MVP enfocado**. |

Recuérdalo: ReadShelf es un producto *nuevo pero enfocado*. Esta elección será importante en el paso 4, al elegir el "nivel" de BATHOS.

---

## 2. Preparación previa (una sola vez)

- **Claude Code v2.1.32+** + la funcionalidad experimental **Agent Teams**.
- **Toolchain de Rust** (cargo 1.92+) — solo hace falta para compilar el motor una vez.
- **`jq`** — lo usan los hooks de seguridad.
- Shell de macOS/Linux.

El propio paquete BATHOS también debe estar en algún lugar del disco. Aquí asumimos que lo clonaste en `~/tools/bathos`.

---

## 3. Paso 1 — instalar BATHOS en mi proyecto (modo "B")

Crea una carpeta de proyecto vacía y, **desde el repo de BATHOS**, ejecuta el script de instalación con `--into` apuntando a tu proyecto:

```bash
# Ubicación del proyecto nuevo (puede estar vacío o ser un repo existente)
mkdir -p ~/projects/readshelf

# Desde el paquete BATHOS:
cd ~/tools/bathos
./install.sh --into ~/projects/readshelf
```

Lo que hace este comando (y lo que **no** hace):

- Compila el motor una vez → `~/tools/bathos/core/target/release/bathos` (~5.6MB).
- Copia **3 carpetas** a `~/projects/readshelf/`:
  - `.claude/` — comandos slash, definiciones de los 17 roles, 6 hooks de seguridad, `settings.json`
  - `assets/` — plantillas/workflows/checklists que usan Wave 3 y los plugs
  - `modules/` — módulos plug opcionales (`ip-pack`, `research-pack`)
- **Nunca borra nada**, y si ya existe un `.claude/` no lo sobrescribe sin `--force`.
- **No copia** el binario compilado al proyecto. El motor se queda en el repo de BATHOS y tu proyecto solo lo apunta (siguiente paso).

Ahora conecta las dos variables de entorno (el script de instalación imprime la guía):

```bash
# Para que los hooks encuentren el motor:
export BATHOS_BIN="$HOME/tools/bathos/core/target/release/bathos"

# Activar Agent Teams (ya viene en el settings.json copiado, pero explícito):
export CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS=1
```

> Si pones esas dos líneas `export` en tu perfil de shell, se aplican en cada sesión.

El proyecto ahora se ve así:

```
~/projects/readshelf/
├── .claude/        ← (recién instalado) comandos · roles · hooks · settings
├── assets/         ← (recién instalado) plantillas · workflows · checklists
└── modules/        ← (recién instalado) ip-pack · research-pack
```

Todavía no hay nada más — ni `.agent-team/` ni `src/`. Aparecen en los siguientes pasos.

---

## 4. Paso 2 — abrir Claude Code y hacer el kickoff

Abre una sesión de Claude Code en el directorio de tu proyecto (`~/projects/readshelf`). Esa sesión es **el líder, "Paul"**. Y entonces:

```text
/team-kickoff
```

**Qué ocurre:** BATHOS crea el área de trabajo y la fuente única de verdad.

**Lo que aparece en disco:**

```
~/projects/readshelf/
├── .claude/  assets/  modules/        (lo instalado)
└── .agent-team/                       ← nuevo
    ├── 00-plan/        (charter, task graph)
    ├── 01-reverse/ … 12-report/       (carpetas de salida por rol)
    └── _state/
        └── manifest.json              ← SSOT: todo el estado del proyecto, validado por esquema
```

> **Dos directorios, dos propósitos.** `.agent-team/` son las *notas de trabajo* de BATHOS (planificación · diseño · reviews · reportes · estado). Tu *código de servicio real* se acumula en `src/`. No se mezclan.

---

## 5. Paso 3 — fijar el nivel con `/route` (decides tú)

BATHOS no ejecuta todas las waves para cualquier trabajo. Ajusta el esfuerzo al tamaño del trabajo. Cuéntale a BATHOS los "stakes" de ReadShelf:

```text
/route /Users/you/projects/readshelf
```

Internamente el motor puntúa cuatro ejes:

| Eje | Respuesta para ReadShelf | Razón |
|-----|--------------------------|-------|
| `scope` | `feature` | MVP enfocado, no una mega-plataforma → 1 punto |
| `novelty` | `true` | Idea de producto nueva → +1 |
| `regulation_ip` | `false` | No es un dominio centrado en regulación/patentes → +0 |
| `team_size` | `solo` | Solo tú → +0 |

Total = **2 → recomendación Level 2.** El motor devuelve algo así:

```json
{ "recommended_level": 2, "wave_set": ["W1","W2","W3","W5","W6"],
  "requires_confirmation": true }
```

> **Esta es tu decisión (User Sovereignty).** BATHOS *solo recomienda*. Tú tienes que confirmar, y solo la confirmación queda registrada:
>
> ```bash
> bathos -s .agent-team/_state route decide \
>   --stakes-json '{"scope":"feature","novelty":true,"regulation_ip":false,"team_size":"solo"}' \
>   --confirm 2
> ```
>
> Ahora `manifest.json → current_level = 2`.

**Lo que Level 2 significa para ReadShelf** — estas waves corren en orden:

```
W1 discovery → W2 diseño → W3 story gate ★ → W5 implementación → W6 verificación
```

(Con Level 0 sería solo el arreglo de bug; con Level 3–4 se añaden el análisis de mercado de W0 y el IP/investigación de W4 — excesivo para un MVP enfocado. Si ReadShelf crece luego hasta una gran plataforma multi-módulo, basta con re-enrutar a Lv3.)

---

## 6. Paso 4 — Wave 1: discovery & mercado (¿qué construimos?)

```text
/wave1-discovery /Users/you/projects/readshelf
```

**Quién trabaja:** aquí el protagonista es Caleb (analista de mercado). John (especialista en reverse) solo tiene sentido si le das un codebase de *referencia* para estudiar — en una app greenfield puedes omitirlo, o darle el repo open source de un servicio competidor.

**Qué hace:** Caleb investiga apps similares de lectura/social, encuentra los huecos y propone la **USP** (la razón para usar ReadShelf).

**Qué obtienes:** `.agent-team/02-market-analysis/` — paisaje competitivo, recomendación de USP.

**Gate:** *USP Readiness.* Antes de avanzar, tú (el líder) confirmas que la USP tiene sentido.

> Al final de cada wave, los compañeros que trabajaron se apagan antes de la siguiente. Mantener los activos concurrentes ≤ 3 ahorra tokens.

---

## 7. Paso 5 — Wave 2: planificación · arquitectura · diseño

```text
/wave2-design /Users/you/projects/readshelf
```

Esta wave corre en un orden deliberado:

1. **Joshua (planificación de servicio)** convierte la USP en **Core Features + User Stories + Service Story** concretas. *Su salida es el gate* — nada más arranca hasta que la planificación esté sólida.
2. Luego, en paralelo:
   - **James (arquitecto)** diseña el modelo de datos (ERD) · contratos de API · secuencias de servicio · manejo de excepciones — p. ej. tablas `users`, `books`, `reviews`, `follows`; `POST /reviews`, `GET /feed`, etc.
   - **Jonnathan (diseñador)** diseña los flujos UX y la UI — registro, estantería (shelf), editor de reseñas, feed de amigos.

**Qué obtienes:** `.agent-team/03-service-planning/`, `.agent-team/04-architecture/`, `.agent-team/07-design/`.

**Gate:** *Plan Readiness.*

> Refuerzo opcional: antes de fijar la planificación puedes ejecutar gates de revisión como `/plan-ceo-review` (¿esto es un producto de 10?) o `/plan-eng-review` (arquitectura/casos límite/tests). Un revisor a la vez.

---

## 8. Paso 6 — Wave 3: story gate ★ (el corazón de BATHOS)

```text
/wave3-story-gate /Users/you/projects/readshelf
```

Este es el paso que hace diferente a BATHOS. El punto donde se derrumban la mayoría de intentos de "que la IA me construya la app" es justamente **diseño → implementación**: quien construye olvida la mitad del diseño o directamente nunca la vio. Wave 3 cierra esa grieta.

**Quién trabaja:** **Matthew (#17, story engineer)** + los verificadores *independientes* **Thomas** y **Matthias**.

**Lo que hace Matthew:** condensa todo el diseño de W2 en **archivos de historia dev autocontenidos** bajo `.agent-team/03-story-engineering/`. Cada historia (p. ej. `story-1-2-write-review-es.md`) está escrita para que el implementador pueda arrancar *solo con la historia*. Lo que el motor hace cumplir:

- Deben existir las **6 secciones obligatorias** y `developer_context` no puede estar vacío (`story_requirements`, `developer_context`, `architecture_compliance`, `library_framework_requirements`, `file_structure_requirements`, `testing_requirements`). Falta una → la compilación falla con **`E-CTX-LOSS`**.
- Cada afirmación técnica lleva la marca **`[Source: …]`** → trazable hasta los documentos de arquitectura/diseño.

**El gate (aquí está lo importante):** Thomas y Matthias revisan las historias de forma independiente y el gate devuelve **PASS / CONCERNS / FAIL**:

- **PASS** → se avanza a implementación.
- **CONCERNS** → se avanza, pero registrando el riesgo.
- **FAIL** → el hook (`gate-enforce`) **bloquea físicamente el arranque de Wave 5** (el motor sale con exit code 2). Literalmente no se puede empezar a implementar sobre un gate de readiness fallido. Corrige las historias y re-gatea.

> Es la implementación física de "generación ≠ verificación": el mismo modelo no puede colar su propio trabajo.

---

## 9. Paso 7 — Wave 5: implementación (escribir el código de verdad)

```text
/wave5-implement /Users/you/projects/readshelf
```

**Quién trabaja (solo los roles que ReadShelf necesita):**

- **Phillip (backend)** — API Express + esquema/migraciones de PostgreSQL, autenticación, endpoints `/reviews` · `/feed`.
- **Andrew (front/móvil)** — la UI Next.js basada en el diseño de Jonnathan, conectada a los contratos de API de James.
- *(El principal de AI/ML, Stephen, **no se genera** — no hay ML en el MVP de ReadShelf.)*

**Adónde va el código:** al árbol de fuentes real — `~/projects/readshelf/src/` (y `api/`, `web/`, etc., donde viva el código del proyecto). Cada compañero edita **solo sus rutas propias**, así que el trabajo de backend y front no colisiona.

```
~/projects/readshelf/
├── src/  api/  web/ …      ← el código real aparece aquí, historia por historia
└── .agent-team/08-impl-notes/   ← notas de implementación (backend.md, frontend.md)
```

**Gate:** se verifica el cierre por historia y entonces esa historia se considera done.

> Los hooks de seguridad están activos todo el tiempo: `careful-guard` bloquea los comandos de shell destructivos (`rm -rf`, `DROP TABLE` …) y `freeze-guard` encierra a cada compañero dentro de sus rutas propias.

---

## 10. Paso 8 — Wave 6: verificación · docs · reporte

```text
/wave6-verify-report /Users/you/projects/readshelf
```

**Quién trabaja:** **Thomas** (revisión de código), **Timothy** (documentación de desarrollo), **Matthias** (QA / tests E2E) — en paralelo — y luego **Martin** agrega todo en un único **reporte HTML**.

**Qué obtienes:** `.agent-team/10-review/`, `.agent-team/09-docs/`, `.agent-team/11-qa/`, `.agent-team/12-report/report.html`.

**Gate:** *Release Readiness* (PASS / CONCERNS / FAIL). Si la revisión independiente encuentra un defecto bloqueante, se corrige y se re-gatea — exactamente el mismo bucle que BATHOS atravesó consigo mismo.

---

## 11. Paso 9 — confirmación final

```text
/team-confirm
```

El líder hace el sign-off y limpia a los compañeros. Lo que ahora tienes:

```
~/projects/readshelf/
├── src/ api/ web/ …                  ← el servicio ReadShelf funcionando
└── .agent-team/
    ├── 02-market-analysis/  (USP)
    ├── 03-service-planning/ (features + historias)
    ├── 04-architecture/     (ERD, API, secuencias)
    ├── 03-story-engineering/(archivos de historia dev)
    ├── 07-design/           (UX/UI)
    ├── 08-impl-notes/       (cómo se construyó)
    ├── 10-review/ 11-qa/    (verificación independiente)
    ├── 12-report/report.html
    └── _state/              (manifest.json, wave-log.md, signoff.md)
```

Todo es trazable: mercado → USP → features → arquitectura → historias → código → verificación.

---

## 12. El flujo completo en una página

```text
# Una sola vez
cd ~/tools/bathos && ./install.sh --into ~/projects/readshelf
export BATHOS_BIN="$HOME/tools/bathos/core/target/release/bathos"
export CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS=1

# Dentro de Claude Code abierto en ~/projects/readshelf:
/team-kickoff                                   # esqueleto de .agent-team + manifest
/route          /Users/you/projects/readshelf   # → recomienda Lv2, tú das --confirm 2
/wave1-discovery   /Users/you/projects/readshelf # Caleb: mercado + USP
/wave2-design      /Users/you/projects/readshelf # Joshua → James + Jonnathan
/wave3-story-gate  /Users/you/projects/readshelf # ★ historias + gate de readiness (FAIL bloquea la implementación)
/wave5-implement   /Users/you/projects/readshelf # Phillip + Andrew → código en src/
/wave6-verify-report /Users/you/projects/readshelf # review + QA + docs → report.html
/team-confirm                                   # sign-off + limpieza

# Revisión de progreso en cualquier momento:
/team-status

# Guardar todo antes de parar — reanudar en la siguiente sesión:
/save            # o simplemente "guardar" / "checkpoint"
/resume          # siguiente sesión: o simplemente "continuar" / "cargar"
```

---

## 13. Consejos y trampas comunes

- **Guarda antes de parar, reanuda al volver.** Un build real como ReadShelf abarca varias sesiones. Termina cada sesión con **`/save`** (una palabra — mete estado del motor · git · decisiones · wave en curso · siguiente comando en `_state/SESSION-SNAPSHOT.md`). Empieza la siguiente con **`/resume`**. Funciona en lenguaje natural: "guardar"/"save", "continuar"/"resume". Eso sí, al reanudar los compañeros no reviven — vuelve a ejecutar el comando `/waveN-…` correspondiente (sin pérdidas gracias a los artefactos en disco).
- **Compañeros concurrentes ≤ 3.** El costo en tokens es proporcional al número de roles activos. BATHOS ya secuencia las waves; no intentes ejecutarlo todo a la vez.
- **Genera solo los roles necesarios.** Sin ML, omite a Stephen. En greenfield, omite a John (reverse) o dale un repo de referencia.
- **El que decide eres tú.** Nivel · USP · planificación · veredictos de gate — BATHOS recomienda, tú confirmas. Cuando BATHOS recomienda un cambio de dirección debe presentar "recomendación + fundamento + contexto omitido", y no puede ejecutarlo por su cuenta.
- **FAIL detiene la implementación de verdad.** Si `/wave5-implement` no arranca, revisa el veredicto de Wave 3: `bathos -s .agent-team/_state gate show`. Corrige las historias y re-gatea.
- **Nada de claves de comentario en el bloque `hooks` de `settings.json`** — provocan una espera infinita al arrancar los compañeros.
- **El traspaso es solo por disco.** Los compañeros no comparten el historial de conversación del líder; todo se transmite por archivos de `.agent-team/`. No es una limitación sino una funcionalidad (zero context loss).
- **Si un compañero parece "detenido" sin salida**, normalmente es por el límite de uso de la cuenta, no por un bug — re-genera tras el reinicio del límite; los artefactos en disco se preservan.

---

## 14. ¿Y si ReadShelf fuera más grande?

| Si ReadShelf fuera… | Cambio de stakes | Nivel | Waves añadidas |
|---------------------|------------------|-------|----------------|
| Un arreglo de bug de una línea | scope=bug | **Lv0** | Solo W5 (+ W6 ligera) |
| Un MVP enfocado *(este documento)* | scope=feature, novelty | **Lv2** | W1+W2+W3+W5+W6 |
| Un producto/plataforma nuevo en serio | scope=product, team=large | **Lv3** | Se añade **W0** (análisis) + **W4** opcional (IP/investigación) |
| Un producto centrado en regulación/patentes | + regulation_ip=true | **Lv4** | W0–W6 completas + **W4 obligatoria** |

Para activar un plug (p. ej. el borrador de patente de un producto Lv3+):

```bash
bathos -s .agent-team/_state --modules-dir modules plug enable ip
```

---

## 15. Licencia

BATHOS es MIT, reimplementado de forma independiente desde primeros principios tras un análisis inverso cuidadoso de [BMAD-METHOD](https://github.com/bmad-code-org/BMAD-METHOD) — con respeto por el trabajo previo que le sirvió de base, y sin usar sus marcas. Texto completo: [`../README.md`](../README.md).

---

<div align="center">

**BATHOS** · βάθος — profundidad, no superficie
한국어 [`USECASE-kr.md`](USECASE-kr.md) · English [`USECASE-en.md`](USECASE-en.md)

</div>
