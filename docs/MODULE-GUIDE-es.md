# BATHOS — Guía para escribir módulos plug personalizados

> ¿Quieres extender BATHOS con tus propias funcionalidades de dominio (p. ej. un pack de auditoría de seguridad, un pack de diseño de juegos) **sin tocar el core**? Para eso existen exactamente los módulos plug. Esta guía te acompaña de principio a fin: escribir el módulo → instalarlo → activarlo → ejecutarlo.
>
> **Ver también:** [Conceptos (FEATURES-es)](FEATURES-es.md) · [Uso (USAGE-es)](USAGE-es.md) · 한국어: [`MODULE-GUIDE-kr.md`](MODULE-GUIDE-kr.md) · English: [`MODULE-GUIDE-en.md`](MODULE-GUIDE-en.md)

---

## 0. Qué es un módulo (y la única regla)

El **core de BATHOS se mantiene esbelto**. Las funcionalidades específicas de dominio se entregan como **módulos plug opt-in** bajo `modules/`. Por defecto se incluyen dos: `ip-pack` (especificaciones de solicitud de patente) · `research-pack` (Abstract/Intro académicos).

> **La única regla (invariante A9): el core nunca depende de los módulos.** Los módulos se descubren y activan en tiempo de ejecución; el motor tiene cero conocimiento en tiempo de compilación de ningún módulo concreto. Esto es lo que mantiene el core pequeño y permite añadir dominios libremente.

Un módulo aporta workflows, plantillas y un directorio de salida a la **Wave 4 (IP · Investigación)** — la wave plug opcional, fuera de la línea principal. Se activa con `bathos plug enable <id>` y el estado se persiste en `manifest.modules[]`.

---

## 1. Anatomía de un módulo

Un módulo es, al final, solo un directorio bajo `modules/<your-id>/` — esta es la estructura a la que apuntar:

```
modules/security-pack/
├── module.yaml            # obligatorio — la autodeclaración del módulo (el contrato)
├── README.md              # recomendado — qué es, cómo usarlo
├── workflows/             # procedimientos que provee el módulo
│   └── threat-model.md
├── templates/             # plantillas de salida que los workflows rellenan
│   └── threat-model-report.md
└── checklists/            # (opcional) checklists de calidad / adversariales
    └── threat-model-quality.md
```

En rigor, lo único que el motor necesita para **descubrir** un módulo es `module.yaml`; el resto existe para ayudar al rol que ejecuta el módulo a *usarlo bien*.

---

## 2. El contrato de `module.yaml`

`module.yaml` es donde el módulo se presenta ante el motor. Abajo está el esquema exacto que el motor (`bathos-plug`) parsea en realidad, con cada campo anotado:

```yaml
module_id: security               # obligatorio — id único (ip | research | game | security ...)
name: Security Pack               # obligatorio — nombre legible por humanos
wave: W4                          # obligatorio — la wave a la que se enchufa (W4)
trigger: "Lv>=4 OR domain=security"  # obligatorio — condición de auto-activación (DSL abajo)
enabled_default: false            # opcional (por defecto false)
provides:                         # opcional
  workflows: [threat-model]       #   basenames de archivos bajo workflows/
  templates: [threat-model-report]#   basenames de archivos bajo templates/
outputs: ".agent-team/13-security/"  # opcional — dónde escribe sus salidas este módulo
evidence_trace: true              # opcional (por defecto false) — exige trazado de evidencia [Source:]
```

| Campo | ¿Obligatorio? | Significado |
|-------|:-------------:|-------------|
| `module_id` | ✓ | Id único usado por `plug enable/disable <id>` y los triggers (`domain=<id>`) |
| `name` | ✓ | Nombre mostrado por `plug list` |
| `wave` | ✓ | La wave en la que corre el módulo (actualmente `W4`) |
| `trigger` | ✓ | Condición de auto-activación (§3) |
| `enabled_default` | — | ¿Activado por defecto? (por defecto `false`) |
| `provides.workflows` | — | Basenames de los workflows provistos |
| `provides.templates` | — | Basenames de las plantillas provistas |
| `outputs` | — | Directorio de salida (elige un nuevo `.agent-team/NN-<name>/`) |
| `evidence_trace` | — | Si es `true`, las afirmaciones deben llevar marcador `[Source:]` (recomendado) |

> Para `outputs`, elige un número nuevo que no colisione con los directorios de rol existentes (`00`–`12` están ocupados) — p. ej. `13-security/`.

---

## 3. El DSL de triggers

`trigger` le dice al router *cuándo* auto-activar este módulo. Sintaxis (parseada por `bathos-plug`):

- **Nivel:** `Lv>=N` · `Lv>N` · `Lv<=N` · `Lv<N` · `Lv=N` (N = 0–4)
- **Dominio:** `domain=X` (o `domain:X`) — verdadero cuando el dominio del proyecto coincide con `X` (normalmente el `module_id`)
- **Combinación:** une términos con ` OR ` — se activa si **cualquiera** es verdadero.
- **Seguridad:** los tokens no interpretables evalúan a `false` (conservador).

Ejemplos:
- `"Lv>=3 OR domain=ip"` — activo para trabajos grandes/enterprise, o cuando el dominio es explícitamente IP.
- `"Lv>=4 OR domain=security"` — activo solo para builds enterprise/regulados o un dominio security explícito.
- `"domain=game"` — activo solo cuando el dominio del proyecto es `game` (el nivel por sí solo nunca lo auto-activa).

