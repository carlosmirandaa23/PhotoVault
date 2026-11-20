#!/usr/bin/env python3
"""Compare exact Rust pHash pair decisions with Python ORB results, without modifying photos.

Usage: py experiments/compare_hybrid.py resultado_phash.csv resultado_orb.csv
Both CSV files must contain all 2080 unordered pairs from the same 65-image fixture.
"""
import argparse
import csv
from collections import Counter
from pathlib import Path

def load(path, name):
    with Path(path).open("r", encoding="utf-8-sig", newline="") as stream:
        reader = csv.DictReader(stream)
        required = {"first", "second", "same_family", "predicted"}
        if not required.issubset(reader.fieldnames or []):
            raise ValueError(f"{name}: faltan columnas {sorted(required - set(reader.fieldnames or []))}")
        rows = {}
        for row in reader:
            a, b = row["first"], row["second"]
            if not a or not b or a == b:
                raise ValueError(f"{name}: pareja inválida: {a!r}, {b!r}")
            key = tuple(sorted((a, b)))
            if key in rows:
                raise ValueError(f"{name}: pareja duplicada {key}")
            if row["same_family"] not in ("0", "1") or row["predicted"] not in ("0", "1"):
                raise ValueError(f"{name}: etiquetas inválidas en {key}")
            rows[key] = (row["same_family"] == "1", row["predicted"] == "1")
    return rows

def metrics(predictions, truth):
    counts = Counter()
    for key, predicted in predictions.items():
        actual = truth[key]
        counts["TP" if predicted and actual else "FP" if predicted else "FN" if actual else "TN"] += 1
    tp, fp, fn = counts["TP"], counts["FP"], counts["FN"]
    precision = 100 * tp / (tp + fp) if tp + fp else 0.0
    recall = 100 * tp / (tp + fn) if tp + fn else 0.0
    return counts, precision, recall

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("phash_csv", type=Path)
    parser.add_argument("orb_csv", type=Path)
    args = parser.parse_args()
    phash = load(args.phash_csv, "pHash")
    orb = load(args.orb_csv, "ORB")
    if len(phash) != 2080 or len(orb) != 2080:
        parser.error(f"Se esperaban 2080 parejas por archivo; pHash={len(phash)}, ORB={len(orb)}")
    if phash.keys() != orb.keys():
        parser.error("Los CSV no contienen exactamente las mismas parejas")
    truth = {key: value[0] for key, value in phash.items()}
    if any(truth[key] != orb[key][0] for key in truth):
        parser.error("Las etiquetas de familia no coinciden entre archivos")
    predictions = {
        "pHash": {key: value[1] for key, value in phash.items()},
        "ORB": {key: value[1] for key, value in orb.items()},
        "Unión (pHash O ORB)": {key: phash[key][1] or orb[key][1] for key in truth},
        "Intersección (pHash Y ORB)": {key: phash[key][1] and orb[key][1] for key in truth},
    }
    print("BENCHMARK HÍBRIDO — mismas 2080 parejas, sin modificar fotografías")
    print("Método                         TP   FP   FN   TN   Precisión   Recall")
    for label, decisions in predictions.items():
        counts, precision, recall = metrics(decisions, truth)
        print(f"{label:29} {counts['TP']:>3}  {counts['FP']:>3}  {counts['FN']:>3} {counts['TN']:>4}   {precision:>7.2f}%  {recall:>7.2f}%")
    categories = Counter()
    for key in truth:
        p, o = phash[key][1], orb[key][1]
        categories[("misma captura" if truth[key] else "capturas diferentes",
                    "ambos" if p and o else "solo pHash" if p else "solo ORB" if o else "ninguno")] += 1
    print("\nDesglose de decisiones:")
    for label in ("misma captura", "capturas diferentes"):
        for source in ("ambos", "solo pHash", "solo ORB", "ninguno"):
            print(f"  {label:20} / {source:10}: {categories[(label,source)]}")
    new_fp = [key for key in truth if not truth[key] and orb[key][1] and not phash[key][1]]
    print("\nFalsos positivos añadidos por ORB (hasta 20):")
    for a, b in new_fp[:20]:
        print(f"  {a} <> {b}")
    print("\nIMPORTANTE: unión e intersección son escenarios experimentales, no probabilidades calibradas.")
    print("No se ha integrado ORB al escáner de producción. No se modificó ningún archivo.")

if __name__ == "__main__":
    main()
