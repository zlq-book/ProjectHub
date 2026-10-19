// @vitest-environment jsdom
import { cleanup, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import AiSettingsDialog from './AiSettingsDialog';
const api = vi.hoisted(() => ({ aiSettings: vi.fn(), saveAiSettings: vi.fn(), aiModels: vi.fn(), testAiConnection: vi.fn() }));
vi.mock('../services/projects', () => ({ repository: api, errorMessage: (e: unknown) => String(e) }));
beforeEach(() => {
  vi.resetAllMocks();
  api.aiSettings.mockResolvedValue({ baseUrl: 'https://example.com/v1', model: 'test-model', hasApiKey: true });
  api.saveAiSettings.mockResolvedValue({});
  HTMLDialogElement.prototype.showModal = function () { this.setAttribute('open', ''); };
  HTMLDialogElement.prototype.close = function () { this.removeAttribute('open'); };
});
afterEach(cleanup);
it('用未保存的地址和密钥获取模型，保留手填模型并提供候选项', async () => {
  api.aiModels.mockResolvedValue(['deepseek-flash', 'custom-model']);
  render(<AiSettingsDialog onClose={vi.fn()} />); const user = userEvent.setup();
  await waitFor(() => expect((screen.getByLabelText('模型名称') as HTMLInputElement).value).toBe('test-model'));
  await user.clear(screen.getByLabelText('API 基础地址')); await user.type(screen.getByLabelText('API 基础地址'), 'https://new.example/v1');
  await user.type(screen.getByLabelText('API Key'), 'new-fixture-key');
  await user.click(screen.getByRole('button', { name: '获取模型列表' }));
  await waitFor(() => expect(document.querySelectorAll('datalist option')).toHaveLength(2));
  expect(api.aiModels).toHaveBeenCalledWith({ baseUrl: 'https://new.example/v1', model: 'test-model', apiKey: 'new-fixture-key', clearApiKey: false });
  expect((screen.getByLabelText('模型名称') as HTMLInputElement).value).toBe('test-model');
  expect(api.saveAiSettings).not.toHaveBeenCalled();
});
it('测试连接显示耗时，失败时保留表单且不保存', async () => {
  api.testAiConnection.mockResolvedValueOnce({ latencyMs: 125, model: 'test-model' }).mockRejectedValueOnce(new Error('AI 服务拒绝授权'));
  render(<AiSettingsDialog onClose={vi.fn()} />); const user = userEvent.setup();
  await waitFor(() => expect((screen.getByLabelText('模型名称') as HTMLInputElement).value).toBe('test-model'));
  await user.click(screen.getByRole('button', { name: '测试连接' }));
  expect((await screen.findByRole('status')).textContent).toContain('125');
  await user.click(screen.getByRole('button', { name: '测试连接' }));
  expect((await screen.findByRole('alert')).textContent).toContain('拒绝授权');
  expect((screen.getByLabelText('模型名称') as HTMLInputElement).value).toBe('test-model');
  expect(api.saveAiSettings).not.toHaveBeenCalled();
});
it('读取配置不回显密钥，空密钥更新保留已有值', async () => {
  const onClose = vi.fn(); render(<AiSettingsDialog onClose={onClose} />); const user = userEvent.setup();
  await waitFor(() => expect((screen.getByLabelText('模型名称') as HTMLInputElement).value).toBe('test-model'));
  expect((screen.getByLabelText('API Key') as HTMLInputElement).value).toBe('');
  await user.click(screen.getByRole('button', { name: '保存 API 配置' }));
  expect(api.saveAiSettings).toHaveBeenCalledWith({ baseUrl: 'https://example.com/v1', model: 'test-model', apiKey: '', clearApiKey: false });
  expect(onClose).toHaveBeenCalledOnce();
});
it('明确勾选清除密钥，保存失败时保留配置窗口', async () => {
  api.saveAiSettings.mockRejectedValue(new Error('无法保存配置'));
  const onClose = vi.fn(); render(<AiSettingsDialog onClose={onClose} />); const user = userEvent.setup();
  await user.click(await screen.findByRole('checkbox', { name: '清除已保存的 API Key' }));
  await user.click(screen.getByRole('button', { name: '保存 API 配置' }));
  expect((await screen.findByRole('alert')).textContent).toContain('无法保存配置');
  expect(api.saveAiSettings).toHaveBeenCalledWith(expect.objectContaining({ clearApiKey: true, apiKey: '' }));
  expect(onClose).not.toHaveBeenCalled();
});
