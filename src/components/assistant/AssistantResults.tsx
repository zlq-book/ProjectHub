import { useEffect, useRef, useState } from 'react';
import { categories, type Project } from '../../types';
import type { AssistantApply, AssistantRequest, AssistantResult } from '../../types/assistant';
import { errorMessage } from '../../services/projects';

export default function AssistantResults({ result, request, blocked, projects, onApply, onRead, onViewProject, onViewDevelopment, onShowResults, onSaving }: {
  result: AssistantResult; request: AssistantRequest; blocked: boolean; projects: Project[];
  onApply: (input: AssistantApply) => Promise<Project[]>;
  onRead: (request: AssistantRequest, ids: string[]) => void;
  onViewProject: (project: Project) => void; onViewDevelopment?: (project: Project, sha?:string) => void; onShowResults: (ids: string[]) => void; onSaving: (value: boolean) => void;
}) {
  const [values, setValues] = useState<Record<string, string>>(() => Object.fromEntries(result.items.map(item => [item.project.id, result.kind === 'description' ? item.description ?? '' : item.category ?? item.project.category])));
  const [checked, setChecked] = useState<Set<string>>(() => new Set(result.items.filter(item => result.kind === 'readContext' ? result.items.length === 1 && item.project.available : item.certainty === 'confirmed' && item.category && item.category !== item.project.category).map(item => item.project.id)));
  const [saved, setSaved] = useState<number | null>(null);
  const [dismissed, setDismissed] = useState(false);
  const [expired, setExpired] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  const applyRef = useRef(false);
  useEffect(() => { const timer = setTimeout(() => setExpired(true), 10 * 60 * 1000); return () => clearTimeout(timer); }, []);
  const locked = blocked || busy || saved !== null || dismissed || expired || !!error;
  const changes = result.items.filter(item => result.kind === 'description' || checked.has(item.project.id) && values[item.project.id] !== item.project.category).map(item => ({ id: item.project.id, value: values[item.project.id] }));
  const validDescription = result.kind !== 'description' || changes.every(change => change.value.trim() && Array.from(change.value).length <= 4000);
  function toggle(id: string) { setChecked(before => { const next = new Set(before); if (next.has(id)) next.delete(id); else if (result.kind !== 'readContext' || next.size < 3) next.add(id); return next; }); }
  async function apply() {
    if (locked || applyRef.current || !result.proposalId || !changes.length || !validDescription) return;
    applyRef.current = true; setBusy(true); onSaving(true);
    try { const updated = await onApply({ sessionId: request.sessionId, proposalId: result.proposalId, changes }); setSaved(updated.length); }
    catch (e) { setError(errorMessage(e)); }
    finally { setBusy(false); applyRef.current = false; onSaving(false); }
  }
  return <div className="assistant-results">
    {result.items.map(item => {
      const p = projects.find(project => project.id === item.project.id) ?? item.project;
      return <article className="assistant-result-card" key={p.id}>
        <div className="assistant-result-heading"><strong>{p.name}</strong><span>{item.certainty === 'confirmed' ? '依据已核对' : '待确认'}</span></div>
        <p className="assistant-result-category" aria-label="项目分类">{categories[p.category]}</p>
        <p className="assistant-project-description">{p.description || '暂无简介'}</p>
        <div className="tags">{p.techStack.map(stack => <span className="tag" key={stack}>{stack}</span>)}</div>
        <p className="assistant-local-path" title={p.path}>{p.path}</p>
        {item.evidence.length > 0 && <details className="assistant-evidence"><summary>匹配依据（{item.evidence.length}）</summary>{item.evidence.map((e, index) => <p key={index}><span>{e.source}</span>：{e.quote}{result.kind==='git' && e.source==='git.commit' && /^[a-f0-9]{40,64} · /.test(e.quote) && onViewDevelopment && <button className="assistant-text-button" onClick={()=>onViewDevelopment(p,e.quote.split(' · ')[0])}>查看该提交</button>}</p>)}</details>}
        {result.kind === 'description' && <label className="assistant-editor-label">建议简介<textarea aria-label={`建议简介：${p.name}`} rows={5} maxLength={4000} disabled={locked} value={values[p.id]} onChange={e => setValues(previous => ({ ...previous, [p.id]: e.target.value }))} /></label>}
        {result.kind === 'categories' && <div className="assistant-category-edit"><label><input type="checkbox" aria-label={`修改分类：${p.name}`} disabled={locked || !item.category || !projects.some(project => project.id === p.id)} checked={checked.has(p.id)} onChange={() => toggle(p.id)} />{categories[item.project.category]} →</label><select aria-label={`目标分类：${p.name}`} disabled={locked || !item.category} value={values[p.id]} onChange={e => setValues(previous => ({ ...previous, [p.id]: e.target.value }))}>{Object.entries(categories).map(([key, label]) => <option value={key} key={key}>{label}</option>)}</select>{!item.category && item.certainty === 'uncertain' ? <small>待补充资料</small> : values[p.id] === item.project.category && <small>无需修改</small>}</div>}
        {result.kind === 'readContext' && <label className="assistant-read-check"><input type="checkbox" aria-label={`读取资料：${p.name}`} checked={checked.has(p.id)} disabled={blocked || busy || !p.available || !projects.some(project => project.id === p.id)} onChange={() => toggle(p.id)} />{p.available ? '允许读取此项目资料' : '目录暂时不可用'}</label>}
        {result.kind === 'git' && onViewDevelopment && <button className="button" disabled={busy} onClick={() => onViewDevelopment(p)}>查看开发进度</button>}
        <button className="assistant-text-button" disabled={busy || !projects.some(project => project.id === p.id)} aria-label={`查看详情：${p.name}`} onClick={() => onViewProject(p)}>查看详情</button>
      </article>;
    })}
    {(result.kind === 'search' || result.kind === 'categories') && <div className="assistant-result-actions">{result.kind === 'search' && result.items.length > 0 && <button className="button" onClick={() => onShowResults(result.items.map(item => item.project.id))}>在项目列表中查看</button>}{result.items.length === 0 && <p className="muted">当前资料中没有找到可确认的结果，可选择项目补充资料后继续。</p>}<button className="assistant-text-button" disabled={blocked} onClick={() => onRead(request, [])}>选择项目补充 README／依赖</button></div>}
    {result.kind === 'readContext' && <div className="assistant-read-consent"><p>将向当前模型发送勾选项目的根目录 README、依赖清单及一级目录名；不读取源码正文、.env 或独立密钥文件。文档内已有的敏感文本也会发送，请先核对。每轮最多 3 个项目。</p>{result.items.length === 0 && <button className="assistant-text-button" disabled={blocked} onClick={() => onRead(request, [])}>从本轮范围选择项目</button>}<button className="button primary" disabled={blocked || !checked.size} onClick={() => onRead(request, [...checked])}>读取资料并继续（{checked.size} 个项目）</button></div>}
    {(result.kind === 'description' || result.kind === 'categories') && result.proposalId && <div className="assistant-proposal-actions"><small>仅保存本次建议字段 · 建议 10 分钟有效</small><button className="button primary" disabled={locked || !changes.length || !validDescription} onClick={() => void apply()}>{busy ? '正在保存…' : result.kind === 'description' ? '保存简介' : `确认修改 ${changes.length} 个项目`}</button><button className="assistant-text-button" disabled={locked} onClick={() => setDismissed(true)}>放弃建议</button></div>}
    {saved !== null && <p className="assistant-success" role="status">已保存 {saved} 个项目</p>}
    {dismissed && <p className="muted">已放弃，项目资料保持原值。</p>}
    {expired && saved === null && !dismissed && result.proposalId && <p className="muted">建议已过期，请重新生成。</p>}
    {error && <p className="error-message" role="alert">{error}。请重新生成并核对建议。</p>}
  </div>;
}
