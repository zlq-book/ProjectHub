import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { ArrowDownWideNarrow, ChevronDown, Clock3, Folder, FolderKanban, Grid2X2, Home, List, LoaderCircle, Plus, Search, Settings as SettingsIcon, ShieldCheck, Star, Tags as TagsIcon, X, PanelRightOpen, CheckCircle2, AlertCircle, Command, Sparkles } from 'lucide-react';
import { categories, defaultSettings, type BatchPatch, type Category, type Project, type Settings, type TagSource } from './types';
import { errorMessage, filterProjects, native, repository } from './services/projects';
import ProjectCard from './components/ProjectCard';
import HomeStart from './components/HomeStart';
import ProjectTable from './components/ProjectTable';
import AiSettingsDialog from './components/AiSettingsDialog';
import ProjectDetail from './components/ProjectDetail';
import ProjectDialog from './components/ProjectDialog';
import SettingsDialog from './components/SettingsDialog';
import Modal from './components/Modal';
import BatchToolbar from './components/BatchToolbar';
import DiscoveryDialog from './components/DiscoveryDialog';
import TagManagerDialog from './components/TagManagerDialog';
import AssistantPanel from './components/assistant/AssistantPanel';
import { assistantApi } from './services/assistant';
import type { AssistantApply } from './types/assistant';
import DevelopmentWorkspace, { type DevelopmentDraft } from './components/development/DevelopmentWorkspace';
import RecordsPanel from './components/development/RecordsPanel';
import ArchivedSpaces from './components/development/ArchivedSpaces';
import SummaryDialog from './components/development/SummaryDialog';
import WeeklyReportWorkspace from './components/weekly-report/WeeklyReportWorkspace';
import type { SummarySelection } from './components/development/GitPanel';

