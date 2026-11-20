#!/usr/bin/env python3
"""Experimental ORB + RANSAC crop detector for PhotoVault's labeled 65-image fixture.

This is a read-only offline benchmark, NOT a production detector.
Install: py -m pip install opencv-python
Run: py experiments/crop_orb_ransac.py "$HOME\\Desktop\\pruebav_PhotoVault_Test"
"""
from __future__ import annotations

import argparse
import csv
import itertools
import time
from collections import Counter, defaultdict
from pathlib import Path

import cv2
import numpy as np

EXTS = {".jpg", ".jpeg", ".png", ".webp"}
ORB_FEATURES = 1600
RATIO = 0.75
RANSAC_THRESHOLD = 3.0
MIN_INLIERS = 12
MIN_INLIER_RATIO = 0.35


def load_gray(path: Path):
    # imdecode handles Windows paths with spaces and Unicode.
    raw = np.fromfile(str(path), dtype=np.uint8)
    img = cv2.imdecode(raw, cv2.IMREAD_GRAYSCALE)
    if img is None:
        raise ValueError(f"No se pudo leer: {path}")
    return img


def prepare(path: Path, orb):
    gray = load_gray(path)
    # Bound memory and CPU; identical scaling is not required for ORB.
    h, w = gray.shape
    scale = min(1.0, 1200.0 / max(h, w))
    if scale < 1:
        gray = cv2.resize(gray, (round(w * scale), round(h * scale)),
                          interpolation=cv2.INTER_AREA)
    keypoints, descriptors = orb.detectAndCompute(gray, None)
    return keypoints, descriptors


def compare(a, b, matcher):
    ka, da = a
    kb, db = b
    if da is None or db is None or len(da) < 2 or len(db) < 2:
        return 0, 0, 0.0, False
    # Mutual nearest-neighbor ratio matches reduce repeated-pattern matches.
    forward = matcher.knnMatch(da, db, k=2)
    reverse = matcher.knnMatch(db, da, k=2)
    good_f = {m.queryIdx: m for pair in forward if len(pair) == 2
              for m, n in [pair] if m.distance < RATIO * n.distance}
    good_r = {m.queryIdx: m for pair in reverse if len(pair) == 2
              for m, n in [pair] if m.distance < RATIO * n.distance}
    mutual = [m for m in good_f.values()
              if m.trainIdx in good_r and good_r[m.trainIdx].trainIdx == m.queryIdx]
    count = len(mutual)
    if count < MIN_INLIERS:
        return count, 0, 0.0, False
    src = np.float32([ka[m.queryIdx].pt for m in mutual]).reshape(-1, 1, 2)
    dst = np.float32([kb[m.trainIdx].pt for m in mutual]).reshape(-1, 1, 2)
    # Partial affine RANSAC tolerates translation, uniform scale and rotation.
    # It avoids the extra degrees of freedom of a homography.
    try:
        matrix, mask = cv2.estimateAffinePartial2D(
            src, dst, method=cv2.RANSAC, ransacReprojThreshold=RANSAC_THRESHOLD,
            maxIters=2000, confidence=0.995, refineIters=10)
    except cv2.error:
        return count, 0, 0.0, False
    if matrix is None or mask is None:
        return count, 0, 0.0, False
    inliers = int(mask.sum())
    fraction = inliers / count
    # Require spatial distribution: a tiny repeated logo is not enough.
    inlier_points = src[mask.ravel().astype(bool), 0, :]
    spread = np.ptp(inlier_points, axis=0)
    all_points = np.float32([kp.pt for kp in ka])
    extent = np.maximum(np.ptp(all_points, axis=0), 1)
    spread_ok = bool(np.all(spread / extent >= 0.12))
    passed = inliers >= MIN_INLIERS and fraction >= MIN_INLIER_RATIO and spread_ok
    return count, inliers, fraction, passed


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("folder", type=Path, help="Carpeta con las 65 imágenes F01...F05")
    parser.add_argument("--csv", type=Path, default=None,
                        help="Ruta opcional para exportar las 2080 comparaciones")
    args = parser.parse_args()
    folder = args.folder.expanduser().resolve()
    if not folder.is_dir():
        parser.error(f"No existe la carpeta: {folder}")
    images = sorted(p for p in folder.iterdir() if p.is_file()
                    and p.suffix.lower() in EXTS and p.name.startswith("F"))
    if len(images) != 65:
        parser.error(f"Se esperaban 65 imágenes etiquetadas; encontradas {len(images)}")
    families = [p.stem.split("__")[0] for p in images]
    counts = Counter(families)
    if len(counts) != 5 or any(n != 13 for n in counts.values()):
        parser.error(f"Se esperaban 5 familias de 13 imágenes: {dict(counts)}")

    started = time.perf_counter()
    orb = cv2.ORB_create(nfeatures=ORB_FEATURES, fastThreshold=12)
    matcher = cv2.BFMatcher(cv2.NORM_HAMMING)
    features = [prepare(p, orb) for p in images]
    print(f"Imágenes: {len(images)}; keypoints por imagen: "
          f"mín={min(len(k) for k, _ in features)}, "
          f"máx={max(len(k) for k, _ in features)}")
    metrics = Counter()
    variants = defaultdict(Counter)
    false_positives = []
    false_negatives = []
    rows = []
    for i, j in itertools.combinations(range(len(images)), 2):
        matches, inliers, ratio, predicted = compare(features[i], features[j], matcher)
        actual = families[i] == families[j]
        key = "TP" if predicted and actual else "FP" if predicted else "FN" if actual else "TN"
        metrics[key] += 1
        if key == "FP" and len(false_positives) < 12:
            false_positives.append((images[i].name, images[j].name, inliers))
        if key == "FN" and len(false_negatives) < 12:
            false_negatives.append((images[i].name, images[j].name, inliers))
        for idx in (i, j):
            if actual and images[idx].stem.split("__")[1] in ("07", "08"):
                variants[images[idx].stem.split("__")[1]][key] += 1
        if args.csv:
            rows.append((images[i].name, images[j].name, int(actual), int(predicted),
                         matches, inliers, round(ratio, 4), key))
    tp, fp, fn = metrics["TP"], metrics["FP"], metrics["FN"]
    precision = 100 * tp / (tp + fp) if tp + fp else 0
    recall = 100 * tp / (tp + fn) if tp + fn else 0
    print("\nORB + RANSAC — prueba independiente (no combinado con pHash)")
    print(f"TP={tp} FP={fp} FN={fn} TN={metrics['TN']}")
    print(f"Precisión={precision:.2f}%  Recall={recall:.2f}%")
    print(f"Tiempo total={time.perf_counter() - started:.2f}s")
    print("Recortes por variante (comparaciones positivas que incluyen esa variante):")
    for variant in ("07", "08"):
        print(f"  {variant}: {dict(variants[variant])}")
    print("Falsos positivos (hasta 12):", false_positives)
    print("Falsos negativos (hasta 12):", false_negatives)
    print("Referencia anterior: detector orientado pHash TP=280 FP=0 FN=110 TN=1690.")
    print("No sumar TP entre detectores: se requiere calcular la unión por par.")
    if args.csv:
        args.csv.parent.mkdir(parents=True, exist_ok=True)
        with args.csv.open("w", newline="", encoding="utf-8-sig") as f:
            writer = csv.writer(f)
            writer.writerow(("first", "second", "same_family", "predicted",
                             "mutual_matches", "ransac_inliers", "inlier_ratio", "outcome"))
            writer.writerows(rows)
        print(f"Detalle CSV: {args.csv}")
    print("Ninguna fotografía fue modificada.")


if __name__ == "__main__":
    main()
