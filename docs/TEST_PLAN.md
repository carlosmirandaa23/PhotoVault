# Plan de pruebas de PhotoVault

## Inventario M1
- Directorio vacío: 0 archivos y 0 bytes.
- Extensiones mixtas en mayúsculas y minúsculas.
- Carpetas anidadas y archivos con nombres Unicode.
- Enlaces simbólicos: no se siguen.
- Directorios y archivos sin permisos: registrar errores, no detener todo el recorrido.
- Formatos no reconocidos: ignorar sin modificar.
- Archivos de gran tamaño: usar contadores de 64 bits.
- Rutas largas de Windows y carpetas en unidades externas.
- Comprobar que el contenido, tamaño y fecha de modificación no cambian después de escanear.

## Duplicados M2
- Bytes idénticos en distintas rutas.
- Mismo tamaño y distinto contenido.
- Mismo contenido y nombres diferentes.
- Archivo modificado entre análisis y ejecución.
- Error de lectura durante comparación.
- Copia conservada eliminada antes de la operación.
- No borrar por hash sin comparación completa.

## Variantes M3–M4
- Misma imagen JPEG recomprimida.
- Redimensionado con distinta calidad.
- Recorte parcial con rotación y sin rotación.
- Fotos similares de una ráfaga que no son duplicados.
- Orientación EXIF y metadatos divergentes.
- Falsos positivos: siempre exigir revisión humana.
