# PhotoVault

Administrador local de fotografías para Windows, desarrollado con Rust, Tauri 2, React y TypeScript.

## Principios

- Analizar primero; nunca mover ni borrar archivos durante el escaneo.
- Verificar duplicados exactos mediante hash y comparación byte a byte.
- Tratar recortes, recomprensiones y redimensionados como relaciones visuales que requieren revisión.
- Mantener registro de operaciones y proteger el original.

> Proyecto en desarrollo. Ninguna función de eliminación está implementada.