export default function App() {
  const [projects, setProjects] = useState<Project[]>([]);
  const [settings, setSettings] = useState<Settings>(defaultSettings);
  const [query, setQuery] = useState('');
  const [scope, setScope] = useState('home');
  const projectViews = useRef<Record<string,{query:string;category:string;status:string;tag:string;tagSource?:TagSource;sort:string;assistantFilter:Set<string>|null}>>({});
  const [weeklyStarted,setWeeklyStarted] = useState(false);
  const [weeklyEpoch,setWeeklyEpoch]=useState(0);
  const [developmentProjectId, setDevelopmentProjectId] = useState<string | null>(null);
  const [developmentCommitSha,setDevelopmentCommitSha]=useState<string|null>(null);
  const [developmentDraft, setDevelopmentDraft] = useState<DevelopmentDraft | null>(null);
  const [summary, setSummary] = useState<{ project: Project; selection: SummarySelection } | null>(null);
  const [diffOpen, setDiffOpen] = useState(false);
  const changeDiffOpen = useCallback((open: boolean) => setDiffOpen(open), []);
  const [category, setCategory] = useState('');
  const [status, setStatus] = useState('');
  const [sort, setSort] = useState('updated');
  const [tag, setTag] = useState('');
  const [tagSource, setTagSource] = useState<TagSource | undefined>();
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [detailOpen, setDetailOpen] = useState(() => window.innerWidth > 1060);
  const [panelMode, setPanelMode] = useState<'detail' | 'assistant'>('detail');
  const [detailFromAssistant, setDetailFromAssistant] = useState(false);
  const [assistantVersion, setAssistantVersion] = useState(0);
  const [assistantFilter, setAssistantFilter] = useState<Set<string> | null>(null);
  const [dialog, updateDialog] = useState<'add' | 'edit' | 'settings' | 'api' | 'tags' | 'remove' | 'discover' | 'batch-category' | 'batch-remove' | null>(null);
  const [assistantApplying, setAssistantApplying] = useState(false);
  const [batchMode, setBatchMode] = useState(false);
  const [batchIds, setBatchIds] = useState<Set<string>>(new Set());
  const [batchBusy, setBatchBusy] = useState(false);
  const [batchCategory, setBatchCategory] = useState<Category>('other');
  const [batchError, setBatchError] = useState('');
  const [importReport, setImportReport] = useState<string[] | null>(null);
  const [loading, setLoading] = useState(true);
  const [loadError, setLoadError] = useState('');
  const [pending, setPending] = useState(false);
  const [toast, setToast] = useState<{ text: string; error: boolean } | null>(null);
  const searchRef = useRef<HTMLInputElement>(null);
  const [view, setView] = useState<'grid' | 'list'>('list');
  const toastTimer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  const operationRef = useRef(false);
  function setDialog(next: typeof dialog) {
    // 助手提交期间不能打开持有旧快照的完整资料表单。
    if (next && operationRef.current) return;
    updateDialog(next);
  }
  const notify = useCallback((text: string, error = false) => {
    clearTimeout(toastTimer.current); setToast({ text, error });
    toastTimer.current = setTimeout(() => setToast(null), error ? 7000 : 3500);
  }, []);
  const refresh = useCallback(async () => {
    try { const data = await repository.list(); setProjects(data); return data; }
    catch (e) { setLoadError(errorMessage(e)); throw e; }
  }, []);
  const reloadWorkspace = useCallback(async () => {
    setLoading(true);
    try {
      const [data, preferences] = await Promise.all([repository.list(), repository.settings()]);
      setSettings(preferences); setView(preferences.view); setSelectedId(data[0]?.id || null); setDevelopmentDraft(null); setDevelopmentCommitSha(null); setSummary(null); setDiffOpen(false);
      setProjects(data); setLoadError('');
      setBatchIds(new Set()); setBatchMode(false);
    } catch (e) { setLoadError(errorMessage(e)); throw e; }
    finally { setLoading(false); }
  }, []);
  useEffect(() => {
    void reloadWorkspace().catch(() => {});
    return () => clearTimeout(toastTimer.current);
  }, [reloadWorkspace]);
  useEffect(() => {
    const media = window.matchMedia('(prefers-color-scheme: dark)');
    const apply = () => { document.documentElement.dataset.theme = settings.theme === 'system' ? (media.matches ? 'dark' : 'light') : settings.theme; };
    apply(); media.addEventListener('change', apply); return () => media.removeEventListener('change', apply);
  }, [settings.theme]);
  useEffect(() => {
    function keydown(e: KeyboardEvent) { if (!dialog && (e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'k') { e.preventDefault(); if(scope==='weekly-report'||scope==='development'){navigate('all');setTimeout(()=>searchRef.current?.focus(),0);}else searchRef.current?.focus(); } }
    window.addEventListener('keydown', keydown); return () => window.removeEventListener('keydown', keydown);
  }, [dialog,scope,query,category,status,tag,tagSource,sort,assistantFilter]);
  const selected = projects.find(p => p.id === selectedId);
  const filtered = useMemo(() => filterProjects(projects, { query, scope, category, status, sort, tag, tagSource }).filter(project => !assistantFilter || assistantFilter.has(project.id)), [projects, query, scope, category, status, sort, tag, tagSource, assistantFilter]);
  const visibleSelected = selected && filtered.some(p => p.id === selected.id) ? selected : undefined;
  const detailProject = visibleSelected || (detailFromAssistant ? selected : undefined);
  const assistantVisible = scope !== 'weekly-report' && !diffOpen && !batchMode && detailOpen && panelMode === 'assistant';
  const panelVisible = scope !== 'weekly-report' && !diffOpen && !batchMode && detailOpen && (panelMode === 'assistant' || !!detailProject);
  const panelTabs = <div className="project-panel-tabs" role="tablist" aria-label="右侧面板"><button role="tab" aria-selected={panelMode === 'detail'} disabled={!detailProject} onClick={() => { setPanelMode('detail'); setDetailOpen(true); }}>项目详情</button><button role="tab" aria-selected={panelMode === 'assistant'} onClick={() => { setPanelMode('assistant'); setDetailOpen(true); }}>AI 助手</button></div>;
  const batchSelected = projects.filter(p => batchIds.has(p.id));
  const allVisibleSelected = filtered.length > 0 && filtered.every(p => batchIds.has(p.id));
  const hiddenSelectionCount = batchSelected.filter(p => !filtered.some(item => item.id === p.id)).length;
  const favoriteCount = projects.filter(p => p.favorite).length;
  const recentCount = projects.filter(p => p.lastOpenedAt).length;
  function clearFilters() { setQuery(''); setCategory(''); setStatus(''); setTag(''); setTagSource(undefined); setAssistantFilter(null); }
  function toggleBatchMode() { setBatchMode(mode => !mode); setBatchIds(new Set()); setDetailOpen(false); }
  function checkProject(id: string) {
    if (batchBusy) return;
    if (!batchIds.has(id) && batchIds.size >= 500) { notify('每次最多选择 500 个项目，请分批整理。', true); return; }
    setBatchIds(previous => { const next = new Set(previous); next.has(id) ? next.delete(id) : next.add(id); return next; });
  }
  function selectVisible() {
    const next = new Set(batchIds); for (const p of filtered) allVisibleSelected ? next.delete(p.id) : next.add(p.id);
    if (next.size > 500) { notify('当前结果超过 500 个项目，请缩小筛选范围后分批整理。', true); return; }
    setBatchIds(next);
  }
  async function applyBatch(patch: BatchPatch) {
    if (operationRef.current || !batchSelected.length) return;
    operationRef.current = true; setBatchBusy(true); setBatchError('');
    try {
      const changed = await repository.batchUpdate(batchSelected.map(p => p.id), patch);
      const updates = new Map(changed.map(p => [p.id, p]));
      setProjects(previous => previous.map(p => updates.get(p.id) || p));
      updateDialog(current => current === 'batch-category' ? null : current); notify(`已更新 ${changed.length} 个项目`);
    } catch (e) { setBatchError(errorMessage(e)); notify(errorMessage(e), true); }
    finally { operationRef.current = false; setBatchBusy(false); }
  }
  async function removeBatch() {
    if (operationRef.current || !batchSelected.length) return;
    operationRef.current = true; setBatchBusy(true); setBatchError('');
    const ids = batchSelected.map(p => p.id);
    try {
      await repository.batchRemove(ids); const removed = new Set(ids);
      setProjects(previous => previous.filter(p => !removed.has(p.id))); setBatchIds(new Set());
      if (selectedId && removed.has(selectedId)) setSelectedId(null);
      setDialog(null); notify(`已移除 ${ids.length} 个项目索引，本地文件保留`);
    } catch (e) { setBatchError(errorMessage(e)); }
    finally { operationRef.current = false; setBatchBusy(false); }
  }
  function rememberProjectView(){if(!['weekly-report','development'].includes(scope)) projectViews.current[scope]={query,category,status,tag,tagSource,sort,assistantFilter};}
  function navigate(next: string, nextCategory = '') {
    rememberProjectView();
    if(next==='weekly-report'){setWeeklyStarted(true);setBatchMode(false);setBatchIds(new Set());}
    setDevelopmentCommitSha(null);if(next==='development'){setBatchMode(false);setBatchIds(new Set());}
    if(next==='development'&&window.innerWidth<=1060)setDetailOpen(false);
    setDiffOpen(false);setDevelopmentProjectId(null);setDevelopmentDraft(null);setScope(next);
    const saved=projectViews.current[next];setCategory(nextCategory||saved?.category||'');setTag(saved?.tag||'');setTagSource(saved?.tagSource);setQuery(saved?.query||'');setStatus(saved?.status||'');setAssistantFilter(saved?.assistantFilter||null);if(saved)setSort(saved.sort);
  }

  async function applyAssistant(input: AssistantApply) {
    if (operationRef.current || loading || loadError || dialog) throw new Error('项目库正在处理其他操作，请关闭编辑窗口后重新生成建议。');
    operationRef.current = true; setAssistantApplying(true);
    try {
      const changed = await assistantApi.apply(input);
      const updates = new Map(changed.map(project => [project.id, project]));
      setProjects(previous => previous.map(project => updates.get(project.id) || project));
      return changed;
    } finally { operationRef.current = false; setAssistantApplying(false); }
  }
  async function changeTag(ids: string[], source: string, target: string | null) {
    if (operationRef.current) throw new Error('其他操作正在进行，请稍后重试。');
    operationRef.current = true;
    try {
      const changed = await repository.changeTag(ids, source, target);
      const updates = new Map(changed.map(p => [p.id, p]));
      setProjects(previous => previous.map(p => updates.get(p.id) || p));
      if (tag === source && tagSource !== 'techStack') setTag(target ?? '');
      notify(`已${target === null ? '移除' : '更新'} ${changed.length} 个项目的标签`);
    } finally { operationRef.current = false; }
  }
  async function favorite(p: Project) {
    if (operationRef.current) return;
    operationRef.current = true;
    try { const updated = await repository.update(p.id, { ...p, favorite: !p.favorite }); setProjects(list => list.map(item => item.id === p.id ? updated : item)); }
    catch (e) { notify(errorMessage(e), true); } finally { operationRef.current = false; }
  }
  async function open(p: Project, action: string) {
    if (operationRef.current) return;
    operationRef.current = true; setPending(true);
    try { await repository.open(p.id, action); await refresh(); notify('已启动打开工具'); }
    catch (e) { notify(errorMessage(e), true); } finally { operationRef.current = false; setPending(false); }
  }
  async function copy() { if (selected) try { await navigator.clipboard.writeText(selected.path); notify('项目路径已复制'); } catch { notify('复制失败，请手动复制详情中的路径。', true); } }
  async function saveSettings(next: Settings) {
    if (loading || loadError) throw new Error('项目库和设置尚未完整加载，请先重新读取。');
    await repository.saveSettings(next); setSettings(next); setView(next.view); notify('设置已保存');
  }
  async function changeView(next: 'grid' | 'list') { try { await saveSettings({ ...settings, view: next }); } catch (e) { notify(errorMessage(e), true); } }
  const projectArea=!['weekly-report','development'].includes(scope);
  const pageTitle = category ? categories[category as keyof typeof categories] : scope === 'recent' ? '最近打开' : scope === 'favorites' ? '收藏项目' : scope === 'home' ? '工作台' : '全部项目';
  return <div className={`app-shell ${panelVisible ? 'with-detail' : ''}`}>
    <a href="#main-content" className="skip-link">跳至主要内容</a>
    <aside className="sidebar">
      <a className="brand" href="#" onClick={e => { e.preventDefault(); navigate('home'); }}><img className="brand-icon" src="/projecthub-icon.png" alt="" width={30} height={30} /><span>ProjectHub</span><span className="version-pill">v1.5</span></a>
      <nav aria-label="主导航"><div className="nav-group">
        <button aria-label="首页" title="首页" aria-current={scope==='home'?'page':undefined} className={`nav-item ${scope === 'home' && !category ? 'active' : ''}`} onClick={() => navigate('home')}><Home size={20} /><span>首页</span></button>
        <button aria-label="全部项目" title="全部项目" aria-current={scope==='all'?'page':undefined} className={`nav-item ${scope === 'all' && !category ? 'active' : ''}`} onClick={() => navigate('all')}><FolderKanban size={20} /><span>全部项目</span><span className="nav-count">{projects.length}</span></button>
        <button aria-label="最近打开" title="最近打开" aria-current={scope==='recent'?'page':undefined} className={`nav-item ${scope === 'recent' ? 'active' : ''}`} onClick={() => navigate('recent')}><Clock3 size={20} /><span>最近打开</span><span className="nav-count">{recentCount}</span></button>
        <button aria-label="收藏项目" title="收藏项目" aria-current={scope==='favorites'?'page':undefined} className={`nav-item ${scope === 'favorites' ? 'active' : ''}`} onClick={() => navigate('favorites')}><Star size={20} /><span>收藏项目</span><span className="nav-count">{favoriteCount}</span></button>
        <button aria-label="开发状态中心" title="开发状态中心" className={`nav-item ${scope === 'development' ? 'active' : ''}`} onClick={() => navigate('development')}><FolderKanban size={20} /><span>开发状态中心</span></button>
        <button aria-label="AI 智能周报" title="AI 智能周报" aria-current={scope==='weekly-report'?'page':undefined} className={`nav-item ${scope === 'weekly-report' ? 'active' : ''}`} onClick={() => navigate('weekly-report')}><Sparkles size={20}/><span>AI 智能周报</span></button>
      </div><div className="nav-divider" /><button className="nav-item" aria-label="设置" onClick={() => setDialog('settings')}><SettingsIcon size={20}/><span>设置</span></button></nav>

      <div className="sidebar-api"><button className="nav-item" aria-label="配置 API" onClick={() => setDialog('api')}><SettingsIcon size={18} /><span>配置 API</span></button></div>
    </aside>
    <header className="topbar">{projectArea ? <div className="search-box"><Search size={19} /><input ref={searchRef} value={query} onChange={e => setQuery(e.target.value)} placeholder="搜索项目名称、简介、标签…" aria-label="搜索项目" />{query && <button className="icon-button" aria-label="清空搜索" onClick={() => setQuery('')}><X size={16} /></button>}<kbd>Ctrl + K</kbd></div> : <div className="workspace-context"><strong>{scope==='weekly-report'?'周报工作区':'开发工作区'}</strong><span>Ctrl+K 搜索项目</span></div>}<div className="topbar-actions">{scope !== 'weekly-report' && <button className={`button assistant-entry ${assistantVisible ? 'active' : ''}`} aria-label="打开 AI 助手" aria-expanded={assistantVisible} disabled={batchMode} onClick={() => { setPanelMode('assistant'); setDetailOpen(true); }}><Sparkles size={17} /><span>AI 助手</span></button>}{projectArea && <button className="button primary" disabled={assistantApplying} onClick={() => setDialog('add')}><Plus size={19} />添加项目</button>}<button className="icon-button settings-button" disabled={assistantApplying} aria-label="打开设置" onClick={() => setDialog('settings')}><SettingsIcon size={21} /></button></div></header>
    <main className="workspace" id="main-content" tabIndex={-1}>
      {scope==='home'&&!loading&&!loadError&&<HomeStart navigate={navigate}/>}
      {weeklyStarted && <div hidden={scope !== 'weekly-report'}><WeeklyReportWorkspace key={weeklyEpoch} active={scope==='weekly-report'} projects={projects} onSource={source => { const project=projects.find(p=>source.projectIds.includes(p.id)); if(project){setDevelopmentProjectId(project.id);setDevelopmentCommitSha(source.sha);rememberProjectView();setScope('development');setDetailOpen(false);} }} /></div>}
      {scope === 'weekly-report' ? null : scope === 'development' ? <DevelopmentWorkspace key={`${assistantVersion}-${developmentProjectId || 'overview'}-${developmentCommitSha || ''}`} projects={projects} version={assistantVersion} initialProjectId={developmentProjectId} initialCommitSha={developmentCommitSha} initialDraft={developmentDraft} onDraftConsumed={() => setDevelopmentDraft(null)} onProjectSelect={project=>{setDevelopmentCommitSha(null);setSelectedId(project.id);setDevelopmentProjectId(project.id);if(window.innerWidth<=1060)setDetailOpen(false);}} onOverview={()=>{setDevelopmentCommitSha(null);setDevelopmentProjectId(null);}} onDiffOpen={changeDiffOpen} records={(project, kind, draft, clear) => <RecordsPanel key={`${project?.id}-${kind}`} project={project} kind={kind} draft={draft} onDraftUsed={clear} version={assistantVersion} />} archive={<ArchivedSpaces projects={projects} version={assistantVersion} />} onSummary={(project, selection) => setSummary({ project, selection })} /> : <>
      <div className="page-heading"><h1>{pageTitle}<span className="count-pill">{filtered.length}</span></h1><div className="filter-toolbar"><div className="filter-selects"><label className="select-wrap"><select aria-label="筛选状态" value={status} onChange={e => setStatus(e.target.value)}><option value="">全部状态</option><option value="available">路径可用</option><option value="unavailable">路径不可用</option></select><ChevronDown size={14} /></label><label className="select-wrap sort-select"><ArrowDownWideNarrow size={15} /><select aria-label="项目排序" value={sort} onChange={e => setSort(e.target.value)}><option value="updated">资料更新</option><option value="opened">最近打开</option><option value="name">名称排序</option></select><ChevronDown size={14} /></label></div><div className="view-actions"><button className={`button batch-toggle ${batchMode ? 'active' : ''}`} disabled={loading || !!loadError || batchBusy} onClick={toggleBatchMode}>{batchMode ? '完成管理' : '批量管理'}</button><div className="view-switch" role="group" aria-label="展示方式"><button className={view === 'grid' ? 'active' : ''} aria-label="卡片视图" aria-pressed={view === 'grid'} onClick={() => changeView('grid')}><Grid2X2 size={18} /></button><button className={view === 'list' ? 'active' : ''} aria-label="列表视图" aria-pressed={view === 'list'} onClick={() => changeView('list')}><List size={19} /></button></div>{!batchMode && visibleSelected && <button className="icon-button detail-toggle" aria-label={detailOpen ? '收起详情' : '展开详情'} onClick={() => setDetailOpen(!detailOpen)}><PanelRightOpen size={18} /></button>}</div></div></div>
      <div className="project-management-actions"><button className="button" aria-label="发现项目" disabled={batchBusy || loading || !!loadError} onClick={()=>setDialog('discover')}><FolderKanban size={16}/>发现项目</button><button className="button" aria-label="标签管理" disabled={batchBusy || loading || !!loadError} onClick={()=>setDialog('tags')}><TagsIcon size={16}/>标签管理</button></div><div className="category-tabs" role="group" aria-label="项目分类筛选"><button className={!category ? 'active' : ''} aria-pressed={!category} onClick={() => setCategory('')}>全部</button>{Object.entries(categories).map(([key, name]) => <button key={key} className={category === key ? 'active' : ''} aria-pressed={category === key} onClick={() => setCategory(key)}>{name.replace('项目', '').trim()}</button>)}</div>
      {(query.trim() || category || status || tag || assistantFilter) && <div className="filter-summary" aria-label="当前筛选条件">
        {assistantFilter && <span className="filter-chip">AI 检索结果：{assistantFilter.size} 个<button className="icon-button" aria-label="清除 AI 结果筛选" onClick={() => setAssistantFilter(null)}><X size={13} /></button></span>}
        {query.trim() && <span className="filter-chip">搜索：{query.trim()}<button className="icon-button" aria-label="清除关键词" onClick={() => setQuery('')}><X size={13} /></button></span>}
        {category && <span className="filter-chip">{categories[category as keyof typeof categories]}<button className="icon-button" aria-label="清除分类筛选" onClick={() => setCategory('')}><X size={13} /></button></span>}
        {status && <span className="filter-chip">{status === 'available' ? '路径可用' : '路径不可用'}<button className="icon-button" aria-label="清除状态筛选" onClick={() => setStatus('')}><X size={13} /></button></span>}
        {tag && <span className="filter-chip">{tagSource === 'techStack' ? '技术栈' : '标签'}：{tag}<button className="icon-button" aria-label="清除标签筛选" onClick={() => { setTag(''); setTagSource(undefined); }}><X size={13} /></button></span>}
        <button className="clear-filters" onClick={clearFilters}>清除全部</button>
      </div>}
      {!native && <div className="preview-notice"><span className="status-dot" />浏览器示例预览<span>本地路径识别与程序启动请使用桌面版</span></div>}
      {loading ? <div className="empty-state"><LoaderCircle className="spin" size={32} /><h2>正在读取项目库…</h2></div> : loadError ? <div className="empty-state"><AlertCircle size={32} /><h2>项目库读取失败</h2><p>{loadError}</p><button className="button" onClick={() => { void reloadWorkspace().catch(() => {}); }}>重新读取</button></div> : filtered.length ? (view === 'list' ? <ProjectTable projects={filtered} selectedId={selectedId} batchMode={batchMode} checkedIds={batchIds} busy={batchBusy} onSelect={p => { setSelectedId(p.id); setDetailOpen(true); }} onOpen={p => open(p, 'system')} onCheck={checkProject} onFavorite={favorite} /> : <div className="project-grid">{filtered.map(p => <ProjectCard key={p.id} project={p} busy={batchBusy} batchMode={batchMode} checked={batchIds.has(p.id)} onCheck={() => checkProject(p.id)} selected={!batchMode && selectedId === p.id} onSelect={showDetail => { setSelectedId(p.id); if (showDetail) setDetailOpen(true); }} onFavorite={() => favorite(p)} onEdit={() => { setSelectedId(p.id); setDialog('edit'); }} onOpen={() => open(p, 'system')} />)}</div>) : <div className="empty-state"><div className="empty-icon"><Folder size={38} /></div><h2>{projects.length ? '没有找到匹配的项目' : '你的项目工作台，从这里开始'}</h2><p>{projects.length ? '试试其他关键词，或调整分类与筛选条件。' : '粘贴一个本地文件夹路径，自动识别并整理你的第一个项目。'}</p><button className="button primary" onClick={() => projects.length ? (delete projectViews.current[scope], clearFilters(), setScope('all')) : setDialog('add')}>{projects.length ? '清除筛选' : <><Plus size={17} />添加第一个项目</>}</button></div>}
      <footer className="workspace-footer"><span>共 {filtered.length} 个项目{query ? ` · 搜索“${query}”` : ''}</span><span><Command size={12} />Ctrl + K 快速搜索</span></footer>
      {batchMode && <BatchToolbar count={batchSelected.length} hiddenCount={hiddenSelectionCount} allVisibleSelected={allVisibleSelected} busy={batchBusy} onSelectAll={selectVisible} onClear={() => setBatchIds(new Set())} onFavorite={favorite => applyBatch({ favorite })} onCategory={() => { setBatchError(''); setBatchCategory(settings.defaultCategory); setDialog('batch-category'); }} onRemove={() => { setBatchError(''); setDialog('batch-remove'); }} onExit={toggleBatchMode} />}
      </>}
    </main>
    {summary && <SummaryDialog project={summary.project} selection={summary.selection} onClose={() => setSummary(null)} onSave={draft => { setDevelopmentCommitSha(null); rememberProjectView();setScope('development'); setDevelopmentProjectId(summary.project.id); setDevelopmentDraft(draft); setSummary(null); setDiffOpen(false); }} />}
    {dialog === 'api' && <AiSettingsDialog onClose={() => setDialog(null)} onSaved={() => setAssistantVersion(value => value + 1)} />}
    {scope !== 'weekly-report' && !diffOpen && !batchMode && detailOpen && panelMode === 'detail' && detailProject && <ProjectDetail key={detailProject.id} project={detailProject} panelHeader={panelTabs} onDevelopment={() => { setDevelopmentCommitSha(null); setDevelopmentDraft(null); rememberProjectView();setScope('development'); setDevelopmentProjectId(detailProject.id); setDetailOpen(false); }} onClose={() => setDetailOpen(false)} onEdit={() => setDialog('edit')} onRemove={() => setDialog('remove')} onFavorite={() => favorite(detailProject)} onOpen={action => open(detailProject, action)} onCopy={copy} onTag={value => { setTag(value); setTagSource(undefined); setScope('all'); setCategory(''); setQuery(''); setStatus(''); }} />}
    <AssistantPanel visible={assistantVisible} header={panelTabs} projects={projects} filteredProjects={filtered} selectedProject={selected} workspaceVersion={assistantVersion} onConfigure={() => setDialog('api')} onClose={() => setDetailOpen(false)} onApply={applyAssistant} onViewDevelopment={(project,sha) => { setDevelopmentCommitSha(sha||null); setSelectedId(project.id); setDevelopmentDraft(null); rememberProjectView();setScope('development'); setDevelopmentProjectId(project.id); setDetailOpen(false); }} onViewProject={project => { setSelectedId(project.id); setDetailFromAssistant(true); setPanelMode('detail'); setDetailOpen(true); }} onShowResults={ids => { navigate('all'); setAssistantFilter(new Set(ids)); }} />
    {pending && <div className="pending-indicator" role="status"><LoaderCircle size={16} className="spin" />正在启动工具…</div>}
    {(dialog === 'add' || dialog === 'edit' && selected) && <ProjectDialog key={dialog === 'add' ? 'new' : selectedId} project={dialog === 'edit' ? selected : undefined} settings={settings} onClose={() => setDialog(null)} onSave={p => { setDialog(null); setProjects(list => dialog === 'edit' ? list.map(item => item.id === p.id ? p : item) : [p, ...list]); setSelectedId(p.id); setDetailOpen(true); navigate('all'); notify(dialog === 'edit' ? '项目资料已更新' : '项目已添加'); }} />}
    {dialog === 'settings' && <SettingsDialog settings={settings} onClose={() => setDialog(null)} onSave={saveSettings} onImported={async () => { setWeeklyEpoch(n=>n+1);projectViews.current={};setAssistantVersion(value => value + 1); setAssistantFilter(null); await reloadWorkspace(); setDialog(null); notify('项目库已导入，原数据已自动备份'); }} />}
    {dialog === 'discover' && <DiscoveryDialog settings={settings} onClose={() => setDialog(null)} onImported={result => {
      setProjects(previous => [...result.projects, ...previous]); setBatchMode(false); setBatchIds(new Set());
      if (result.projects[0]) { setSelectedId(result.projects[0].id); setDetailOpen(true); }
      setDialog(null); navigate('all'); notify(`已导入 ${result.projects.length} 个项目${result.skipped.length ? `，跳过 ${result.skipped.length} 个重复或不可用目录` : ''}`);
      if (result.skipped.length) { setImportReport(result.skipped); }
    }} />}
    {(dialog === 'batch-category' || dialog === 'batch-remove') && <Modal closable={!batchBusy} title={dialog === 'batch-remove' ? '批量移除项目索引？' : '批量调整分类'} subtitle={`操作范围：全部 ${batchSelected.length} 个已选项目${hiddenSelectionCount ? `，包含 ${hiddenSelectionCount} 个筛选外项目` : ''}`} onClose={() => setDialog(null)}>
      <div className="batch-confirm-body"><ul className="batch-project-list">{batchSelected.map(p => <li key={p.id}><strong>{p.name}</strong><span>{p.path}</span></li>)}</ul>
        {dialog === 'batch-category' ? <label className="batch-category-label">目标分类<select value={batchCategory} disabled={batchBusy} onChange={e => setBatchCategory(e.target.value as Category)}>{Object.entries(categories).map(([key, label]) => <option value={key} key={key}>{label}</option>)}</select></label> : <div className="notice"><ShieldCheck size={18} />仅删除项目库索引，本地文件夹和源文件不会被删除。</div>}
        {batchError && <p className="error-message" role="alert">{batchError}</p>}
      </div><footer className="modal-footer"><button className="button" disabled={batchBusy} onClick={() => setDialog(null)}>取消</button><button className={`button ${dialog === 'batch-remove' ? 'danger' : 'primary'}`} disabled={batchBusy || !batchSelected.length} onClick={() => dialog === 'batch-remove' ? removeBatch() : applyBatch({ category: batchCategory })}>{batchBusy && <LoaderCircle size={16} className="spin" />}{dialog === 'batch-remove' ? '确认批量移除' : '保存分类'}</button></footer>
    </Modal>}
    {importReport && <Modal title="导入结果说明" subtitle={`以下 ${importReport.length} 个候选未导入，已有项目保持原资料。`} onClose={() => setImportReport(null)}><div className="batch-confirm-body"><ul className="scan-report">{importReport.map((message, index) => <li key={index}>{message}</li>)}</ul></div><footer className="modal-footer"><button className="button primary" onClick={() => setImportReport(null)}>知道了</button></footer></Modal>}
    {dialog === 'tags' && <TagManagerDialog projects={projects} onClose={() => setDialog(null)} onChangeTag={changeTag} onFilter={(value, source) => { navigate('all'); setTag(value); setTagSource(source); setDialog(null); }} />}
    {dialog === 'remove' && selected && <Modal closable={!pending} title="从项目库移除？" onClose={() => setDialog(null)}><div className="confirm-body"><p>即将移除 <strong>{selected.name}</strong> 的项目索引。</p><div className="notice"><ShieldCheck size={18} />本地文件夹和源文件不会被删除。</div><p className="muted">之后可以通过添加项目重新录入。</p></div><footer className="modal-footer"><button className="button" disabled={pending} onClick={() => setDialog(null)}>取消</button><button className="button danger" disabled={pending} onClick={async () => { setPending(true); try { await repository.remove(selected.id); setProjects(list => list.filter(p => p.id !== selected.id)); setSelectedId(null); setDialog(null); notify('已移除项目索引，本地文件保留'); } catch (e) { notify(errorMessage(e), true); } finally { setPending(false); } }}>确认移除索引</button></footer></Modal>}
    {toast && <div className={`toast ${batchMode ? 'toast-batch' : ''} ${toast.error ? 'toast-error' : ''}`} role={toast.error ? 'alert' : 'status'}>{toast.error ? <AlertCircle size={18} /> : <CheckCircle2 size={18} />}<span>{toast.text}</span><button className="icon-button" aria-label="关闭提示" onClick={() => setToast(null)}><X size={16} /></button></div>}
  </div>;
}







