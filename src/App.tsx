import { useEffect, useMemo, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { FolderOpen, Images, ScanSearch, ShieldCheck, AlertTriangle, ArrowRight, CheckCircle2, Clock3, Layers3, Sparkles } from "lucide-react";

type ScanSummary = { root: string; files: number; bytes: number; errors: number; by_extension: Record<string, number> };
type SimilarityReport = { analyzed: number; skipped: number; errors: number; threshold: number; pairs: { first: string; second: string; distance: number }[] };
type PlannedFile = { path: string; category: "photo" | "snapshot" | "possible_trash" | "undated" | "video"; suggested_folder: string; reason: string; date_source: string; width: number | null; height: number | null };
type OrganizationPlan = { analyzed: number; skipped: number; errors: number; files: PlannedFile[]; read_only: boolean };
type ScanProgress = { phase: string; processed: number; total: number };
type DuplicateGroup = { size_bytes: number; paths: string[] };
type DuplicateReport = { scanned_files: number; errors: number; groups: DuplicateGroup[]; redundant_bytes: number; cache_hits: number; hashes_computed: number; cache_available: boolean };
const fmt = (n: number) => n.toLocaleString("es-MX");
const gb = (n: number) => (n / 1073741824).toFixed(2) + " GB";
const humanSize = (n: number) => n >= 1073741824 ? (n / 1073741824).toFixed(2) + " GB" : n >= 1048576 ? (n / 1048576).toFixed(2) + " MB" : n >= 1024 ? (n / 1024).toFixed(1) + " KB" : n + " B";

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
  return <div className="photo-tile">
    <div className="photo-tile-media">
      {src ? <img src={src} alt={name} loading="lazy" /> : <span style={{ opacity: .65 }}>{failed ? "Vista previa no disponible" : "Cargando miniatura…"}</span>}
    </div>
    <div className="photo-tile-name" title={path}>{name}</div>
  </div>;
}

