// @vitest-environment jsdom
import { cleanup, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { defaultSettings, type Project } from '../types';
import ProjectDialog from './ProjectDialog';

const api = vi.hoisted(() => ({ update: vi.fn(), analyze: vi.fn(), aiSettings: vi.fn() }));
vi.mock('../services/projects', () => ({ native: false, repository: api, errorMessage: (e: unknown) => String(e) }));
const project: Project = { id: 'a', name: '项目甲', path: 'D:\\Alpha', description: '', category: 'other', tags: ['业务，重要', '普通'], techStack: ['Java, SDK'],
  favorite: false, notes: '', defaultIde: 'system', createdAt: '2026-10-13', updatedAt: '2026-10-13', lastOpenedAt: null, available: true };
beforeEach(() => {
  vi.resetAllMocks(); api.update.mockResolvedValue(project);
  api.aiSettings.mockResolvedValue({ baseUrl: 'https://example.com/v1', model: 'test-model', hasApiKey: true });
  HTMLDialogElement.prototype.showModal = function () { this.setAttribute('open', ''); };
  HTMLDialogElement.prototype.close = function () { this.removeAttribute('open'); };
});
afterEach(cleanup);
it('未配置模型时提示配置 API，不发起解析请求', async () => {
  api.aiSettings.mockResolvedValue({ baseUrl: '', model: '', hasApiKey: false });
  render(<ProjectDialog project={project} settings={defaultSettings} onClose={vi.fn()} onSave={vi.fn()} />);
  const user = userEvent.setup(); await user.click(await screen.findByRole('button', { name: 'AI 解析' }));
  expect((await screen.findByRole('alert')).textContent).toContain('配置 API');
  expect(api.analyze).not.toHaveBeenCalled();
});
it('AI 结果先预览，应用后回填五项字段，保留路径和备注且不自动保存', async () => {
  api.analyze.mockResolvedValue({ name: '智能监测服务', description: '提供环境数据分析', category: 'backend', techStack: ['Java', 'Spring Boot'], tags: ['环境监测', 'AI'] });
  render(<ProjectDialog project={{ ...project, notes: '保留备注' }} settings={defaultSettings} onClose={vi.fn()} onSave={vi.fn()} />);
  const user = userEvent.setup();
  await user.click(await screen.findByRole('button', { name: 'AI 解析' }));
  await screen.findByRole('button', { name: '应用解析结果' });
  expect((screen.getByLabelText(/项目名称/) as HTMLInputElement).value).toBe('项目甲');
  expect(api.update).not.toHaveBeenCalled();
  expect(api.analyze).toHaveBeenCalledWith(project.path, project.id);
  await user.click(screen.getByRole('button', { name: '应用解析结果' }));
  expect((screen.getByLabelText(/项目名称/) as HTMLInputElement).value).toBe('智能监测服务');
  expect((screen.getByLabelText('项目简介') as HTMLTextAreaElement).value).toBe('提供环境数据分析');
  expect((screen.getByLabelText('项目分类') as HTMLSelectElement).value).toBe('backend');
  expect((screen.getByLabelText('技术栈') as HTMLInputElement).value).toBe('Java, Spring Boot');
  expect((screen.getByLabelText('项目标签') as HTMLInputElement).value).toBe('环境监测, AI');
  expect((screen.getByLabelText('备注') as HTMLTextAreaElement).value).toBe('保留备注');
  await user.click(screen.getByRole('button', { name: '保存修改' }));
  await waitFor(() => expect(api.update).toHaveBeenCalledWith(project.id, expect.objectContaining({ path: project.path, notes: '保留备注', tags: ['环境监测', 'AI'] })));
});
it('AI 请求失败保留所有已填写内容', async () => {
  api.analyze.mockRejectedValue(new Error('模型服务连接失败'));
  render(<ProjectDialog project={project} settings={defaultSettings} onClose={vi.fn()} onSave={vi.fn()} />);
  const user = userEvent.setup();
  await user.click(await screen.findByRole('button', { name: 'AI 解析' }));
  expect((await screen.findByRole('alert')).textContent).toContain('模型服务连接失败');
  expect((screen.getByLabelText(/项目名称/) as HTMLInputElement).value).toBe(project.name);
  expect(api.update).not.toHaveBeenCalled();
});
it('仅修改名称时完整保留含逗号的标签和技术栈', async () => {
  render(<ProjectDialog project={project} settings={defaultSettings} onClose={vi.fn()} onSave={vi.fn()} />);
  const user = userEvent.setup();
  await user.clear(screen.getByLabelText(/项目名称/)); await user.type(screen.getByLabelText(/项目名称/), '改名项目');
  await user.click(screen.getByRole('button', { name: '保存修改' }));
  await waitFor(() => expect(api.update).toHaveBeenCalledWith('a', expect.objectContaining({ name: '改名项目', tags: project.tags, techStack: project.techStack })));
});
it('用户主动更改标签文本时仍按逗号分隔去重', async () => {
  render(<ProjectDialog project={project} settings={defaultSettings} onClose={vi.fn()} onSave={vi.fn()} />);
  const user = userEvent.setup();
  await user.clear(screen.getByLabelText('项目标签')); await user.type(screen.getByLabelText('项目标签'), '新标签,新标签，另一标签');
  await user.click(screen.getByRole('button', { name: '保存修改' }));
  await waitFor(() => expect(api.update).toHaveBeenCalledWith('a', expect.objectContaining({ tags: ['新标签', '另一标签'], techStack: project.techStack })));
});
