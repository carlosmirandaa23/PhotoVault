# Desarrollo local (Windows)

## Requisitos
- Node.js LTS y npm.
- Rust estable (rustup) con toolchain MSVC.
- Microsoft C++ Build Tools y Windows SDK.
- WebView2 (incluido en Windows 11; verificar en Windows 10).

## Ejecutar
```powershell
npm install
npm run tauri dev
```

## Compilar
```powershell
npm run build
npm run tauri build
```

## Alcance de la primera implementación
El botón Analizar recorre directorios y cuenta archivos multimedia por extensión, sin modificar archivos. Aún no calcula hashes ni identifica duplicados. El recorrido es síncrono dentro del comando Rust y puede tardar en colecciones grandes; se reemplazará por un proceso cancelable con progreso en M1.

## Advertencias
No se ha validado todavía una compilación nativa en Windows ni creado instalador. No se recomienda ejecutar la app sobre archivos importantes hasta contar con pruebas de integración.
