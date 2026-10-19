import { useEffect, useState } from 'react';
import { developmentApi } from '../../services/development';
import { activityBuckets, type ActivityGrain } from '../../services/activity';
import { errorMessage } from '../../services/projects';
import type { GitActivity } from '../../types/development';
export default function ActivityPanel({ projectId, head }: { projectId: string; head: string | null }) {
  const [data, setData] = useState<GitActivity>();
  const [error, setError] = useState('');
  const [grain, setGrain] = useState<ActivityGrain>('day');
  const [extended, setExtended] = useState(false);
  useEffect(() => { let active = true; setData(undefined); setError(''); developmentApi.activity(projectId).then(value => { if (active) setData(value); }, e => { if (active) setError(errorMessage(e)); }); return () => { active = false; }; }, [projectId, head]);
  const buckets = activityBuckets(data?.timestamps || [], grain, new Date(), data?.complete ?? false, extended ? (grain === 'day' ? 731 : grain === 'week' ? 105 : 24) : grain === 'day' ? 30 : 12);
  const max = Math.max(1, ...buckets.map(b => b.count || 0));
  const days = new Set((data?.timestamps || []).filter(time => { const d = new Date(time); return d <= new Date() && `${d.getFullYear()}-${String(d.getMonth()+1).padStart(2,'0')}-${String(d.getDate()).padStart(2,'0')}` >= buckets[0].label; }).map(time => new Date(time).toLocaleDateString('sv-SE'))).size;
  return <section className="development-section"><div className="development-toolbar"><h2>已提交的开发活动</h2><select aria-label="活跃度周期" value={grain} onChange={e => setGrain(e.target.value as ActivityGrain)}><option value="day">按天 · 30 天</option><option value="week">按周 · 12 周</option><option value="month">按月 · 12 月</option></select><label><input type="checkbox" checked={extended} onChange={e => setExtended(e.target.checked)} />回看两年</label></div><p className="muted">当前分支 · 全部作者 · 含合并提交 · 提交者时间 · 本地自然日期，周一起算。不包含未提交编辑。</p>
    {error && <p role="alert" className="error-message">{error}</p>}{!data && !error && <p role="status">读取提交历史…</p>}
    {data && <><div className="development-stats"><strong>{data.complete ? '' : '至少 '}{buckets.reduce((n, b) => n + (b.count || 0), 0)}<span>次提交</span></strong><strong>{data.complete ? '' : '至少 '}{days}<span>个活跃日期</span></strong><strong>{data.complete ? '完整' : '部分'}<span>读取状态{data.shallow ? ' · 浅克隆' : ''}</span></strong></div>{(data.warning || !data.complete) && <p className="notice">{data.warning || '统计未完成，未读取的日期不能视为 0 次提交。'}</p>}<div className="activity-chart" role="img" aria-label="提交频次趋势，完整数值见下方表格">{buckets.map(b => <div key={b.label} title={`${b.label}：${b.count === null ? '未知' : `${b.count} 次`}`}><span style={{ height: `${Math.max(2, (b.count || 0) / max * 100)}%` }} className={b.count === null ? 'unknown' : ''} /></div>)}</div><p className="muted">每柱代表一个自然{grain === 'day' ? '日' : grain === 'week' ? '周' : '月'}；灰色为空间未知。读取于 {new Date(data.readAt).toLocaleString()}，HEAD {data.head?.slice(0, 8) || '暂无提交'}。</p><details><summary>查看统计表格</summary><table className="development-table"><thead><tr><th>周期起点</th><th>提交数</th></tr></thead><tbody>{buckets.map(b => <tr key={b.label}><td>{b.label}</td><td>{b.count ?? '未知'}</td></tr>)}</tbody></table></details></>}
  </section>;
}
