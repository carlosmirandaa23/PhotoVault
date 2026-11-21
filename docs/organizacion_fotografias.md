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

## Segunda fase: EXIF y eventos

- **Fecha de captura**: EXIF `DateTimeOriginal` (o `DateTimeDigitized`/`DateTime`) → fecha en el nombre (`YYYYMMDD`, `YYYYMMDD_HHMMSS`, `YYYY-MM-DD`, `WhatsApp Image … at HH.MM.SS`) → desconocida. Se muestra la procedencia. `mtime` nunca se usa.
- **Cámara**: marca y modelo EXIF. Una foto con EXIF de cámara se clasifica como fotografía aunque sea pequeña o tenga un nombre desconocido.
- **Capturas**: la proporción solo cuenta si es de pantalla de teléfono alargada (18:9 a 21:9) y no hay EXIF de cámara. 4:3, 3:2 y 16:9 son formatos normales de cámara.
- **Nombres de cámara**: se reconocen también `YYYYMMDD_HHMMSS` (Samsung/Android), `P10…`, `GOPR`, `DJI_`.
- **Eventos**: fotos y videos fechados se ordenan por hora; una brecha de más de 12 h separa eventos; los de 20 archivos o más van a `Fotos/AAAA/AAAA-MM-DD Evento`, el resto a `Fotos/AAAA/AAAA-MM`. Ambos valores son configurables (`organizer::Options`).
- **Decisiones**: una sola conexión SQLite por análisis. «Conservar» y «Es fotografía» ahora participan en la agrupación por fecha.

## Pendiente

1. **Videos**: la fecha sale solo del nombre; falta leer metadatos de MP4/MOV.
2. **HEIC/RAW**: se lee EXIF de HEIC cuando existe, pero no hay miniaturas ni dimensiones.
3. **Zona horaria**: las horas EXIF se tratan como hora local sin zona.
4. **Operaciones reales**: no existe botón para mover archivos. Requerirá vista previa, confirmación, verificación de espacio, prevención de colisiones, registro reversible y manejo de errores (como la limpieza de duplicados).

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
