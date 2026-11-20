# PhotoVault

Administrador local de fotografías para Windows, desarrollado con Rust, Tauri 2, React y TypeScript.

## Principios

- Analizar primero; nunca mover ni borrar archivos durante el escaneo.
- Verificar duplicados exactos mediante hash y comparación byte a byte.
- Tratar recortes, recomprensiones y redimensionados como relaciones visuales que requieren revisión.
- Limpiar duplicados exactos solo con confirmación: se conserva la copia elegida, las demás van a la Papelera del sistema tras una nueva comparación byte a byte, y cada limpieza queda registrada y se puede deshacer.
- Mantener registro de operaciones y proteger el original.

> Proyecto en desarrollo. No existe borrado definitivo: la única operación que cambia la biblioteca es enviar duplicados exactos confirmados a la Papelera.
