import { useEffect, useState } from 'react';
import { developmentApi } from '../../services/development';
import { errorMessage, native } from '../../services/projects';
import type { Project } from '../../types';
import type { DevelopmentSpace } from '../../types/development';
import RecordsPanel from './RecordsPanel';
export default function ArchivedSpaces({ projects, version }: { projects: Project[]; version: number }) {
  const [spaces, setSpaces] = useState<DevelopmentSpace[]>([]);
  const [selected, setSelected] = useState('');
  const [target, setTarget] = useState('');
  const [kind, setKind] = useState<'logs' | 'tasks'>('logs');
  const [error, setError] = useState('');
  useEffect(() => { let active = true; if (native) developmentApi.spaces().then(data => { if (active) setSpaces(data.filter(s => !s.projectId)); }, e => { if (active) setError(errorMessage(e)); }); return () => { active = false; }; }, [version, projects]);
  return <details className="archived-spaces"><summary>已移除项目的开发记录（{spaces.length}）</summary>{error && <p role="alert">{error}</p>}{spaces.map(space => <button key={space.id} className="button" onClick={() => { setSelected(space.id); setTarget(''); }}>{space.projectName}</button>)}{selected && <><div className="development-toolbar"><select aria-label="归档记录类型" value={kind} onChange={e => setKind(e.target.value as 'logs' | 'tasks')}><option value="logs">开发日志</option><option value="tasks">任务</option></select><select aria-label="重新关联项目" value={target} onChange={e => setTarget(e.target.value)}><option value="">手动选择新项目</option>{projects.map(p => <option value={p.id} key={p.id}>{p.name}</option>)}</select><button className="button" disabled={!target} onClick={async () => { try { await developmentApi.rebind(selected, target); setSpaces(current => current.filter(s => s.id !== selected)); setSelected(''); } catch (e) { setError(errorMessage(e)); } }}>确认重新关联</button></div><p className="muted">仅手动重新关联；旧提交仍保留原仓库来源。</p><RecordsPanel key={`${selected}-${kind}`} spaceId={selected} kind={kind} version={version} /></>}</details>;
}
