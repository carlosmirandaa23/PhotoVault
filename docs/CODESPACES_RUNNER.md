# PhotoVault: GitHub Actions ejecutado en Codespaces

## Objetivo y límites

El workflow `.github/workflows/photovault-codespace-tests.yml` usa **exclusivamente** un runner `self-hosted` con la etiqueta `photovault-codespace`. No utiliza máquinas de GitHub para compilar. Solo se activa manualmente (`workflow_dispatch`), nunca por cada push o pull request. El Codespace **debe estar encendido y el runner ejecutándose** para procesar el trabajo.

**Importante:** GitHub Actions no puede iniciar por sí mismo un Codespace detenido. El almacenamiento de Codespaces sigue consumiéndose mientras está detenido. La cuota y el presupuesto de Codespaces continúan aplicándose. En repositorios públicos, GitHub puede restringir el uso de runners autohospedados o imponer requisitos adicionales; comprobar los ajustes reales.

## Antes de comenzar

- Confirmar presupuesto Codespaces de **USD 0**, con **Stop usage when budget limit is reached** activado; comprobar que no hay otros presupuestos que permitan cargos.
- Utilizar solo un Codespace de **2 cores**. Detener/eliminar el segundo tras comprobar que no contiene cambios sin guardar.
- Configurar timeout de inactividad de 5 minutos si es posible.
- **No publicar tokens, capturas con tokens, ni ejecutar workflows de terceros.**
- No habilitar disparadores `pull_request` ni `push` para este runner; los cambios de terceros podrían ejecutar código arbitrario en la máquina.

## Registrar el runner (una sola vez)

1. Abrir el repositorio > **Settings > Actions > Runners > New self-hosted runner**: https://github.com/carlosmirandaa23/PhotoVault/settings/actions/runners/new
2. Seleccionar **Linux x64**. GitHub mostrará comandos actualizados para descargar y configurar el runner. **Usar los comandos de descarga que muestra GitHub**, porque incluyen la versión y hash actuales.
3. Dentro del Codespace, abrir terminal y crear una carpeta **fuera del repositorio**:
   ```bash
   mkdir -p "$HOME/actions-runner"
   cd "$HOME/actions-runner"
   ```
4. Ejecutar en esa carpeta los comandos de descarga/extracción que proporciona GitHub. Para la configuración, utilizar el comando de GitHub agregando la etiqueta **`--labels photovault-codespace`** y, si se desea, **`--name photovault-codespace`**. **No pegar aquí ni compartir el token de registro.**
5. Iniciar el runner en la terminal:
   ```bash
   cd "$HOME/actions-runner"
   ./run.sh
   ```
6. Confirmar en **Settings > Actions > Runners** que está **Online** e **Idle**. La terminal del runner debe permanecer ejecutándose mientras procesamos trabajos.

No usar `sudo ./run.sh`. No instalarlo como servicio permanente: preferimos inicio manual para controlar la cuota. Si el Codespace se reconstruye o elimina, puede ser necesario volver a registrar el runner y quitar el registro obsoleto desde GitHub.

## Ejecutar las pruebas

- El workflow debe estar en la **rama por defecto** para aparecer y poder lanzarse desde la pestaña Actions. Este archivo se añadió inicialmente a `feature/perceptual-similarity`; **no estará disponible en Actions hasta integrarlo a la rama por defecto**, o hasta que GitHub lo reconozca allí. No fusionar cambios de producto sin revisarlos solo para activar las pruebas.
- Una vez que el workflow esté en la rama por defecto, abrir **Actions > PhotoVault - Codespace tests (self-hosted only) > Run workflow**.
- Elegir la rama de PhotoVault a validar y dejar **Rust tests** desactivado para el primer intento.
- El resultado de cada ejecución se publica en **Actions**, junto con sus logs. Los tests Rust se pueden activar después de instalar las bibliotecas Linux de Tauri.

## Seguridad

Un self-hosted runner ejecuta comandos definidos en workflows; el usuario debe tratarlo como una máquina con acceso a su repositorio y archivos. Usarlo solo con workflows propios y ramas confiables. No registrar runners en repositorios públicos para ejecutar contribuciones de desconocidos. Evitar `pull_request_target`, privilegios de escritura, secretos innecesarios y permisos elevados.

## Limitación de la integración actual

El asistente puede editar el repositorio con la integración de GitHub. Sin embargo, **la integración disponible actualmente no incluye herramientas para iniciar workflows ni leer sus ejecuciones**; los resultados se deben compartir o habilitar mediante una conexión compatible. Configurar el runner no otorga automáticamente al asistente acceso a la terminal.
