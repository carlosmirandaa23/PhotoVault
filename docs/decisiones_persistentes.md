# Decisiones persistentes de organización

## Implementación

- `src-tauri/src/decisions.rs`: almacenamiento SQLite por biblioteca, ubicado en el directorio de datos local de PhotoVault, no dentro de las fotos.
- Comandos Tauri `save_photo_decision` y `get_photo_decision`.
- La interfaz permite `keep`, `photo`, `snapshot`, `review_later` y `clear` (retirar decisión).
- Se registra fecha UTC de revisión, ruta canónica, tamaño y fecha de modificación en nanosegundos.
- `organizer.rs` consulta la decisión persistida: `keep` y `photo` prevalecen sobre la clasificación automática; `snapshot` se mantiene como captura; si el archivo cambió, solo se muestra advertencia y se usa la clasificación provisional.
- Ningún comando modifica, mueve, renombra o borra medios. `clear` borra únicamente el registro SQLite.

## Límites conocidos

- La identidad actual se basa en ruta canónica y huella ligera tamaño/mtime. Si se mueve o renombra una foto, la decisión no se traslada automáticamente. Si el contenido cambia sin cambiar tamaño/mtime, la huella puede no detectarlo.
- Las decisiones son individuales. Relaciones entre dos fotografías («no son duplicadas») requieren una tabla de pares con identidad robusta.
- XMP sidecar y XMP incrustado **no están implementados**. Deben ser opt-in y disponer de exportación/importación, manejo de conflictos y validación de formatos. No modificar originales por defecto.
- La pantalla carga como máximo 100 filas visibles; las consultas SQLite son individuales y necesitan optimización por lotes antes de bibliotecas grandes.
- Aún no se han ejecutado compilación y pruebas automatizadas en este entorno.

## Pruebas manuales propuestas

1. Generar propuesta en una biblioteca de prueba.
2. Marcar una imagen pequeña como «Conservar».
3. Generar de nuevo el plan y verificar que no se sugiera como «Basura Probable».
4. Reiniciar la aplicación y verificar persistencia.
5. Modificar una copia de prueba y verificar la advertencia de nueva revisión.
6. Retirar decisión y comprobar retorno a clasificación provisional.
7. Comparar SHA-256 de los archivos originales antes y después; deben permanecer idénticos.

## Evolución

- Leer EXIF y agrupar eventos.
- Optimizar lectura SQLite por lotes y crear identidad de contenido resistente a cambios de ruta.
- Agregar decisiones por parejas y auditoría/historial de revisiones.
- Diseñar exportación/importación XMP externo opcional antes de considerar metadatos incrustados.
