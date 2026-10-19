export type ActivityGrain = 'day' | 'week' | 'month';
export function activityBuckets(timestamps: string[], grain: ActivityGrain, reference = new Date(), complete = true, length = grain === 'day' ? 30 : 12) {
  const start = new Date(reference.getFullYear(), reference.getMonth(), reference.getDate());
  if (grain === 'week') start.setDate(start.getDate() - (start.getDay() + 6) % 7);
  if (grain === 'month') start.setDate(1);
  const move = (date: Date, delta: number) => {
    const next = new Date(date);
    if (grain === 'month') next.setMonth(next.getMonth() + delta);
    else next.setDate(next.getDate() + delta * (grain === 'week' ? 7 : 1));
    return next;
  };
  const localLabel = (d: Date) => `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}${grain === 'month' ? '' : `-${String(d.getDate()).padStart(2, '0')}`}`;
  const size = Math.min(Math.max(length, 1), grain === 'day' ? 731 : grain === 'week' ? 105 : 24);
  return Array.from({ length: size }, (_, index) => {
    const date = move(start, index - size + 1), end = move(date, 1);
    const count = timestamps.filter(value => { const time = Date.parse(value); return time >= +date && time < +end && time <= +reference; }).length;
    return { label: localLabel(date), count: count || (complete ? 0 : null) };
  });
}
