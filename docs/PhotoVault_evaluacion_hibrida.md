# PhotoVault — evaluación cruzada pHash + ORB/RANSAC

**Fecha:** 20 de noviembre de 2026  
**Estado:** evaluación experimental; no constituye autorización para borrar, mover o modificar fotografías.

## Descripción

Se compararon cuatro CSV: `resultado_phash.csv`, `resultado_orb.csv`, `resultado_phash_v2.csv` y `resultado_orb_v2.csv`. Cada conjunto contiene 65 variantes sintéticas derivadas de cinco fotografías originales: 2,080 pares, de los cuales 390 pertenecen a la misma familia y 1,690 a familias distintas. En total se evaluaron 4,160 pares: 780 positivos y 3,380 negativos. Los pares se alinearon por nombre de ambos archivos y etiqueta de familia; se comprobó la unicidad y coincidencia de las claves.

## Explicación de las métricas

Regla ensayada: **aceptar como sugerencia visual si pHash orientado predice similitud, O si ORB predice similitud y tiene al menos N inliers RANSAC**. No equivale a afirmar identidad binaria ni duplicación exacta. TP = par de la misma familia correctamente sugerido; FP = par de familias diferentes incorrectamente sugerido; FN = par de la misma familia omitido; TN = par de familias distintas rechazado. Precisión = TP/(TP+FP); recall = TP/(TP+FN).

| Umbral ORB adicional | TP v1 | FP v1 | TP v2 | FP v2 | TP total | FP total | FN total | Recall total |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| Sin filtro adicional | 376 | 24 | 377 | 0 | 753 | 24 | 27 | 96.54% |
| ≥18 | 374 | 4 | 374 | 0 | 748 | 4 | 32 | 95.90% |
| ≥20 | 373 | 1 | 374 | 0 | 747 | 1 | 33 | 95.77% |
| ≥21 | 371 | 0 | 374 | 0 | 745 | 0 | 35 | 95.51% |
| ≥22 | 371 | 0 | 374 | 0 | 745 | 0 | 35 | 95.51% |
| ≥25 | 370 | 0 | 373 | 0 | 743 | 0 | 37 | 95.26% |

Los 24 FP observados de ORB en v1 tienen de 12 a 20 inliers. El umbral ≥21 es el mínimo entero de los examinados que elimina todos esos FP; esto es **observación del conjunto de desarrollo**, no garantía de generalización. El umbral ≥25 es más conservador y pierde dos TP adicionales respecto a ≥21 en estas muestras.

## Aplicación recomendada

1. Mantener el detector de duplicados exactos separado, basado en hash de contenido.
2. Mantener pHash orientado como primera capa de sugerencias visuales.
3. Probar ORB/RANSAC como capa complementaria **solo de sugerencias**, inicialmente con umbral conservador de ≥25 inliers, configurable internamente; no clasificarlo como «seguro que son iguales».
4. Validar en un **tercer conjunto independiente**, elegido antes de volver a ajustar el umbral, con escenas repetitivas, capturas diferentes del mismo objeto, imágenes de baja textura, recortes, espejos, fotografías familiares y resolución reducida. Evaluar por foto de origen, no tratar pares correlacionados como observaciones independientes.
5. Registrar TP/FP/FN/TN, precisión, recall, tiempo, consumo de memoria y errores de decodificación. Revisar manualmente cada falso positivo y cada falso negativo.
6. Solo después de validar, implementar ORB en el backend Rust (o integrar un componente nativo bien delimitado), con caché de descriptores, prefiltrado de candidatos y ejecución cancelable. Comparar tiempos con bibliotecas de miles de imágenes; evitar comparación exhaustiva O(n²) sin estrategia de candidatos.
7. Interfaz: «Duplicados exactos confirmados» para igualdad de contenido; «Posibles versiones de la misma fotografía» para similitud. Mostrar razones y permitir revisión; **nunca borrar ni modificar originales automáticamente**.

## Limitaciones y conclusión

Las 65 imágenes de cada conjunto se generan a partir de solo cinco fotografías; muchos pares comparten contenido, por lo que las métricas no prueban una tasa de error en bibliotecas reales. El segundo conjunto parece provenir de una galería de producto, pero la naturaleza de las tomas originales no se ha verificado visualmente. La precisión observada del 100% con ≥21 o ≥25 se refiere únicamente a estos dos conjuntos. La regla híbrida es prometedora: con ≥25 obtiene 743 TP, 0 FP, 37 FN y 3,380 TN en 4,160 pares; **se recomienda validación independiente antes de integración definitiva**.

## Reproducibilidad

Ejecutar `analyze_thresholds.py` con los cuatro CSV y revisar `threshold_comparison.csv`. El script es de solo lectura respecto a los CSV de entrada.

### Comando de ejemplo

```powershell
py -m pip install pandas
py .\experiments\analyze_thresholds.py --phash-v1 "$HOME\Desktop\resultado_phash.csv" --orb-v1 "$HOME\Desktop\resultado_orb.csv" --phash-v2 "$HOME\Desktop\resultado_phash_v2.csv" --orb-v2 "$HOME\Desktop\resultado_orb_v2.csv" --output "$HOME\Desktop\threshold_comparison.csv"
```
