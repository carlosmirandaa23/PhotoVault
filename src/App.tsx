import { useState } from "react";
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

export default function App() {
  const [folder, setFolder] = useState("");
  const [result, setResult] = useState<ScanSummary | null>(null);
  const [duplicates, setDuplicates] = useState<DuplicateReport | null>(null);
  const [similar, setSimilar] = useState<SimilarityReport | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [progress, setProgress] = useState<ScanProgress | null>(null);
  const [cancelling, setCancelling] = useState(false);
  async function chooseFolder() {
    setError("");
    try {
      const selected = await open({ directory: true, multiple: false, title: "Selecciona tu biblioteca de fotos" });
      if (typeof selected === "string") { setFolder(selected); setResult(null); setDuplicates(null); setSimilar(null); }
    } catch (e) { setError(String(e)); }
  }
  async function scan() {
    if (!folder || busy) return;
    setBusy(true); setError(""); setResult(null); setDuplicates(null);
    try { setResult(await invoke<ScanSummary>("scan_folder", { root: folder })); }
    catch (e) { setError(String(e)); }
    finally { setBusy(false); }
  }
  async function checkDuplicates() {
    if (!folder || busy) return;
    setBusy(true); setError(""); setDuplicates(null); setProgress({ phase: "preparando", processed: 0, total: 0 }); setCancelling(false);
    let unlisten: (() => void) | undefined;
    try {
      unlisten = await listen<ScanProgress>("photovault:scan-progress", event => setProgress(event.payload));
      setDuplicates(await invoke<DuplicateReport>("find_exact_duplicates", { root: folder }));
    } catch (e) { setError(String(e)); }
    finally { unlisten?.(); setBusy(false); setProgress(null); setCancelling(false); }
  }
  async function checkSimilar() {
    if (!folder || busy) return;
    setBusy(true); setError(""); setSimilar(null); setProgress({ phase: "preparando", processed: 0, total: 0 }); setCancelling(false);
    let unlisten: (() => void) | undefined;
    try {
      unlisten = await listen<ScanProgress>("photovault:scan-progress", event => setProgress(event.payload));
      setSimilar(await invoke<SimilarityReport>("find_similar_photos", { root: folder }));
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
    <aside className="sidebar"><div className="brand"><Images size={26}/><span>PhotoVault</span></div><div className="nav active"><ScanSearch size={18}/> Explorar biblioteca</div><div className="nav"><ShieldCheck size={18}/> Duplicados exactos</div><div className="foot">v0.1.0 · Solo lectura</div></aside>
    <main><div className="eyebrow">BIBLIOTECA LOCAL</div><h1>Tu colección, bajo control.</h1><p className="intro">Explora tus fotografías sin mover, modificar ni eliminar ningún archivo.</p>
      <section className="panel"><h2>Seleccionar carpeta</h2><p>Elige una carpeta de Windows para analizar archivos multimedia.</p>
        <div className="actions"><button className="secondary" onClick={chooseFolder} disabled={busy}><FolderOpen size={18}/> Examinar</button><div className="path" title={folder}>{folder || "Ninguna carpeta seleccionada"}</div><button disabled={!folder || busy} onClick={scan}>{busy ? "Analizando…" : "Inventariar"}</button><button disabled={!folder || busy} onClick={checkDuplicates}><ShieldCheck size={17}/> Buscar duplicados</button><button disabled={!folder || busy} onClick={checkSimilar}><Images size={17}/> Buscar similares (experimental)</button>{busy && progress && <button className="secondary" onClick={cancelScan} disabled={cancelling}>{cancelling ? "Cancelando…" : "Cancelar análisis"}</button>}</div>
      </section>
      {progress && <section className="panel" role="status"><h2>Progreso del análisis</h2><p>Etapa: {progress.phase} · {fmt(progress.processed)} / {progress.total ? fmt(progress.total) : "calculando"} archivos candidatos</p>{progress.total > 0 && <progress max={progress.total} value={progress.processed} style={{width:"100%"}} />}</section>}
      {error && <div className="error"><AlertTriangle size={18}/>{error}</div>}
      {result && <><div className="stats"><div className="stat"><span>Archivos multimedia</span><strong>{fmt(result.files)}</strong></div><div className="stat"><span>Espacio total</span><strong>{gb(result.bytes)}</strong></div><div className="stat"><span>Errores de acceso</span><strong>{result.errors}</strong></div></div>
        <section className="panel"><h2>Formatos encontrados</h2><div className="formats">{extensions.map(([ext,count]) => <div key={ext} className="format"><span>{ext.toUpperCase()}</span><strong>{fmt(count)}</strong></div>)}</div></section></>}
      {duplicates && <><div className="stats"><div className="stat"><span>Grupos exactos</span><strong>{fmt(duplicates.groups.length)}</strong></div><div className="stat"><span>Espacio redundante estimado</span><strong>{gb(duplicates.redundant_bytes)}</strong></div><div className="stat"><span>Hashes reutilizados</span><strong>{fmt(duplicates.cache_hits)}</strong></div></div>
        <section className="panel"><h2>Archivos idénticos por bytes</h2><p>Hashes calculados: {fmt(duplicates.hashes_computed)} · Caché SQLite: {duplicates.cache_available ? "disponible" : "no disponible"} · Errores: {fmt(duplicates.errors)}</p><p>Coincidencias verificadas al analizar. Ningún archivo será eliminado; los resultados pueden quedar obsoletos si cambia la biblioteca.</p>
        {duplicates.groups.length === 0 && <p>No se encontraron duplicados exactos.</p>}
        {duplicates.groups.map((group, i) => <div className="duplicate-group" key={i}><strong>Grupo {i+1} · {fmt(group.paths.length)} copias · {gb(group.size_bytes)} por archivo</strong>{group.paths.map(path => <div className="duplicate-path" key={path} title={path}>{path}</div>)}</div>)}</section></>}
      {similar && <section className="panel"><h2>Fotografías visualmente similares · Experimental</h2>
        <p>Analizadas: {fmt(similar.analyzed)} · Omitidas por límite de 3,000: {fmt(similar.skipped)} · Errores: {fmt(similar.errors)} · Umbral pHash: {similar.threshold}/64</p>
        <p>Coincidencias probables, no duplicados confirmados. Distancia menor indica mayor parecido; revisa cada pareja antes de actuar.</p>
        {similar.pairs.length === 0 && <p>No se encontraron coincidencias dentro del umbral.</p>}
        {similar.pairs.slice(0, 300).map((pair,i) => <div className="duplicate-group" key={i}><strong>Pareja {i+1} · Distancia {pair.distance}/64</strong><div className="duplicate-path">{pair.first}</div><div className="duplicate-path">{pair.second}</div></div>)}
        {similar.pairs.length > 300 && <p>Mostrando 300 de {fmt(similar.pairs.length)} parejas.</p>}
      </section>}
      <div className="notice"><ShieldCheck size={20}/><span>Modo seguro: inventario y comparación de bytes de solo lectura. No se eliminan archivos. La similitud visual es experimental y no implica igualdad de archivos.</span></div>
    </main></div>;
}