export default function App() {
  const [page, setPage] = useState<"library" | "duplicates" | "similar" | "organize">("library");
  const [folder, setFolder] = useState("");
  const [result, setResult] = useState<ScanSummary | null>(null);
  const [duplicates, setDuplicates] = useState<DuplicateReport | null>(null);
  const [similar, setSimilar] = useState<SimilarityReport | null>(null);
  const [organization, setOrganization] = useState<OrganizationPlan | null>(null);
  const [organizeFilter, setOrganizeFilter] = useState<"all" | PlannedFile["category"]>("all");
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
      if (typeof selected === "string") { setFolder(selected); setResult(null); setDuplicates(null); setSimilar(null); setOrganization(null); setPage("library"); }
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
  async function previewOrganization() {
    if (!folder || busy) return;
    setBusy(true); setError(""); setOrganization(null);
    try { setOrganization(await invoke<OrganizationPlan>("preview_organization", { root: folder })); setPage("organize"); }
    catch (e) { setError(String(e)); }
    finally { setBusy(false); }
  }
  async function cancelScan() {
    if (!busy || !progress || cancelling) return;
    setCancelling(true);
    try { await invoke("cancel_duplicate_scan"); }
    catch (e) { setError(String(e)); setCancelling(false); }
  }
  const organizationCategories: { key: PlannedFile["category"]; title: string }[] = [
    {key:"photo",title:"Fotografías"}, {key:"video",title:"Videos"},
    {key:"snapshot",title:"Capturas / mensajería"}, {key:"possible_trash",title:"Posible basura"},
    {key:"undated",title:"Sin fecha / revisión"}
  ];
  const extensions = Object.entries(result?.by_extension ?? {}).sort((a,b) => b[1]-a[1]);
  return <div className="shell">
    <aside className="sidebar">
      <div className="brand"><span className="brand-mark"><Images size={23}/></span><span>PhotoVault<small>PHOTO LIBRARY</small></span></div><div className="sidebar-label">EXPLORAR</div>
      <button className={page === "library" ? "nav active" : "nav"} onClick={() => setPage("library")} style={{ width: "100%", textAlign: "left", border: 0, cursor: "pointer", background: page === "library" ? undefined : "transparent" }}><ScanSearch size={18}/> Biblioteca</button>
      <button className={page === "duplicates" ? "nav active" : "nav"} onClick={() => setPage("duplicates")} style={{ width: "100%", textAlign: "left", border: 0, cursor: "pointer", background: page === "duplicates" ? undefined : "transparent" }}><ShieldCheck size={18}/> Duplicados exactos</button>
      <button className={page === "similar" ? "nav active" : "nav"} onClick={() => setPage("similar")} style={{ width: "100%", textAlign: "left", border: 0, cursor: "pointer", background: page === "similar" ? undefined : "transparent" }}><Images size={18}/> Similitud visual</button>
      <button className={page === "organize" ? "nav active" : "nav"} onClick={() => setPage("organize")} style={{ width: "100%", textAlign: "left", border: 0, cursor: "pointer", background: page === "organize" ? undefined : "transparent" }}><FolderOpen size={18}/> Organizar fotografías</button>
      <div className="sidebar-status"><ShieldCheck size={17}/><div><strong>Tu biblioteca está protegida</strong><span>Sin cambios en tus archivos</span></div></div><div className="foot">PhotoVault v0.1.0 · Vista previa</div>
    </aside>
    <main>
      <div className="topline"><div className="eyebrow">{page === "library" ? "BIBLIOTECA LOCAL" : page === "duplicates" ? "DUPLICADOS EXACTOS" : page === "organize" ? "PLAN DE ORGANIZACIÓN" : "SIMILITUD VISUAL"}</div><span className="readonly-pill"><ShieldCheck size={14}/> Modo seguro · Solo lectura</span></div>
      <h1>{page === "library" ? "Tu colección, bajo control." : page === "duplicates" ? "Duplicados confirmados." : page === "organize" ? "Organiza sin arriesgar tus recuerdos." : "Posibles fotografías relacionadas."}</h1>
      <p className="intro">{page === "library" ? "Selecciona una biblioteca y explora su contenido." : page === "duplicates" ? "Archivos con el mismo contenido, verificados mediante hash criptográfico. No se borrará nada." : page === "organize" ? "Vista previa de carpetas sugeridas, capturas y posible basura. Sin mover, renombrar ni borrar archivos." : "Sugerencias basadas en características visuales; pueden existir coincidencias incorrectas. Tú decides qué conservar."}</p>
      <section className="panel library-selector">
        <div className="section-heading"><span className="section-icon"><FolderOpen size={20}/></span><div><h2>{page === "library" ? "Tu carpeta de fotografías" : "Biblioteca seleccionada"}</h2><p>{folder ? "Origen de los archivos que analizaremos" : "Elige una carpeta para comenzar. No modificaremos su contenido."}</p></div></div>
        <div className="actions" style={{ flexWrap: "wrap" }}>
          <button className="secondary" onClick={chooseFolder} disabled={busy}><FolderOpen size={18}/> {folder ? "Cambiar carpeta" : "Examinar"}</button>
          <div className="path" title={folder} style={{ overflowWrap: "anywhere", minWidth: 0 }}>{folder || "Ninguna carpeta seleccionada"}</div>
          {page === "library" && <button disabled={!folder || busy} onClick={scan}>{busy ? "Analizando…" : "Inventariar"}</button>}
          {page === "duplicates" && <button disabled={!folder || busy} onClick={checkDuplicates}><ShieldCheck size={17}/> {duplicates ? "Volver a analizar" : "Buscar duplicados"}</button>}
          {page === "organize" && <button disabled={!folder || busy} onClick={previewOrganization}><FolderOpen size={17}/> {organization ? "Actualizar propuesta" : "Preparar propuesta"}</button>}
          {page === "similar" && <button disabled={!folder || busy} onClick={checkSimilar}><Images size={17}/> {similar ? "Volver a analizar" : "Buscar similares"}</button>}
          {busy && progress && <button className="secondary" onClick={cancelScan} disabled={cancelling}>{cancelling ? "Cancelando…" : "Cancelar análisis"}</button>}
        </div>
      </section>
      {page === "library" && folder && <section className="workflows"><div className="section-heading"><div><h2>¿Qué te gustaría revisar?</h2><p>Elige una herramienta para explorar tu colección.</p></div></div><div className="workflow-grid"><button className="workflow-card" onClick={() => setPage("duplicates")}><span className="workflow-icon"><Layers3 size={23}/></span><strong>Duplicados confirmados</strong><span>Archivos idénticos verificados mediante hash criptográfico. Descubre cuánto espacio ocupan las copias.</span><span className="workflow-link">Explorar copias <ArrowRight size={16}/></span></button><button className="workflow-card" onClick={() => setPage("similar")}><span className="workflow-icon violet"><Sparkles size={23}/></span><strong>Posibles fotografías relacionadas</strong><span>Explora sugerencias visuales de versiones editadas o tomas parecidas. Ninguna coincidencia se considera confirmada.</span><span className="workflow-link">Explorar similitudes <ArrowRight size={16}/></span></button><button className="workflow-card" onClick={() => setPage("organize")}><span className="workflow-icon"><FolderOpen size={23}/></span><strong>Plan de organización</strong><span>Clasifica fotos, videos, capturas y posibles archivos prescindibles. Solo propone carpetas, sin modificar nada.</span><span className="workflow-link">Ver propuesta <ArrowRight size={16}/></span></button></div></section>}
      {progress && <section className="panel progress-panel" role="status" aria-live="polite"><div className="progress-header"><div><h2>Análisis en curso</h2><p>Etapa: {progress.phase} · {fmt(progress.processed)} de {progress.total ? fmt(progress.total) : "total por calcular"} archivos</p></div><Clock3 size={22}/></div>{progress.total > 0 && <><progress max={progress.total} value={progress.processed} /><span className="progress-caption">{Math.min(100,Math.round(progress.processed/progress.total*100))}% completado</span></>}</section>}
      {error && <div className="error"><AlertTriangle size={18}/>{error}</div>}
      {page === "library" && result && <><div className="stats"><div className="stat"><span>Archivos multimedia</span><strong>{fmt(result.files)}</strong></div><div className="stat"><span>Espacio total</span><strong>{gb(result.bytes)}</strong></div><div className="stat"><span>Errores de acceso</span><strong>{result.errors}</strong></div></div>
        <section className="panel"><h2>Formatos encontrados</h2><div className="formats">{extensions.map(([ext,count]) => <div key={ext} className="format"><span>{ext.toUpperCase()}</span><strong>{fmt(count)}</strong></div>)}</div></section></>}
      {page === "organize" && !organization && !busy && <section className="panel"><p>Prepara una propuesta sin cambios sobre tus archivos. La detección de basura es solo una advertencia, no una orden de borrado.</p></section>}
      {page === "organize" && organization && <section className="panel">
        <h2>Propuesta de organización · Solo lectura</h2>
        <p>Archivos analizados: {fmt(organization.analyzed)} · Omitidos por límite: {fmt(organization.skipped)} · Errores: {fmt(organization.errors)}</p>
        <p>La fecha se extrae del nombre cuando existe. Esta versión todavía no verifica EXIF ni agrupa eventos; las carpetas son sugerencias provisionales. Ningún archivo se mueve, renombra ni elimina.</p>
        <div className="actions" style={{ flexWrap: "wrap" }}>
          <button className={organizeFilter === "all" ? "" : "secondary"} onClick={() => setOrganizeFilter("all")}>Todos ({fmt(organization.files.length)})</button>
          {organizationCategories.map(({key,title}) => <button key={key} className={organizeFilter === key ? "" : "secondary"} onClick={() => setOrganizeFilter(key)}>{title} ({fmt(organization.files.filter(f => f.category === key).length)})</button>)}
        </div>
        {organization.files.filter(f => organizeFilter === "all" || f.category === organizeFilter).slice(0,100).map(f => <div className="duplicate-group" key={f.path}>
          <strong style={{overflowWrap:"anywhere"}}>{f.path.split(/[\\/]/).pop()}</strong>
          <p>{f.reason}</p>
          <p>Destino sugerido: <strong>{f.suggested_folder}</strong> · Fecha: {f.date_source}{f.width && f.height ? ` · ${f.width} × ${f.height}` : ""}</p>
          <small style={{overflowWrap:"anywhere"}}>{f.path}</small>
        </div>)}
        <p>Se muestran como máximo 100 archivos del filtro seleccionado. Los demás permanecen incluidos en el resumen.</p>
      </section>}
      {page === "duplicates" && !duplicates && !busy && <section className="panel"><p>Ejecuta el análisis para encontrar archivos idénticos en la carpeta seleccionada.</p></section>}
      {page === "duplicates" && duplicates && <><div className="stats"><div className="stat"><span>Grupos exactos</span><strong>{fmt(duplicates.groups.length)}</strong></div><div className="stat"><span>Espacio redundante estimado</span><strong>{humanSize(duplicates.redundant_bytes)}</strong></div><div className="stat"><span>Hashes reutilizados</span><strong>{fmt(duplicates.cache_hits)}</strong></div></div>
        <section className="panel"><h2>Duplicados confirmados · mismo contenido</h2><p>Hashes calculados: {fmt(duplicates.hashes_computed)} · Caché SQLite: {duplicates.cache_available ? "disponible" : "no disponible"} · Errores: {fmt(duplicates.errors)}</p><p>Confirmación de contenido idéntico mediante hash criptográfico al momento del análisis. El nombre, la ubicación y los metadatos del sistema de archivos pueden diferir. Ningún archivo será eliminado; los resultados pueden quedar obsoletos si cambia la biblioteca.</p>
        {duplicates.groups.length === 0 && <p>No se encontraron duplicados exactos.</p>}
        {duplicates.groups.map((group, i) => <div className="duplicate-group" key={i}><strong>Grupo {i+1} · {fmt(group.paths.length)} archivos · {humanSize(group.size_bytes)} por archivo · Contenido idéntico verificado</strong><div className="photo-grid">{group.paths.map(path => <div className="photo-grid-item" key={path}><PhotoPreview root={folder} path={path}/><div className="duplicate-path" title={path}>{path}</div></div>)}</div></div>)}</section></>}
      {page === "similar" && !similar && !busy && <section className="panel"><p>Ejecuta el análisis para descubrir fotografías visualmente similares.</p></section>}
      {page === "similar" && similar && <section className="panel similarity-results"><div className="section-heading"><span className="section-icon"><Sparkles size={21}/></span><div><h2>Posibles fotografías relacionadas</h2><p>Sugerencias experimentales de pHash · No equivalen a duplicados confirmados</p></div></div>
        <p>Analizadas: {fmt(similar.analyzed)} · Omitidas por límite de 3,000: {fmt(similar.skipped)} · Errores: {fmt(similar.errors)} · Umbral pHash: {similar.threshold}/64</p>
        <p>Estas fotografías podrían provenir de la misma captura o compartir rasgos visuales. La distancia pHash no es un porcentaje de probabilidad ni una garantía. La agrupación exige coincidencia entre todos sus miembros; no elimina archivos.</p>
        <div className="actions view-switcher">
          <button className={similarityView === "groups" ? "" : "secondary"} onClick={() => setSimilarityView("groups")}>Galería de grupos ({fmt(visualGroups.length)})</button>
          <button className={similarityView === "pairs" ? "" : "secondary"} onClick={() => setSimilarityView("pairs")}>Ver parejas ({fmt(similar.pairs.length)})</button>
        </div>
        {similarityView === "groups" ? <>
          {visualGroups.length === 0 && <p>No se encontraron grupos visuales con al menos dos fotografías.</p>}
          {visualGroups.slice(0, visibleGroups).map((group,i) => <div className="duplicate-group visual-group" key={group.paths[0]}>
            <strong>Grupo {i+1} · {fmt(group.paths.length)} fotografías · Posible misma captura</strong><p>Distancia pHash máxima entre miembros: {group.maxDistance}/64 (menor indica mayor semejanza perceptual; no representa una probabilidad).</p>
            <div className="photo-grid">
              {group.paths.map((path,j) => <div key={path} className="photo-grid-item">
                {j === 0 && <div className="reference-label">Primera coincidencia · no verificada como original</div>}
                <PhotoPreview root={folder} path={path} />
              </div>)}
            </div>
          </div>)}
          {visualGroups.length > visibleGroups && <button className="secondary" onClick={() => setVisibleGroups(n => n + 10)}>Mostrar 10 grupos más ({fmt(visualGroups.length - visibleGroups)} pendientes)</button>}
        </> : <>
          {similar.pairs.length === 0 && <p>No se encontraron coincidencias dentro del umbral.</p>}
          {similar.pairs.slice(0, visiblePairs).map((pair,i) => <div className="duplicate-group" key={i}>
            <strong>Pareja {i+1} · Posible relación visual · Distancia pHash {pair.distance}/64</strong>
            <div className="photo-pair">
              <PhotoPreview root={folder} path={pair.first} />
              <PhotoPreview root={folder} path={pair.second} />
            </div>
          </div>)}
          {similar.pairs.length > visiblePairs && <button className="secondary" onClick={() => setVisiblePairs(n => n + 20)}>Mostrar 20 parejas más ({fmt(similar.pairs.length - visiblePairs)} pendientes)</button>}
        </>}
      </section>}
      <div className="notice"><CheckCircle2 size={20}/><span>PhotoVault funciona en modo solo lectura. Los duplicados exactos se verifican por contenido; las similitudes visuales son sugerencias. Nunca se eliminan ni modifican tus fotografías.</span></div>
    </main></div>;
}
