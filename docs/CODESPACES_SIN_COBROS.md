# Codespaces para PhotoVault: uso sin cobros adicionales

## Antes de crear uno

1. Abrir https://github.com/settings/billing y revisar el consumo y presupuestos de Codespaces.
2. Si hay un método de pago, configurar un presupuesto específico de Codespaces con **Stop usage when budget limit is reached** activado. Verificar que no exista otro presupuesto que permita gasto adicional. Un presupuesto sin bloqueo **no es un límite de gasto**.
3. Si no hay método de pago válido, GitHub indica que detiene el uso al agotar la cuota gratuita de una cuenta personal.
4. Verificar que el repositorio sea propiedad de la cuenta personal, no de una organización que facture de otra forma.

## Cuota gratuita publicada por GitHub

- GitHub Free personal: **120 core-hours/mes** y **15 GB-month/mes**.
- GitHub Pro personal: **180 core-hours/mes** y **20 GB-month/mes**.
- Una máquina de 2 núcleos consume 2 core-hours por hora encendida; 120 core-hours equivalen a **60 horas** de una máquina de 2 núcleos, siempre que no se agote antes la cuota de almacenamiento.
- Los códigos y dependencias descargadas ocupan almacenamiento incluso con la máquina detenida.
- No confundir ChatGPT Plus con GitHub Pro.

Referencias:
https://docs.github.com/en/billing/concepts/product-billing/github-codespaces
https://docs.github.com/en/codespaces/troubleshooting/troubleshooting-included-usage

## Creación segura

1. Ir a https://github.com/carlosmirandaa23/PhotoVault/tree/feature/perceptual-similarity
2. **Code > Codespaces > Create codespace** en la rama `feature/perceptual-similarity`.
3. Elegir **2-core**. No elegir 4/8/16 cores.
4. El archivo `.devcontainer/devcontainer.json` no define una imagen personalizada: se utiliza la imagen predeterminada de Codespaces, cuya base no consume cuota de almacenamiento según GitHub.
5. No se instalan paquetes pesados ni se lanzan pruebas automáticamente al crear el Codespace.

## Configuración para ahorrar

- En https://github.com/settings/codespaces configurar **idle timeout = 5 minutos** (si el ajuste lo permite).
- Configurar la retención automática lo más corta que resulte práctica.
- Detener manualmente desde https://github.com/codespaces al terminar. **Cerrar la pestaña no detiene inmediatamente la máquina**.
- Eliminar Codespaces ya innecesarios: los detenidos siguen consumiendo almacenamiento.
- No habilitar prebuilds ni GitHub Actions para este laboratorio.
- Revisar periódicamente https://github.com/settings/billing.

## Pruebas bajo demanda

En la terminal del Codespace, ejecutar **una fase a la vez**:

```bash
node --version
cargo --version
npm ci
npm run build
```

Para `cargo test` o `cargo check` de Tauri 2 pueden faltar bibliotecas nativas de Linux. Instalarlas manualmente y solo cuando vayamos a ejecutar esa fase; no instalar Wine ni escritorio gráfico de entrada.

```bash
cargo test --manifest-path src-tauri/Cargo.toml
```

Si falta Rust o una dependencia nativa, detenerse y revisar el error antes de instalar nada. Los tests del benchmark de 65 fotografías privadas no se ejecutan en Codespaces sin transferir explícitamente datos de prueba; preferir fixtures sintéticos.

## Alcance real

Codespaces es un equipo remoto controlado por GitHub; agregar este archivo al repositorio **no crea un Codespace ni inicia facturación**. El asistente puede preparar y modificar el código mediante la integración GitHub, pero **no obtiene automáticamente acceso a la terminal de Codespaces**. Para ejecutar pruebas remotas necesitamos un mecanismo de acceso compatible o que el usuario ejecute los comandos y comparta resultados.
