# Hoja de ruta

Objetivo original: **organizar fotos y eliminar duplicados de forma segura**.

## M0 — Base
- [x] Repositorio, principios y arquitectura documentados.
- [x] Proyecto compilable Tauri 2 + React + TypeScript (icono incluido).
- [x] Pruebas Rust y build del frontend (workflow manual en Codespace).
- [x] Ramas consolidadas en `main`.

## M1 — Catálogo
- [x] Selector de carpeta e inventario por formato.
- [x] Escaneo cancelable con progreso (duplicados y similitud).
- [x] Caché SQLite de hashes invalidada por tamaño/mtime.
- [x] Miniaturas con caché y concurrencia limitada.
- [ ] Exclusiones de carpetas.
- [ ] Cuadrícula virtualizada para bibliotecas grandes.

## M2 — Duplicados exactos
- [x] Agrupación por tamaño y BLAKE3.
- [x] Verificación byte a byte.
- [x] Sugerencia de qué copia conservar, editable por grupo.
- [x] Envío a la Papelera con nueva verificación inmediata.
- [x] Registro de operaciones, historial y deshacer.
- [ ] Verificación en Windows real (Papelera de reciclaje).

## M3 — Variantes visuales
- [x] pHash con rotaciones y espejos; grupos y parejas.
- [x] Revisión humana obligatoria (solo sugerencias).
- [ ] Acción de limpieza para variantes, con revisión foto por foto.
- [ ] Búsqueda de candidatos sin límite de 3 000 imágenes.

## M4 — Organización
- [x] Propuesta en solo lectura: fotos, videos, capturas, posible basura, sin fecha.
- [x] Fecha EXIF con procedencia; nunca mtime.
- [x] Eventos por brecha de 12 h y umbral de 20 archivos.
- [x] Decisiones del usuario persistentes (SQLite, fuera de las fotos).
- [ ] Fecha de videos desde metadatos MP4/MOV.
- [ ] Vista previa de movimientos, colisiones y espacio.
- [ ] Ejecución confirmada con registro y deshacer.

## M5 — Experimental (no bloquea lo anterior)
- [ ] ORB + RANSAC nativo para recortes (hoy: prototipo Python).
- [ ] Exportar/importar decisiones como XMP sidecar opcional.
- [ ] Decisiones por pareja («no son la misma foto»).

## Definición de terminado
Cada hito requiere pruebas automatizadas, revisión de errores, verificación en Windows y documentación de limitaciones.
