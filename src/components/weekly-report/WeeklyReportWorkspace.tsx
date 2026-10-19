import { useEffect, useRef, useState } from 'react';
import type { Project } from '../../types';
import type { ReportContext, Collection, ReportContent, ReportGroup, ReportHints, ReportPreferences, ReportPreview, ReportSource, ReportVersion } from '../../types/weeklyReport';
import { matchesReportSearch, defaultRange, makeRange, validateContent, weeklyReportApi as api } from '../../services/weeklyReport';
import { native, errorMessage } from '../../services/projects';
import Modal from '../Modal';
import OutcomeEditor from './OutcomeEditor';
import ReportForm from './ReportForm';
const emptyHints = (): ReportHints => ({ difficulties: '', plans: '', reflection: '' });
const blank = (prefs: ReportPreferences): ReportContent => ({ department: prefs.department, reporter: prefs.reporter, range: defaultRange(), outcomes: [], selectedOutcomeIds: [], difficulties: [], plans: [], reflection: '' });
export default function WeeklyReportWorkspace({ projects, onSource,active=true }: {
    projects: Project[];
    active?:boolean;
    onSource?: (s: ReportSource) => void;
}) {
    const [prefs, setPrefs] = useState<ReportPreferences>({ department: '', reporter: '', authors: [] });
    const [identities, setIdentities] = useState('');
    const [authorCandidates, setAuthorCandidates] = useState<ReportPreferences['authors']>([]);
    const [view, setView] = useState<'prepare' | 'edit' | 'history'>('prepare');
    const [content, setContent] = useState<ReportContent>(() => blank(prefs));
    const [hints, setHints] = useState(emptyHints);
    const [projectIds, setProjectIds] = useState<string[]>([]);
    const [collection, setCollection] = useState<Collection | null>(null);
    const [sourceIds, setSourceIds] = useState<string[]>([]);
    const [diffs, setDiffs] = useState<{
        sourceId: string;
        path: string;
    }[]>([]);
    const [group, setGroup] = useState<ReportGroup | null>(null);
    const [current, setCurrent] = useState<ReportVersion | null>(null);
    const [sources, setSources] = useState<ReportSource[]>([]);
    const [context,setContext]=useState<ReportContext>({authors:[],projects:[],diffs:[],model:'',baseUrl:'',warnings:[]});
    const [history, setHistory] = useState<ReportGroup[]>([]);
    const [preview, setPreview] = useState<ReportPreview | null>(null);
    const [busy, setBusy] = useState('');
    const [error, setError] = useState('');
    const [notice, setNotice] = useState('');
    const [dirty, setDirty] = useState(false);
    const [projectSearch,setProjectSearch]=useState('');const [materialSearch,setMaterialSearch]=useState('');const [historySearch,setHistorySearch]=useState('');
    const visibleProjects=projects.filter(p=>matchesReportSearch(projectSearch,[p.name,p.path]));
    const visibleSources=(collection?.sources??[]).filter(s=>matchesReportSearch(materialSearch,[s.projectName,s.title,s.sha,s.body]));
    const visibleHistory=history.filter(g=>g.versions.some(v=>matchesReportSearch(historySearch,[v.content.reporter,v.content.department,v.content.range.startDate,v.content.range.endDate])));
    const epoch = useRef(0), request = useRef(''), token = useRef('');
    useEffect(() => { let live = true; if (native) {
        setBusy('载入');
        Promise.all([api.preferences(), api.list()]).then(([p, h]) => { if (live) {
            setPrefs(p);
            setIdentities(p.authors.map(a => `${a.kind === 'email' ? '邮箱' : '姓名'}:${a.value}`).join('\n'));
            setContent(blank(p));
            setHistory(h);
        } }).catch(e => { if (live)
            setError(errorMessage(e)); }).finally(() => { if (live)
            setBusy(''); });
    } return () => { live = false; epoch.current++; if (request.current)
        void api.cancelCollection(request.current).catch(() => { }); if (token.current)
        void api.cancel(token.current).catch(() => { }); }; }, []);
    function edit(c: ReportContent) { setContent(c); setDirty(true); setNotice(''); }
    function invalid() { setCollection(null); setSourceIds([]); setDiffs([]); setPreview(null); if (token.current) {
        void api.cancel(token.current).catch(() => { });
        token.current = '';
    } }
    function changeDate(key: 'start' | 'end', value: string) { if (!value)
        return; try {
        edit({ ...content, range: makeRange(key === 'start' ? value : content.range.startDate, key === 'end' ? value : content.range.endDate) });
        invalid();
    }
    catch {
        setError('日期无效');
    } }
    function choose(v: ReportVersion, g: ReportGroup) { setGroup(g); setCurrent(v); setContent(v.content); setSources(v.sources);
        setContext(v.context??{authors:[],projects:[],diffs:[],model:'',baseUrl:'',warnings:[]}); setHints(v.hints); setDirty(false); setView('edit'); invalid(); setError(''); }
    async function perform<T>(name: string, work: () => Promise<T>): Promise<T | undefined> { const e = ++epoch.current; setBusy(name); setError(''); setNotice(''); try {
        const result = await work();
        if (e !== epoch.current)
            return;
        return result;
    }
    catch (e2) {
        if (e === epoch.current)
            setError(errorMessage(e2));
    }
    finally {
        if (e === epoch.current)
            setBusy('');
    } }
    function authorPrefs() { const authors = identities.split('\n').map(s => s.trim()).filter(Boolean).map(s => { const colon = s.indexOf(':'); return colon >= 0 ? { kind: (s.slice(0, colon) === '姓名' ? 'name' : 'email') as 'name' | 'email', value: s.slice(colon + 1).trim() } : { kind: (s.includes('@') ? 'email' : 'name') as 'name' | 'email', value: s }; }); return { ...prefs, department: content.department, reporter: content.reporter, authors }; }
    async function collect() { await perform('汇总材料', async () => { invalid(); const id = crypto.randomUUID(); request.current = id; const p = authorPrefs(); await api.savePreferences(p); if (request.current !== id)
        throw new Error('汇总已取消'); setPrefs(p); const result = await api.collect({ projectIds, range: content.range, authors: p.authors, requestId: id }); if (request.current !== id)
        throw new Error('汇总已取消'); setCollection(result); setSourceIds(result.sources.map(s => s.id)); return result; }); }
    async function saveContent(c = content, append = false) { const validation = validateContent(c); if (validation)
        throw new Error(validation); const result = await api.save({ reportId: group?.id ?? null, versionId: append ? null : current?.id ?? null, parentVersionId: append ? current?.id ?? null : null, revision: append ? null : current?.revision ?? null, content: c, sources, hints, context }); const v = append || !current ? result.versions[result.versions.length - 1] : result.versions.find(v => v.id === current.id)!; setGroup(result); setCurrent(v); setDirty(false); return v; }
    async function beginRegenerate() { await perform('保存原稿', async () => { await saveContent(); setView('prepare'); invalid(); setNotice('原稿已保存。重新汇总并生成后会创建新版本，不覆盖原稿。'); }); }
    async function preparePreview() { await perform('准备发送预览', async () => { if (!collection)
        throw new Error('请先汇总材料'); const activeEpoch = epoch.current; const result = await api.preview({ collectionId: collection.id, sourceIds, diffs, hints }); if (activeEpoch !== epoch.current) {
        void api.cancel(result.token).catch(() => { });
        throw new Error('预览已取消');
    } token.current = result.token; setPreview(result); }); }
    async function generate() { if (!preview || !collection)
        return; const active = preview.token; await perform('生成周报', async () => { try { const activeEpoch=epoch.current;const result = await api.generate(active); if (token.current !== active)
        throw new Error('生成已取消'); setBusy('保存生成结果'); const next: ReportContent = { ...content, ...result, selectedOutcomeIds: [] }; const selectedSources = collection.sources.filter(s => sourceIds.includes(s.id)); const nextContext:ReportContext={authors:collection.authors,projects:collection.projects,diffs,model:preview!.model,baseUrl:preview!.baseUrl,warnings:preview!.warnings};
            const saved = await api.save({ reportId: group?.id ?? null, versionId: null, parentVersionId: current?.id ?? null, revision: null, content: next, sources: selectedSources, hints, context:nextContext }); if(activeEpoch!==epoch.current||token.current!==active)return;const v = saved.versions[saved.versions.length - 1]; setGroup(saved); setCurrent(v); setContent(v.content); setSources(v.sources);
        setContext(v.context??{authors:[],projects:[],diffs:[],model:'',baseUrl:'',warnings:[]}); setDirty(false); setPreview(null); token.current = ''; setView('edit'); setNotice('已生成并保存新版本，请编辑并选择最多 4 条成果。');
        } catch (error) {
            if (token.current === active) { setPreview(null); token.current = ''; }
            throw error;
        } }); }
    function cancel() { epoch.current++; setBusy(''); if (request.current) {
        void api.cancelCollection(request.current).catch(() => { });
        request.current = '';
    } if (token.current) {
        void api.cancel(token.current).catch(() => { });
        token.current = '';
    } setPreview(null); setNotice('本次操作已取消，原稿保留。'); }
    async function loadHistory() { await perform('载入历史', async () => { if (dirty)
        await saveContent(); const rows = await api.list(); setHistory(rows); setView('history'); }); }
    async function startNew() { await perform('保存原稿', async () => { if (dirty)
        await saveContent(); setGroup(null); setCurrent(null); setSources([]);
            setContext({authors:[],projects:[],diffs:[],model:'',baseUrl:'',warnings:[]}); setContent(blank(prefs)); setHints(emptyHints()); setDirty(false); invalid(); setView('prepare'); }); }
    async function exportReport(format: 'xlsx' | 'md') { await perform('导出周报', async () => { const validation = validateContent(content, true); if (validation)
        throw new Error(validation); const v = await saveContent(); const result = await api.export(v, format); setNotice(result.status === 'saved' ? `已导出：${result.path}` : '已取消导出'); }); }
    useEffect(()=>{if(!active||!native)return;const handle=(e:KeyboardEvent)=>{if((e.ctrlKey||e.metaKey)&&e.key.toLowerCase()==='s'&&view!=='history'&&!document.querySelector('dialog[open]')){e.preventDefault();if(!busy)void perform('保存草稿',async()=>{await saveContent();setNotice('草稿已保存');});}};window.addEventListener('keydown',handle);return()=>window.removeEventListener('keydown',handle);});
    async function switchVersion(id:string){if(!group||!current||id===current.id)return;await perform('切换版本',async()=>{if(dirty)await saveContent();const latest=await api.get(group.id);const selected=latest.versions.find(v=>v.id===id);if(!selected)throw new Error('版本已变化，请重新打开历史');choose(selected,latest);});}
    return <section className="weekly-workspace" aria-busy={!!busy}><div className="weekly-heading"><div><h1>AI 智能周报</h1><p className="muted">从实际开发记录整理成果，编辑后导出固定表格。</p></div><div className="weekly-inline-actions"><button className="button" disabled={!!busy} onClick={() => void startNew()}>新建周报</button><button className="button" disabled={!!busy || !native} onClick={() => void loadHistory()}>历史周报</button></div></div>
 {!native && <p className="notice">当前为浏览器预览。实际 Git 汇总、AI 生成、版本保存与导出请使用桌面版；这里不生成示例工作记录。</p>}
 <div className="weekly-tabs" role="tablist" aria-label="周报工作区"><button role="tab" aria-selected={view === 'prepare'} disabled={!!busy} onClick={() => { if (current)
        void beginRegenerate();
    else
        setView('prepare'); }}>准备数据</button><button role="tab" aria-selected={view === 'edit'} disabled={!!busy} onClick={() => setView('edit')}>编辑周报{dirty ? ' · 未保存' : ''}</button></div>
 {error && <p role="alert" className="error-message">{error}</p>}{notice && <p role="status" className="notice">{notice}</p>}{busy && <p role="status" className="notice">{busy}… {['汇总材料', '准备发送预览', '生成周报'].includes(busy) && <button className="button" onClick={cancel}>取消本次操作</button>}</p>}
 {view === 'prepare' && <fieldset className="weekly-preparation" disabled={!!busy}><div className="weekly-fields"><label>开始日期<input type="date" value={content.range.startDate} onChange={e => changeDate('start', e.target.value)}/></label><label>结束日期<input type="date" value={content.range.endDate} onChange={e => changeDate('end', e.target.value)}/></label><button className="button" onClick={() => { edit({ ...content, range: defaultRange() }); invalid(); }}>本周一至周五</button></div><p className="muted">按提交者时间统计，使用 {content.range.timeZone} 时区；各项目仅当前分支。日志按记录日期筛选。</p>
 <details className="weekly-preferences" open={!prefs.department||!prefs.reporter||!prefs.authors.length}><summary>周报信息与 Git 身份<span>{prefs.reporter?` ${prefs.department} · ${prefs.reporter} · ${prefs.authors.length} 条作者规则`:'首次使用请确认'}</span></summary><div className="weekly-fields"><label>所属部门<input maxLength={200} value={content.department} onChange={e => edit({ ...content, department: e.target.value })}/></label><label>填报人<input maxLength={100} value={content.reporter} onChange={e => edit({ ...content, reporter: e.target.value })}/></label></div>
 <label className="weekly-field">本人 Git 身份（每行一个姓名或邮箱）<textarea rows={3} value={identities} onChange={e => { setIdentities(e.target.value); invalid(); }} placeholder={'邮箱:yourname@example.com\n姓名:你的 Git 姓名'}/></label><p className="muted">确认后跨项目记住。邮箱精确匹配且忽略大小写；姓名精确匹配，可能与他人同名。Git 作者筛选不适用于无作者字段的本地开发日志。</p></details>
 <section><div className="weekly-section-title"><h2>选择项目</h2><span>{projectIds.length}/20</span><button className="button" onClick={() => { if (new Set([...projectIds,...visibleProjects.map(p=>p.id)]).size > 20) {
            setError('项目超过 20 个，请手动选择');
            return;
        } setProjectIds([...new Set([...projectIds,...visibleProjects.map(p => p.id)])]); invalid(); }}>全选</button><button className="button" onClick={() => { setProjectIds([]); invalid(); }}>清空</button></div><label className="weekly-field">筛选周报项目<input type="search" value={projectSearch} onChange={e=>setProjectSearch(e.target.value)} placeholder="按项目名称或路径查找"/></label><p className="muted">显示 {visibleProjects.length} 个项目 · 已选 {projectIds.length} 个{projectIds.some(id=>!visibleProjects.some(p=>p.id===id))?`（${projectIds.filter(id=>!visibleProjects.some(p=>p.id===id)).length} 个在筛选范围外）`:""}</p><div className="weekly-projects">{visibleProjects.map(p => <label key={p.id}><input type="checkbox" checked={projectIds.includes(p.id)} onChange={() => { if (!projectIds.includes(p.id) && projectIds.length >= 20) {
            setError('每次最多选择 20 个项目');
            return;
        } setProjectIds(projectIds.includes(p.id) ? projectIds.filter(id => id !== p.id) : [...projectIds, p.id]); invalid(); }}/>{p.name}</label>)}</div>{projects.length>0&&!visibleProjects.length&&<p className="notice">没有匹配项目。<button className="button" onClick={()=>setProjectSearch('')}>清除项目检索</button></p>}{!projects.length && <p className="notice">请先在项目库添加本地项目。</p>}<button className="button" disabled={!native || !projectIds.length} onClick={() => void perform('读取作者候选', async () => { setAuthorCandidates(await api.authors(projectIds)); })}>读取 Git 作者候选</button>{authorCandidates.length > 0 && <details><summary>确认本人身份（不会自动勾选）</summary>{authorCandidates.map(a => <label className="weekly-diff-choice" key={a.kind + ':' + a.value}><input type="checkbox" checked={identities.split('\n').includes(`${a.kind === 'email' ? '邮箱' : '姓名'}:${a.value}`)} onChange={e => { const value = `${a.kind === 'email' ? '邮箱' : '姓名'}:${a.value}`; setIdentities(e.target.checked ? [identities, value].filter(Boolean).join('\n') : identities.split('\n').filter(v => v !== value).join('\n')); invalid(); }}/>{a.kind === 'email' ? '邮箱' : '姓名'}：{a.value}</label>)}</details>}<button className={collection?'button':'button primary'} disabled={!native || !projectIds.length} onClick={() => void collect()}>汇总提交与开发日志</button><p className="muted">生成时按功能主题归纳为 4–8 条候选成果，不会逐条罗列提交。</p></section>
 {collection && <section><h2>本次材料</h2>{collection.projects.map(p => <p key={p.projectId}>{p.projectName} · {p.branch ?? '无分支'} · {p.head?.slice(0, 8) ?? '无 HEAD'} {p.warning && <span className="error-message">{p.warning}</span>}</p>)}<p className="muted">最多 20 项目；每仓库扫描 20,000 条、纳入 500 条提交；200 条日志；请求最多 64 KiB。可取消不相关材料，Diff 默认不读取。</p><div className="weekly-material-toolbar"><label>筛选材料<input type="search" value={materialSearch} onChange={e=>setMaterialSearch(e.target.value)} placeholder="项目、提交、日志关键词"/></label><span>已选 {sourceIds.length}/{collection.sources.length}</span><button className="button" onClick={()=>setSourceIds([...new Set([...sourceIds,...visibleSources.map(s=>s.id)])])}>全选当前材料</button><button className="button" onClick={()=>{const ids=new Set(visibleSources.map(s=>s.id));setSourceIds(sourceIds.filter(id=>!ids.has(id)));setDiffs(diffs.filter(d=>!ids.has(d.sourceId)));}}>取消当前材料</button></div><div className="weekly-materials">{visibleSources.map(s => <details key={s.id}><summary><input aria-label={`纳入材料 ${s.title}`} type="checkbox" checked={sourceIds.includes(s.id)} onClick={e => e.stopPropagation()} onChange={() => { setSourceIds(sourceIds.includes(s.id) ? sourceIds.filter(id => id !== s.id) : [...sourceIds, s.id]); setDiffs(diffs.filter(d => d.sourceId !== s.id)); }}/>{s.projectName} · {s.title} · {s.kind === 'log' ? '本地日志' : s.sha?.slice(0, 8)}</summary><p className="muted">{s.author} {s.authorEmail} {s.kind==='log'?s.logDate:s.commitTime}</p><pre>{s.body}</pre>{s.files.map(path => <label className="weekly-diff-choice" key={path}><input type="checkbox" disabled={!sourceIds.includes(s.id)} checked={diffs.some(d => d.sourceId === s.id && d.path === path)} onChange={() => { const has = diffs.some(d => d.sourceId === s.id && d.path === path); if (!has && diffs.length >= 100) {
            setError('Diff 最多选择 100 个文件');
            return;
        } setDiffs(has ? diffs.filter(d => !(d.sourceId === s.id && d.path === path)) : [...diffs, { sourceId: s.id, path }]); }}/>附带 Diff：{path}</label>)}</details>)}</div>{collection.sources.length>0&&!visibleSources.length&&<p className="notice">没有匹配材料，已选内容仍保留。<button className="button" onClick={()=>setMaterialSearch('')}>清除材料检索</button></p>}{!collection.sources.length && <p className="notice">没有符合条件的材料，可调整范围或手动编辑周报。</p>}</section>}
 <section><h2>补充本周要点</h2>{([['difficulties', '困难与需要协调的问题'], ['plans', '下周计划'], ['reflection', '感悟关键词']] as const).map(([key, label]) => <label className="weekly-field" key={key}>{label}<textarea rows={3} maxLength={8000} value={hints[key]} onChange={e => {setHints({ ...hints, [key]: e.target.value });setDirty(true);}} placeholder="可选；未填写则保持空白"/></label>)}</section><button className="button primary" disabled={!native || !collection || !sourceIds.length} onClick={() => void preparePreview()}>预览发送内容</button></fieldset>}
 {view === 'edit' && <fieldset className="weekly-editor" disabled={!!busy}><div className="weekly-section-title weekly-editor-toolbar"><p>{content.range.startDate} — {content.range.endDate}{current ? ` · 版本 ${group?.versions.findIndex(v => v.id === current.id)! + 1} · ${group?.adoptedVersionId === current.id ? '已采用' : '尚未采用'}` : ' · 新草稿'}</p><span className="save-state">{dirty?'有未保存修改':current?`已保存 ${new Date(current.updatedAt).toLocaleTimeString()}`:'尚未保存'}</span>{group&&current&&<label className="weekly-version-picker">周报版本<select aria-label="周报版本" value={current.id} onChange={e=>void switchVersion(e.target.value)}>{group.versions.map((v,i)=><option key={v.id} value={v.id}>版本 {i+1}{group.adoptedVersionId===v.id?' · 已采用':''}</option>)}</select></label>}<button className="button" title="Ctrl+S 保存草稿" disabled={!native} onClick={() => void perform('保存草稿', async () => { await saveContent(); setNotice('草稿已保存'); })}>保存草稿</button><button className="button" disabled={!native} onClick={() => void beginRegenerate()}>重新生成新版本</button>{current && group?.adoptedVersionId !== current.id && <button className="button" onClick={() => void perform('采用版本', async () => { const v = await saveContent(); const g = await api.adopt(v.reportId, v.id); setGroup(g); setNotice('已采用此版本'); })}>采用此版本</button>}</div><details><summary>生成范围与来源快照</summary><p className="muted">{context.model ? `模型：${context.model} · ${context.baseUrl}` : '人工草稿'} · {context.authors.map(a=>a.value).join('、')}</p>{context.projects.map(p=><p key={p.projectId}>{p.projectName} · {p.branch??'无分支'} · {p.head??'无 HEAD'}</p>)}{context.diffs.map(d=><p key={d.sourceId+d.path}>手选 Diff：{d.path}</p>)}{context.warnings.map((w,i)=><p key={i}>{w}</p>)}<p className="muted">这里只保存来源快照与 Diff 路径。原仓库不可用时仍能查看快照；重新读取代码需要原仓库和提交可用。</p></details><OutcomeEditor outcomes={content.outcomes} selectedIds={content.selectedOutcomeIds} onChange={ids => edit({ ...content, selectedOutcomeIds: ids })} onEdit={items => edit({ ...content, outcomes: items, selectedOutcomeIds: content.selectedOutcomeIds.filter(id => items.some(o => o.id === id)) })} sources={sources} onSource={onSource ? source => { void perform('保存草稿', async () => { if (dirty)
        await saveContent(); onSource(source); }); } : undefined}/><ReportForm content={content} onChange={edit}/><div className="weekly-footer"><button className="button primary" disabled={!native} onClick={() => void exportReport('xlsx')}>导出 Excel</button><button className="button" disabled={!native} onClick={() => void exportReport('md')}>导出 Markdown</button><span className="muted">保存当前版本后导出；不再调用 AI。</span></div></fieldset>}
 {view === 'history' && <section className="weekly-history"><h2>历史周报</h2><label className="weekly-field">检索历史周报<input type="search" value={historySearch} onChange={e=>setHistorySearch(e.target.value)} placeholder="日期、部门或填报人"/></label>{history.length>0&&!visibleHistory.length&&<p className="notice">没有符合条件的周报。<button className="button" onClick={()=>setHistorySearch('')}>清除历史检索</button></p>}{!history.length && <p className="notice">还没有已保存周报。</p>}{visibleHistory.map(g => <details key={g.id}><summary>{g.versions[0]?.content.range.startDate} — {g.versions[0]?.content.range.endDate} · {g.versions[0]?.content.reporter || '未填报人'} · {g.versions.length} 个版本</summary>{g.versions.map((v, i) => <div className="weekly-history-row" key={v.id}><span>版本 {i + 1} {g.adoptedVersionId === v.id ? '· 已采用' : ''} · {new Date(v.updatedAt).toLocaleString()}</span><button className="button" onClick={() => choose(v, g)}>打开编辑</button></div>)}</details>)}</section>}
 {preview && <Modal closable={busy!=='保存生成结果'} title="确认发送周报材料" onClose={() => { if (busy)
        cancel();
    else {
        void api.cancel(preview.token).catch(() => { });
        token.current = '';
        setPreview(null);
    } }}><div className="modal-body"><p>接口：{preview.baseUrl} · 模型：{preview.model}</p><p>{preview.bytes} 字节 · 有效至 {new Date(preview.expiresAt).toLocaleTimeString()}</p>{preview.warnings.map((w, i) => <p className="notice" key={i}>{w}</p>)}<p className="notice">仅向此接口发送下列内容，可能产生模型费用。请核对材料和代码 Diff。</p><pre className="weekly-send-preview">{preview.text}</pre>{error && <p role="alert" className="error-message">{error}</p>}</div><footer className="modal-footer"><button className="button" disabled={busy==='保存生成结果'} onClick={cancel}>取消</button><button className="button primary" disabled={!!busy} onClick={() => void generate()}>{busy || '确认发送并生成'}</button></footer></Modal>}
 </section>;
}
