import { useEffect, useState, type ReactNode } from 'react';
import { ArrowLeft, GitBranch, RefreshCw } from 'lucide-react';
import type { Project } from '../../types';
import { useGitStatus, statusLabel } from './useGitStatus';
import ActivityPanel from './ActivityPanel';
import DevelopmentCenter from './DevelopmentCenter';
import GitPanel, { type SummarySelection } from './GitPanel';
import { developmentApi } from '../../services/development';
import { native } from '../../services/projects';
import type { GitCommit } from '../../types/development';

type Tab = 'overview' | 'workspace' | 'history' | 'logs' | 'tasks';
export type DevelopmentDraft = { title: string; body: string; commits: GitCommit[] };
type Props = { projects: Project[]; version: number; onDiffOpen: (open: boolean) => void; initialProjectId?: string | null; initialCommitSha?:string|null; initialDraft?: DevelopmentDraft | null; onProjectSelect?: (project:Project)=>void; onOverview?:()=>void; onDraftConsumed?: () => void; records?: (project: Project | null, kind: 'logs' | 'tasks', draft: DevelopmentDraft | null, clearDraft: () => void) => ReactNode; archive?: ReactNode; onSummary?: (project: Project, selection: SummarySelection) => void; taskCounts?: Record<string, { todo: number; inProgress: number; done: number }> };
export default function DevelopmentWorkspace({ projects, version, onDiffOpen, initialProjectId, records, archive, onSummary, taskCounts, initialDraft, onDraftConsumed, onProjectSelect, onOverview, initialCommitSha }: Props) {
  const [projectId, setProjectId] = useState<string | null>(initialProjectId || null);
  const [tab, setTab] = useState<Tab>(initialDraft ? 'logs' : initialCommitSha ? 'history' : 'overview');
  const [refresh, setRefresh] = useState(0);
  const [counts,setCounts]=useState<Record<string,{todo:number;inProgress:number;done:number}>>({});
  useEffect(() => {
    let active = true;
    setCounts({});
    if (native) void developmentApi.spaces().then(async spaces => {
      const result: Record<string, { todo: number; inProgress: number; done: number }> = {};
      for (const space of spaces.filter(value => value.projectId)) {
        if (!active) return;
        const records = await developmentApi.records(space.id);
        if (!active) return;
        result[space.projectId!] = {
          todo: records.tasks.filter(task => task.status === 'todo').length,
          inProgress: records.tasks.filter(task => task.status === 'inProgress').length,
          done: records.tasks.filter(task => task.status === 'done').length,
        };
      }
      if (active) setCounts(Object.fromEntries(projects.map(project => [project.id, result[project.id] || { todo: 0, inProgress: 0, done: 0 }])));
    }).catch(() => { /* 保持未知，不用零值掩盖读取失败。 */ });
    return () => { active = false; };
  }, [projects, version, projectId, tab, refresh]);
  const [draft, setDraft] = useState<DevelopmentDraft | null>(initialDraft || null);
  useEffect(()=>{if(initialDraft){setDraft(initialDraft);setTab('logs');onDraftConsumed?.();}},[initialDraft,onDraftConsumed]);
  const project = projects.find(p => p.id === projectId);
  const { data, error, loading } = useGitStatus(project?.id || '', version + refresh, !!project);
  if (!project) return <DevelopmentCenter projects={projects} version={version} taskCounts={taskCounts || counts} archive={archive} onProject={value => { onProjectSelect?.(value); setProjectId(value.id); setDraft(null); setTab('overview'); }} />;
  return <div className="development-workspace"><div className="development-toolbar"><button className="button" onClick={() => { onOverview?.(); setProjectId(null); setDraft(null); onDiffOpen(false); }}><ArrowLeft size={16} />开发状态中心</button><h1>{project.name}</h1><button className="button" aria-label="刷新开发状态" onClick={() => setRefresh(n => n + 1)}><RefreshCw size={15} />刷新</button></div><p className="development-path">{project.path}</p><div className="development-repo"><GitBranch size={16} /><strong>{statusLabel(data, error)}</strong>{data && <span>读取于 {new Date(data.readAt).toLocaleTimeString()} · HEAD {data.head?.slice(0, 8) || '—'}{data.shallow ? ' · 浅克隆可见历史' : ''}</span>}</div>{data?.warning && <p className="notice">{data.warning}</p>}{error && <p role="alert" className="error-message">{error}</p>}
    <div className="development-tabs" role="tablist" aria-label="项目开发工作区">{([['overview', '概览'], ['history', '提交记录'], ['workspace', '文件变更'], ['logs', '开发日志'], ['tasks', '任务']] as const).map(([key, label]) => <button key={key} role="tab" aria-selected={tab === key} onClick={() => { onDiffOpen(false); setTab(key); }}>{label}</button>)}</div>
    {(tab === 'workspace' || tab === 'history') && (data ? <GitPanel key={project.id} projectId={project.id} status={data} historyMode={tab === 'history'} initialCommitSha={initialCommitSha} onDiffOpen={onDiffOpen} onSummary={selection => onSummary?.(project, selection)} onLog={commit => { setDraft({ title: commit.subject, body: commit.message, commits: [commit] }); setTab('logs'); }} /> : loading ? <p role="status">读取 Git 状态…</p> : null)}
    {tab === 'overview' && <section className="project-development-overview">
      <div className="center-metrics">
        <div className="center-metric"><span>当前分支</span><strong>{data ? data.repository ? data.branch || (data.head ? '分离 HEAD' : '暂无提交') : '未初始化 Git' : '未知'}</strong><small>{data?.head?.slice(0, 8) || '无 HEAD'}</small></div>
        <div className="center-metric"><span>未提交文件</span><strong>{!data || !data.repository ? '—' : data.complete ? data.files.length : data.files.length ? `至少 ${data.files.length}` : '未知'}</strong><small>暂存与工作区按文件去重</small></div>
        <div className="center-metric"><span>任务进度</span><strong>{(taskCounts || counts)[project.id] ? `${(taskCounts || counts)[project.id].done} / ${Object.values((taskCounts || counts)[project.id]).reduce((sum, n) => sum + n, 0)}` : '未知'}</strong><small>已完成 / 全部任务</small></div>
      </div>
      <p className="muted">最近提交：{data?.latestCommitAt ? new Date(data.latestCommitAt).toLocaleString() : data?.repository && !data.head ? '暂无提交' : '未知'}</p>
      {loading && <p role="status">读取 Git 状态…</p>}
      {data?.repository && <ActivityPanel key={`${project.id}-${refresh}`} projectId={project.id} head={data.head} />}
      {data && !data.repository && <p className="notice">未初始化 Git，暂无提交活动。开发日志和任务仍可使用。</p>}
    </section>}
    {(tab === 'logs' || tab === 'tasks') && (records?.(project, tab, draft, () => setDraft(null)) || <p className="notice">请在桌面版管理本地开发记录。</p>)}
  </div>;
}
