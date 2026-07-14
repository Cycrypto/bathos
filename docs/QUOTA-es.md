# BATHOS — Gestión de tokens y cuota

> Un pipeline multiagente consume tokens más rápido que una conversación única. Esta guía explica **qué genera el costo, cómo controlarlo y cómo recuperarse al alcanzar un límite**. Refleja lecciones de campo aprendidas de forma costosa: **la causa más común de que un compañero parezca "atascado" es el límite de uso de la cuenta, no un bug.**
>
> **Ver también:** [Uso (USAGE-es)](USAGE-es.md) · [Caso de uso (USECASE-es)](USECASE-es.md) · 한국어: [`QUOTA-kr.md`](QUOTA-kr.md) · English: [`QUOTA-en.md`](QUOTA-en.md)

---

## 0. Resumen en una línea

- **Elige el nivel correcto.** Scale-Adaptive Lv0–4 es la mayor palanca de costo — no ejecutes W0–W6 para arreglar un bug.
- **Compañeros concurrentes ≤ 3.** El costo en tokens escala aproximadamente de forma lineal con los roles activos; el motor lo limita duro a 3 (`E-CONCURRENCY`).
- **Genera (spawn) solo los roles necesarios.** Sin ML → omite Stephen. Greenfield → omite John.
- **Secuencia las waves; nunca todo en paralelo.** Cada wave apaga a sus compañeros antes de la siguiente.
- **`/save` antes de detenerte.** Aunque alcances el límite a mitad de una wave, los artefactos en disco permiten reanudar sin pérdidas — tras el reinicio, solo re-ejecuta la wave.
- **Un compañero silencioso sin salida suele ser cuota, no un crash.** Espera el reinicio y vuelve a generarlo.

---

## 1. Qué genera el costo

BATHOS consume tokens a través de estos factores:

| Factor | Por qué importa | Palanca |
|--------|-----------------|---------|
| **Número de compañeros activos** | Cada rol generado es un agente independiente que consume tokens; el costo es ~lineal en roles activos | Concurrencia ≤ 3; genera solo lo necesario |
| **Tier de modelo por rol** | Los roles Opus cuestan más que los Sonnet | Si vas justo, ejecuta primero las waves densas en Sonnet y mantén los roles Opus enfocados |
| **Número de waves ejecutadas** | Más waves → más trabajo total de agentes | Nivel Scale-Adaptive (solo las waves necesarias) |
| **Ciclos de re-gate** | Los bucles FAIL → corregir → re-gate re-ejecutan trabajo | Invierte en la calidad de historias de W3 para que W5 no oscile |

