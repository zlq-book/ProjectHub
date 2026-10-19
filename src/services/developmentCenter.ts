import { activityBuckets } from './activity';
import type { GitActivity, GitStatus } from '../types/development';

export type CenterSnapshot = { status?: GitStatus; activity?: GitActivity; error?: string; activityError?: string };
export type CenterMetric = { count: number; complete: boolean };
export function metricLabel(metric: CenterMetric) {
  return metric.complete ? String(metric.count) : metric.count ? `至少 ${metric.count}` : '未知';
}

export function summarizeDevelopment(snapshots: CenterSnapshot[], now = new Date()) {
  const week = new Date(now.getFullYear(), now.getMonth(), now.getDate());
  week.setDate(week.getDate() - (week.getDay() + 6) % 7);
  const days = activityBuckets([], 'day', now, true, 7).map(day => ({ label: day.label, count: 0, complete: true }));
  let active = 0, commits = 0, changed = 0, activityComplete = true, statusComplete = true;
  for (const snapshot of snapshots) {
    const { status, activity } = snapshot;
    if (!status || (status.repository && !status.complete)) statusComplete = false;
    if (status?.repository && status.files.length) changed++;
    if (status && !status.repository) continue;
    const complete = !!activity?.complete && !activity.shallow;
    if (!complete) activityComplete = false;
    const buckets = activityBuckets(activity?.timestamps || [], 'day', now, complete, 7);
    if (buckets.some(day => (day.count || 0) > 0)) active++;
    commits += (activity?.timestamps || []).filter(value => { const time = Date.parse(value); return time >= +week && time <= +now; }).length;
    buckets.forEach((bucket, index) => { days[index].count += bucket.count || 0; days[index].complete &&= complete; });
  }
  return {
    activeProjects: { count: active, complete: activityComplete },
    weekCommits: { count: commits, complete: activityComplete },
    changedProjects: { count: changed, complete: statusComplete },
    days,
  };
}
