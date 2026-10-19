// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import type { Project } from '../../types';
import type { AssistantResult } from '../../types/assistant';
import AssistantPanel from './AssistantPanel';

const api = vi.hoisted(() => ({ run: vi.fn(), clear: vi.fn(), cancel: vi.fn(), settings: vi.fn() }));
vi.mock('../../services/assistant', () => ({ assistantApi: api }));
vi.mock('../../services/projects', () => ({ native: true, errorMessage: (e: unknown) => e instanceof Error ? e.message : String(e) }));
const project: Project = { id: 'a', name: '预警后端', path: 'D:\\Alpha', description: '原简介', category: 'backend', tags: [], techStack: ['Java'], favorite: false, notes: '私人备注', defaultIde: 'system', createdAt: '2026-10-14', updatedAt: '2026-10-14', lastOpenedAt: null, available: true };
const result = (kind: AssistantResult['kind'] = 'search'): AssistantResult => ({ kind, text: '已根据保存资料查找。', items: [{ project, evidence: [{ source: 'description', quote: '原简介' }], certainty: 'confirmed', description: '建议简介', category: 'ai' }], proposalId: kind === 'search' ? undefined : 'proposal-1', toolSteps: ['检索项目'], scopeCount: 1 });
const props = () => ({ visible: true, projects: [project], filteredProjects: [project], selectedProject: project, header: <div>右栏页签</div>, workspaceVersion: 0, onConfigure: vi.fn(), onViewProject: vi.fn(), onShowResults: vi.fn(), onClose: vi.fn(), onApply: vi.fn().mockResolvedValue([{ ...project, description: '手动修改的简介' }]) });
beforeEach(() => { vi.resetAllMocks(); api.settings.mockResolvedValue({ baseUrl: 'https://model.example/v1', model: 'local-test', hasApiKey: false }); api.clear.mockResolvedValue(undefined); api.cancel.mockResolvedValue(undefined); api.run.mockResolvedValue(result()); });
afterEach(cleanup);
async function start() { const user = userEvent.setup(); await user.click(await screen.findByRole('button', { name: '开始使用 AI 助手' })); return user; }
async function send(user: ReturnType<typeof userEvent.setup>, text = '查找空气质量项目') { await user.type(screen.getByRole('textbox', { name: '发送给 AI 助手' }), text); await user.click(screen.getByRole('button', { name: '发送' })); }
it('主动开始后发送固定范围和空读取授权，结果可查看真实项目', async () => {
  const p = props(); render(<AssistantPanel {...p} />); const user = await start();
  await send(user);
  await screen.findByText('已根据保存资料查找。');
  expect(screen.getByLabelText('项目分类').textContent).toBe('后端项目');
  expect(api.run).toHaveBeenCalledWith(expect.objectContaining({ prompt: '查找空气质量项目', scopeIds: ['a'], contextIds: [] }));
  await user.click(screen.getByRole('button', { name: '查看详情：预警后端' }));
  expect(p.onViewProject).toHaveBeenCalledWith(project);
  expect(p.onApply).not.toHaveBeenCalled();
});
it('简介预览允许编辑，只有点击保存才提交单个建议字段', async () => {
  api.run.mockResolvedValue(result('description')); const p = props(); render(<AssistantPanel {...p} />); const user = await start();
  await send(user, '生成简介');
  const editor = await screen.findByRole('textbox', { name: '建议简介：预警后端' });
  expect(p.onApply).not.toHaveBeenCalled();
  await user.clear(editor); await user.type(editor, '手动修改的简介');
  await user.click(screen.getByRole('button', { name: '保存简介' }));
  await screen.findByText('已保存 1 个项目');
  expect(p.onApply).toHaveBeenCalledWith(expect.objectContaining({ proposalId: 'proposal-1', changes: [{ id: 'a', value: '手动修改的简介' }] }));
  expect((screen.getByRole('button', { name: '保存简介' }) as HTMLButtonElement).disabled).toBe(true);
});
it('含糊分类默认不选，选择和修改分类后才确认对应项目', async () => {
  const r = result('categories'); r.items[0].certainty = 'uncertain'; api.run.mockResolvedValue(r);
  const p = props(); render(<AssistantPanel {...p} />); const user = await start(); await send(user, '整理分类');
  const check = await screen.findByRole('checkbox', { name: '修改分类：预警后端' });
  expect((check as HTMLInputElement).checked).toBe(false);
  await user.click(check); await user.selectOptions(screen.getByRole('combobox', { name: '目标分类：预警后端' }), 'tool');
  await user.click(screen.getByRole('button', { name: '确认修改 1 个项目' }));
  await waitFor(() => expect(p.onApply).toHaveBeenCalledWith(expect.objectContaining({ changes: [{ id: 'a', value: 'tool' }] })));
});
it('读取资料必须点击单独入口，再提交用户勾选的 ID', async () => {
  api.run.mockResolvedValueOnce(result('readContext')).mockResolvedValueOnce(result('description'));
  render(<AssistantPanel {...props()} />); const user = await start(); await send(user, '给预警后端生成简介');
  await screen.findByRole('button', { name: '读取资料并继续（1 个项目）' });
  expect(api.run).toHaveBeenCalledTimes(1);
  await user.click(screen.getByRole('button', { name: '读取资料并继续（1 个项目）' }));
  await screen.findByRole('button', { name: '保存简介' });
  expect(api.run.mock.calls[1][0]).toEqual(expect.objectContaining({ prompt: '给预警后端生成简介', scopeIds: ['a'], contextIds: ['a'] }));
});
it('取消后的晚到结果不会出现，草稿可重试', async () => {
  let finish!: (r: AssistantResult) => void; api.run.mockImplementation(() => new Promise(resolve => { finish = resolve; }));
  render(<AssistantPanel {...props()} />); const user = await start(); await send(user);
  await user.click(screen.getByRole('button', { name: '停止生成' }));
  await act(async () => finish(result('description')));
  expect(screen.queryByRole('button', { name: '保存简介' })).toBeNull();
  expect(api.cancel).toHaveBeenCalledTimes(1);
});
it('中文输入法组合 Enter 不发送，正常 Enter 发送', async () => {
  render(<AssistantPanel {...props()} />); const user = await start();
  const input = screen.getByRole('textbox', { name: '发送给 AI 助手' });
  await user.type(input, '预警'); fireEvent.compositionStart(input); fireEvent.keyDown(input, { key: 'Enter', code: 'Enter', isComposing: true });
  expect(api.run).not.toHaveBeenCalled(); fireEvent.compositionEnd(input); fireEvent.keyDown(input, { key: 'Enter', code: 'Enter' });
  await waitFor(() => expect(api.run).toHaveBeenCalledTimes(1));
});
it('保存冲突显示错误，原建议不能重复提交', async () => {
  api.run.mockResolvedValue(result('description')); const p = props(); p.onApply.mockRejectedValue(new Error('项目资料已变化，请重新生成建议'));
  render(<AssistantPanel {...p} />); const user = await start(); await send(user, '生成简介');
  await user.click(await screen.findByRole('button', { name: '保存简介' }));
  expect(await screen.findByRole('alert')).toBeTruthy();
  expect((screen.getByRole('button', { name: '保存简介' }) as HTMLButtonElement).disabled).toBe(true);
});
it('新建会话重新展示本场会话的资料发送说明', async () => {
  render(<AssistantPanel {...props()} />); const user = await start();
  await send(user); await screen.findByText('已根据保存资料查找。');
  await user.click(screen.getByRole('button', { name: '新建会话' }));
  expect(await screen.findByRole('button', { name: '开始使用 AI 助手' })).toBeTruthy();
  expect(screen.queryByText('已根据保存资料查找。')).toBeNull();
});
it('不可用目录不会默认选中并提交资料读取', async () => {
  const r = result('readContext'); r.items[0].project = { ...project, available: false };
  api.run.mockResolvedValue(r); render(<AssistantPanel {...props()} projects={[r.items[0].project]} />); const user = await start(); await send(user);
  const check = await screen.findByRole('checkbox', { name: '读取资料：预警后端' });
  expect((check as HTMLInputElement).checked).toBe(false);
  expect((screen.getByRole('button', { name: '读取资料并继续（0 个项目）' }) as HTMLButtonElement).disabled).toBe(true);
});
it('零命中仍可主动选择项目补充资料，不自行读取', async () => {
  api.run.mockResolvedValue({ ...result(), items: [] });
  render(<AssistantPanel {...props()} />); const user = await start(); await send(user, '查找未知业务项目');
  await user.click(await screen.findByRole('button', { name: '选择项目补充 README／依赖' }));
  expect(await screen.findByRole('checkbox', { name: '读取资料：预警后端' })).toBeTruthy();
  expect(api.run).toHaveBeenCalledTimes(1);
});
it('混合分类预览只提交有建议的项目，资料不足行需先补充', async () => {
  const unknown: Project = { ...project, id: 'b', name: '待分析项目', path: 'D:\\Beta' };
  const r = result('categories'); r.items.push({ project: unknown, evidence: [], certainty: 'uncertain' });
  api.run.mockResolvedValue(r); const p = props(); render(<AssistantPanel {...p} projects={[project, unknown]} />);
  const user = await start(); await send(user, '整理分类');
  const check = await screen.findByRole('checkbox', { name: '修改分类：待分析项目' });
  expect((check as HTMLInputElement).disabled).toBe(true);
  expect((check as HTMLInputElement).checked).toBe(false);
  await user.click(screen.getByRole('button', { name: '确认修改 1 个项目' }));
  await waitFor(() => expect(p.onApply).toHaveBeenCalledWith(expect.objectContaining({ changes: [{ id: 'a', value: 'ai' }] })));
});
