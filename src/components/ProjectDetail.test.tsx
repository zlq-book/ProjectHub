// @vitest-environment jsdom
import { act, cleanup, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, expect, it, vi } from 'vitest';
import type { Project } from '../types';
import ProjectDetail from './ProjectDetail';

const api = vi.hoisted(() => ({ gitInfo: vi.fn() }));
vi.mock('../services/projects', () => ({ repository: api }));
afterEach(cleanup);
const project: Project = { id: 'a', name: '项目甲', path: 'D:\\Alpha', description: '项目简介', category: 'backend', tags: [], techStack: ['Java'], favorite: false, notes: '', defaultIde: 'system', createdAt: '2026-10-14', updatedAt: '2026-10-14', lastOpenedAt: null, available: true };
const actions = () => ({ onClose: vi.fn(), onEdit: vi.fn(), onRemove: vi.fn(), onFavorite: vi.fn(), onOpen: vi.fn(), onCopy: vi.fn(), onTag: vi.fn() });
it('展示真实仓库与分支，主要快捷操作明确打开 IDEA', async () => {
  api.gitInfo.mockResolvedValue({ repository: true, branch: 'feature/list', remote: 'git@example.com:team/project.git' });
  const props = actions(); render(<ProjectDetail project={project} {...props} />);
  expect(await screen.findByText('feature/list')).toBeTruthy();
  expect(screen.getByText('git@example.com:team/project.git')).toBeTruthy();
  const user = userEvent.setup(); await user.click(screen.getByRole('button', { name: '打开 IDEA' }));
  expect(props.onOpen).toHaveBeenCalledWith('idea');
});
it('读取失败不虚构分支和仓库，并显示实际错误', async () => {
  api.gitInfo.mockRejectedValue(new Error('未找到 Git，请先安装 Git'));
  render(<ProjectDetail project={project} {...actions()} />);
  expect(await screen.findByText('未找到 Git，请先安装 Git')).toBeTruthy();
  expect(screen.queryByText('main')).toBeNull();
});
it('切换项目后忽略上个项目较晚返回的 Git 信息', async () => {
  let finish!: (info: { repository: boolean; branch: string; remote: null }) => void;
  api.gitInfo.mockReturnValueOnce(new Promise(resolve => { finish = resolve; }));
  api.gitInfo.mockResolvedValueOnce({ repository: true, branch: 'new-project', remote: null });
  const props = actions();
  const { rerender } = render(<ProjectDetail project={project} {...props} />);
  rerender(<ProjectDetail project={{ ...project, id: 'b', path: 'D:\\Beta' }} {...props} />);
  await screen.findByText('new-project');
  await act(async () => finish({ repository: true, branch: 'stale-project', remote: null }));
  expect(screen.getByText('new-project')).toBeTruthy();
  expect(screen.queryByText('stale-project')).toBeNull();
});
