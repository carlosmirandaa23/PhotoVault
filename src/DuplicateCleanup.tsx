import { useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Trash2, Undo2, CheckCircle2, AlertTriangle } from "lucide-react";

export type DuplicateGroup = { size_bytes: number; paths: string[] };
type Failure = { path: string; reason: string };
type CleanupResult = { batch_id: string; moved: string[]; failed: Failure[]; bytes_freed: number };
type BatchSummary = { batch_id: string; created_at: string; files: number; bytes: number; restored: number };
type RestoreResult = { restored: string[]; failed: Failure[] };

const fmt = (n: number) => n.toLocaleString("es-MX");
export const humanSize = (n: number) => n >= 1073741824 ? (n / 1073741824).toFixed(2) + " GB" : n >= 1048576 ? (n / 1048576).toFixed(2) + " MB" : n >= 1024 ? (n / 1024).toFixed(1) + " KB" : n + " B";
const fileName = (p: string) => p.split(/[\\/]/).pop() ?? p;

// Prefer the copy that looks like the "original": not named as a copy, not in a
// downloads/messaging/temp folder, then the shortest path. The user can change it.
const COPY_NAME = /(copy|copia|\(\d+\)|[-_ ]\d{1,2}$)/i;
const SECONDARY_FOLDER = /[\\/](downloads?|descargas|whatsapp|telegram|temp|tmp|backup|respaldo|copias?)[\\/]/i;
export function suggestKeeper(paths: string[]): string {
  const score = (p: string) => {
    const stem = fileName(p).replace(/\.[^.]+$/, "");
    return (COPY_NAME.test(stem) ? 2 : 0) + (SECONDARY_FOLDER.test(p) ? 1 : 0);
  };
  return [...paths].sort((a, b) => score(a) - score(b) || a.length - b.length || a.localeCompare(b))[0];
}

type Props = {
  root: string;
  groups: DuplicateGroup[];
  renderPreview: (path: string) => React.ReactNode;
  onCleaned: (removed: string[]) => void;
};

