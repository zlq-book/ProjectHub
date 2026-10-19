// @vitest-environment jsdom
import { cleanup, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, it, expect, vi } from 'vitest';
import Workspace from './WeeklyReportWorkspace';
it('直接切换版本前保存修改，保存失败则停留原版本',async()=>{
 const user=userEvent.setup();render(<Workspace projects={[project]}/>);await collect(user);await generate(user);
 await user.clear(screen.getByLabelText('成果 1'));await user.type(screen.getByLabelText('成果 1'),'旧稿内容');
 await user.click(screen.getByRole('button',{name:'重新生成新版本'}));await screen.findByText('原稿已保存。重新汇总并生成后会创建新版本，不覆盖原稿。');
 await user.click(screen.getByRole('button',{name:'汇总提交与开发日志'}));await screen.findByRole('heading',{name:'本次材料'});mock.generate.mockClear();await generate(user);
 await user.clear(screen.getByLabelText('成果 1'));await user.type(screen.getByLabelText('成果 1'),'新版人工修订');
 mock.save.mockRejectedValueOnce(new Error('保存失败，请重试'));
 await user.selectOptions(screen.getByRole('combobox',{name:'周报版本'}),'v1');
 await screen.findByText('保存失败，请重试');expect((screen.getByRole('combobox',{name:'周报版本'}) as HTMLSelectElement).value).toBe('v2');
 expect((screen.getByLabelText('成果 1') as HTMLTextAreaElement).value).toBe('新版人工修订');
 await user.selectOptions(screen.getByRole('combobox',{name:'周报版本'}),'v1');
 await waitFor(()=>expect((screen.getByLabelText('成果 1') as HTMLTextAreaElement).value).toBe('旧稿内容'));
 expect(stored!.versions[1].content.outcomes[0].text).toBe('新版人工修订');
});

