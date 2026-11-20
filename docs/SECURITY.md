# Política de seguridad de archivos

PhotoVault debe ser conservador con archivos personales.

## Categorías
- **Exacto por bytes:** mismo tamaño, mismo BLAKE3 y verificación completa de bytes.
- **Píxeles equivalentes:** decodificación y normalización controlada; los metadatos pueden diferir.
- **Derivado visual:** recorte, redimensionado, recompresión o edición; nunca eliminación automática.
- **Similar:** escena o contenido parecido; solo agrupación.

## Operaciones
1. Inventariar sin modificar.
2. Generar un plan explícito con origen, destino, tamaño, identidad y razón.
3. Mostrar vista previa y solicitar aprobación.
4. Revalidar estado y copia conservada.
5. Ejecutar una operación a la vez con registro durable.
6. Verificar resultado y registrar fallos.

No se considera segura la eliminación permanente automática en las primeras versiones. El uso de Papelera depende del volumen y del sistema de archivos. La restauración debe tratarse como una capacidad verificada, no asumida.
