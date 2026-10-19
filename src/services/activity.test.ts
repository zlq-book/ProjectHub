import { expect, it } from 'vitest';
import { activityBuckets } from './activity';

it('按本地自然日统计提交，完整范围补零，不按资料更新时间统计', () => {
  const result = activityBuckets(['2026-10-16T02:00:00Z', '2026-10-16T03:00:00Z'], 'day', new Date(2026, 9, 16, 12), true, 3);
  expect(result.map(b => b.count)).toEqual([0, 0, 2]);
});
it('周一为起点，月使用自然月，未完成范围的空桶保持未知', () => {
  const weeks = activityBuckets([new Date(2026, 9, 12, 12).toISOString()], 'week', new Date(2026, 9, 16, 12), true, 2);
  expect(weeks.map(b => b.count)).toEqual([0, 1]);
  const months = activityBuckets([], 'month', new Date(2026, 0, 2), false, 2);
  expect(months.map(b => b.label)).toEqual(['2025-12', '2026-01']);
  expect(months.every(b => b.count === null)).toBe(true);
});
