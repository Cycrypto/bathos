# BATHOS — Arquitectura (para contribuidores)

> Un mapa para contribuidores de cómo está construido el motor y qué garantiza. Léelo antes de tocar `core/` o los hooks — este documento es precisamente lo que evita que un cambio rompa una garantía en silencio.
>
> **Ver también:** [Conceptos (FEATURES-es)](FEATURES-es.md) · [Uso (USAGE-es)](USAGE-es.md) · 한국어: [`ARCHITECTURE-kr.md`](ARCHITECTURE-kr.md) · English: [`ARCHITECTURE-en.md`](ARCHITECTURE-en.md)

---

## 1. Los dos planos de ejecución

BATHOS se divide limpiamente en dos planos de ejecución. Interiorizar esta distinción es lo primero que hay que hacer antes de leer el código.

| Plano | Qué | Dónde |
|-------|-----|-------|
| Orquestación | Comandos slash · definiciones de roles · hooks (markdown/bash) | `.claude/` |
| Motor | Un único binario Rust estático `bathos` | `core/` |

**Lo que debe ser confiable y reproducible** vive en el motor (con tests unitarios); lo que editan las personas vive en el plano de orquestación. Los hooks y comandos invocan `bathos <subcommand>` por debajo.

## 2. Workspace de Rust (`core/`, 7 crates)

El motor es un único workspace de Cargo con 7 crates — un crate por cada responsabilidad de módulo.

| Crate | Módulo | Responsabilidad |
|-------|--------|-----------------|
| `bathos-state` | M1 | SSOT: `manifest.json` (validación de esquema · escrituras atómicas) + cadena de hashes de auditoría tamper-evident |
| `bathos-router` | M2 | Enrutamiento Scale-Adaptive Lv0–4 (recomendar/confirmar) |
| `bathos-wave-engine` | M3 | Transiciones de estado de 7 waves; tope de concurrencia |
| `bathos-gate-engine` | M4 | Veredictos PASS/CONCERNS/FAIL; critical→FAIL |
| `bathos-story-engine` | M5 | Compilación de historias (D1/D2/D3) — zero-context-loss |
| `bathos-plug` | M12 | Gestor de módulos plug (module.yaml · DSL de triggers) |
| `bathos-cli` | bin | El binario `bathos` — despacho de subcomandos |

Dirección de dependencias: `bathos-cli` → crates del motor → `bathos-state`. **Ningún crate del motor depende de `bathos-plug`** (invariante A9).

## 3. Invariantes (no romper)

Estas son las garantías que sostienen la estructura, impuestas por código, no por convención. Si modificas código relacionado con alguna de ellas, mantén honesto el punto de código que la hace cumplir.

| ID | Invariante | Impuesta por |
|----|------------|--------------|
| **A9** | El core nunca depende de módulos plug | Grafo de dependencias de crates (verificado con `cargo tree`) |
| Gate FACILITATOR | El `facilitator` de un veredicto nunca puede estar vacío; no hay auto-PASS sin fundamento | `bathos-gate-engine` |
| `critical > 0 → FAIL` | Un solo issue critical significa FAIL | `bathos-gate-engine` |
| Concurrencia ≤ 3 | `MAX_CONCURRENT_ROLES = 3`; el 4.º spawn se rechaza | `bathos-wave-engine` (`E-CONCURRENCY`) |
| Cadena de auditoría | `hash_prev[n] == hash_self[n-1]`, ancla genesis, `seq` monótono, escritor único | `bathos-state::audit` (`bathos audit verify`) |
| Escrituras atómicas | El manifest nunca queda escrito a medias | `bathos-state::store` |
| Completitud de historia (D1) | 6 secciones obligatorias + `developer_context` no vacío | `bathos-story-engine` (`E-CTX-LOSS`) |
| FAIL en W3 bloquea W5 | Un veredicto FAIL bloquea físicamente la entrada a implementación | `gate-enforce.sh` (exit 2) |

## 4. Códigos de salida y taxonomía de errores

Los errores se manifiestan de dos formas — códigos de salida de proceso que los hooks usan para ramificar, y E-codes simbólicos que dan nombre a los fallos.

- **Códigos de salida:** `0` éxito · `1` error · **`2` gate FAIL** (los hooks lo usan para bloquear).
- **E-codes:** `E-LEVEL-DRIFT`, `E-CONCURRENCY`, `E-CTX-LOSS`, `E-STALE`, `E-STATE-CORRUPT`, `E-AUDIT-TAMPER`, `E-PLUG-NOTFOUND`.

## 5. Puntuación del router (determinista)

El router convierte un perfil de stakes en un nivel usando solo aritmética pura — sin heurísticas, así que la misma entrada siempre produce la misma recomendación.

`scope` (0–3: bug=0, feature=1, large/module=2, product/platform=3, incierto=1) + `novelty` (+1) + `regulation_ip` (+2) + `team_size` (0–2: solo=0, medium=1, large=2). Total → nivel: 0→Lv0, 1→Lv1, 2–3→Lv2, 4–5→Lv3, 6+→Lv4. Recomendación y confirmación están separadas (User Sovereignty).

## 6. Hooks (`.claude/hooks/`)

Seis hooks fail-safe enlazados a eventos de Claude Code por `settings.json`: `careful-guard` · `freeze-guard` · `audit-log` · `artifact-verify` · `gate-enforce` · `next-action`. Invocan el motor vía `$BATHOS_BIN`. Autoverificación: `bash .claude/hooks/_test-hooks.sh` (46 tests de determinismo). **Solo nombres de eventos válidos en el bloque `hooks`** — una clave de comentario mezclada envía el arranque de subagentes a una espera infinita (`bathos doctor` lo detecta).

## 7. ADRs clave

- **ADR-0006:** motor core = Rust (binario estático único); hooks = bash; comandos/roles/assets = markdown; configuración = JSON/YAML.
- El conjunto completo de ADRs vive en el registro de diseño (`.agent-team/04-architecture/adr/`).

## 8. Reglas de contribución al motor

1. Mantén el core esbelto — las nuevas funcionalidades de dominio van a `modules/`, nunca al core (A9).
2. `cd core && cargo test && cargo clippy --all-targets -- -D warnings` debe estar en verde, y añade tests de invariantes.
3. Ejecuta `cargo fmt --all` antes de commitear.
4. Si cambias hooks, ejecuta `bash .claude/hooks/_test-hooks.sh` y preserva el determinismo y la seguridad fail-safe.
5. CI (`.github/workflows/ci.yml`): fmt · clippy(-D) · test · release · jq · harness de hooks.

---

<div align="center">한국어: <a href="ARCHITECTURE-kr.md">ARCHITECTURE-kr</a> · English: <a href="ARCHITECTURE-en.md">ARCHITECTURE-en</a></div>