it('筛选周报项目不会丢失已选项目，清空关键词恢复列表',async()=>{
 const user=userEvent.setup();render(<Workspace projects={[project,{...project,id:'p2',name:'项目乙'}]}/>);
 await waitFor(()=>expect((screen.getByRole('button',{name:'新建周报'}) as HTMLButtonElement).disabled).toBe(false));
 await user.click(screen.getByLabelText('项目甲'));
 await user.type(screen.getByRole('searchbox',{name:'筛选周报项目'}),'乙');
 expect(screen.queryByLabelText('项目甲')).toBeNull();expect(screen.getByLabelText('项目乙')).toBeTruthy();
 await user.clear(screen.getByRole('searchbox',{name:'筛选周报项目'}));
 expect((screen.getByLabelText('项目甲') as HTMLInputElement).checked).toBe(true);
});
it('编辑周报时 Ctrl+S 保存当前人工修改',async()=>{
 const user=userEvent.setup();render(<Workspace projects={[project]}/>);await collect(user);await generate(user);
 await user.clear(screen.getByLabelText('成果 1'));await user.type(screen.getByLabelText('成果 1'),'快捷键保存的内容');
 await user.keyboard('{Control>}s{/Control}');
 await waitFor(()=>expect(stored!.versions[0].content.outcomes[0].text).toBe('快捷键保存的内容'));
});
import { defaultRange } from '../../services/weeklyReport';
import type { ReportGroup, ReportSource } from '../../types/weeklyReport';
import type { Project } from '../../types';
const mock = vi.hoisted(() => ({ get:vi.fn(), preferences: vi.fn(), list: vi.fn(), savePreferences: vi.fn(), collect: vi.fn(), cancelCollection: vi.fn(), preview: vi.fn(), generate: vi.fn(), cancel: vi.fn(), save: vi.fn(), adopt: vi.fn(), export: vi.fn() }));
vi.mock('../../services/weeklyReport', async (original) => ({ ...await original<typeof import('../../services/weeklyReport')>(), weeklyReportApi: mock }));
vi.mock('../../services/projects', async (original) => ({ ...await original<typeof import('../../services/projects')>(), native: true }));
const project = { id: 'p1', name: '项目甲' } as Project;
const source: ReportSource = { id: 's1', kind: 'log', projectIds: ['p1'], projectName: '项目甲', title: '接口开发记录', body: '调整查询接口', repositoryId: null, sha: null, author: '', authorEmail: '', authorTime: '', commitTime: '', logId: 'log1', revision: 1, files: [] };
let stored: ReportGroup | null;
beforeEach(() => { vi.resetAllMocks(); stored = null; HTMLDialogElement.prototype.showModal = function () { this.setAttribute('open', ''); }; HTMLDialogElement.prototype.close = function () { this.removeAttribute('open'); }; mock.preferences.mockResolvedValue({ department: '技术部', reporter: '本人', authors: [{ kind: 'email', value: 'me@example.com' }] }); mock.list.mockResolvedValue([]);mock.get.mockImplementation(async()=>structuredClone(stored)); mock.savePreferences.mockResolvedValue(undefined); mock.collect.mockImplementation(async (input) => ({ id: input.requestId, range: defaultRange(), authors: input.authors, projects: [], sources: [source], warnings: [] })); mock.preview.mockResolvedValue({ token: 't', model: 'mock', baseUrl: 'http://localhost/v1', text: '完整请求材料', bytes: 100, expiresAt: new Date(Date.now() + 60000).toISOString(), warnings: [] }); mock.generate.mockResolvedValue({ outcomes: [{ id: 'o1', projectIds: ['p1'], text: '项目甲—调整接口', sourceIds: ['s1'], origin: 'ai' }], difficulties: [], plans: [], reflection: '' }); mock.save.mockImplementation(async (input) => { const id = input.versionId ?? `v${(stored?.versions.length ?? 0) + 1}`; const version = { id, reportId: 'g', parentVersionId: input.parentVersionId, revision: (input.revision ?? 0) + 1, content: structuredClone(input.content), sources: input.sources, hints: input.hints, createdAt: new Date().toISOString(), updatedAt: new Date().toISOString() }; if (!stored)
    stored = { id: 'g', adoptedVersionId: id, versions: [], createdAt: new Date().toISOString() }; const index = stored.versions.findIndex(v => v.id === id); if (index >= 0)
    stored.versions[index] = version;
else
    stored.versions.push(version); return structuredClone(stored); }); mock.cancel.mockResolvedValue(undefined); mock.cancelCollection.mockResolvedValue(undefined); });
