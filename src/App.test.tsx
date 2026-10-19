// @vitest-environment jsdom
import { act, cleanup, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { defaultSettings, type Project } from './types';
import { previewRepository } from './services/projects';
import App from './App';
it('收藏视图清除筛选后再次进入不会恢复已清除的旧条件',async()=>{
 const user=userEvent.setup();render(<App/>);await screen.findByRole('table',{name:'项目列表'});
 await user.click(screen.getByRole('button',{name:'收藏项目'}));await user.type(screen.getByRole('textbox',{name:'搜索项目'}),'不存在');
 await user.click(screen.getByRole('button',{name:'全部项目'}));await user.click(screen.getByRole('button',{name:'收藏项目'}));
 await user.click(screen.getByRole('button',{name:'清除筛选'}));await user.click(screen.getByRole('button',{name:'收藏项目'}));
 expect((screen.getByRole('textbox',{name:'搜索项目'}) as HTMLInputElement).value).toBe('');
});
it('从项目详情进入开发中心后返回仍保留项目检索',async()=>{
 const user=userEvent.setup();render(<App/>);await screen.findByRole('table',{name:'项目列表'});
 await user.click(screen.getByRole('button',{name:'全部项目'}));await user.type(screen.getByRole('textbox',{name:'搜索项目'}),'项目甲');
 await user.click(screen.getByRole('button',{name:'查看 项目甲'}));await user.click(screen.getByRole('button',{name:'查看开发进度'}));
 await user.click(screen.getByRole('button',{name:'全部项目'}));
 expect((screen.getByRole('textbox',{name:'搜索项目'}) as HTMLInputElement).value).toBe('项目甲');
});
it('空状态清除筛选确实恢复项目列表',async()=>{
 const user=userEvent.setup();render(<App/>);await screen.findByRole('table',{name:'项目列表'});
 await user.click(screen.getByRole('button',{name:'全部项目'}));await user.type(screen.getByRole('textbox',{name:'搜索项目'}),'完全不存在');
 await user.click(screen.getByRole('button',{name:'清除筛选'}));
 expect((screen.getByRole('textbox',{name:'搜索项目'}) as HTMLInputElement).value).toBe('');expect(screen.getByRole('table',{name:'项目列表'})).toBeTruthy();
});
it('修改API配置不会重建并清空周报编辑状态',async()=>{
 const user=userEvent.setup();render(<App/>);await screen.findByRole('table',{name:'项目列表'});
 await user.click(screen.getByRole('button',{name:'AI 智能周报'}));await user.click(screen.getByRole('tab',{name:'编辑周报'}));
 await user.click(screen.getByRole('button',{name:'新增成果'}));await user.type(screen.getByLabelText('成果 1'),'必须保留的草稿');
 await user.click(screen.getByRole('button',{name:'配置 API'}));await screen.findByRole('dialog',{name:'配置 API'});
 await user.click(screen.getByRole('button',{name:'保存 API 配置'}));
 await waitFor(()=>expect(screen.queryByRole('dialog',{name:'配置 API'})).toBeNull());
 expect((screen.getByLabelText('成果 1') as HTMLTextAreaElement).value).toBe('必须保留的草稿');
});

it('首页提供工作入口，功能页不展示无效项目搜索，Ctrl+K 回到项目检索', async () => {
 const user=userEvent.setup();render(<App/>);await screen.findByRole('table',{name:'项目列表'});
 expect(screen.getByRole('heading',{name:'继续工作'})).toBeTruthy();
 await user.click(screen.getByRole('button',{name:'AI 智能周报'}));
 expect(screen.queryByRole('textbox',{name:'搜索项目'})).toBeNull();
 expect(screen.queryByRole('button',{name:'添加项目'})).toBeNull();
 await user.keyboard('{Control>}k{/Control}');
 await waitFor(()=>expect(document.activeElement).toBe(screen.getByRole('textbox',{name:'搜索项目'})));
});
it('项目搜索跨周报导航后恢复，不默默清空筛选',async()=>{
 const user=userEvent.setup();render(<App/>);await screen.findByRole('table',{name:'项目列表'});
 await user.click(screen.getByRole('button',{name:/^全部项目/}));
 await user.type(screen.getByRole('textbox',{name:'搜索项目'}),'项目甲');
 await user.click(screen.getByRole('button',{name:'AI 智能周报'}));
 await user.click(screen.getByRole('button',{name:/^全部项目/}));
 expect((screen.getByRole('textbox',{name:'搜索项目'}) as HTMLInputElement).value).toBe('项目甲');
 expect(screen.queryByRole('button',{name:'查看 项目乙'})).toBeNull();
});

const api = vi.hoisted(() => ({ list: vi.fn(), settings: vi.fn(), changeTag: vi.fn(), saveSettings: vi.fn(), open: vi.fn(), gitInfo: vi.fn(), aiSettings: vi.fn(), saveAiSettings:vi.fn(), native: false }));
const assistant = vi.hoisted(() => ({ run: vi.fn(), apply: vi.fn(), cancel: vi.fn(), clear: vi.fn(), settings: vi.fn() }));
vi.mock('./services/assistant', () => ({ assistantApi: assistant }));
vi.mock('./services/projects', async importOriginal => ({ ...await importOriginal<typeof import('./services/projects')>(), get native() { return api.native; }, repository: api }));
const project: Project = { id: 'a', name: '项目甲', path: 'D:\\Alpha', description: '', category: 'other', tags: ['旧标签'], techStack: ['旧标签'],
  favorite: false, notes: '', defaultIde: 'system', createdAt: '2026-10-13', updatedAt: '2026-10-13', lastOpenedAt: null, available: true };
beforeEach(() => {
  vi.resetAllMocks();
  api.native = false;
  assistant.settings.mockResolvedValue({ baseUrl: 'https://example.com/v1', model: 'test-model', hasApiKey: false }); assistant.clear.mockResolvedValue(undefined);
  const repo = previewRepository([project, { ...project, id: 'b', name: '项目乙', path: 'D:\\Beta', tags: [] }]);
  api.list.mockImplementation(repo.list); api.changeTag.mockImplementation(repo.changeTag); api.settings.mockResolvedValue(defaultSettings);
  api.gitInfo.mockResolvedValue({ repository: false, branch: null, remote: null });
  api.saveAiSettings.mockResolvedValue(undefined);
  api.aiSettings.mockResolvedValue({ baseUrl: '', model: '', hasApiKey: false });
  vi.stubGlobal('matchMedia', () => ({ matches: false, addEventListener() {}, removeEventListener() {} }));
  HTMLDialogElement.prototype.showModal = function () { this.setAttribute('open', ''); };
  HTMLDialogElement.prototype.close = function () { this.removeAttribute('open'); };
});
afterEach(() => { cleanup(); vi.unstubAllGlobals(); });
it('空项目库也能打开 AI 助手，关闭后恢复项目列表', async () => {
  api.list.mockResolvedValue([]);
  render(<App />); const user = userEvent.setup();
  await screen.findByText('你的项目工作台，从这里开始');
  await user.click(screen.getByRole('button', { name: '打开 AI 助手' }));
  expect(screen.getByRole('complementary', { name: 'AI 助手' })).toBeTruthy();
  expect(document.querySelector('.app-shell.with-detail')).toBeTruthy();
  expect((screen.getByRole('tab', { name: '项目详情' }) as HTMLButtonElement).disabled).toBe(true);
  await user.click(screen.getByRole('button', { name: '收起 AI 助手' }));
  expect(screen.queryByRole('complementary', { name: 'AI 助手' })).toBeNull();
  expect(document.querySelector('.app-shell.with-detail')).toBeNull();
});
it('助手和项目详情共用右栏，切换后保留助手草稿', async () => {
  render(<App />); const user = userEvent.setup();
  await user.click(await screen.findByRole('button', { name: '查看 项目甲' }));
  await user.click(screen.getByRole('button', { name: '打开 AI 助手' }));
  await user.type(screen.getByRole('textbox', { name: '发送给 AI 助手' }), '查找空气质量预警项目');
  expect(screen.queryByRole('complementary', { name: '项目详情' })).toBeNull();
  await user.click(screen.getByRole('tab', { name: '项目详情' }));
  expect(screen.getByRole('complementary', { name: '项目详情' })).toBeTruthy();
  await user.click(screen.getByRole('tab', { name: 'AI 助手' }));
  expect((screen.getByRole('textbox', { name: '发送给 AI 助手' }) as HTMLTextAreaElement).value).toBe('查找空气质量预警项目');
});
it('取消 API 配置窗口保留助手草稿', async () => {
  render(<App />); const user = userEvent.setup();
  await screen.findByRole('table', { name: '项目列表' });
  await user.click(screen.getByRole('button', { name: '打开 AI 助手' }));
  await user.type(screen.getByRole('textbox', { name: '发送给 AI 助手' }), '继续整理');
  await user.click(screen.getByRole('button', { name: '配置 API' }));
  await screen.findByRole('dialog', { name: '配置 API' });
  await user.click(screen.getByRole('button', { name: '取消' }));
  expect((screen.getByRole('textbox', { name: '发送给 AI 助手' }) as HTMLTextAreaElement).value).toBe('继续整理');
});
it('助手提交期间阻止打开完整项目编辑表单，结束后恢复', async () => {
  api.native = true;
  let finish!: (value: Project[]) => void;
  assistant.apply.mockImplementation(() => new Promise(resolve => { finish = resolve; }));
  assistant.run.mockResolvedValue({ kind: 'description', text: '建议已生成', proposalId: 'p', toolSteps: [], scopeCount: 1, items: [{ project, description: '新简介', certainty: 'confirmed', evidence: [] }] });
  render(<App />); const user = userEvent.setup();
  await screen.findByRole('table', { name: '项目列表' });
  await user.click(screen.getByRole('button', { name: '打开 AI 助手' }));
  await user.click(await screen.findByRole('button', { name: '开始使用 AI 助手' }));
  await user.type(screen.getByRole('textbox', { name: '发送给 AI 助手' }), '生成简介');
  await user.click(screen.getByRole('button', { name: '发送' }));
  await user.click(await screen.findByRole('button', { name: '保存简介' }));
  expect((screen.getByRole('button', { name: '添加项目' }) as HTMLButtonElement).disabled).toBe(true);
  await user.click(screen.getByRole('tab', { name: '项目详情' }));
  await user.click(screen.getByRole('button', { name: '编辑项目简介' }));
  expect(screen.queryByRole('dialog')).toBeNull();
  await act(async () => finish([{ ...project, description: '新简介' }]));
  expect((screen.getByRole('button', { name: '添加项目' }) as HTMLButtonElement).disabled).toBe(false);
});
it('左侧配置 API 入口打开独立配置窗口', async () => {
  render(<App />); const user = userEvent.setup();
  await user.click(screen.getByRole('button', { name: '配置 API' }));
  expect(await screen.findByRole('dialog', { name: '配置 API' })).toBeTruthy();
  expect(screen.getByLabelText('API Key')).toBeTruthy();
});
it('默认表格显示五列，收起详情保留选中；双击使用默认 IDE', async () => {
  expect(defaultSettings.view).toBe('list');
  render(<App />); const user = userEvent.setup();
  const table = await screen.findByRole('table', { name: '项目列表' });
  expect(table.querySelectorAll('th')).toHaveLength(5);
  const item = screen.getByRole('button', { name: '查看 项目甲' });
  await user.click(item);
  expect(api.open).not.toHaveBeenCalled();
  await user.click(screen.getByRole('button', { name: '收起项目详情' }));
  expect(item.getAttribute('aria-pressed')).toBe('true');
  expect(screen.queryByRole('complementary', { name: '项目详情' })).toBeNull();
  await user.dblClick(item);
  await waitFor(() => expect(api.open).toHaveBeenCalledWith('a', 'system'));
});
it('可切换卡片并保存偏好，批量模式下双击不启动项目', async () => {
  render(<App />); const user = userEvent.setup();
  await screen.findByRole('table', { name: '项目列表' });
  await user.click(screen.getByRole('button', { name: '卡片视图' }));
  expect(api.saveSettings).toHaveBeenCalledWith(expect.objectContaining({ view: 'grid' }));
  expect(screen.queryByRole('table')).toBeNull();
  await user.click(screen.getByRole('button', { name: '列表视图' }));
  await user.click(screen.getByRole('button', { name: '批量管理' }));
  await user.click(screen.getByRole('checkbox', { name: '选择 项目甲' }));
  expect((screen.getByRole('checkbox', { name: '选择 项目甲' }) as HTMLInputElement).checked).toBe(true);
  await user.dblClick(screen.getByRole('button', { name: '切换选择 项目乙' }));
  expect(api.open).not.toHaveBeenCalled();
});
it('保留已保存的卡片偏好，键盘选择项目后可展开详情', async () => {
  api.settings.mockResolvedValue({ ...defaultSettings, view: 'grid' });
  render(<App />); const user = userEvent.setup();
  const item = await screen.findByRole('button', { name: '查看 项目甲' });
  expect(screen.queryByRole('table')).toBeNull();
  item.focus(); await user.keyboard('{Enter}');
  expect(screen.getByRole('complementary', { name: '项目详情' })).toBeTruthy();
  expect(api.open).not.toHaveBeenCalled();
});
async function rename(user: ReturnType<typeof userEvent.setup>) {
  await user.click(screen.getByRole('button', { name: '标签管理' }));
  await user.click(screen.getByRole('button', { name: '重命名标签 旧标签' }));
  await user.clear(screen.getByLabelText('新标签名称')); await user.type(screen.getByLabelText('新标签名称'), '新标签');
  await user.click(screen.getByRole('button', { name: '确认重命名' }));
  await screen.findByRole('button', { name: '重命名标签 新标签' });
  await user.click(screen.getByRole('button', { name: '关闭弹窗' }));
}
it('详情产生的标签筛选随重命名同步，项目保持可见', async () => {
  render(<App />); const user = userEvent.setup();
  await user.click(await screen.findByRole('button', { name: '查看 项目甲' }));
  await user.click(screen.getAllByRole('button', { name: '旧标签' })[0]);
  await rename(user);
  expect(screen.getByLabelText('当前筛选条件').textContent).toContain('标签：新标签');
  await waitFor(() => expect(screen.getAllByRole('button', { name: /^查看 项目/ })).toHaveLength(1));
});
it('明确的技术栈筛选在同名自定义标签改名后保持原值', async () => {
  render(<App />); const user = userEvent.setup();
  await screen.findByRole('button', { name: '查看 项目甲' });
  await user.click(screen.getByRole('button', { name: '标签管理' }));
  await user.click(screen.getByRole('button', { name: '技术栈' }));
  await user.click(screen.getByRole('button', { name: '筛选技术栈 旧标签：2 个项目' }));
  await rename(user);
  expect(screen.getByLabelText('当前筛选条件').textContent).toContain('技术栈：旧标签');
  expect(screen.getAllByRole('button', { name: /^查看 项目/ })).toHaveLength(2);
});
it('移除当前自定义标签时清空筛选并保留所有项目', async () => {
  render(<App />); const user = userEvent.setup();
  await screen.findByRole('button', { name: '查看 项目甲' });
  await user.click(screen.getByRole('button', { name: '标签管理' }));
  await user.click(screen.getByRole('button', { name: '筛选标签 旧标签：1 个项目' }));
  expect(screen.getAllByRole('button', { name: /^查看 项目/ })).toHaveLength(1);
  await user.click(screen.getByRole('button', { name: '标签管理' }));
  await user.click(screen.getByRole('button', { name: '移除标签 旧标签' }));
  await user.click(screen.getByRole('button', { name: '确认移除标签' }));
  await screen.findByText(/暂无标签/);
  await user.click(screen.getByRole('button', { name: '关闭弹窗' }));
  expect(screen.queryByLabelText('当前筛选条件')).toBeNull();
  expect(screen.getAllByRole('button', { name: /^查看 项目/ })).toHaveLength(2);
});
