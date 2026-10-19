// @vitest-environment jsdom
import { cleanup, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import type { Project } from '../../types';
import type { GitStatus } from '../../types/development';
import DevelopmentCenter from './DevelopmentCenter';
const api = vi.hoisted(() => ({ status: vi.fn(), activity: vi.fn() }));
vi.mock('../../services/development', () => ({ developmentApi: api }));
const project = { id: 'a', name: '项目 A', path: 'D:\\A' } as Project;
const status: GitStatus = { repository: true, repositoryId: 'repo', branch: 'main', head: 'head', readAt: new Date().toISOString(), complete: true, warning: null, files: [], latestCommitAt: new Date().toISOString(), shallow: false };
beforeEach(() => {
  api.status.mockReset().mockResolvedValue(status);
  api.activity.mockReset().mockResolvedValue({ head: 'head', timestamps: [new Date().toISOString()], complete: true, shallow: false });
});
afterEach(cleanup);

it('展示真实统计、七日趋势、项目入口，筛选不改变全局指标，刷新重新读取', async () => {
  const open = vi.fn();
  render(<DevelopmentCenter projects={[project]} version={0} onProject={open} />);
  const user = userEvent.setup();
  await waitFor(() => expect(within(screen.getByRole('group', { name: '本周提交' })).getByText('1')).toBeTruthy());
  expect(screen.getByRole('img', { name: '最近 7 天提交趋势' })).toBeTruthy();
  await user.type(screen.getByRole('textbox', { name: '查找开发项目' }), '不存在');
  expect(screen.getByText('没有符合条件的项目。')).toBeTruthy();
  expect(within(screen.getByRole('group', { name: '本周提交' })).getByText('1')).toBeTruthy();
  await user.clear(screen.getByRole('textbox', { name: '查找开发项目' }));
  await user.click(screen.getByRole('button', { name: '查看开发进度 项目 A' }));
  expect(open).toHaveBeenCalledWith(project);
  await user.click(screen.getByRole('button', { name: '刷新开发状态中心' }));
  await waitFor(() => expect(api.activity).toHaveBeenCalledTimes(2));
});

it('失败和非 Git 项目分别显示未知与不适用，活动失败不影响工作区状态', async () => {
  api.status.mockImplementation((id: string) => id === 'a' ? Promise.reject(new Error('磁盘离线')) : Promise.resolve({ ...status, repository: false }));
  render(<DevelopmentCenter projects={[project, { ...project, id: 'b', name: '普通目录' }]} version={0} onProject={() => {}} />);
  await screen.findByText('磁盘离线');
  expect(within(screen.getByRole('group', { name: '本周提交' })).getByText('未知')).toBeTruthy();
  expect(screen.getByText('未初始化 Git')).toBeTruthy();
  expect(api.activity).not.toHaveBeenCalled();
});

it('全量读取最多并发两个项目，慢请求在卸载后不继续调度', async () => {
  let release!: (value: GitStatus) => void;
  api.status.mockImplementation(() => new Promise<GitStatus>(resolve => { release = resolve; }));
  const { unmount } = render(<DevelopmentCenter projects={['a', 'b', 'c'].map(id => ({ ...project, id }))} version={0} onProject={() => {}} />);
  await waitFor(() => expect(api.status).toHaveBeenCalledTimes(2));
  unmount();
  release(status);
  await Promise.resolve();
  expect(api.status).toHaveBeenCalledTimes(2);
  expect(api.activity).not.toHaveBeenCalled();
});

it('活动读取失败保留已读分支和文件数，并提供异常筛选', async () => {
  api.activity.mockRejectedValue(new Error('历史读取超时'));
  render(<DevelopmentCenter projects={[project]} version={0} onProject={() => {}} />);
  await screen.findByText('活动读取失败：历史读取超时');
  expect(screen.getByText('main')).toBeTruthy();
  expect(screen.getByText('0 个文件')).toBeTruthy();
  expect(within(screen.getByRole('group', { name: '有未提交更改' })).getByText('0')).toBeTruthy();
  expect(within(screen.getByRole('group', { name: '本周提交' })).getByText('未知')).toBeTruthy();
  await userEvent.setup().selectOptions(screen.getByRole('combobox', { name: '筛选开发状态' }), 'error');
  expect(screen.getByRole('button', { name: '查看开发进度 项目 A' })).toBeTruthy();
});
