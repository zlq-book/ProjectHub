import { useState } from 'react';
import type { Outcome, ReportSource } from '../../types/weeklyReport';
interface Props {
    outcomes: Outcome[];
    selectedIds: string[];
    onChange: (ids: string[]) => void;
    onEdit: (items: Outcome[]) => void;
    sources: ReportSource[];
    onSource?: (source: ReportSource) => void;
}
export default function OutcomeEditor({ outcomes, selectedIds, onChange, onEdit, sources, onSource }: Props) {
    const [error, setError] = useState('');
    const aiOutcomes = outcomes.filter(o => o.origin === 'ai');
    const covered = new Set(aiOutcomes.flatMap(o => o.sourceIds)).size;
    function toggle(id: string) { if (selectedIds.includes(id)) {
        onChange(selectedIds.filter(v => v !== id));
        setError('');
    }
    else if (selectedIds.length >= 4)
        setError('主要完成工作最多选择 4 条，请先取消一条');
    else {
        onChange([...selectedIds, id]);
        setError('');
    } }
    function move(id: string, delta: number) { const i = selectedIds.indexOf(id), j = i + delta; if (i < 0 || j < 0 || j >= selectedIds.length)
        return; const next = [...selectedIds]; [next[i], next[j]] = [next[j], next[i]]; onChange(next); }
    return <section className="weekly-outcomes"><div className="weekly-section-title"><h2>候选工作成果</h2><span>已选 {selectedIds.length}/4 · 点击勾选决定导出顺序</span><button className="button" disabled={outcomes.length >= 100} onClick={() => onEdit([...outcomes, { id: crypto.randomUUID(), projectIds: [], text: '', sourceIds: [], origin: 'manual' }])}>新增成果</button></div>{aiOutcomes.length > 0 && <p className="muted weekly-outcome-summary">AI 已按功能主题归纳为 {aiOutcomes.length} 条候选，覆盖 {covered} 条材料。</p>}{error && <p role="alert" className="error-message">{error}</p>}{!outcomes.length && <p className="notice">尚无候选成果。可以生成候选，也可以手动新增。</p>}{outcomes.map((o, i) => <div className={`weekly-outcome ${selectedIds.includes(o.id) ? 'chosen' : ''}`} key={o.id}><div className="weekly-outcome-heading"><label><input type="checkbox" checked={selectedIds.includes(o.id)} onChange={() => toggle(o.id)}/>{selectedIds.includes(o.id) ? `导出第 ${selectedIds.indexOf(o.id) + 1} 条` : '未入选'} · {o.origin === 'manual' ? '人工填写' : `AI 草稿${o.sourceIds.length ? ` · 覆盖 ${o.sourceIds.length} 条材料` : ''}`}</label><span className="weekly-inline-actions"><button className="button" disabled={!selectedIds.includes(o.id) || selectedIds.indexOf(o.id) === 0} onClick={() => move(o.id, -1)}>上移</button><button className="button" disabled={!selectedIds.includes(o.id) || selectedIds.indexOf(o.id) === selectedIds.length - 1} onClick={() => move(o.id, 1)}>下移</button><button className="button" onClick={() => { onChange(selectedIds.filter(id => id !== o.id)); onEdit(outcomes.filter(v => v.id !== o.id)); }}>移除</button></span></div><textarea aria-label={`成果 ${i + 1}`} rows={2} maxLength={4000} value={o.text} onChange={e => onEdit(outcomes.map(v => v.id === o.id ? { ...v, text: e.target.value } : v))}/>{o.sourceIds.length > 0 && <details><summary>查看依据（{o.sourceIds.length} 条材料）</summary>{o.sourceIds.map(id => { const s = sources.find(s => s.id === id); return s ? <div className="weekly-source-trace" key={id}><strong>{s.projectName} · {s.title}</strong><p>{s.kind === 'commit' ? `${s.author} <${s.authorEmail}> · ${s.commitTime} · ${s.sha}` : '开发日志 · ' + (s.logDate??'') + ' · ' + s.logId}</p><pre>{s.body}</pre>{s.renames?.map(([old,path])=><p key={old+path}>重命名：{old} → {path}</p>)}{s.logCommits?.map(link=><p key={link.repositoryId+link.sha}>关联提交：{link.sha} · {link.subject}</p>)}{s.files.length > 0 && <p>{s.files.join('、')}</p>}{onSource && <button className="button" onClick={() => onSource(s)}>查看原始开发记录</button>}</div> : <p key={id}>来源不可用：{id}</p>; })}</details>}</div>)}</section>;
}
