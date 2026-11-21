# Caché incremental de hashes (M1)

- Los hashes BLAKE3 se almacenan en SQLite en la carpeta de caché del usuario, fuera de la biblioteca.
- Una base por ruta raíz canónica evita mezclar bibliotecas.
- La clave es la ruta del archivo; el hash solo se reutiliza cuando coinciden tamaño y fecha de modificación (nanosegundos).
- **Seguridad:** los hashes cacheados son solo filtros de candidatos. Para declarar dos archivos idénticos se leen y comparan todos sus bytes.
- Si SQLite no está disponible, el análisis sigue sin caché y muestra ese estado.
- El programa no elimina, renombra ni mueve fotografías.
- Limitación: metadatos sin cambios no prueban que el contenido sea idéntico. No usar este índice como criterio de borrado; en futuras versiones revalidar el hash en el momento de cualquier operación destructiva.
- Limitación: no hay progreso ni cancelación todavía. Las entradas obsoletas de SQLite se conservan hasta que implementemos limpieza de caché.
- Validación pendiente en Windows: cargo test, cargo check, npm build, y pruebas de bibliotecas grandes.
