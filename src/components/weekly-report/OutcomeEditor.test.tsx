// @vitest-environment jsdom
import { useState } from 'react';
import { cleanup, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, it, expect } from 'vitest';
import OutcomeEditor from './OutcomeEditor';
import type { Outcome } from '../../types/weeklyReport';
afterEach(cleanup);
const outcomes: Outcome[] = Array.from({ length: 5 }, (_, i) => ({ id: `o${i}`, projectIds: ['p'], text: `成果${i}`, sourceIds: [`s${i}`], origin: 'ai' }));
it('第五条不会入选，取消一条后可以改选且保留正文编辑', async () => { const user = userEvent.setup(); function Harness() { const [ids, setIds] = useState<string[]>([]); const [data, setData] = useState(outcomes); return <OutcomeEditor outcomes={data} selectedIds={ids} onChange={setIds} onEdit={setData} sources={[]}/>; } render(<Harness />); expect(screen.getByText('AI 已按功能主题归纳为 5 条候选，覆盖 5 条材料。')).toBeTruthy(); expect(screen.getAllByText('查看依据（1 条材料）')).toHaveLength(5); const checks = screen.getAllByRole('checkbox'); for (const c of checks.slice(0, 4))
    await user.click(c); await user.click(checks[4]); expect(screen.getByRole('alert').textContent).toContain('4 条'); expect((checks[4] as HTMLInputElement).checked).toBe(false); await user.click(checks[0]); await user.click(checks[4]); expect((checks[4] as HTMLInputElement).checked).toBe(true); await user.clear(screen.getByLabelText('成果 5')); await user.type(screen.getByLabelText('成果 5'), '手动修订'); expect((screen.getByLabelText('成果 5') as HTMLTextAreaElement).value).toBe('手动修订'); });
