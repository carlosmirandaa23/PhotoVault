# Arquitectura de PhotoVault

## Objetivo
Aplicación local para Windows: inventariar, comparar, revisar y organizar fotografías sin riesgo innecesario de pérdida.

## Stack
- Tauri 2 + Rust para escritorio y acceso al sistema de archivos.
- React + TypeScript para la interfaz oscura.
- SQLite para catálogo, análisis incremental y registro de operaciones.
- BLAKE3 para filtrado de duplicados exactos, seguido de verificación byte a byte.
- Hash perceptual para candidatos visuales; comparación adicional para confirmar relaciones.
- Motor de recortes mediante correspondencias locales y verificación geométrica (fase posterior).

## Módulos
1. scanner: inventario recursivo, cancelación, errores y rutas excluidas.
2. metadata: fechas y fuentes, orientación, dimensiones, formatos.
3. exact: agrupación por tamaño, BLAKE3 y comparación de bytes.
4. visual: candidatos por huellas perceptuales y verificación.
5. catalog: SQLite y miniaturas.
6. planner: propuestas de organización y limpieza, sin efectos secundarios.
7. executor: operaciones verificadas, registro y recuperación.

## Invariantes de seguridad
- Escanear no modifica archivos.
- Nunca deducir identidad exacta de un hash perceptual.
- Nunca borrar automáticamente una imagen recortada, recomprimida o redimensionada.
- Comprobar que la copia conservada sigue disponible e íntegra antes de actuar.
- Revalidar rutas y metadatos antes de ejecutar un plan.
- No seguir enlaces simbólicos fuera de las raíces autorizadas sin permiso explícito.
- Por defecto enviar a Papelera cuando esté disponible; no prometer reversibilidad universal.
- No almacenar fotos del usuario en el repositorio.

## Hitos
- M0: repositorio, especificación y proyecto Tauri inicial.
- M1: escaneo, catálogo y miniaturas.
- M2: duplicados exactos y flujo de revisión seguro.
- M3: equivalencias visuales y selección de calidad.
- M4: recortes y organización física opcional.
