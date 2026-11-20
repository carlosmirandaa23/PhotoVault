import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { FolderOpen, Images, ScanSearch, ShieldCheck, AlertTriangle } from "lucide-react";

type ScanSummary = { root: string; files: number; bytes: number; errors: number; by_extension: Record<string, number> };
type DuplicateGroup = { size_bytes: number; paths: string[] };
type DuplicateReport = { scanned_files: number; errors: number; groups: DuplicateGroup[]; redundant_bytes: number };
const fmt = (n: number) => n.toLocaleString("es-MX");
const gb = (n: number) => (n / 1073741824).toFixed(2) + " GB";

export default function App() {
  const [folder, setFolder] = useState("");
  const [result, setResult] = useState<ScanSummary | null>(null);
  const [duplicates, setDuplicates] = useState<DuplicateReport | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  async function chooseFolder() {
    setError("");
    try {
      const selected = await open({ directory: true, multiple: false, title: "Selecciona tu biblioteca de fotos" });
      if (typeof selected === "string") { setFolder(selected); setResult(null); setDuplicates(null); }
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
    setBusy(true); setError(""); setDuplicates(null);
    try { setDuplicates(await invoke<DuplicateReport>("find_exact_duplicates", { root: folder })); }
    catch (e) { setError(String(e)); }
    finally { setBusy(false); }
  }
  const extensions = Object.entries(result?.by_extension ?? {}).sort((a,b) => b[1]-a[1]);
  return <div className="shell">
    <aside className="sidebar"><div className="brand"><Images size={26}/><span>PhotoVault</span></div><div className="nav active"><ScanSearch size={18}/> Explorar biblioteca</div><div className="nav"><ShieldCheck size={18}/> Duplicados exactos</div><div className="foot">v0.1.0 · Solo lectura</div></aside>
    <main><div className="eyebrow">BIBLIOTECA LOCAL</div><h1>Tu colección, bajo control.</h1><p className="intro">Explora tus fotografías sin mover, modificar ni eliminar ningún archivo.</p>
      <section className="panel"><h2>Seleccionar carpeta</h2><p>Elige una carpeta de Windows para analizar archivos multimedia.</p>
        <div className="actions"><button className="secondary" onClick={chooseFolder} disabled={busy}><FolderOpen size={18}/> Examinar</button><div className="path" title={folder}>{folder || "Ninguna carpeta seleccionada"}</div><button disabled={!folder || busy} onClick={scan}>{busy ? "Analizando…" : "Inventariar"}</button><button disabled={!folder || busy} onClick={checkDuplicates}><ShieldCheck size={17}/> Buscar duplicados</button></div>
      </section>
      {error && <div className="error"><AlertTriangle size={18}/>{error}</div>}
      {result && <><div className="stats"><div className="stat"><span>Archivos multimedia</span><strong>{fmt(result.files)}</strong></div><div className="stat"><span>Espacio total</span><strong>{gb(result.bytes)}</strong></div><div className="stat"><span>Errores de acceso</span><strong>{result.errors}</strong></div></div>
        <section className="panel"><h2>Formatos encontrados</h2><div className="formats">{extensions.map(([ext,count]) => <div key={ext} className="format"><span>{ext.toUpperCase()}</span><strong>{fmt(count)}</strong></div>)}</div></section></>}
      {duplicates && <><div className="stats"><div className="stat"><span>Grupos exactos</span><strong>{fmt(duplicates.groups.length)}</strong></div><div className="stat"><span>Espacio redundante estimado</span><strong>{gb(duplicates.redundant_bytes)}</strong></div><div className="stat"><span>Errores de análisis</span><strong>{fmt(duplicates.errors)}</strong></div></div>
        <section className="panel"><h2>Archivos idénticos por bytes</h2><p>Coincidencias verificadas al analizar. Ningún archivo será eliminado; los resultados pueden quedar obsoletos si cambia la biblioteca.</p>
        {duplicates.groups.length === 0 && <p>No se encontraron duplicados exactos.</p>}
        {duplicates.groups.map((group, i) => <div className="duplicate-group" key={i}><strong>Grupo {i+1} · {fmt(group.paths.length)} copias · {gb(group.size_bytes)} por archivo</strong>{group.paths.map(path => <div className="duplicate-path" key={path} title={path}>{path}</div>)}</div>)}</section></>}
      <div className="notice"><ShieldCheck size={20}/><span>Modo seguro: inventario y comparación de bytes de solo lectura. No se eliminan archivos. Las fotos recortadas o recomprimidas requieren un análisis visual posterior.</span></div>
    </main></div>;
}
