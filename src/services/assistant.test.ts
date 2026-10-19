import { beforeEach, expect, it, vi } from 'vitest';
import { assistantApi } from './assistant';
const api = vi.hoisted(() => ({ native: false, invoke: vi.fn(), settings: vi.fn() }));
vi.mock('@tauri-apps/api/core', () => ({ invoke: api.invoke }));
vi.mock('./projects', () => ({ get native() { return api.native; }, repository: { aiSettings: api.settings } }));
beforeEach(() => { vi.clearAllMocks(); api.native = false; });
const request = { sessionId: 's', requestId: 'r', prompt: '查找', scopeIds: ['a'], referenceTime: '2026-10-15T00:00:00Z', history: [], contextIds: [] };
it('浏览器不能运行助手或保存建议，不调用原生桥', async () => {
  expect(() => assistantApi.run(request)).toThrow('需要桌面版');
  expect(() => assistantApi.apply({ sessionId: 's', proposalId: 'p', changes: [] })).toThrow('需要桌面版');
  await assistantApi.clear('s'); await assistantApi.cancel('s', 'r');
  expect(api.invoke).not.toHaveBeenCalled();
});
it('桌面桥只提交约定的工具协议和用户选择', async () => {
  api.native = true; api.invoke.mockResolvedValue({ kind: 'search', items: [] });
  await assistantApi.run(request);
  expect(api.invoke).toHaveBeenCalledWith('assistant_run', { input: request });
  const input = { sessionId: 's', proposalId: 'p', changes: [{ id: 'a', value: '简介' }] };
  await assistantApi.apply(input);
  expect(api.invoke).toHaveBeenCalledWith('apply_assistant_proposal', { input });
});
