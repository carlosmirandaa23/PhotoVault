import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { FolderOpen, Images, ScanSearch, ShieldCheck, AlertTriangle } from "lucide-react";

type ScanSummary = { root: string; files: number; bytes: number; errors: number; by_extension: Record<string, number> };

export default function App() {
  const [folder, setFolder] = useState("");
  const [result, setResult] = useState<ScanSummary | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  async function chooseFolder() {
    setError("");
    try {
      const selected = await open({ directory: true, multiple: false, title: "Selecciona tu biblioteca de fotos" });
      if (typeof selected === "string") { setFolder(selected); setResult(null); }
    } catch (e) { setError(String(e)); }
  }
  async function scan() {
    if (!folder || busy) return;
    setBusy(true); setError(""); setResult(null);
    try { setResult(await invoke<ScanSummary>("scan_folder", { root: folder })); }
    catch (e) { setError(String(e)); }
    finally { setBusy(false); }
  }
  const extensions = Object.entries(result?.by_extension ?? {}).sort((a,b) => b[1]-a[1]);
  return <div className="shell">
    <aside className="sidebar"><div className="brand"><Images size={26}/><span>PhotoVault</span></div><div className="nav active"><ScanSearch size={18}/> Explorar biblioteca</div><div className="nav disabled"><ShieldCheck size={18}/> Duplicados (próximamente)</div><div className="foot">v0.1.0 · Vista de solo lectura</div></aside>
    <main><div className="eyebrow">BIBLIOTECA LOCAL</div><h1>Tu colección, bajo control.</h1><p className="intro">Explora tus fotografías sin mover, modificar ni eliminar ningún archivo.</p>
      <section className="panel"><h2>Seleccionar carpeta</h2><p>Elige una carpeta de Windows para contar los archivos compatibles.</p>
        <div className="actions"><button className="secondary" onClick={chooseFolder}><FolderOpen size={18}/> Examinar</button><div className="path" title={folder}>{folder || "Ninguna carpeta seleccionada"}</div><button disabled={!folder || busy} onClick={scan}>{busy ? "Analizando…" : "Analizar"}</button></div>
      </section>
      {error && <div className="error"><AlertTriangle size={18}/>{error}</div>}
      {result && <><div className="stats"><div className="stat"><span>Archivos multimedia</span><strong>{result.files.toLocaleString("es-MX")}</strong></div><div className="stat"><span>Espacio total</span><strong>{(result.bytes / 1073741824).toFixed(2)} GB</strong></div><div className="stat"><span>Errores de acceso</span><strong>{result.errors}</strong></div></div>
        <section className="panel"><h2>Formatos encontrados</h2><div className="formats">{extensions.map(([ext,count]) => <div key={ext} className="format"><span>{ext.toUpperCase()}</span><strong>{count.toLocaleString("es-MX")}</strong></div>)}</div></section></>}
      <div className="notice"><ShieldCheck size={20}/><span>Modo seguro: el análisis únicamente lee metadatos del sistema de archivos. La detección de duplicados llegará en el siguiente hito.</span></div>
    </main></div>;
}
