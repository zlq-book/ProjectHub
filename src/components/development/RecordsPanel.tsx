import { useEffect, useRef, useState } from 'react';
import { developmentApi } from '../../services/development';
import { errorMessage } from '../../services/projects';
import type { Project } from '../../types';
import type { CommitLink, DevelopmentLog, DevelopmentRecords, DevelopmentTask, GitCommit, TaskStatus } from '../../types/development';
import type { DevelopmentDraft } from './DevelopmentWorkspace';
import Modal from '../Modal';

const states: Record<TaskStatus, string> = { todo: '待办', inProgress: '进行中', done: '已完成' };
const today = () => new Date().toLocaleDateString('sv-SE');
type Editor = { id: string | null; title: string; body: string; date: string; status: TaskStatus; revision: number | null; commits: CommitLink[] };
function editor(record?: DevelopmentLog | DevelopmentTask): Editor {
  return { id: record?.id || null, title: record?.title || '', body: record ? 'body' in record ? record.body : record.description : '', date: record && 'date' in record ? record.date : today(), status: record && 'status' in record ? record.status : 'todo', revision: record?.revision || null, commits: record?.commits || [] };
}
export default function RecordsPanel({ project, spaceId, kind, draft, onDraftUsed, version = 0, onChanged }: { project?: Project | null; spaceId?: string; kind: 'logs' | 'tasks'; draft?: DevelopmentDraft | null; onDraftUsed?: () => void; version?: number; onChanged?: () => void }) {
  const [data, setData] = useState<DevelopmentRecords>();
  const [error, setError] = useState('');
  const [form, setForm] = useState<Editor | null>(null);
  const [busy, setBusy] = useState(false);
  const [removing, setRemoving] = useState<DevelopmentLog | DevelopmentTask>();
  const [commits, setCommits] = useState<GitCommit[]>([]);
  const [cursor, setCursor] = useState<string | null>();
  const token = useRef(0);
  useEffect(() => {
    let active = true; token.current++; setData(undefined); setError(''); setForm(null); setRemoving(undefined); setCommits([]); setCursor(undefined);
    async function load() { try { const id = spaceId || (await developmentApi.space(project!.id)).id; const value = await developmentApi.records(id); if (active) setData(value); } catch (e) { if (active) setError(errorMessage(e)); } }
    void load(); return () => { active = false; token.current++; };
  }, [project?.id, spaceId, version]);
  useEffect(() => {
    if (!data || !draft || kind !== 'logs') return;
    setForm({ ...editor(), title: draft.title.slice(0, 200), body: draft.body, commits: draft.commits.map(commit => ({ repositoryId: data.space.repositoryId || '', sha: commit.sha, subject: commit.subject, author: commit.author, commitTime: commit.commitTime })) });
    onDraftUsed?.();
  }, [data, draft, kind, onDraftUsed]);
  async function save() {
    if (!form || !data || busy) return;
    const generation = token.current; setBusy(true); setError('');
    const links = form.commits.map(({ sha, repositoryId }) => ({ sha, repositoryId }));
    try { const next = kind === 'logs' ? await developmentApi.saveLog({ id: form.id, spaceId: data.space.id, title: form.title, body: form.body, date: form.date, revision: form.revision, commits: links }) : await developmentApi.saveTask({ id: form.id, spaceId: data.space.id, title: form.title, description: form.body, status: form.status, revision: form.revision, commits: links }); if (generation === token.current) { setData(next); setForm(null); onChanged?.(); } }
    catch (e) { if (generation === token.current) setError(errorMessage(e)); } finally { if (generation === token.current) setBusy(false); }
  }
  async function loadCommits() {
    if (!project || busy) return; const generation = token.current; setBusy(true); setError('');
    try { const history = await developmentApi.history(project.id, cursor); if (generation === token.current) { setCommits(current => cursor ? [...current, ...history.commits] : history.commits); setCursor(history.nextCursor); } }
    catch (e) { if (generation === token.current) setError(errorMessage(e)); } finally { if (generation === token.current) setBusy(false); }
  }
  async function remove() {
    if (!removing || !data || busy) return; const generation = token.current; setBusy(true); setError('');
    try { const next = await developmentApi.removeRecord(data.space.id, removing.id, kind, removing.revision); if (generation === token.current) { setData(next); setRemoving(undefined); onChanged?.(); } }
    catch (e) { if (generation === token.current) setError(errorMessage(e)); } finally { if (generation === token.current) setBusy(false); }
  }
  const renderRecord = (record: DevelopmentLog | DevelopmentTask) => <article key={record.id} className="development-record"><div className="development-toolbar"><h3>{record.title}</h3><button className="text-button" disabled={busy} onClick={() => { setForm(editor(record)); setError(''); }}>编辑</button><button className="text-button" disabled={busy} onClick={() => setRemoving(record)}>删除</button></div><p className="muted">{'date' in record ? record.date : `${states[record.status]}${record.completedAt ? ` · 完成于 ${new Date(record.completedAt).toLocaleString()}` : ''}`} · 版本 {record.revision}</p><p className="record-body">{'body' in record ? record.body : record.description}</p>{record.commits.length > 0 && <div className="record-commits">{record.commits.map(c => <details key={`${c.repositoryId}-${c.sha}`}><summary>{c.sha.slice(0, 8)} · {c.subject}</summary><p>{c.author} · {new Date(c.commitTime).toLocaleString()}</p><p className="muted">仓库来源：{c.repositoryId}。这是保存的关联快照；历史改写后对象可能已不可读。</p>{project && <LinkedCommit projectId={project.id} link={c} currentRepositoryId={data?.space.repositoryId} />}</details>)}</div>}</article>;
  return <section className="development-section"><div className="development-toolbar"><h2>{kind === 'logs' ? '开发日志' : '任务状态'}</h2><button className="button primary" disabled={!data || busy} onClick={() => { setForm(editor()); setError(''); }}>新建{kind === 'logs' ? '日志' : '任务'}</button></div>{error && <p role="alert" className="error-message">{error}</p>}{!data && !error && <p role="status">读取开发记录…</p>}
    {data && !form && (kind === 'logs' ? data.logs.length ? [...data.logs].sort((a,b) => b.date.localeCompare(a.date)).map(renderRecord) : <p className="notice">还没有开发日志。可以手动记录，也可以从提交或 AI 摘要创建草稿。</p> : <div className="task-groups">{(Object.entries(states) as [TaskStatus, string][]).map(([value, label]) => <div key={value}><h3>{label} <span className="count-pill">{data.tasks.filter(t => t.status === value).length}</span></h3>{data.tasks.filter(t => t.status === value).map(renderRecord)}</div>)}</div>)}
    {form && <form className="record-editor" onSubmit={e => { e.preventDefault(); void save(); }}><label>标题<input required maxLength={200} value={form.title} disabled={busy} onChange={e => setForm({ ...form, title: e.target.value })} /></label>{kind === 'logs' ? <label>日志日期<input type="date" required value={form.date} disabled={busy} onChange={e => setForm({ ...form, date: e.target.value })} /></label> : <label>任务状态<select value={form.status} disabled={busy} onChange={e => setForm({ ...form, status: e.target.value as TaskStatus })}>{Object.entries(states).map(([value, label]) => <option key={value} value={value}>{label}</option>)}</select></label>}<label>{kind === 'logs' ? '日志正文' : '任务描述'}<textarea rows={10} maxLength={kind === 'logs' ? 16000 : 8000} value={form.body} disabled={busy} onChange={e => setForm({ ...form, body: e.target.value })} /></label><p className="muted">关联提交不会改变任务状态；开发记录不会修改项目资料更新时间。</p><div className="record-links">{form.commits.map(c => <label key={`${c.repositoryId}-${c.sha}`}><input type="checkbox" checked disabled={busy} onChange={() => setForm({ ...form, commits: form.commits.filter(item => item !== c) })} />{c.sha.slice(0, 8)} · {c.subject}</label>)}</div>{project && <details><summary>选择关联提交</summary><button type="button" className="button" disabled={busy || cursor === null} onClick={() => void loadCommits()}>{commits.length ? '更多提交' : '读取当前分支提交'}</button>{commits.map(c => <label className="commit-choice" key={c.sha}><input type="checkbox" disabled={busy || !data?.space.repositoryId} checked={form.commits.some(link => link.sha === c.sha && link.repositoryId === data?.space.repositoryId)} onChange={e => setForm({ ...form, commits: e.target.checked ? [...form.commits, { repositoryId: data!.space.repositoryId!, sha: c.sha, subject: c.subject, author: c.author, commitTime: c.commitTime }] : form.commits.filter(link => link.sha !== c.sha || link.repositoryId !== data?.space.repositoryId) })} />{c.sha.slice(0, 8)} · {c.subject}</label>)}</details>}<div className="development-toolbar"><button type="button" className="button" disabled={busy} onClick={() => setForm(null)}>取消编辑</button><button type="submit" className="button primary" disabled={busy}>{busy ? '保存中…' : '保存记录'}</button></div></form>}
    {removing && <Modal title={`删除${kind === 'logs' ? '日志' : '任务'}？`} closable={!busy} onClose={() => setRemoving(undefined)}><div className="confirm-body"><p>{removing.title}</p><p>同时移除 {removing.commits.length} 条关联记录；本地 Git 提交保留。</p>{error && <p role="alert">{error}</p>}</div><footer className="modal-footer"><button className="button" disabled={busy} onClick={() => setRemoving(undefined)}>取消</button><button className="button danger" disabled={busy} onClick={() => void remove()}>确认删除</button></footer></Modal>}
  </section>;
}
function LinkedCommit({ projectId, link, currentRepositoryId }: { projectId: string; link: CommitLink; currentRepositoryId?: string | null }) {
  const [message, setMessage] = useState('');
  return <><button className="button" onClick={async () => { if (link.repositoryId !== currentRepositoryId) { setMessage('当前项目仓库来源已改变，保留原关联快照。'); return; } try { const files = await developmentApi.files(projectId, link.sha); setMessage(`${files.files.length} 个变更文件${files.complete ? '' : '（不完整）'}：${files.files.map(f => f.path).join('、') || '空提交'}`); } catch { setMessage('该提交当前不可读取，关联快照已保留。'); } }}>检查提交对象</button>{message && <p role="status">{message}</p>}</>;
}
