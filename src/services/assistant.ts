import { invoke } from '@tauri-apps/api/core';
import { native, repository } from './projects';
import type { Project } from '../types';
import type { AssistantApply, AssistantRequest, AssistantResult } from '../types/assistant';

function desktop() { if (!native) throw new Error('AI 助手需要桌面版，浏览器不会发送项目资料或密钥。'); }
export const assistantApi = {
  settings: () => repository.aiSettings(),
  run(input: AssistantRequest): Promise<AssistantResult> { desktop(); return invoke('assistant_run', { input }); },
  apply(input: AssistantApply): Promise<Project[]> { desktop(); return invoke('apply_assistant_proposal', { input }); },
  cancel(sessionId: string, requestId: string): Promise<void> { return native ? invoke('cancel_assistant', { sessionId, requestId }) : Promise.resolve(); },
  clear(sessionId: string): Promise<void> { return native ? invoke('clear_assistant', { sessionId }) : Promise.resolve(); },
};
