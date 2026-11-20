# Progreso y cancelación de duplicados

- El backend emite eventos `photovault:scan-progress` durante inventario, hashing y finalización.
- El contador durante hashing representa archivos candidatos (no bytes ni tiempo estimado).
- El frontend registra el listener antes de iniciar el comando y lo retira al terminar.
- `cancel_duplicate_scan` activa un indicador atómico. El escaneo lo comprueba entre entradas y bloques de lectura.
- Solo se permite un escaneo de duplicados simultáneo por proceso.
- La cancelación devuelve error controlado y descarta el informe parcial; la caché de hashes ya verificados puede conservarse.
- No hay modificación de fotos ni operaciones destructivas.
- Pendiente: pruebas con Windows, integración y condiciones de carrera al cerrar la ventana.
