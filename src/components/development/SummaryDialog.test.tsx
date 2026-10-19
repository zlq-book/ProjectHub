// @vitest-environment jsdom
import { act, cleanup, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import SummaryDialog from './SummaryDialog';
import type { Project } from '../../types';
const api=vi.hoisted(()=>({summaryPreview:vi.fn(),summaryGenerate:vi.fn(),summaryCancel:vi.fn().mockResolvedValue(undefined)}));
vi.mock('../../services/development',()=>({developmentApi:api}));
afterEach(cleanup);
beforeEach(()=>{vi.clearAllMocks();api.summaryCancel.mockResolvedValue(undefined);HTMLDialogElement.prototype.showModal=function(){this.setAttribute('open','');};HTMLDialogElement.prototype.close=function(){this.removeAttribute('open');};});
it('预览不发送，只有独立确认后生成，结果仍需手动转日志草稿',async()=>{
  HTMLDialogElement.prototype.showModal=function(){this.setAttribute('open','');};HTMLDialogElement.prototype.close=function(){this.removeAttribute('open');};
  api.summaryPreview.mockResolvedValue({token:'token',model:'local-model',baseUrl:'http://127.0.0.1',text:'完整发送请求',bytes:100,files:['src/main.rs'],excluded:['.env']});api.summaryGenerate.mockResolvedValue('主要变更\n依据 src/main.rs');const save=vi.fn();
  render(<SummaryDialog project={{id:'p',name:'项目'} as Project} selection={{sha:null,stage:'worktree',files:['src/main.rs'],title:'变更'}} onClose={()=>{}} onSave={save}/>);const user=userEvent.setup();
  await screen.findByText('完整发送请求');expect(api.summaryGenerate).not.toHaveBeenCalled();
  await user.click(screen.getByRole('button',{name:'确认发送并生成'}));expect((await screen.findByLabelText('摘要正文') as HTMLTextAreaElement).value).toBe('主要变更\n依据 src/main.rs');expect(save).not.toHaveBeenCalled();
  await user.clear(screen.getByLabelText('摘要正文'));expect(screen.getByLabelText('摘要正文')).toBeTruthy();expect((screen.getByRole('button',{name:'编辑并保存为开发日志'}) as HTMLButtonElement).disabled).toBe(true);await user.type(screen.getByLabelText('摘要正文'),'主要变更\n依据 src/main.rs');
  await user.click(screen.getByRole('button',{name:'编辑并保存为开发日志'}));expect(save).toHaveBeenCalledWith(expect.objectContaining({body:'主要变更\n依据 src/main.rs'}));
});
it('范围切换和关闭后释放已取得及晚到的预览令牌',async()=>{
  let late!:(value:unknown)=>void;
  api.summaryPreview.mockResolvedValueOnce({token:'first',model:'mock',baseUrl:'http://127.0.0.1',text:'首次预览',bytes:10,files:['a','b'],excluded:[]}).mockImplementationOnce(()=>new Promise(resolve=>{late=resolve;}));
  const {unmount}=render(<SummaryDialog project={{id:'p',name:'项目'} as Project} selection={{sha:null,stage:'index',files:['a','b'],title:'变更'}} onClose={()=>{}} onSave={()=>{}}/>);const user=userEvent.setup();await screen.findByText('首次预览');await user.click(screen.getByRole('checkbox',{name:'b'}));await waitFor(()=>expect(api.summaryCancel).toHaveBeenCalledWith('first'));unmount();await act(async()=>late({token:'late',model:'mock',baseUrl:'http://127.0.0.1',text:'晚到预览',bytes:10,files:['a'],excluded:[]}));expect(api.summaryCancel).toHaveBeenCalledWith('late');
});
it('慢模型生成可取消，晚到结果不保存为日志',async()=>{
  let finish!:(value:string)=>void;api.summaryPreview.mockResolvedValue({token:'running',model:'mock',baseUrl:'http://127.0.0.1',text:'发送预览',bytes:10,files:['a'],excluded:[]});api.summaryGenerate.mockImplementation(()=>new Promise(resolve=>{finish=resolve;}));const close=vi.fn(),save=vi.fn();const {unmount}=render(<SummaryDialog project={{id:'p',name:'项目'} as Project} selection={{sha:null,stage:'worktree',files:['a'],title:'变更'}} onClose={close} onSave={save}/>);const user=userEvent.setup();await screen.findByText('发送预览');await user.click(screen.getByRole('button',{name:'确认发送并生成'}));await user.click(screen.getByRole('button',{name:'取消本次摘要'}));expect(close).toHaveBeenCalled();expect(api.summaryCancel).toHaveBeenCalledWith('running');unmount();await act(async()=>finish('晚到摘要'));expect(save).not.toHaveBeenCalled();
});
