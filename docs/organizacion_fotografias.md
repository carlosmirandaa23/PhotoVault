# Recuperación del organizador original y estado de la implementación

## Descripción de la fuente original

El script aportado por el usuario, `organizar_fotos.py`, clasifica en `Fotos`, `Snapshots`, `Basura Probable` y `_Sin_Fecha`. Agrupa fotografías y videos fechados por año y evento/mes, usando una brecha de 12 horas y un umbral de 20 archivos para considerar un evento grande. Detecta fechas mediante EXIF `DateTimeOriginal`, nombre y, como último recurso para fotos, fecha de modificación; los videos sin fecha en el nombre quedan sin fecha. La propuesta original permitía simulación y ejecución real con `--ejecutar`, renombrando y moviendo archivos.

## Características recuperadas en la primera fase Rust

- Nueva sección «Organizar fotografías» en la aplicación Tauri/React.
- Escaneo recursivo de imágenes y videos reconocidos; se ignoran enlaces simbólicos.
- Clasificación provisional en fotos, videos, capturas/mensajería, posible basura y revisión.
- Razón textual de cada sugerencia y carpeta de destino orientativa.
- Detección de fechas del nombre `YYYYMMDD` y `YYYY-MM-DD`.
- Regla de menos de 1 MP para **sugerir revisión**, nunca para eliminar.
- Preferencia por nombres de cámara y de capturas para reducir falsos positivos.
- Sin operaciones de copia, movimiento, renombrado ni borrado.
- Límite de 20 000 archivos por análisis; el resto se contabiliza como omitido.

## Diferencias pendientes con respecto al script original

1. **EXIF**: todavía no se leen `DateTimeOriginal`, modelo, ISO, exposición, etc. Por ello, ninguna clasificación debe considerarse definitiva; en particular una fotografía de cámara sin EXIF detectado puede parecer captura o «posible basura».
2. **Eventos**: aún no se implementa agrupación por brecha temporal ni umbral de evento; se propone organización por mes cuando hay fecha en el nombre.
3. **Videos**: no se extrae metadata de contenedor; se usa fecha del nombre solamente.
4. **Formatos RAW/HEIC**: la primera versión del planificador no inspecciona formatos sin decodificador compatible.
5. **Fechas**: no se toma `mtime` como fecha de captura porque puede representar una copia o descarga. Se mostrará la procedencia de cada fecha cuando se agregue EXIF.
6. **Operaciones reales**: no existe botón para mover archivos. Requerirá revisión individual, simulación, confirmación explícita, verificación de espacio, prevención de colisiones, registro reversible y manejo de errores.

## Principios de seguridad

- «Basura probable» significa **revisión prioritaria**, no certeza ni autorización de borrado.
- Las fotografías antiguas, comprimidas y compartidas pueden carecer de EXIF o tener resolución pequeña.
- Un aspecto 4:3, 3:2 o 16:9 no distingue por sí solo una captura de una fotografía.
- Un nombre `IMG_...` o `Screenshot...` es indicio, no prueba.
- Nunca modificar originales durante análisis.
- El organizador es independiente de duplicados exactos y similitud visual.

## Pruebas recomendadas

```powershell
cargo test --manifest-path src-tauri/Cargo.toml organizer
npm run build
```

La implementación fue incorporada al repositorio, pero estas pruebas aún requieren ejecución en un entorno con Rust, Node y dependencias Tauri.