> Los triggers son *pistas de auto-activación*. El usuario siempre conserva el control (User Sovereignty): puedes hacer `plug enable`/`disable` independientemente, y verificas qué corre realmente.

---

## 4. Paso a paso — construyendo un "security-pack"

Basta de teoría — construyamos uno. Levantaremos `security-pack` desde cero, archivo por archivo.

### 4.1 Andamiaje del directorio
```bash
cd your-project    # (o el repo de bathos)
mkdir -p modules/security-pack/{workflows,templates,checklists}
```

### 4.2 Escribir `module.yaml`
```yaml
module_id: security
name: Security Pack
wave: W4
trigger: "Lv>=4 OR domain=security"
enabled_default: false
provides:
  workflows: [threat-model]
  templates: [threat-model-report]
outputs: ".agent-team/13-security/"
evidence_trace: true
```

### 4.3 Escribir el workflow (`workflows/threat-model.md`)
Un workflow es un procedimiento en markdown que un rol sigue. Hazlo concreto y basado en pasos — para un pack de seguridad, p. ej. una pasada OWASP Top 10 + STRIDE que rellena la plantilla. Indica qué entradas leer y adónde van las salidas, y (dado que `evidence_trace: true`) deja claro que cada hallazgo necesita un marcador `[Source:]`.

### 4.4 Escribir la plantilla (`templates/threat-model-report.md`)
El entregable estructurado que el workflow rellena, con encabezados para alcance, activos, amenazas (por STRIDE), severidad, mitigaciones y riesgo residual. Esto es lo que se acumula bajo `outputs`.

### 4.5 (Opcional) checklist de calidad (`checklists/threat-model-quality.md`)
Un checklist de autorrevisión adversarial que el rol ejecuta antes de declarar el workflow completado (ver `ip-pack/checklists/patent-quality.md` como referencia).

### 4.6 Añadir un `README.md`
Una pantalla: qué produce, qué lo activa, cómo se habilita.

---

## 5. Instalar · descubrir · activar

Con los archivos en su sitio, tres comandos llevan el módulo de "estar en disco" a "vivir en el manifest":

```bash
# 1) Descubrir — el motor lee modules/<id>/module.yaml
bathos --modules-dir modules plug list
#   → security-pack aparece con su estado de activación (JSON)

# 2) Activar — se persiste en manifest.modules[]
bathos -s .agent-team/_state --modules-dir modules plug enable security

# 3) Desactivar al terminar
bathos -s .agent-team/_state --modules-dir modules plug disable security
#   id inexistente → exit 1 (E-PLUG-NOTFOUND)
```

Si adoptaste BATHOS en tu proyecto con `install.sh --into`, coloca el módulo bajo el `modules/` de ese proyecto (es decir, el directorio al que apunta `--modules-dir`).

---

## 6. Ejecutarlo en una wave

Los módulos se enchufan a la **W4**. Tras activarlo, ejecuta el comando de W4 y el rol responsable (Mark/Nathanael para los packs incluidos; para tu propio pack, asigna el rol que mejor encaje o deja que el líder lo ejecute directamente) realiza el workflow y escribe en el directorio `outputs`:

```
/wave4-ip-research /abs/path/to/project
```

W4 está fuera de la línea principal (W0→W1→W2→W3→W5→W6), así que puede ejecutarse en cualquier momento después de W2 — si los tokens escasean, puedes posponerla sin problema.

---

## 7. Validar un módulo

Antes de confiar en él, pasa estas cuatro comprobaciones rápidas — cada una verifica un eslabón de la cadena que va del parseo al trigger:

- **Comprobación de parseo:** `bathos --modules-dir modules plug list` — si el módulo aparece, `module.yaml` se parseó. Si no, revisa los 4 campos obligatorios (`module_id`, `name`, `wave`, `trigger`) y la sintaxis YAML.
- **Comprobación de activación/persistencia:** tras `plug enable <id>`, ejecuta `bathos -s _state state show` — confirma el id en `modules[]`.
- **Comprobación de trigger:** configura el nivel/dominio del proyecto y confirma que el módulo se auto-recomienda cuando el `trigger` debería dispararse.
- **Doctor:** `bathos doctor` sigue pasando (el módulo no toca el core — A9).

---

## 8. Buenos hábitos

Unos pocos hábitos mantienen los módulos limpios, componibles y fieles al ethos de BATHOS:

- **No toques el core.** Todo vive bajo `modules/<id>/`. Si te descubres editando un crate de `core/` por un módulo, detente — es una violación de A9.
- **Elige rutas de `outputs` que no se solapen** (`13-…`, `14-…`).
- **Activa `evidence_trace`** y exige `[Source:]` en las afirmaciones — encaja con el ethos "generación ≠ verificación".
- **Escribe un checklist** para que el rol haga una autorrevisión adversarial antes de declarar completado.
- **Toma como modelo los packs incluidos** — `modules/ip-pack/` es la implementación de referencia (module.yaml + workflows + templates + checklists + README).

---

## 9. Licencia

MIT. Ver [`../README.md`](../README.md).

---

<div align="center">

**BATHOS** · βάθος — profundidad, no superficie
한국어: [`MODULE-GUIDE-kr.md`](MODULE-GUIDE-kr.md) · English: [`MODULE-GUIDE-en.md`](MODULE-GUIDE-en.md)

</div>