afterEach(cleanup);
it('生成失败后保留材料与要点，并通过新预览重试',async()=>{
 const user=userEvent.setup();render(<Workspace projects={[project]}/>);await collect(user);
 await user.type(screen.getByLabelText('下周计划'),'继续联调');
 mock.generate.mockRejectedValueOnce(new Error('模型输出被截断（finish_reason=length），请减少材料后重新预览'));
 await user.click(screen.getByRole('button',{name:'预览发送内容'}));await screen.findByText('完整请求材料');
 await user.click(screen.getByRole('button',{name:'确认发送并生成'}));
 await waitFor(()=>expect(screen.queryByRole('dialog',{name:'确认发送周报材料'})).toBeNull());
 expect(screen.getByText(/模型输出被截断/)).toBeTruthy();
 expect((screen.getByLabelText('下周计划') as HTMLTextAreaElement).value).toBe('继续联调');
 expect(stored).toBeNull();
 mock.preview.mockResolvedValueOnce({token:'fresh',model:'mock',baseUrl:'http://localhost/v1',text:'新的完整请求材料',bytes:100,expiresAt:new Date(Date.now()+60000).toISOString(),warnings:[]});
 await user.click(screen.getByRole('button',{name:'预览发送内容'}));await screen.findByText('新的完整请求材料');
 expect(mock.generate).toHaveBeenCalledTimes(1);
 await user.click(screen.getByRole('button',{name:'确认发送并生成'}));await screen.findByRole('heading',{name:'候选工作成果'});
 expect(mock.generate).toHaveBeenLastCalledWith('fresh');
});
it('保存生成结果期间禁止取消，完成后保留可编辑结果',async()=>{
 const user=userEvent.setup();render(<Workspace projects={[project]}/>);await collect(user);
 let release!:()=>void;const original=mock.save.getMockImplementation()!;mock.save.mockImplementation(async input=>{await new Promise<void>(r=>{release=r;});return original(input);});
 await user.click(screen.getByRole('button',{name:'预览发送内容'}));await screen.findByText('完整请求材料');await user.click(screen.getByRole('button',{name:'确认发送并生成'}));
 await waitFor(()=>expect((screen.getByRole('button',{name:/^取消$/}) as HTMLButtonElement).disabled).toBe(true));
 expect((screen.getByRole('button',{name:'关闭弹窗'}) as HTMLButtonElement).disabled).toBe(true);
 release();await screen.findByRole('heading',{name:'候选工作成果'});expect(screen.getByLabelText('成果 1')).toBeTruthy();
});
async function collect(user: ReturnType<typeof userEvent.setup>) { await waitFor(() => expect((screen.getByRole('button', { name: '新建周报' }) as HTMLButtonElement).disabled).toBe(false)); await user.click(screen.getByLabelText('项目甲')); await user.click(screen.getByRole('button', { name: '汇总提交与开发日志' })); await screen.findByRole('heading', { name: '本次材料' }); }
async function generate(user: ReturnType<typeof userEvent.setup>) { await user.click(screen.getByRole('button', { name: '预览发送内容' })); await screen.findByText('完整请求材料'); expect(mock.generate).not.toHaveBeenCalled(); await user.click(screen.getByRole('button', { name: '确认发送并生成' })); await screen.findByRole('heading', { name: '候选工作成果' }); }
it('预览确认后才生成，重新生成保存原稿并追加未采用版本', async () => { const user = userEvent.setup(); render(<Workspace projects={[project]}/>); await collect(user); await generate(user); expect(mock.preview.mock.calls[0][0].diffs).toEqual([]); await user.clear(screen.getByLabelText('成果 1')); await user.type(screen.getByLabelText('成果 1'), '人工修改的原稿'); await user.click(screen.getByRole('button', { name: '重新生成新版本' })); await screen.findByText('原稿已保存。重新汇总并生成后会创建新版本，不覆盖原稿。'); expect(stored!.versions[0].content.outcomes[0].text).toBe('人工修改的原稿'); await user.click(screen.getByRole('button', { name: '汇总提交与开发日志' })); await screen.findByRole('heading', { name: '本次材料' }); mock.generate.mockClear(); await generate(user); expect(stored!.versions).toHaveLength(2); expect(stored!.adoptedVersionId).toBe('v1'); expect(stored!.versions[0].content.outcomes[0].text).toBe('人工修改的原稿'); expect(screen.getByRole('button', { name: '采用此版本' })).toBeTruthy(); });
it('取消汇总后晚到材料不覆盖当前视图', async () => { const user = userEvent.setup(); let resolve!: (v: unknown) => void; mock.collect.mockImplementation(() => new Promise(r => { resolve = r; })); render(<Workspace projects={[project]}/>); await waitFor(() => expect((screen.getByRole('button', { name: '新建周报' }) as HTMLButtonElement).disabled).toBe(false)); await user.click(screen.getByLabelText('项目甲')); await user.click(screen.getByRole('button', { name: '汇总提交与开发日志' })); await screen.findByRole('button', { name: '取消本次操作' }); await user.click(screen.getByRole('button', { name: '取消本次操作' })); resolve({ id: 'late', sources: [source], projects: [], warnings: [] }); await waitFor(() => expect(mock.cancelCollection).toHaveBeenCalled()); expect(screen.queryByRole('heading', { name: '本次材料' })).toBeNull(); });
