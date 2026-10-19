import { invoke } from '@tauri-apps/api/core';
import { native } from './projects';
import type { CommitFiles, DiffStage, GitActivity, GitDiff, GitHistory, GitStatus } from '../types/development';
import type { DevelopmentRecords, DevelopmentSpace, LogInput, TaskInput, SummaryPreview } from '../types/development';

function desktop<T>(command: string, args: Record<string, unknown>): Promise<T> {
  if (!native) return Promise.reject(new Error('请在 ProjectHub 桌面版中读取本地 Git 和开发记录；浏览器预览不生成示例数据。'));
  return invoke<T>(command, args);
}
// 同时最多两个可见项目读取任务；后端也限制真实 Git 进程并发。
let active = 0;
const queue: (() => void)[] = [];
async function scheduled<T>(read: () => Promise<T>): Promise<T> {
  if (active >= 2) await new Promise<void>(resolve => queue.push(resolve));
  active++;
  try { return await read(); } finally { active--; queue.shift()?.(); }
}
export const developmentApi = {
  status: (projectId: string, refresh = false, isCurrent: () => boolean = () => true) => scheduled(() => {
    if (!isCurrent()) return Promise.reject(new Error('读取已取消'));
    return desktop<GitStatus>('development_git_status', { projectId, refresh });
  }),
  history: (projectId: string, cursor?: string | null) => desktop<GitHistory>('development_git_history', { projectId, cursor: cursor || null }),
  files: (projectId: string, sha: string) => desktop<CommitFiles>('development_git_files', { projectId, sha }),
  diff: (projectId: string, filePath: string, stage: DiffStage, sha?: string | null) => desktop<GitDiff>('development_git_diff', { projectId, filePath, stage, sha: sha || null }),
  activity: (projectId: string) => desktop<GitActivity>('development_git_activity', { projectId }),
  spaces: () => desktop<DevelopmentSpace[]>('development_spaces', {}),
  space: (projectId: string) => desktop<DevelopmentSpace>('development_space', { projectId }),
  records: (spaceId: string) => desktop<DevelopmentRecords>('development_records', { spaceId }),
  saveLog: (input: LogInput) => desktop<DevelopmentRecords>('development_save_log', { input }),
  saveTask: (input: TaskInput) => desktop<DevelopmentRecords>('development_save_task', { input }),
  removeRecord: (spaceId: string, id: string, kind: 'logs' | 'tasks', revision: number) => desktop<DevelopmentRecords>('development_delete_record', { spaceId, id, kind, revision }),
  rebind: (spaceId: string, projectId: string) => desktop<DevelopmentSpace>('development_rebind', { spaceId, projectId }),
  summaryPreview: (projectId: string, sha: string | null, stage: DiffStage, files: string[]) => desktop<SummaryPreview>('development_summary_preview', { projectId, sha, stage, files }),
  summaryCancel: (token: string) => desktop<void>('development_summary_cancel', { token }),
  summaryGenerate: (token: string) => desktop<string>('development_summary_generate', { token }),
};
