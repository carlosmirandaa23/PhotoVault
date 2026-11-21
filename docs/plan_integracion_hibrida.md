# Plan de integración segura del detector híbrido

## Estado actual (20-11-2026)

- `similarity.rs`: detector pHash orientado usado por la aplicación. Sin cambios funcionales.
- `hybrid_policy.rs`: política experimental pura, compilada por `lib.rs`, sin acceso a disco, sin invocación desde la interfaz y sin extracción ORB.
- `experiments/crop_orb_ransac.py`: referencia Python del extractor ORB + RANSAC.
- `experiments/analyze_thresholds.py`: evaluación de métricas por pares.
- `experiments/generar_pruebas_photovault.py`: generador reproducible de 65 imágenes de prueba.

## Contrato de entrada del módulo experimental

`HybridPolicy::classify(phash_similar, orb: Option<OrbEvidence>) -> Option<EvidenceKind>`

`OrbEvidence` incluye `mutual_matches`, `ransac_inliers`, `inlier_ratio` y `spatially_distributed`. La regla exige al menos 12 matches e inliers, ratio ≥0.35, distribución espacial y umbral adicional configurable (25 por defecto). Un pHash positivo permanece positivo aunque ORB no esté disponible.

Este contrato no representa una probabilidad calibrada ni una confirmación de igualdad binaria.

## Próximas fases independientes del usuario

1. **Compilación y pruebas:** ejecutar `cargo test --manifest-path src-tauri/Cargo.toml hybrid_policy` en un runner con dependencias de Tauri; corregir cualquier fallo. La inclusión del módulo en GitHub no demuestra por sí sola que compile en el entorno Windows del usuario.
2. **Extractor nativo:** implementar detección de keypoints y descriptores, matching bidireccional, filtro de Lowe, estimación afín RANSAC y comprobación de distribución espacial; cotejar sus salidas con Python para parejas etiquetadas. No reemplazar el detector actual antes de la comparación.
3. **Caché:** indexar por ruta canónica, tamaño, fecha de modificación y versión del algoritmo; invalidar al cambiar la foto. No guardar contenido de imagen sin necesidad.
4. **Candidatos:** evitar ejecutar ORB para todas las parejas de una biblioteca grande. Diseñar y medir un prefiltrado que no elimine los recortes que se quieren recuperar.
5. **Cancelación y límites:** limitar concurrencia, RAM y número de candidatos; admitir cancelación; no escribir en los directorios de fotografías.
6. **Validación:** tercera colección independiente con positivos y negativos difíciles. Medir precisión/recall, errores y tiempo; elegir umbrales sin ajustar sobre la misma colección de evaluación.
7. **Activación:** introducir opción experimental desactivada por defecto, mostrar evidencia como sugerencia y mantener las coincidencias por hash criptográfico en sección separada.

## Comprobaciones

```powershell
git switch experiment/crop-orb-ransac
git pull --ff-only
cargo test --manifest-path src-tauri/Cargo.toml hybrid_policy
```

La política experimental **no borra, mueve ni modifica fotografías**. El comportamiento público de `find_similar_photos` continúa siendo pHash.
