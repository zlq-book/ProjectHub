export interface GitFile { path: string; oldPath?: string | null; indexStatus: string; worktreeStatus: string; untracked: boolean; conflicted: boolean }
export interface GitStatus { repository: boolean; repositoryId: string | null; branch: string | null; head: string | null; readAt: string; complete: boolean; warning: string | null; files: GitFile[]; latestCommitAt: string | null; shallow: boolean }
export interface GitCommit { sha: string; subject: string; message: string; author: string; authorTime: string; commitTime: string }
export interface GitHistory { head: string | null; commits: GitCommit[]; nextCursor: string | null; complete: boolean; readAt: string; warning: string | null }
export interface CommitFile { path: string; oldPath?: string | null; status: string; additions: number | null; deletions: number | null; binary: boolean }
export interface CommitFiles { head: string | null; sha: string; firstParent?: string | null; files: CommitFile[]; complete: boolean; warning: string | null }
export interface GitDiff { text: string; binary: boolean; truncated: boolean; warning: string | null }
export interface GitActivity { head: string | null; timestamps: string[]; complete: boolean; readAt: string; shallow: boolean; warning: string | null }
export type DiffStage = 'index' | 'worktree' | 'untracked';
export interface DevelopmentSpace { id: string; projectId: string | null; projectName: string; projectPath: string; repositoryId: string | null }
export interface CommitLink { repositoryId: string; sha: string; subject: string; author: string; commitTime: string }
export interface DevelopmentLog { id: string; spaceId: string; title: string; date: string; body: string; revision: number; createdAt: string; updatedAt: string; commits: CommitLink[] }
export type TaskStatus = 'todo' | 'inProgress' | 'done';
export interface DevelopmentTask { id: string; spaceId: string; title: string; description: string; status: TaskStatus; completedAt: string | null; revision: number; createdAt: string; updatedAt: string; commits: CommitLink[] }
export interface DevelopmentRecords { space: DevelopmentSpace; logs: DevelopmentLog[]; tasks: DevelopmentTask[] }
export interface LogInput { id: string | null; spaceId: string; title: string; date: string; body: string; revision: number | null; commits: { repositoryId: string; sha: string }[] }
export interface TaskInput { id: string | null; spaceId: string; title: string; description: string; status: TaskStatus; revision: number | null; commits: { repositoryId: string; sha: string }[] }
export interface SummaryPreview { token: string; model: string; baseUrl: string; text: string; bytes: number; excluded: string[]; files: string[] }
