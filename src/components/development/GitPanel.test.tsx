// @vitest-environment jsdom
import { cleanup, render, screen, act, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import GitPanel from './GitPanel';
import type { GitStatus } from '../../types/development';
const api = vi.hoisted(() => ({ history: vi.fn(), files: vi.fn(), diff: vi.fn() }));
vi.mock('../../services/development', () => ({ developmentApi: api }));
const status: GitStatus = { repository:true, repositoryId:'r',branch:'main',head:'abc',readAt:'2026-10-16T00:00:00Z',complete:true,warning:null,latestCommitAt:null,shallow:false,files:[{path:'new.txt', indexStatus:'?',worktreeStatus:'?',untracked:true,conflicted:false},{path:'tracked.txt',indexStatus:'.',worktreeStatus:'M',untracked:false,conflicted:false}] };
const onDiffOpen=vi.fn(),onSummary=vi.fn(),onLog=vi.fn();
beforeEach(()=>{vi.clearAllMocks();api.diff.mockResolvedValue({text:'test',binary:false,truncated:false,warning:null});});
afterEach(cleanup);
it('助手传入真实提交ID时定位历史并懒加载该提交文件',async()=>{
 const sha='a'.repeat(40);api.history.mockResolvedValue({head:sha,commits:[{sha,subject:'定位到的真实提交',message:'说明',author:'作者',authorTime:'2026-10-16',commitTime:'2026-10-16'}],nextCursor:null,complete:true,readAt:'2026-10-16',warning:null});api.files.mockResolvedValue({head:sha,sha,files:[],complete:true,warning:null,firstParent:null});render(<GitPanel projectId="p" status={{...status,head:sha}} historyMode initialCommitSha={sha} onDiffOpen={onDiffOpen} onSummary={onSummary} onLog={onLog}/>);await waitFor(()=>expect(api.files).toHaveBeenCalledWith('p',sha));expect(await screen.findByRole('heading',{name:'定位到的真实提交'})).toBeTruthy();
});
it('未跟踪预览不会污染已跟踪文件比较范围',async()=>{
 render(<GitPanel projectId="p" status={status} historyMode={false} onDiffOpen={onDiffOpen} onSummary={onSummary} onLog={onLog}/>);const user=userEvent.setup();
 await user.click(screen.getByRole('button',{name:'预览文本'}));
 await waitFor(()=>expect(api.diff).toHaveBeenCalledWith('p','new.txt','untracked',null));
 expect((screen.getByLabelText('变更比较范围') as HTMLSelectElement).value).toBe('worktree');
 await user.click(screen.getByRole('button',{name:'查看差异'}));
 await waitFor(()=>expect(api.diff).toHaveBeenCalledWith('p','tracked.txt','worktree',null));
});
it('历史加载中切到工作区后会结束忙碌状态并忽略旧结果',async()=>{
 let resolve!: (value:unknown)=>void;api.history.mockReturnValue(new Promise(r=>{resolve=r;}));
 const props={projectId:'p',status,onDiffOpen,onSummary,onLog};
 const {rerender}=render(<GitPanel {...props} historyMode/>);
 expect(screen.getByRole('status').textContent).toBe('读取中…');
 rerender(<GitPanel {...props} historyMode={false}/>);
 await act(async()=>resolve({head:'abc',commits:[],nextCursor:null,complete:true,readAt:'now',warning:null}));
 expect(screen.queryByRole('status')).toBeNull();
});
