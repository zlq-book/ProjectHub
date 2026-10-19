import { useMemo, useRef, useState } from 'react';
import { LoaderCircle, Pencil, Search, ShieldCheck, Tags, Trash2 } from 'lucide-react';
import type { Project, TagSource } from '../types';
import { errorMessage } from '../services/projects';
import { trimTagName } from '../services/tags';
import Modal from './Modal';

type Props = {
  projects: Project[];
  onClose: () => void;
  onFilter: (value: string, source: TagSource) => void;
  onChangeTag: (ids: string[], source: string, target: string | null) => Promise<void>;
};

type Action = { source: string; kind: 'rename' | 'remove'; projects: Project[] };

export default function TagManagerDialog({ projects, onClose, onFilter, onChangeTag }: Props) {
  const [tab, setTab] = useState<TagSource>('tags');
  const [query, setQuery] = useState('');
  const [action, setAction] = useState<Action | null>(null);
  const [target, setTarget] = useState('');
  const [error, setError] = useState('');
  const [busy, setBusy] = useState(false);
  const saving = useRef(false);
  const entries = useMemo(() => {
    const counts = new Map<string, number>();
    for (const p of projects) for (const value of new Set(p[tab])) counts.set(value, (counts.get(value) || 0) + 1);
    return [...counts].sort(([a, ac], [b, bc]) => bc - ac || a.localeCompare(b, 'zh-CN'));
  }, [projects, tab]);
  const visible = entries.filter(([value]) => value.toLocaleLowerCase().includes(query.trim().toLocaleLowerCase()));
  function begin(source: string, kind: Action['kind']) {
    const affected = projects.filter(p => p.tags.includes(source));
    if (affected.length > 500) { setError('该标签关联超过 500 个项目，请通过项目资料分批整理。'); return; }
    setError(''); setTarget(source); setAction({ source, kind, projects: affected });
  }
  async function save() {
    if (!action || saving.current) return;
    const name = trimTagName(target);
    if (action.kind === 'rename' && (!name || [...name].length > 80 || name === action.source)) {
      setError('请输入与原名称不同的标签，长度为 1–80 个字符。'); return;
    }
    saving.current = true; setBusy(true); setError('');
    try {
      await onChangeTag(action.projects.map(p => p.id), action.source, action.kind === 'remove' ? null : name);
      setAction(null);
    } catch (e) { setError(errorMessage(e)); }
    finally { saving.current = false; setBusy(false); }
  }
  const label = tab === 'tags' ? '标签' : '技术栈';
  return <Modal wide closable={!busy} title={action ? (action.kind === 'rename' ? '重命名标签' : '移除标签？') : '标签管理'}
    subtitle={action ? `“${action.source}” · 将更新全部 ${action.projects.length} 个关联项目` : '整理自定义标签，或按技术栈查找项目。'} onClose={onClose}>
    <div className="tag-management-body">
      {action ? <>
        <ul className="batch-project-list">{action.projects.map(p => <li key={p.id}><strong>{p.name}</strong><span>{p.path}</span></li>)}</ul>
        {action.kind === 'rename' ? <label className="tag-name-label">新标签名称<input autoFocus aria-label="新标签名称" value={target} disabled={busy} onChange={e => setTarget(e.target.value)} />
          <span className="muted">区分大小写，最多 80 个字符；同一个项目的重复标签将合并。</span></label> : null}
        {action.kind === 'rename' && trimTagName(target) !== action.source && projects.some(p => p.tags.includes(trimTagName(target))) && <p className="tag-merge-notice">将与已有标签合并，已有标签的其他项目保持原资料。</p>}
        <div className="notice"><ShieldCheck size={18} /><span>{action.kind === 'remove' ? '仅移除自定义标签，关联项目、技术栈和源文件均保留。' : '仅更新自定义标签，技术栈和源文件保持原样。'}</span></div>
      </> : <>
        <div className="tag-manager-tabs" role="group" aria-label="标签来源">
          <button className={`button ${tab === 'tags' ? 'primary' : ''}`} aria-pressed={tab === 'tags'} onClick={() => { setTab('tags'); setQuery(''); setError(''); }}>自定义标签</button>
          <button className={`button ${tab === 'techStack' ? 'primary' : ''}`} aria-pressed={tab === 'techStack'} onClick={() => { setTab('techStack'); setQuery(''); setError(''); }}>技术栈</button>
        </div>
        <label className="search-box tag-search"><Search size={18} /><input aria-label="搜索标签" placeholder={`搜索${label}…`} value={query} onChange={e => setQuery(e.target.value)} /></label>
        <p className="muted">共 {entries.length} 个{label} · 点击名称筛选项目{tab === 'techStack' ? '，技术栈请在项目资料中编辑。' : '，重命名或移除会更新全部关联项目。'}</p>
        <div className="tag-management-list">{visible.map(([value, count]) => <div className="tag-management-row" key={value}>
          <button className="tag-filter-button" aria-label={`筛选${label} ${value}：${count} 个项目`} onClick={() => onFilter(value, tab)}><Tags size={16} /><span>{value}</span><span>{count}</span></button>
          {tab === 'tags' && <div className="tag-row-actions"><button className="icon-button" aria-label={`重命名标签 ${value}`} title="重命名" onClick={() => begin(value, 'rename')}><Pencil size={17} /></button><button className="icon-button" aria-label={`移除标签 ${value}`} title="移除标签" onClick={() => begin(value, 'remove')}><Trash2 size={17} /></button></div>}
        </div>)}</div>
        {!visible.length && <p className="tag-empty muted">{entries.length ? `没有匹配的${label}，请调整搜索。` : `暂无${label}，可在项目资料中添加。`}</p>}
      </>}
      {error && <p className="error-message" role="alert">{error}</p>}
    </div>
    {action && <footer className="modal-footer"><button className="button" disabled={busy} onClick={() => { setAction(null); setError(''); }}>返回标签列表</button>
      <button className={`button ${action.kind === 'remove' ? 'danger' : 'primary'}`} disabled={busy || !action.projects.length} onClick={() => { void save(); }}>{busy && <LoaderCircle size={16} className="spin" />}{action.kind === 'remove' ? '确认移除标签' : '确认重命名'}</button></footer>}
  </Modal>;
}
