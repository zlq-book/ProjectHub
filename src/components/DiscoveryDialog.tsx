import { useEffect, useRef, useState } from 'react';
import { FolderOpen, LoaderCircle, ScanSearch, ShieldCheck, Search, CheckCircle2 } from 'lucide-react';
import { categories, type Category, type ImportResult, type ProjectInput, type ScanResult, type Settings } from '../types';
import { errorMessage, native, repository } from '../services/projects';
import Modal from './Modal';
import { Tags } from './ProjectCard';

export default function DiscoveryDialog({ settings, onClose, onImported }: {
  settings: Settings; onClose: () => void; onImported: (result: ImportResult) => void;
}) {
  const [roots, setRoots] = useState('');
  const [depth, setDepth] = useState(2);
  const [category, setCategory] = useState<Category>(settings.defaultCategory);
  const [result, setResult] = useState<ScanResult | null>(null);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [query, setQuery] = useState('');
  const [busy, setBusy] = useState<'scan' | 'choose' | 'import' | ''>('');
  const [error, setError] = useState('');
  const [canceling, setCanceling] = useState(false);
  const scanId = useRef<string | null>(null);
  const cancelRequested = useRef(false);
  const alive = useRef(true);
  useEffect(() => {
    alive.current = true;
    return () => { alive.current = false; if (scanId.current) void repository.cancelScan(scanId.current).catch(() => {}); };
  }, []);
  const candidates = result?.projects || [];
  const visible = candidates.filter(p => [p.name, p.path, ...p.techStack].join(' ').toLowerCase().includes(query.trim().toLowerCase()));
  const selectable = visible.filter(p => !p.existing);
  const allVisible = selectable.length > 0 && selectable.every(p => selected.has(p.path));
  function toggle(path: string) { setSelected(previous => { const next = new Set(previous); next.has(path) ? next.delete(path) : next.add(path); return next; }); }
  async function choose() {
    setBusy('choose'); setError('');
    try { const path = await repository.choose(); if (path && alive.current) { setRoots(value => [...new Set([...value.split(/\r?\n/).map(v => v.trim()).filter(Boolean), path])].join('\n')); setResult(null); setSelected(new Set()); } }
    catch (e) { if (alive.current) setError(errorMessage(e)); }
    finally { if (alive.current) setBusy(''); }
  }
  async function scan() {
    const paths = [...new Set(roots.split(/\r?\n/).map(p => p.trim()).filter(Boolean))];
    if (!paths.length || paths.length > 20) { setError('请输入 1–20 个根目录，每行一个。'); return; }
    const id = crypto.randomUUID(); scanId.current = id; cancelRequested.current = false;
    setBusy('scan'); setCanceling(false); setError(''); setResult(null); setSelected(new Set()); setQuery('');
    try {
      const data = await repository.scan(paths, depth, id);
      if (alive.current && scanId.current === id) {
        if (cancelRequested.current) setError('扫描已取消，没有保存任何项目。');
        else setResult(data);
      }
    }
    catch (e) { if (alive.current) setError(cancelRequested.current ? '扫描已取消，没有保存任何项目。' : errorMessage(e)); }
    finally { if (scanId.current === id) scanId.current = null; if (alive.current) { setBusy(''); setCanceling(false); } }
  }
  async function cancel() {
    if (!scanId.current) return;
    cancelRequested.current = true;
    setCanceling(true);
    try { await repository.cancelScan(scanId.current); }
    catch (e) { setError(errorMessage(e)); setCanceling(false); }
  }
  async function save() {
    setBusy('import'); setError('');
    const inputs: ProjectInput[] = candidates.filter(p => selected.has(p.path) && !p.existing).map(p => ({
      name: p.name, path: p.path, description: p.description, techStack: p.techStack, category,
      tags: [], notes: '', favorite: false, defaultIde: 'system',
    }));
    try { onImported(await repository.importProjects(inputs)); }
    catch (e) { setError(errorMessage(e)); }
    finally { if (alive.current) setBusy(''); }
  }
  return <Modal wide closable={busy !== 'import' && busy !== 'choose'} title="发现本地项目" subtitle="扫描你指定的目录，预览并选择需要加入项目库的项目。" onClose={onClose}>
    <div className="discovery-body">
      {!native && <div className="notice">本地扫描需要桌面版。浏览器预览可以在项目库中体验批量收藏、分类和移除。</div>}
      <fieldset className="discovery-fields" disabled={!!busy}>
        <label>扫描根目录<textarea rows={3} value={roots} onChange={e => { setRoots(e.target.value); setResult(null); setSelected(new Set()); }} placeholder={'D:\\Projects\nE:\\Code（每行一个目录）'} /></label>
        <div className="scan-options"><button className="button" onClick={choose}><FolderOpen size={16} />选择目录</button><label>扫描深度<select value={depth} onChange={e => { setDepth(Number(e.target.value)); setResult(null); setSelected(new Set()); }}>{[1, 2, 3, 4].map(n => <option value={n} key={n}>子目录 {n} 层</option>)}</select></label><button className="button primary" onClick={scan}><ScanSearch size={17} />开始扫描</button></div>
      </fieldset>
      {busy === 'scan' && <div className="scan-progress" role="status"><LoaderCircle className="spin" size={22} /><div><strong>{canceling ? '正在取消…' : '正在发现项目…'}</strong><p>仅读取目录和配置，发现结果不会自动保存。</p></div><button className="button" disabled={canceling} onClick={cancel}>取消扫描</button></div>}
      {result && <><div className="discovery-results-heading"><span><CheckCircle2 size={16} />发现 {candidates.length} 个候选 · 已访问 {result.scannedDirectories} 个目录</span><span>{candidates.filter(p => p.existing).length} 个已录入</span></div>
        <div className="discovery-filters"><div className="search-box"><Search size={16} /><input aria-label="搜索候选项目" value={query} onChange={e => setQuery(e.target.value)} placeholder="筛选候选名称、路径或技术栈" disabled={busy === 'import'} /></div><button className="button" disabled={!selectable.length || !!busy} onClick={() => setSelected(previous => { const next = new Set(previous); for (const p of selectable) allVisible ? next.delete(p.path) : next.add(p.path); return next; })}>{allVisible ? '取消当前选择' : '全选当前结果'}</button></div>
        <div className="candidate-list">{visible.length ? visible.map(p => <label key={p.path} className={`candidate ${p.existing ? 'candidate-existing' : ''}`}><input type="checkbox" disabled={p.existing || !!busy} checked={selected.has(p.path)} onChange={() => toggle(p.path)} /><div><strong>{p.name}</strong><p>{p.path}</p><Tags values={p.techStack.slice(0, 5)} /></div>{p.existing && <span className="existing-label">已录入</span>}</label>) : <div className="candidate-empty">{candidates.length ? '没有匹配的候选项目' : '未发现项目特征文件，请调整根目录或扫描深度。普通目录可手动添加。'}</div>}</div>
        <label className="import-category">统一导入分类<select value={category} disabled={!!busy} onChange={e => setCategory(e.target.value as Category)}>{Object.entries(categories).map(([key, name]) => <option key={key} value={key}>{name}</option>)}</select><span>已选 {selected.size} 个项目，可在导入后单独编辑资料。</span></label>
        {(result.truncated || result.warnings.length > 0) && <details className="scan-warnings" open={result.truncated}><summary>{result.truncated ? '结果可能不完整' : '扫描提示'}（{result.warnings.length}）</summary><ul>{result.warnings.map((message, i) => <li key={i}>{message}</li>)}</ul></details>}
      </>}
      {error && <p className="error-message" role="alert">{error}</p>}
      <p className="discovery-help"><ShieldCheck size={15} />跳过依赖、构建目录与目录链接；导入时再次校验路径，重复记录自动跳过。</p>
    </div>
    <footer className="modal-footer"><span>预览后勾选导入 · 不改动项目文件</span><button className="button" disabled={busy === 'import' || busy === 'choose'} onClick={onClose}>关闭</button><button className="button primary" disabled={!selected.size || !!busy} onClick={save}>{busy === 'import' ? <LoaderCircle size={16} className="spin" /> : <ScanSearch size={16} />}导入 {selected.size} 个项目</button></footer>
  </Modal>;
}
