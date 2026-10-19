import { useEffect, useState, type ReactNode } from 'react';
import { RefreshCw } from 'lucide-react';
import type { Project } from '../../types';
import { developmentApi } from '../../services/development';
import { errorMessage } from '../../services/projects';
import { metricLabel, summarizeDevelopment, type CenterSnapshot } from '../../services/developmentCenter';

export type TaskCounts = Record<string, { todo: number; inProgress: number; done: number }>;
export default function DevelopmentCenter({ projects, version, onProject, taskCounts, archive }: {
  projects: Project[]; version: number; onProject: (project: Project) => void; taskCounts?: TaskCounts; archive?: ReactNode;
}) {
  const [snapshots, setSnapshots] = useState<Record<string, CenterSnapshot>>({});
  const [refresh, setRefresh] = useState(0);
  const [reading, setReading] = useState(false);
  const [search, setSearch] = useState('');
  const [filter, setFilter] = useState('');
  const [reference, setReference] = useState(() => new Date());
  useEffect(() => {
    let active = true, next = 0;
    setSnapshots({}); setReference(new Date()); setReading(true);
    const publish = (id: string, value: CenterSnapshot) => { if (active) setSnapshots(current => ({ ...current, [id]: value })); };
    async function worker() {
      while (active && next < projects.length) {
        const project = projects[next++];
        let snapshot: CenterSnapshot;
        try {
          const status = await developmentApi.status(project.id, refresh > 0 || version > 0, () => active);
          if (!active) return;
          snapshot = { status }; publish(project.id, snapshot);
          if (status.repository) {
            try { snapshot = { status, activity: await developmentApi.activity(project.id) }; }
            catch (error) { snapshot = { status, activityError: errorMessage(error) }; }
          }
        } catch (error) { snapshot = { error: errorMessage(error) }; }
        publish(project.id, snapshot);
      }
    }
    void Promise.all([worker(), worker()]).then(() => { if (active) { setReference(new Date()); setReading(false); } });
    return () => { active = false; };
  }, [projects, version, refresh]);
  const summary = summarizeDevelopment(projects.map(project => snapshots[project.id] || {}), reference);
  const max = Math.max(1, ...summary.days.map(day => day.count));
  const filtered = projects.filter(project => {
    if (!project.name.toLocaleLowerCase().includes(search.toLocaleLowerCase())) return false;
    const snapshot = snapshots[project.id];
    return !filter || (filter === 'unread' ? !snapshot : filter === 'changed' ? !!snapshot?.status?.files.length : !!snapshot?.error || !!snapshot?.activityError || snapshot?.status?.complete === false || snapshot?.activity?.complete === false || !!snapshot?.activity?.shallow);
  });
  const readCount = projects.filter(project => snapshots[project.id]).length;
  return <div className="development-workspace development-center">
    <div className="development-toolbar"><h1>开发状态中心</h1><button className="button" aria-label="刷新开发状态中心" disabled={reading} onClick={() => setRefresh(value => value + 1)}><RefreshCw size={15} className={reading ? 'spin' : ''} />{reading ? '读取中…' : '刷新'}</button></div>
    <p className="muted">集中查看项目开发状态，点击项目进入详情。</p>
    <div className="center-metrics">{([
      ['活跃项目', summary.activeProjects, '最近 7 天有提交'],
      ['本周提交', summary.weekCommits, '周一起算 · 按项目累计'],
      ['有未提交更改', summary.changedProjects, '存在变更的项目数'],
    ] as const).map(([label, metric, note]) => <div className="center-metric" role="group" aria-label={label} key={label}><span>{label}</span><strong>{metricLabel(metric)}</strong><small>{note}</small></div>)}</div>
    <section className="center-trend" aria-labelledby="center-trend-title"><div className="development-toolbar"><h2 id="center-trend-title">最近 7 天提交趋势</h2><span className="muted">当前分支 · 全部作者</span></div>
      <div className="center-chart" role="img" aria-label="最近 7 天提交趋势">{summary.days.map(day => <div className="center-chart-column" key={day.label}><span className="center-chart-value">{metricLabel(day)}</span><div className="center-chart-track"><span className={!day.complete ? 'partial' : ''} style={{ height: `${Math.max(day.count ? 3 : 0, day.count / max * 100)}%` }} /></div><time dateTime={day.label}>{day.label.slice(5).replace('-', '/')}</time></div>)}</div>
      <details><summary>查看趋势数据</summary><table className="development-table"><thead><tr><th>日期</th><th>提交数</th></tr></thead><tbody>{summary.days.map(day => <tr key={day.label}><td>{day.label}</td><td>{metricLabel(day)}</td></tr>)}</tbody></table></details>
      <p className="muted">按提交者时间及本地自然日统计，不含未提交编辑。同仓库的不同项目分别累计。{summary.weekCommits.complete ? '' : '统计尚不完整：“至少”为已读取下限，“未知”不能视为零。'}</p>
    </section>
    <section aria-labelledby="center-projects-title"><div className="development-toolbar"><h2 id="center-projects-title">项目状态</h2><span className="muted">已读取 {readCount} / {projects.length} 个项目</span></div>
      <div className="center-filters"><input className="development-search" aria-label="查找开发项目" placeholder="按项目名称查找" value={search} onChange={event => setSearch(event.target.value)} /><select aria-label="筛选开发状态" value={filter} onChange={event => setFilter(event.target.value)}><option value="">全部状态</option><option value="changed">有未提交文件</option><option value="error">异常或读取不完整</option><option value="unread">尚未读取</option></select></div>
      <div className="development-table-scroll"><table className="development-table center-project-table"><thead><tr><th>项目</th><th>分支</th><th>未提交</th><th>最近开发</th><th>待办 / 进行中 / 完成</th></tr></thead><tbody>{filtered.map(project => {
        const snapshot = snapshots[project.id], status = snapshot?.status, counts = taskCounts?.[project.id];
        return <tr key={project.id}><td><button className="text-button" aria-label={`查看开发进度 ${project.name}`} onClick={() => onProject(project)}>{project.name}</button>{snapshot?.error && <small className="center-row-warning">{snapshot.error}</small>}{snapshot?.activityError && <small className="center-row-warning">活动读取失败：{snapshot.activityError}</small>}{status?.warning && <small className="center-row-warning">{status.warning}</small>}{snapshot?.activity?.warning && <small className="center-row-warning">{snapshot.activity.warning}</small>}</td><td>{status ? status.repository ? <code>{status.branch || (status.head ? '分离 HEAD' : '暂无提交')}</code> : '未初始化 Git' : '未知'}</td><td>{status ? status.repository ? `${metricLabel({ count: status.files.length, complete: status.complete })}${status.complete || status.files.length ? ' 个文件' : ''}` : '不适用' : '未知'}</td><td>{status?.repository ? status.latestCommitAt ? <time dateTime={status.latestCommitAt}>{new Date(status.latestCommitAt).toLocaleString()}</time> : status.head ? '未知' : '暂无提交' : status ? '不适用' : '未知'}</td><td>{counts ? `${counts.todo} / ${counts.inProgress} / ${counts.done}` : '未知'}</td></tr>;
      })}</tbody></table></div>
      {!projects.length ? <p className="notice">暂无项目，添加项目后即可查看开发状态。</p> : !filtered.length && <p className="notice">没有符合条件的项目。<button className="button" onClick={()=>{setSearch('');setFilter('');}}>清空筛选</button></p>}
      <p className="muted">最近开发为当前分支最近提交时间；不以项目资料更新时间替代。状态为本次读取快照，可点击刷新更新。</p>
    </section>{archive}
  </div>;
}
