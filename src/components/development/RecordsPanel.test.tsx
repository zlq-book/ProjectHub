// @vitest-environment jsdom
import { cleanup, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import RecordsPanel from './RecordsPanel';
import type { DevelopmentRecords } from '../../types/development';
import type { Project } from '../../types';
const api = vi.hoisted(() => ({ space: vi.fn(), records: vi.fn(), saveTask: vi.fn(), saveLog: vi.fn(), history: vi.fn() }));
vi.mock('../../services/development', () => ({ developmentApi: api }));
const data: DevelopmentRecords = { space: { id: 'space', projectId: 'p', projectName: '项目', projectPath: 'D:\\p', repositoryId: 'repo' }, logs: [], tasks: [] };
beforeEach(() => { vi.resetAllMocks(); api.space.mockResolvedValue(data.space); api.records.mockResolvedValue(data); });
afterEach(cleanup);
it('新任务默认待办，用户选择进行中后按明确状态保存，不改项目资料', async () => {
  api.saveTask.mockResolvedValue(data);
  render(<RecordsPanel project={{ id: 'p' } as Project} kind="tasks" />);
  const user = userEvent.setup();
  await waitFor(() => expect((screen.getByRole('button', { name: '新建任务' }) as HTMLButtonElement).disabled).toBe(false));
  await user.click(screen.getByRole('button', { name: '新建任务' }));
  expect((screen.getByLabelText('任务状态') as HTMLSelectElement).value).toBe('todo');
  await user.type(screen.getByLabelText('标题'), '修复历史分页');
  await user.selectOptions(screen.getByLabelText('任务状态'), 'inProgress');
  await user.click(screen.getByRole('button', { name: '保存记录' }));
  await waitFor(() => expect(api.saveTask).toHaveBeenCalledWith(expect.objectContaining({ title: '修复历史分页', status: 'inProgress', commits: [], revision: null })));
});
it('日志保存冲突时保留用户编辑内容，不自动覆盖', async () => {
  api.saveLog.mockRejectedValue(new Error('记录已被其他窗口修改，请重新载入'));
  render(<RecordsPanel spaceId="space" kind="logs" />); const user = userEvent.setup();
  await waitFor(() => expect((screen.getByRole('button', { name: '新建日志' }) as HTMLButtonElement).disabled).toBe(false));
  await user.click(screen.getByRole('button', { name: '新建日志' }));
  await user.type(screen.getByLabelText('标题'), '保留草稿');
  await user.click(screen.getByRole('button', { name: '保存记录' }));
  expect(await screen.findByRole('alert')).toBeTruthy();
  expect((screen.getByLabelText('标题') as HTMLInputElement).value).toBe('保留草稿');
});
