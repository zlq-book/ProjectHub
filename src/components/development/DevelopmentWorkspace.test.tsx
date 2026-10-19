// @vitest-environment jsdom
import { cleanup, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, expect, it, vi } from 'vitest';
import { defaultSettings, type Project } from '../../types';
import DevelopmentWorkspace from './DevelopmentWorkspace';
void defaultSettings;
const recordsApi = vi.hoisted(() => ({ native: false, spaces: vi.fn(), records: vi.fn() }));
vi.mock('../../services/projects', async original => ({ ...await original<typeof import('../../services/projects')>(), get native() { return recordsApi.native; } }));
vi.mock('../../services/development', () => ({ developmentApi: { status: vi.fn().mockRejectedValue(new Error('桌面 Git 不可用')), history: vi.fn(), activity: vi.fn().mockRejectedValue(new Error('桌面 Git 不可用')), spaces: recordsApi.spaces, records: recordsApi.records } }));
const project = { id: 'a', name: '真实项目', path: 'D:\\Work', updatedAt: '2026-10-16', available: true } as Project;
afterEach(() => { cleanup(); recordsApi.native = false; });
it('开发概览读取失败保持未知，仍能进入日志任务工作区', async () => {
  render(<DevelopmentWorkspace projects={[project]} version={0} onDiffOpen={() => {}} />);
  const user = userEvent.setup();
  await user.click(screen.getByRole('button', { name: '查看开发进度 真实项目' }));
  expect(await screen.findByText('桌面 Git 不可用')).toBeTruthy();
  expect(screen.getByRole('tab', { name: '开发日志' })).toBeTruthy();
  expect(screen.getByRole('tab', { name: '任务' })).toBeTruthy();
  expect(screen.getAllByRole('tab').map(tab => tab.textContent)).toEqual(['概览', '提交记录', '文件变更', '开发日志', '任务']);
  expect(screen.getByRole('tab', { name: '概览' }).getAttribute('aria-selected')).toBe('true');
  expect(screen.queryByText('工作区干净')).toBeNull();
});
it('同一项目的后续摘要草稿会重新进入日志，而不丢失第二份结果',async()=>{
  const consume=vi.fn();const props={projects:[project],version:0,onDiffOpen:()=>{},initialProjectId:'a',onDraftConsumed:consume,records:(_project:unknown,_kind:unknown,draft:{body:string}|null,clear:()=>void)=><div><span>{draft?.body||'无草稿'}</span><button onClick={clear}>已读取草稿</button></div>};
  const {rerender}=render(<DevelopmentWorkspace {...props} initialDraft={{title:'第一份',body:'首次摘要',commits:[]}}/>);const user=userEvent.setup();await screen.findByText('首次摘要');await user.click(screen.getByRole('button',{name:'已读取草稿'}));rerender(<DevelopmentWorkspace {...props} initialDraft={null}/>);await screen.findByText('无草稿');rerender(<DevelopmentWorkspace {...props} initialDraft={{title:'第二份',body:'第二次摘要',commits:[]}}/>);expect(await screen.findByText('第二次摘要')).toBeTruthy();expect(consume).toHaveBeenCalledTimes(2);
});
it('修改任务后回到概览和手动刷新均重新读取任务进度', async () => {
  recordsApi.native = true;
  recordsApi.spaces.mockResolvedValue([{ id: 'space', projectId: 'a' }]);
  let done = false;
  recordsApi.records.mockImplementation(async () => ({ tasks: [{ status: done ? 'done' : 'todo' }] }));
  render(<DevelopmentWorkspace projects={[project]} version={0} initialProjectId="a" onDiffOpen={() => {}} records={() => <button onClick={() => { done = true; }}>完成任务</button>} />);
  const user = userEvent.setup();
  await screen.findByText('0 / 1');
  await user.click(screen.getByRole('tab', { name: '任务' }));
  await user.click(screen.getByRole('button', { name: '完成任务' }));
  await user.click(screen.getByRole('tab', { name: '概览' }));
  await screen.findByText('1 / 1');
  done = false;
  await user.click(screen.getByRole('button', { name: '刷新开发状态' }));
  expect(await screen.findByText('0 / 1')).toBeTruthy();
});
it('提交定位入口直接选择提交记录，返回后恢复开发状态中心', async () => {
  render(<DevelopmentWorkspace projects={[project]} version={0} initialProjectId="a" initialCommitSha="sha" onDiffOpen={() => {}} />);
  expect(screen.getByRole('tab', { name: '提交记录' }).getAttribute('aria-selected')).toBe('true');
  await userEvent.setup().click(screen.getByRole('button', { name: '开发状态中心' }));
  expect(await screen.findByRole('heading', { name: '开发状态中心' })).toBeTruthy();
});
