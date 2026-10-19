// @vitest-environment jsdom
import { act, cleanup, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { defaultSettings, type ScanResult } from '../types';
import DiscoveryDialog from './DiscoveryDialog';

const api = vi.hoisted(() => ({ scan: vi.fn(), cancelScan: vi.fn(), choose: vi.fn(), importProjects: vi.fn() }));
vi.mock('../services/projects', () => ({ native: true, repository: api, errorMessage: (e: unknown) => e instanceof Error ? e.message : String(e) }));
const result: ScanResult = {
  scannedDirectories: 3, warnings: [], truncated: false,
  projects: [
    { name: '新项目', path: 'D:\\Code\\New', description: '候选说明', techStack: ['React'], existing: false },
    { name: '已有项目', path: 'D:\\Code\\Old', description: '', techStack: ['Java'], existing: true },
  ],
};
beforeEach(() => {
  vi.resetAllMocks();
  api.cancelScan.mockResolvedValue(undefined);
  api.scan.mockResolvedValue(structuredClone(result));
  api.choose.mockResolvedValue('E:\\Other');
  // jsdom 不实现原生 dialog，用最小平台垫片测试组件行为。
  HTMLDialogElement.prototype.showModal = function () { this.setAttribute('open', ''); };
  HTMLDialogElement.prototype.close = function () { this.removeAttribute('open'); };
});
afterEach(cleanup);
function setup() {
  const onImported = vi.fn(); const onClose = vi.fn();
  render(<DiscoveryDialog settings={defaultSettings} onImported={onImported} onClose={onClose} />);
  return { user: userEvent.setup(), onImported, onClose };
}
async function scan(user: ReturnType<typeof userEvent.setup>) {
  await user.type(screen.getByLabelText('扫描根目录'), 'D:\\Code');
  await user.click(screen.getByRole('button', { name: '开始扫描' }));
  await screen.findByText('新项目');
}
describe('项目发现交互', () => {
  it('默认不选择候选，已录入项目不能再次勾选', async () => {
    const { user } = setup(); await scan(user);
    const inputs = screen.getAllByRole('checkbox') as HTMLInputElement[];
    expect(inputs[0].checked).toBe(false); expect(inputs[1].disabled).toBe(true);
    expect((screen.getByRole('button', { name: '导入 0 个项目' }) as HTMLButtonElement).disabled).toBe(true);
    await user.click(screen.getByRole('button', { name: '全选当前结果' }));
    expect(inputs[0].checked).toBe(true); expect(inputs[1].checked).toBe(false);
  });
  it('取消请求后忽略迟到的成功结果', async () => {
    let finish!: (value: ScanResult) => void;
    api.scan.mockImplementation(() => new Promise<ScanResult>(resolve => { finish = resolve; }));
    const { user } = setup();
    await user.type(screen.getByLabelText('扫描根目录'), 'D:\\Code'); await user.click(screen.getByRole('button', { name: '开始扫描' }));
    await user.click(screen.getByRole('button', { name: '取消扫描' }));
    await act(async () => { finish(result); });
    expect(screen.queryByText('新项目')).toBeNull(); expect(screen.getByRole('alert').textContent).toContain('扫描已取消');
    expect(api.cancelScan).toHaveBeenCalledWith(expect.any(String));
  });
  it('选择新根目录后清除旧候选与勾选', async () => {
    const { user } = setup(); await scan(user);
    await user.click(screen.getByRole('button', { name: '全选当前结果' })); await user.click(screen.getByRole('button', { name: '选择目录' }));
    expect(screen.queryByText('新项目')).toBeNull(); expect((screen.getByLabelText('扫描根目录') as HTMLTextAreaElement).value).toContain('E:\\Other');
    expect((screen.getByRole('button', { name: '导入 0 个项目' }) as HTMLButtonElement).disabled).toBe(true);
  });
  it('仅导入用户勾选的候选并使用指定分类', async () => {
    api.importProjects.mockResolvedValue({ projects: [], skipped: [] });
    const { user, onImported } = setup(); await scan(user);
    await user.click(screen.getAllByRole('checkbox')[0]);
    await user.selectOptions(screen.getByLabelText(/统一导入分类/), 'ai');
    await user.click(screen.getByRole('button', { name: '导入 1 个项目' }));
    expect(api.importProjects).toHaveBeenCalledWith([expect.objectContaining({ name: '新项目', path: 'D:\\Code\\New', category: 'ai' })]);
    await waitFor(() => expect(onImported).toHaveBeenCalledTimes(1));
  });
  it('关闭扫描弹窗发出取消请求，迟到结果不回写界面', async () => {
    api.scan.mockImplementation(() => new Promise(() => {}));
    const { user } = setup();
    await user.type(screen.getByLabelText('扫描根目录'), 'D:\\Code'); await user.click(screen.getByRole('button', { name: '开始扫描' }));
    cleanup(); expect(api.cancelScan).toHaveBeenCalledWith(expect.any(String));
  });
});
