# PhotoVault — Organización por metadatos (EXIF)

Estado: diseño para implementación incremental. Esta función es **principal**, junto con la biblioteca; los duplicados son herramientas complementarias.

## Objetivo

Permitir explorar, filtrar y organizar fotografías y vídeos usando metadatos reales, sin modificar los originales durante el análisis.

## Principios de seguridad

1. **Análisis de solo lectura por defecto.** Leer EXIF nunca debe modificar un archivo.
2. **Previsualización obligatoria** antes de copiar, mover o renombrar. Ninguna operación destructiva en esta fase.
3. **Destino independiente de origen** para las primeras versiones; evitar recursión y sobrescritura.
4. **Conflictos:** no sobrescribir nunca; proponer sufijo estable o pedir revisión.
5. **Transacciones/registro:** toda operación futura debe registrar origen, destino, resultado y errores, con recuperación ante interrupciones.
6. **Fecha desconocida:** clasificar en «Sin fecha», nunca inventar una fecha a partir de la hora actual.
7. **Privacidad:** GPS es sensible; mostrarlo localmente, sin enviarlo a servicios externos.

## Metadatos a capturar

| Campo normalizado | Fuentes y prioridad | Notas |
|---|---|---|
| fecha_captura | DateTimeOriginal + OffsetTimeOriginal; después CreateDate; después fecha de sistema como respaldo **etiquetado** | EXIF puede carecer de zona horaria |
| cámara | Make + Model | Normalizar espacios; conservar valor original |
| lente | LensModel | Opcional |
| dimensiones | Anchura/altura de imagen | No depender solo de EXIF |
| orientación | EXIF Orientation | Respetar al mostrar miniaturas |
| coordenadas | GPSLatitude/GPSLongitude + hemisferios | Validar rangos y fracciones racionales |
| formato | extensión + tipo detectado | HEIC/RAW/video requieren tratamiento aparte |
| procedencia_fecha | EXIF, contenedor, filesystem o desconocida | Visible en UI |
| archivo | ruta, tamaño, mtime, identificador estable | Para caché incremental |

No inferir ubicación textual (ciudad/país) a partir de GPS sin una base geográfica local o autorización expresa para geocodificar.

## Vistas

- **Biblioteca:** resumen, búsqueda, filtros y acceso a álbumes.
- **Cronología:** años > meses > días, con recuento y miniaturas.
- **Dispositivos:** agrupación por cámara/móvil; sin datos agrupados como «Dispositivo desconocido».
- **Mapa:** fase posterior, opcional y local si es viable.
- **Organizar:** elegir patrón de carpetas, simulación de cambios, conflictos y confirmación.

## Patrones propuestos de destino

- `AAAA/MM`
- `AAAA/AAAA-MM-DD`
- `Cámara/AAAA/MM`
- `AAAA/MM/Evento` (evento asignado por el usuario)

Ejemplo de previsualización: `2022/07/IMG_0042.jpg`.
Si no hay fecha verificable: `Sin fecha/IMG_0042.jpg`.

## Arquitectura

1. Escaneo incremental de archivos.
2. Extracción de metadatos en Rust con biblioteca EXIF evaluada y pruebas de formatos.
3. Caché SQLite separada de huellas perceptuales, invalidada por ruta/tamaño/mtime y versión del extractor.
4. API Tauri de lectura: `scan_metadata`, `query_library`, `preview_organization`.
5. UI React: cronología, filtros y simulador; operaciones de archivos en etapa posterior.

## Casos de prueba obligatorios

- EXIF completo, sin EXIF, fecha inválida, fecha sin zona horaria.
- Orientaciones 1 a 8 y fotos verticales.
- GPS positivo/negativo, datos incompletos.
- Dos fotos con mismo nombre, distintas carpetas.
- Fecha EXIF distinta de fecha de modificación.
- Archivos de solo lectura, rutas Unicode, carpetas anidadas, cancelación.
- JPEG, PNG, TIFF, HEIC, RAW y vídeo: indicar explícitamente soporte real y omisiones.
- Copias de seguridad y cero escrituras durante inventario.

## Validación multiplataforma

- Linux: `cargo test --manifest-path src-tauri/Cargo.toml`, `cargo check --manifest-path src-tauri/Cargo.toml`, `npm ci && npm run build`.
- Tauri en Linux requiere toolchain Rust y dependencias nativas (WebKitGTK, GTK y otras según distribución).
- Windows: prueba de integración y UI final, rutas y permisos específicos de Windows.
- No usar GitHub Actions de pago sin autorización.
- El benchmark de similitud y los datos EXIF privados permanecen en el equipo del usuario.

## Criterio de aceptación de primera entrega

El usuario puede inventariar metadatos, filtrar por año/mes/dispositivo, ver la procedencia de la fecha y simular una estructura de carpetas. **Ninguna fotografía se mueve, modifica o elimina.**
