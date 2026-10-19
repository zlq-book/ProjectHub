import { expect, it } from 'vitest';
import { summarizeDevelopment, type CenterSnapshot } from './developmentCenter';
import type { GitStatus } from '../types/development';

const now = new Date(2026, 9, 16, 12);
const time = (day: number) => new Date(2026, 9, day, 10).toISOString();
const status: GitStatus = { repository: true, repositoryId: 'repo', branch: 'main', head: 'head', readAt: now.toISOString(), complete: true, warning: null, files: [], latestCommitAt: time(16), shallow: false };
const snapshot = (timestamps: string[], complete = true): CenterSnapshot => ({ status, activity: { head: 'head', timestamps, complete, readAt: now.toISOString(), shallow: false, warning: null } });

it('分别统计最近七日活跃项目和周一起算的本周提交，排除未来时间', () => {
  const result = summarizeDevelopment([snapshot([time(10), time(12), time(16), time(17)]), snapshot([time(11)])], now);
  expect(result.activeProjects).toEqual({ count: 2, complete: true });
  expect(result.weekCommits).toEqual({ count: 2, complete: true });
  expect(result.days.map(day => day.count)).toEqual([1, 1, 1, 0, 0, 0, 1]);
});

it('读取失败不变成零，部分历史保留下限，未提交文件按项目计数', () => {
  const partial = snapshot([time(16)], false);
  partial.status = { ...status, complete: false, files: [{ path: 'a', indexStatus: 'M', worktreeStatus: 'M', untracked: false, conflicted: false }] };
  const result = summarizeDevelopment([partial, { error: '离线' }], now);
  expect(result.activeProjects).toEqual({ count: 1, complete: false });
  expect(result.weekCommits).toEqual({ count: 1, complete: false });
  expect(result.changedProjects).toEqual({ count: 1, complete: false });
  expect(result.days.at(-1)).toMatchObject({ count: 1, complete: false });
  expect(result.days[0]).toMatchObject({ count: 0, complete: false });
});

it('非 Git 项目及空项目库可确认没有提交活动', () => {
  const result = summarizeDevelopment([{ status: { ...status, repository: false, head: null } }], now);
  expect(result.weekCommits).toEqual({ count: 0, complete: true });
  expect(summarizeDevelopment([], now).days.every(day => day.complete && day.count === 0)).toBe(true);
});
