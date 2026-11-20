import { useEffect, useMemo, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { FolderOpen, Images, ScanSearch, ShieldCheck, AlertTriangle } from "lucide-react";

type ScanSummary = { root: string; files: number; bytes: number; errors: number; by_extension: Record<string, number> };
type SimilarityReport = { analyzed: number; skipped: number; errors: number; threshold: number; pairs: { first: string; second: string; distance: number }[] };
type ScanProgress = { phase: string; processed: number; total: number };
type DuplicateGroup = { size_bytes: number; paths: string[] };
type DuplicateReport = { scanned_files: number; errors: number; groups: DuplicateGroup[]; redundant_bytes: number; cache_hits: number; hashes_computed: number; cache_available: boolean };
const fmt = (n: number) => n.toLocaleString("es-MX");
const gb = (n: number) => (n / 1073741824).toFixed(2) + " GB";

type VisualGroup = { paths: string[]; maxDistance: number };

// Conservative complete-link grouping: every member must have a reported
// qualifying similarity to every other member. Never infer A~C from A~B~C.
function buildVisualGroups(pairs: SimilarityReport["pairs"]): VisualGroup[] {
  const adjacency = new Map<string, Map<string, number>>();
  for (const { first, second, distance } of pairs) {
    if (first === second) continue;
    if (!adjacency.has(first)) adjacency.set(first, new Map());
    if (!adjacency.has(second)) adjacency.set(second, new Map());
    adjacency.get(first)!.set(second, distance);
    adjacency.get(second)!.set(first, distance);
  }
  const paths = [...adjacency.keys()].sort((a,b) =>
    (adjacency.get(b)!.size - adjacency.get(a)!.size) || a.localeCompare(b));
  const assigned = new Set<string>();
  const groups: VisualGroup[] = [];
  for (const anchor of paths) {
    if (assigned.has(anchor)) continue;
    const members = [anchor];
    let maxDistance = 0;
    const candidates = [...adjacency.get(anchor)!.entries()]
      .filter(([path]) => !assigned.has(path))
      .sort((a,b) => a[1] - b[1] || a[0].localeCompare(b[0]));
    for (const [candidate] of candidates) {
      const distances = members.map(member => adjacency.get(member)?.get(candidate));
      if (distances.every((d): d is number => d !== undefined)) {
        members.push(candidate);
        maxDistance = Math.max(maxDistance, ...distances);
      }
    }
    if (members.length >= 2) {
      members.forEach(path => assigned.add(path));
      groups.push({ paths: members, maxDistance });
    }
  }
  return groups.sort((a,b) => b.paths.length - a.paths.length || a.paths[0].localeCompare(b.paths[0]));
}

// Reuse thumbnails for repeated files and bound decoding work.
// The cache is scoped to one similarity scan and holds at most 120 entries.
const previewCache = new Map<string, Promise<string>>();
const previewQueue: Array<() => void> = [];
let activePreviews = 0;
const MAX_ACTIVE_PREVIEWS = 3;
function schedulePreview(task: () => Promise<string>): Promise<string> {
  return new Promise((resolve, reject) => {
    const run = () => {
      activePreviews++;
      task().then(resolve, reject).finally(() => {
        activePreviews--;
        previewQueue.shift()?.();
      });
    };
    if (activePreviews < MAX_ACTIVE_PREVIEWS) run();
    else previewQueue.push(run);
  });
}
function loadPreview(root: string, path: string): Promise<string> {
  const key = JSON.stringify([root, path]);
  const existing = previewCache.get(key);
  if (existing) return existing;
  const request = schedulePreview(() => invoke<string>("preview_photo", { root, path }));
  previewCache.set(key, request);
  // Remove failed loads so a later attempt can retry.
  void request.catch(() => { if (previewCache.get(key) === request) previewCache.delete(key); });
  if (previewCache.size > 120) {
    const oldest = previewCache.keys().next().value;
    if (oldest !== undefined) previewCache.delete(oldest);
  }
  return request;
}

function PhotoPreview({ root, path }: { root: string; path: string }) {
  const [src, setSrc] = useState("");
  const [failed, setFailed] = useState(false);
  useEffect(() => {
    let active = true;
    setSrc(""); setFailed(false);
    loadPreview(root, path)
      .then(value => { if (active) setSrc(value); })
      .catch(() => { if (active) setFailed(true); });
    return () => { active = false; };
  }, [root, path]);
  const name = path.split(/[\\/]/).pop() ?? path;
  return <div style={{ minWidth: 0, flex: "1 1 220px" }}>
    <div style={{ height: 180, display: "flex", alignItems: "center", justifyContent: "center", background: "#101521", borderRadius: 8, overflow: "hidden" }}>
      {src ? <img src={src} alt={name} style={{ maxWidth: "100%", maxHeight: "100%", objectFit: "contain" }} /> : <span style={{ opacity: .65 }}>{failed ? "Vista previa no disponible" : "Cargando miniatura…"}</span>}
    </div>
    <div className="duplicate-path" title={path} style={{ overflowWrap: "anywhere", marginTop: 8 }}>{name}</div>
  </div>;
}

export default function App() {
  const [page, setPage] = useState<"library" | "duplicates" | "similar">("library");
  const [folder, setFolder] = useState("");
  const [result, setResult] = useState<ScanSummary | null>(null);
  const [duplicates, setDuplicates] = useState<DuplicateReport | null>(null);
  const [similar, setSimilar] = useState<SimilarityReport | null>(null);
  const [visiblePairs, setVisiblePairs] = useState(20);
  const [visibleGroups, setVisibleGroups] = useState(10);
  const [similarityView, setSimilarityView] = useState<"groups" | "pairs">("groups");
  const visualGroups = useMemo(() => buildVisualGroups(similar?.pairs ?? []), [similar]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [progress, setProgress] = useState<ScanProgress | null>(null);
  const [cancelling, setCancelling] = useState(false);
  async function chooseFolder() {
    setError("");
    try {
      const selected = await open({ directory: true, multiple: false, title: "Selecciona tu biblioteca de fotos" });
      if (typeof selected === "string") { setFolder(selected); setResult(null); setDuplicates(null); setSimilar(null); setPage("library"); }
    } catch (e) { setError(String(e)); }
  }
  async function scan() {
    if (!folder || busy) return;
    setBusy(true); setError(""); setResult(null); setDuplicates(null);
    try { setResult(await invoke<ScanSummary>("scan_folder", { root: folder })); setPage("library"); }
    catch (e) { setError(String(e)); }
    finally { setBusy(false); }
  }
  async function checkDuplicates() {
    if (!folder || busy) return;
    setBusy(true); setError(""); setDuplicates(null); setProgress({ phase: "preparando", processed: 0, total: 0 }); setCancelling(false);
    let unlisten: (() => void) | undefined;
    try {
      unlisten = await listen<ScanProgress>("photovault:scan-progress", event => setProgress(event.payload));
      setDuplicates(await invoke<DuplicateReport>("find_exact_duplicates", { root: folder })); setPage("duplicates");
    } catch (e) { setError(String(e)); }
    finally { unlisten?.(); setBusy(false); setProgress(null); setCancelling(false); }
  }
  async function checkSimilar() {
    if (!folder || busy) return;
    setBusy(true); setError(""); setSimilar(null); previewCache.clear(); setVisiblePairs(20); setVisibleGroups(10); setSimilarityView("groups"); setProgress({ phase: "preparando", processed: 0, total: 0 }); setCancelling(false);
    let unlisten: (() => void) | undefined;
    try {
      unlisten = await listen<ScanProgress>("photovault:scan-progress", event => setProgress(event.payload));
      setSimilar(await invoke<SimilarityReport>("find_similar_photos", { root: folder })); setPage("similar");
    } catch (e) { setError(String(e)); }
    finally { unlisten?.(); setBusy(false); setProgress(null); setCancelling(false); }
  }
  async function cancelScan() {
    if (!busy || !progress || cancelling) return;
    setCancelling(true);
    try { await invoke("cancel_duplicate_scan"); }
    catch (e) { setError(String(e)); setCancelling(false); }
  }
  const extensions = Object.entries(result?.by_extension ?? {}).sort((a,b) => b[1]-a[1]);
  return <div className="shell">
    <aside className="sidebar">
      <div className="brand"><Images size={26}/><span>PhotoVault</span></div>
      <button className={page === "library" ? "nav active" : "nav"} onClick={() => setPage("library")} style={{ width: "100%", textAlign: "left", border: 0, cursor: "pointer", background: page === "library" ? undefined : "transparent" }}><ScanSearch size={18}/> Biblioteca</button>
      <button className={page === "duplicates" ? "nav active" : "nav"} onClick={() => setPage("duplicates")} style={{ width: "100%", textAlign: "left", border: 0, cursor: "pointer", background: page === "duplicates" ? undefined : "transparent" }}><ShieldCheck size={18}/> Duplicados exactos</button>
      <button className={page === "similar" ? "nav active" : "nav"} onClick={() => setPage("similar")} style={{ width: "100%", textAlign: "left", border: 0, cursor: "pointer", background: page === "similar" ? undefined : "transparent" }}><Images size={18}/> Similitud visual</button>
      <div className="foot">v0.1.0 · Solo lectura</div>
    </aside>
    <main>
      <div className="eyebrow">{page === "library" ? "BIBLIOTECA LOCAL" : page === "duplicates" ? "DUPLICADOS EXACTOS" : "SIMILITUD VISUAL"}</div>
      <h1>{page === "library" ? "Tu colección, bajo control." : page === "duplicates" ? "Archivos idénticos." : "Fotografías similares."}</h1>
      <p className="intro">{page === "library" ? "Selecciona una biblioteca y explora su contenido." : page === "duplicates" ? "Identifica copias verificadas byte a byte." : "Revisa coincidencias perceptuales antes de tomar decisiones."}</p>
      <section className="panel">
        <h2>{page === "library" ? "Seleccionar carpeta" : "Biblioteca seleccionada"}</h2>
        <div className="actions" style={{ flexWrap: "wrap" }}>
          <button className="secondary" onClick={chooseFolder} disabled={busy}><FolderOpen size={18}/> {folder ? "Cambiar carpeta" : "Examinar"}</button>
          <div className="path" title={folder} style={{ overflowWrap: "anywhere", minWidth: 0 }}>{folder || "Ninguna carpeta seleccionada"}</div>
          {page === "library" && <button disabled={!folder || busy} onClick={scan}>{busy ? "Analizando…" : "Inventariar"}</button>}
          {page === "duplicates" && <button disabled={!folder || busy} onClick={checkDuplicates}><ShieldCheck size={17}/> {duplicates ? "Volver a analizar" : "Buscar duplicados"}</button>}
          {page === "similar" && <button disabled={!folder || busy} onClick={checkSimilar}><Images size={17}/> {similar ? "Volver a analizar" : "Buscar similares"}</button>}
          {busy && progress && <button className="secondary" onClick={cancelScan} disabled={cancelling}>{cancelling ? "Cancelando…" : "Cancelar análisis"}</button>}
        </div>
      </section>
      {page === "library" && folder && <section className="panel"><h2>¿Qué quieres hacer?</h2><div className="actions" style={{ flexWrap: "wrap" }}>
        <button onClick={() => setPage("duplicates")}><ShieldCheck size={17}/> Buscar duplicados exactos</button>
        <button onClick={() => setPage("similar")}><Images size={17}/> Buscar fotografías similares</button>
      </div></section>}
      {progress && <section className="panel" role="status"><h2>Progreso del análisis</h2><p>Etapa: {progress.phase} · {fmt(progress.processed)} / {progress.total ? fmt(progress.total) : "calculando"} archivos candidatos</p>{progress.total > 0 && <progress max={progress.total} value={progress.processed} style={{width:"100%"}} />}</section>}
      {error && <div className="error"><AlertTriangle size={18}/>{error}</div>}
      {page === "library" && result && <><div className="stats"><div className="stat"><span>Archivos multimedia</span><strong>{fmt(result.files)}</strong></div><div className="stat"><span>Espacio total</span><strong>{gb(result.bytes)}</strong></div><div className="stat"><span>Errores de acceso</span><strong>{result.errors}</strong></div></div>
        <section className="panel"><h2>Formatos encontrados</h2><div className="formats">{extensions.map(([ext,count]) => <div key={ext} className="format"><span>{ext.toUpperCase()}</span><strong>{fmt(count)}</strong></div>)}</div></section></>}
      {page === "duplicates" && !duplicates && !busy && <section className="panel"><p>Ejecuta el análisis para encontrar archivos idénticos en la carpeta seleccionada.</p></section>}
      {page === "duplicates" && duplicates && <><div className="stats"><div className="stat"><span>Grupos exactos</span><strong>{fmt(duplicates.groups.length)}</strong></div><div className="stat"><span>Espacio redundante estimado</span><strong>{gb(duplicates.redundant_bytes)}</strong></div><div className="stat"><span>Hashes reutilizados</span><strong>{fmt(duplicates.cache_hits)}</strong></div></div>
        <section className="panel"><h2>Archivos idénticos por bytes</h2><p>Hashes calculados: {fmt(duplicates.hashes_computed)} · Caché SQLite: {duplicates.cache_available ? "disponible" : "no disponible"} · Errores: {fmt(duplicates.errors)}</p><p>Coincidencias verificadas al analizar. Ningún archivo será eliminado; los resultados pueden quedar obsoletos si cambia la biblioteca.</p>
        {duplicates.groups.length === 0 && <p>No se encontraron duplicados exactos.</p>}
        {duplicates.groups.map((group, i) => <div className="duplicate-group" key={i}><strong>Grupo {i+1} · {fmt(group.paths.length)} copias · {gb(group.size_bytes)} por archivo</strong>{group.paths.map(path => <div className="duplicate-path" key={path} title={path}>{path}</div>)}</div>)}</section></>}
      {page === "similar" && !similar && !busy && <section className="panel"><p>Ejecuta el análisis para descubrir fotografías visualmente similares.</p></section>}
      {page === "similar" && similar && <section className="panel"><h2>Fotografías visualmente similares · Experimental</h2>
        <p>Analizadas: {fmt(similar.analyzed)} · Omitidas por límite de 3,000: {fmt(similar.skipped)} · Errores: {fmt(similar.errors)} · Umbral pHash: {similar.threshold}/64</p>
        <p>Coincidencias probables, no duplicados confirmados. La agrupación exige que cada fotografía coincida con todas las demás del mismo grupo; no elimina archivos.</p>
        <div className="actions" style={{ marginBottom: 16 }}>
          <button className={similarityView === "groups" ? "" : "secondary"} onClick={() => setSimilarityView("groups")}>Galería de grupos ({fmt(visualGroups.length)})</button>
          <button className={similarityView === "pairs" ? "" : "secondary"} onClick={() => setSimilarityView("pairs")}>Ver parejas ({fmt(similar.pairs.length)})</button>
        </div>
        {similarityView === "groups" ? <>
          {visualGroups.length === 0 && <p>No se encontraron grupos visuales con al menos dos fotografías.</p>}
          {visualGroups.slice(0, visibleGroups).map((group,i) => <div className="duplicate-group" key={group.paths[0]}>
            <strong>Grupo {i+1} · {fmt(group.paths.length)} fotografías · Distancia máxima interna {group.maxDistance}/64</strong>
            <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fill, minmax(180px, 1fr))", gap: 14, marginTop: 12 }}>
              {group.paths.map((path,j) => <div key={path} style={{ minWidth: 0 }}>
                {j === 0 && <div style={{ marginBottom: 6, fontSize: 12, opacity: .8 }}>Referencia del grupo</div>}
                <PhotoPreview root={folder} path={path} />
              </div>)}
            </div>
          </div>)}
          {visualGroups.length > visibleGroups && <button className="secondary" onClick={() => setVisibleGroups(n => n + 10)}>Mostrar 10 grupos más ({fmt(visualGroups.length - visibleGroups)} pendientes)</button>}
        </> : <>
          {similar.pairs.length === 0 && <p>No se encontraron coincidencias dentro del umbral.</p>}
          {similar.pairs.slice(0, visiblePairs).map((pair,i) => <div className="duplicate-group" key={i}>
            <strong>Pareja {i+1} · Distancia {pair.distance}/64</strong>
            <div style={{ display: "flex", gap: 14, flexWrap: "wrap", marginTop: 12 }}>
              <PhotoPreview root={folder} path={pair.first} />
              <PhotoPreview root={folder} path={pair.second} />
            </div>
          </div>)}
          {similar.pairs.length > visiblePairs && <button className="secondary" onClick={() => setVisiblePairs(n => n + 20)}>Mostrar 20 parejas más ({fmt(similar.pairs.length - visiblePairs)} pendientes)</button>}
        </>}
      </section>}
      <div className="notice"><ShieldCheck size={20}/><span>Modo seguro: inventario y comparación de bytes de solo lectura. No se eliminan archivos. La similitud visual es experimental y no implica igualdad de archivos.</span></div>
    </main></div>;
}