**Mapa rol–modelo** (quién es caro):
- **Opus 4.8:** Paul (líder), John, Caleb, Joshua, James, Jonnathan, Mark, Matthew (#17)
- **Sonnet 5:** Nathanael, Phillip, Andrew, Stephen, Timothy, Thomas, Michael, Hananiah, Matthias, Martin

Es decir, W2 (Joshua→James, Jonnathan) y W3 (Matthew) son densas en Opus (centradas en diseño — la profundidad vale su precio), mientras que W5 (Phillip/Andrew/Stephen) y W6 (Thomas/Timothy/Matthias→Martin) son densas en Sonnet.

---

## 2. Palanca #1 — niveles Scale-Adaptive

Ejecutar menos waves es el mayor ahorro. Escala aproximada:

| Lv | Waves ejecutadas | Waves densas en Opus | Costo relativo |
|----|------------------|----------------------|:--------------:|
| **0** | W5 (+W6 ultraligera) | ninguna | $ |
| **1** | W2 ligera + W3 (abreviada) + W5 + W6 ligera | W2/W3 (abreviadas) | $$ |
| **2** | W1 + W2 + W3 + W5 + W6 | W2/W3 | $$$ |
| **3** | W0–W6 (W4 opcional) | W0/W2/W3 | $$$$ |
| **4** | W0–W6 completo + W4 | W0/W2/W3/W4 | $$$$$ |

**Dimensiona con honestidad.** `/route` recomienda un nivel a partir de los stakes, pero *tú* lo confirmas (User Sovereignty). Si un "producto nuevo" es en realidad un MVP enfocado, Lv2 suele ser mejor que Lv3 — se salta el análisis de mercado de W0 y el plug de W4. Siempre puedes re-enrutar hacia arriba después.

---

## 3. Palanca #2 — concurrencia y selección de roles

- **Tope duro de 3.** `MAX_CONCURRENT_ROLES = 3`; el 4.º spawn dentro de una wave se rechaza con `E-CONCURRENCY`. No luches contra él — respétalo.
- **Genera solo roles con trabajo que hacer.** W5 genera por defecto a Phillip/Andrew/Stephen, pero:
  - Sin AI/ML en el producto → **no generes a Stephen**.
  - App greenfield (sin codebase existente que revertir) → **omite a John** en W1, o dale un solo repo de referencia.
  - Cambio solo de backend → solo Phillip.
- **Secuencia — nunca todo en paralelo.** Las waves ya corren en orden y apagan a los compañeros entre una y otra. Respeta también el orden dentro de la wave (p. ej. en W2, James/Jonnathan arrancan solo tras el gate de planificación de Joshua).

---

## 4. Palanca #3 — posponer lo opcional

- **W4 (IP · Investigación) está fuera de la línea principal.** Si los tokens escasean, ejecuta primero la línea principal (W0→W1→W2→W3→W5→W6) y deja W4 para después — o sáltala por completo en esta iteración.
- **Los gates de plan-review son opcionales.** `/plan-ceo-review`, `/plan-eng-review`, etc. añaden pasadas de revisión con Opus. Úsalos para planificación de alto riesgo; omítelos en trabajos pequeños.

---

## 5. Alcanzar el límite de uso — reconocer y recuperar

**Reconócelo.** Si un compañero generado parece **"atascado" sin salida alguna**, la causa más probable es el **límite de uso (session) de la cuenta**, no un bug de código/hooks. Al alcanzar el límite, el compañero generado no puede razonar — el proceso sigue vivo pero se queda en silencio, lo que parece un hang. (En el transcript de ese compañero quedará un mensaje "you've hit your session limit / resets at …".)

**Recupérate — sin pérdidas.** BATHOS está preparado para esto porque **cada handoff vive en disco** (`.agent-team/`) y los compañeros nunca dependen del historial de conversación en memoria:

1. **No mates nada precipitadamente.** Un compañero silencioso por el límite no está roto — está esperando.
2. **`/save`** (si el líder todavía puede actuar) para tomar un snapshot de la sesión.
3. **Espera el reinicio de la cuota.**
4. **Regenera:** re-ejecuta el comando `/waveN-…` correspondiente. Los artefactos upstream están en disco, así que el compañero regenerado retoma exactamente donde quedó — cero trabajo perdido.

> Por eso importan `/save`+`/resume` y los handoffs solo-disco: un límite de uso se convierte en una pausa, no en una pérdida.

---

## 6. Playbook práctico de cuota baja

Cuando construyas ahorrando tokens:

1. `/route` → confirma el nivel **más bajo** que encaje (normalmente Lv1–Lv2).
2. Ejecuta las waves **una por una** y revisa `/team-status` entre ellas.
3. En cada wave, genera **solo los roles necesarios** (omite Stephen/John si no aplican).
4. **Pospón W4 y los plan-reviews pesados.**
5. **`/save`** al final de cada sesión (y antes de waves largas).
6. Si un compañero se queda en silencio: asume cuota, `/save`, espera el reinicio, `/resume` + re-ejecuta la wave.
7. Cuando la cuota abunde (justo tras el reinicio), prioriza las waves densas en Opus (W2/W3).

---

## 7. Documentos relacionados

- Niveles y enrutamiento: [`USAGE-es.md`](USAGE-es.md) §4 · concurrencia y waves: §2, §3
- Guardar/reanudar: [`USAGE-es.md`](USAGE-es.md) §12, [`FEATURES-es.md`](FEATURES-es.md) §2.11
- La lección "hang = cuota" y otras notas de campo: [`USAGE-es.md`](USAGE-es.md) §13

---

<div align="center">

**BATHOS** · βάθος — profundidad, no superficie
한국어: [`QUOTA-kr.md`](QUOTA-kr.md) · English: [`QUOTA-en.md`](QUOTA-en.md)

</div>
