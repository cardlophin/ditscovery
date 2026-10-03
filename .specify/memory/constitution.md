<!--
Sync Impact Report
- Versión: plantilla → 1.0.0 (ratificación inicial)
- Principios: I. Núcleo Rust puro · II. API Python amigable · III. Código mínimo ·
  IV. Test-first y validación científica · V. Reproducibilidad · VI. Rendimiento y paralelismo ·
  VII. Observabilidad desacoplada · VIII. Configuración única
- Secciones añadidas: Restricciones técnicas, Flujo de desarrollo, Gobernanza
- Secciones eliminadas: ninguna
- Plantillas: plan/spec/tasks leen la constitución en tiempo de ejecución; sin cambios necesarios
- TODOs diferidos: ninguno
-->

# ditscover Constitution

Librería de descubrimiento no supervisado de motifs de longitud variable en series temporales
univariantes mediante algoritmos genéticos multiobjetivo. Núcleo en Rust, API en Python, CLI.

## Core Principles

### I. Núcleo Rust puro

- Todo el algoritmo (carga, preproceso, distancias, GA, objetivos, selección de motifs) DEBE vivir
  en `crates/core`, sin dependencias de PyO3, CLI ni visualización.
- `crates/py` (bindings) y el paquete `python/ditscover` (API, CLI, visualización) son adaptadores
  finos: convierten tipos y delegan; NO DEBEN contener lógica algorítmica.
- El core DEBE ser testeable con `cargo test` sin Python instalado.

Razón: un único motor verificable; las interfaces no divergen.

### II. API Python amigable

- Un flujo completo (cargar → descubrir → visualizar) DEBE caber en ≤ 3 líneas de usuario.
- Entradas: ruta de fichero, `numpy.ndarray`, `list[float]` o `pandas.Series`. Salidas: objetos
  Python con atributos y conversión a `numpy`/`dict`.
- Los parámetros se pasan como kwargs con defaults sensatos o como un YAML; los nombres son
  idénticos en Python, YAML y CLI.
- Los errores de Rust DEBEN llegar como excepciones Python con mensaje accionable.
- Las llamadas largas DEBEN liberar el GIL y respetar `KeyboardInterrupt`.

### III. Código mínimo (NO NEGOCIABLE)

- Sin documentación ni docstrings; comentarios solo para invariantes no evidentes.
- Sin abstracciones especulativas (YAGNI): traits/enums solo en los puntos de extensión declarados
  (`Loader`, `Distance`, `Preprocess`, `StopCriterion`). Todo lo demás, funciones y structs planos.
- Preferir iteradores, `?` y composición sobre jerarquías; ninguna función > 40 líneas sin
  justificación en revisión.
- Cada dependencia nueva DEBE justificarse en `plan.md`.
- Puertas: `cargo fmt`, `cargo clippy -- -D warnings`, `ruff check`, `ruff format`.

Razón: el valor está en el algoritmo; el código verboso oculta errores y frena la investigación.

### IV. Test-first y validación científica (NO NEGOCIABLE)

- Los tests se escriben antes que la implementación y DEBEN fallar primero (Red → Green → Refactor).
- Invariantes del genotipo con `proptest`: ocurrencias sin solape, longitudes en
  `[min_len, max_len]`, ratio de estiramiento acotado, índices dentro de la serie tras cualquier
  operador genético.
- Calidad algorítmica con series sintéticas con motifs plantados (incluidos de distinta longitud y
  con ruido): se mide precisión/recall de ocurrencias con tolerancia de solape.
- La API Python se cubre con `pytest`; la CLI con tests de integración sobre ficheros de ejemplo.

### V. Reproducibilidad

- Toda ejecución DEBE aceptar una semilla; misma semilla + misma config + mismos datos ⇒ mismo
  resultado, independientemente del número de hilos.
- La aleatoriedad DEBE derivarse de forma determinista de `(seed, generación, índice)`; nunca de un
  RNG compartido entre hilos.
- Cada resultado DEBE poder serializar la config efectiva y la versión de la librería.

### VI. Rendimiento y paralelismo

- La evaluación de la población y los operadores independientes DEBEN paralelizarse con Rayon.
- Estadísticas que se reutilizan (sumas/medias/desviaciones acumuladas) DEBEN precomputarse una vez
  por serie; sin asignaciones en el bucle caliente de distancias.
- El rendimiento se vigila con benchmarks `criterion`; una regresión > 10 % en un benchmark de
  referencia bloquea el merge salvo justificación.

### VII. Observabilidad desacoplada

- El GA NUNCA DEBE bloquearse ni ralentizarse por el logging: emite eventos tipados a un canal
  acotado con envío no bloqueante; si el canal está lleno, el evento se descarta y se contabiliza.
- Un consumidor en hilo aparte escribe el historial (JSONL) y los logs de diagnóstico (`tracing`).
- El historial por generación (estadísticas, hipervolumen, frente de Pareto) es la fuente de la
  visualización de la evolución.

### VIII. Configuración única

- Existe un único struct de configuración serializable; YAML, flags de CLI y kwargs de Python se
  mapean a él. La validación ocurre en un solo lugar y antes de ejecutar.
- Precedencia: defaults < YAML < CLI/kwargs.

## Restricciones técnicas

- Rust edición 2024, workspace Cargo: `crates/core` (motor) y `crates/py` (PyO3).
- Python ≥ 3.12, empaquetado con `maturin` (wheels `abi3-py312`), entorno con `uv`;
  paquete `python/ditscover` con la API amigable, la CLI (`ditscover`, console script incluido en el
  wheel) y la visualización (Plotly, HTML). Un solo `pip install` da API + CLI + gráficos.
- Dependencias base: `rayon`, `pyo3`, `numpy` (pyo3), `polars` (CSV), `serde` + YAML,
  `tracing`, `rand` con generador seedable, `crossbeam-channel`.
- Series univariantes en v1; el diseño no DEBE impedir multivariante, pero no se implementa (YAGNI).

## Flujo de desarrollo

- Spec-Driven: `/speckit-specify` → `/speckit-clarify` → `/speckit-plan` → `/speckit-tasks` →
  `/speckit-analyze` → `/speckit-implement`.
- Cada `plan.md` DEBE pasar el "Constitution Check"; cualquier violación se justifica en la tabla de
  complejidad o se rechaza.
- Una feature está terminada cuando: tests en verde (Rust + Python), puertas de estilo en verde,
  benchmark sin regresión y escenarios de aceptación del spec verificados.

## Governance

- Esta constitución prevalece sobre cualquier otra práctica del proyecto.
- Enmiendas: PR que modifica este fichero con el Sync Impact Report actualizado y versión semántica
  (MAJOR: eliminar/redefinir principios; MINOR: añadir principio o sección; PATCH: redacción).
- Toda revisión de código DEBE verificar el cumplimiento; la complejidad no justificada se rechaza.

**Version**: 1.0.0 | **Ratified**: 2026-10-03 | **Last Amended**: 2026-10-03