export function DuplicateCleanup({ root, groups, renderPreview, onCleaned }: Props) {
  const [keepers, setKeepers] = useState<Record<string, string>>({});
  const [skipped, setSkipped] = useState<Set<string>>(new Set());
  const [visible, setVisible] = useState(20);
  const [confirming, setConfirming] = useState(false);
  const [working, setWorking] = useState(false);
  const [error, setError] = useState("");
  const [last, setLast] = useState<CleanupResult | null>(null);
  const [restoreInfo, setRestoreInfo] = useState<RestoreResult | null>(null);
  const [history, setHistory] = useState<BatchSummary[]>([]);

  const groupKey = (g: DuplicateGroup) => g.paths[0];
  const keeperOf = (g: DuplicateGroup) => {
    const chosen = keepers[groupKey(g)];
    return chosen && g.paths.includes(chosen) ? chosen : suggestKeeper(g.paths);
  };
  const selected = useMemo(() => groups.filter(g => !skipped.has(groupKey(g))), [groups, skipped]);
  const plan = selected.map(g => ({ keep: keeperOf(g), remove: g.paths.filter(p => p !== keeperOf(g)) }));
  const filesToRemove = plan.reduce((n, g) => n + g.remove.length, 0);
  const bytesToFree = selected.reduce((n, g) => n + g.size_bytes * (g.paths.length - 1), 0);

  async function refreshHistory() {
    try { setHistory(await invoke<BatchSummary[]>("cleanup_history", { root })); } catch { /* history is optional */ }
  }
  useEffect(() => { void refreshHistory(); }, [root]);

  async function runCleanup() {
    setWorking(true); setError(""); setRestoreInfo(null);
    try {
      const result = await invoke<CleanupResult>("trash_duplicates", { root, groups: plan });
      setLast(result);
      onCleaned(result.moved);
    } catch (e) { setError(String(e)); }
    finally { setWorking(false); setConfirming(false); void refreshHistory(); }
  }

  async function restore(batchId: string) {
    setWorking(true); setError("");
    try {
      const result = await invoke<RestoreResult>("restore_cleanup", { root, batchId });
      setRestoreInfo(result);
      if (last?.batch_id === batchId) setLast(null);
    } catch (e) { setError(String(e)); }
    finally { setWorking(false); void refreshHistory(); }
  }

  return <>
    {last && <section className="panel cleanup-result" role="status">
      <h2><CheckCircle2 size={20}/> {fmt(last.moved.length)} copias enviadas a la Papelera · {humanSize(last.bytes_freed)} liberados</h2>
      <p>Siguen en la Papelera de tu sistema hasta que la vacíes. Puedes deshacer esta limpieza ahora o desde el historial.</p>
      {last.failed.length > 0 && <FailureList title="No se tocaron estos archivos" failures={last.failed}/>}
      <div className="actions"><button className="secondary" disabled={working} onClick={() => restore(last.batch_id)}><Undo2 size={17}/> Deshacer</button></div>
    </section>}
    {restoreInfo && <section className="panel" role="status">
      <h2>{fmt(restoreInfo.restored.length)} archivos restaurados a su ubicación original</h2>
      {restoreInfo.failed.length > 0 && <FailureList title="No se pudieron restaurar" failures={restoreInfo.failed}/>}
      <p>Vuelve a analizar para ver los duplicados actualizados.</p>
    </section>}
    {error && <div className="error"><AlertTriangle size={18}/>{error}</div>}

    {groups.length > 0 && <section className="panel cleanup-bar">
      <div>
        <h2>Limpieza: {fmt(filesToRemove)} copias de {fmt(selected.length)} grupos</h2>
        <p>Liberarás unos {humanSize(bytesToFree)}. En cada grupo se conserva la copia marcada; las demás van a la <strong>Papelera</strong> (no se borran definitivamente). Antes de moverlas se vuelve a comparar byte a byte.</p>
      </div>
      {!confirming
        ? <button disabled={working || filesToRemove === 0} onClick={() => setConfirming(true)}><Trash2 size={17}/> Enviar copias a la Papelera</button>
        : <div className="actions confirm">
            <span>¿Enviar {fmt(filesToRemove)} archivos a la Papelera?</span>
            <button disabled={working} onClick={runCleanup}>{working ? "Moviendo…" : "Sí, enviar"}</button>
            <button className="secondary" disabled={working} onClick={() => setConfirming(false)}>Cancelar</button>
          </div>}
    </section>}

    {groups.slice(0, visible).map((group, i) => {
      const keep = keeperOf(group);
      const included = !skipped.has(groupKey(group));
      return <div className={"duplicate-group" + (included ? "" : " group-skipped")} key={groupKey(group)}>
        <div className="group-header">
          <strong>Grupo {i + 1} · {fmt(group.paths.length)} archivos · {humanSize(group.size_bytes)} cada uno · contenido idéntico verificado</strong>
          <label className="group-toggle"><input type="checkbox" checked={included} onChange={() => setSkipped(prev => {
            const next = new Set(prev); const k = groupKey(group);
            if (next.has(k)) next.delete(k); else next.add(k);
            return next;
          })}/> Incluir en la limpieza</label>
        </div>
        <div className="photo-grid">{group.paths.map(path => {
          const isKeeper = path === keep;
          return <label className={"photo-grid-item dup-choice" + (isKeeper ? " keeper" : included ? " to-trash" : "")} key={path}>
            {renderPreview(path)}
            <span className="dup-choice-row">
              <input type="radio" name={"keep-" + groupKey(group)} checked={isKeeper} onChange={() => setKeepers(prev => ({ ...prev, [groupKey(group)]: path }))}/>
              {isKeeper ? "Conservar" : included ? "A la Papelera" : "Sin cambios"}
            </span>
            <span className="duplicate-path" title={path}>{path}</span>
          </label>;
        })}</div>
      </div>;
    })}
    {groups.length > visible && <button className="secondary" onClick={() => setVisible(n => n + 20)}>Mostrar 20 grupos más ({fmt(groups.length - visible)} pendientes)</button>}

    {history.length > 0 && <section className="panel">
      <h2>Historial de limpiezas</h2>
      <p>Registro guardado fuera de tu biblioteca. «Restaurar» devuelve los archivos desde la Papelera si aún no la has vaciado.</p>
      {history.map(b => <div className="history-row" key={b.batch_id}>
        <span>{new Date(b.created_at).toLocaleString("es-MX")} · {fmt(b.files)} archivos · {humanSize(b.bytes)}{b.restored > 0 ? ` · ${fmt(b.restored)} restaurados` : ""}</span>
        {b.restored < b.files && <button className="secondary" disabled={working} onClick={() => restore(b.batch_id)}><Undo2 size={16}/> Restaurar</button>}
      </div>)}
    </section>}
  </>;
}

function FailureList({ title, failures }: { title: string; failures: Failure[] }) {
  return <details open={failures.length <= 5}>
    <summary>{title} ({fmt(failures.length)})</summary>
    <ul>{failures.map(f => <li key={f.path}><span className="duplicate-path">{f.path}</span> — {f.reason}</li>)}</ul>
  </details>;
}
