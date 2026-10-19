// @vitest-environment jsdom
import { cleanup, render, screen, waitFor, act } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { Project } from '../types';
import TagManagerDialog from './TagManagerDialog';

const project: Project = { id: 'a', name: '项目甲', path: 'D:\\Alpha', description: '', category: 'other', tags: ['Java', '业务', 'Java'], techStack: ['Java'],
  favorite: false, notes: '', defaultIde: 'system', createdAt: '2026-10-13', updatedAt: '2026-10-13', lastOpenedAt: null, available: true };
const projects = [project, { ...project, id: 'b', name: '项目乙', path: 'D:\\Beta', tags: ['业务'], techStack: ['React'] }];
beforeEach(() => {
  HTMLDialogElement.prototype.showModal = function () { this.setAttribute('open', ''); };
  HTMLDialogElement.prototype.close = function () { this.removeAttribute('open'); };
});
afterEach(cleanup);
function setup() {
  const onChangeTag = vi.fn().mockResolvedValue(undefined); const onFilter = vi.fn(); const onClose = vi.fn();
  render(<TagManagerDialog projects={projects} onChangeTag={onChangeTag} onFilter={onFilter} onClose={onClose} />);
  return { user: userEvent.setup(), onChangeTag, onFilter, onClose };
}
describe('标签整理交互', () => {
  it('按项目去重统计标签，技术栈不提供重命名和移除', async () => {
    const { user, onFilter } = setup();
    expect(screen.getByRole('button', { name: '筛选标签 Java：1 个项目' })).toBeDefined();
    await user.click(screen.getByRole('button', { name: '技术栈' }));
    expect(screen.queryByRole('button', { name: /重命名标签/ })).toBeNull();
    await user.click(screen.getByRole('button', { name: '筛选技术栈 Java：1 个项目' }));
    expect(onFilter).toHaveBeenCalledWith('Java', 'techStack');
  });
  it('重命名先展示全部影响对象，合并提示正确，确认后才调用服务', async () => {
    const { user, onChangeTag } = setup();
    await user.click(screen.getByRole('button', { name: '重命名标签 业务' }));
    expect(screen.getByText('项目甲')).toBeDefined(); expect(screen.getByText('项目乙')).toBeDefined();
    expect(onChangeTag).not.toHaveBeenCalled();
    await user.clear(screen.getByLabelText('新标签名称')); await user.type(screen.getByLabelText('新标签名称'), 'Java');
    expect(screen.getByText(/与已有标签合并/)).toBeDefined();
    await user.click(screen.getByRole('button', { name: '确认重命名' }));
    expect(onChangeTag).toHaveBeenCalledWith(['a', 'b'], '业务', 'Java');
    await waitFor(() => expect(screen.queryByLabelText('新标签名称')).toBeNull());
  });
  it('取消移除不调用服务，搜索只改变列表', async () => {
    const { user, onChangeTag } = setup();
    await user.type(screen.getByLabelText('搜索标签'), '业务');
    expect(screen.queryByRole('button', { name: '重命名标签 Java' })).toBeNull();
    await user.click(screen.getByRole('button', { name: '移除标签 业务' }));
    await user.click(screen.getByRole('button', { name: '返回标签列表' }));
    expect(onChangeTag).not.toHaveBeenCalled();
  });
  it('服务失败保留确认清单并允许重试', async () => {
    const { user, onChangeTag } = setup(); onChangeTag.mockRejectedValueOnce(new Error('存储失败'));
    await user.click(screen.getByRole('button', { name: '移除标签 业务' }));
    await user.click(screen.getByRole('button', { name: '确认移除标签' }));
    expect((await screen.findByRole('alert')).textContent).toContain('存储失败');
    expect(screen.getByText('项目甲')).toBeDefined();
    await user.click(screen.getByRole('button', { name: '确认移除标签' }));
    await waitFor(() => expect(onChangeTag).toHaveBeenCalledTimes(2));
  });
  it('保存期间禁止关闭和重复操作', async () => {
    const { user, onChangeTag, onClose } = setup(); let finish!: () => void;
    onChangeTag.mockImplementation(() => new Promise<void>(resolve => { finish = resolve; }));
    await user.click(screen.getByRole('button', { name: '移除标签 Java' }));
    await user.click(screen.getByRole('button', { name: '确认移除标签' }));
    expect((screen.getByRole('button', { name: '关闭弹窗' }) as HTMLButtonElement).disabled).toBe(true);
    expect((screen.getByRole('button', { name: '返回标签列表' }) as HTMLButtonElement).disabled).toBe(true);
    expect(onClose).not.toHaveBeenCalled();
    await act(async () => { finish(); });
  });
});
