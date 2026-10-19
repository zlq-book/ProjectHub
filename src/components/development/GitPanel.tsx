import { useEffect, useRef, useState } from 'react';
import { developmentApi } from '../../services/development';
import { errorMessage } from '../../services/projects';
import type { CommitFiles, DiffStage, GitCommit, GitDiff, GitHistory, GitStatus } from '../../types/development';

export type SummarySelection = { sha: string | null; stage: DiffStage; files: string[]; title: string; commit?: GitCommit };
export default function GitPanel({ projectId, status, historyMode, onDiffOpen, onSummary, onLog, initialCommitSha }: { projectId: string; initialCommitSha?:string|null; status: GitStatus; historyMode: boolean; onDiffOpen: (open: boolean) => void; onSummary: (selection: SummarySelection) => void; onLog: (commit: GitCommit) => void }) {
  const [history, setHistory] = useState<GitHistory>();
  const [commit, setCommit] = useState<GitCommit>();
  const [files, setFiles] = useState<CommitFiles>();
  const [diff, setDiff] = useState<GitDiff>();
  const [file, setFile] = useState('');
  const [stage, setStage] = useState<DiffStage>('worktree');
  const [error, setError] = useState('');
  const [busy, setBusy] = useState(false);
  const request = useRef(0);
  useEffect(() => { let active = true; request.current++; setHistory(undefined); setCommit(undefined); setFiles(undefined); setDiff(undefined); setFile(''); setError(''); setBusy(false); onDiffOpen(false);
    if (historyMode && status.head) { setBusy(true); developmentApi.history(projectId).then(value => { if (active) setHistory(value); }, e => { if (active) setError(errorMessage(e)); }).finally(() => { if (active) setBusy(false); }); }
    return () => { active = false; request.current++; onDiffOpen(false); };
  // onDiffOpen 由上层提供稳定回调。
  }, [projectId, status.head, historyMode, onDiffOpen]);
  useEffect(()=>{if(initialCommitSha&&historyMode&&history&&!commit){const target=history.commits.find(value=>value.sha===initialCommitSha);if(target)void showCommit(target);}},[initialCommitSha,historyMode,history]);
  async function showCommit(value: GitCommit) {
    const token = ++request.current; setCommit(value); setFiles(undefined); setDiff(undefined); setFile(''); setError(''); setBusy(true); onDiffOpen(false);
    try { const result = await developmentApi.files(projectId, value.sha); if (request.current === token) setFiles(result); }
    catch (e) { if (request.current === token) setError(errorMessage(e)); } finally { if (request.current === token) setBusy(false); }
  }
  async function showDiff(path: string, nextStage: DiffStage) {
    const token = ++request.current; setFile(path); if (nextStage !== 'untracked') setStage(nextStage); setDiff(undefined); setError(''); setBusy(true); onDiffOpen(true);
    try { const result = await developmentApi.diff(projectId, path, nextStage, historyMode ? commit?.sha : null); if (request.current === token) setDiff(result); }
    catch (e) { if (request.current === token) setError(errorMessage(e)); } finally { if (request.current === token) setBusy(false); }
  }
  async function more() {
    if (!history?.nextCursor || busy) return;
    const token = ++request.current; setBusy(true); setError('');
    try { const next = await developmentApi.history(projectId, history.nextCursor); if (token === request.current) setHistory({ ...next, commits: [...history.commits, ...next.commits] }); }
    catch (e) { if (token === request.current) setError(errorMessage(e)); } finally { if (token === request.current) setBusy(false); }
  }
  if (!status.repository) return <p className="notice">该目录未初始化 Git。开发日志和任务仍可使用。</p>;
  return <section className="development-section">
    {historyMode ? <><div className="development-toolbar"><h2>提交历史</h2><span className="muted">当前 HEAD · 全部作者</span></div>{!status.head && <p>暂无提交</p>}<div className="commit-list">{history?.commits.map(value => <button key={value.sha} className={commit?.sha === value.sha ? 'selected' : ''} onClick={() => void showCommit(value)}><code>{value.sha.slice(0, 8)}</code><strong>{value.subject || '（无提交说明）'}</strong><span>{value.author} · {new Date(value.commitTime).toLocaleString()}</span></button>)}</div>{history?.warning && <p className="notice">{history.warning}</p>}{history?.nextCursor && <button className="button" disabled={busy} onClick={() => void more()}>加载更多提交</button>}
      {commit && <div className="commit-detail"><h3>{commit.subject}</h3><p><code>{commit.sha}</code></p><p className="muted">作者：{commit.author} · 作者时间：{new Date(commit.authorTime).toLocaleString()} · 提交时间：{new Date(commit.commitTime).toLocaleString()}</p><pre className="commit-message">{commit.message}</pre><p className="muted">{files?.firstParent ? `对第一父提交 ${files.firstParent.slice(0, 8)} 比较（合并提交同口径）` : '根提交对空树比较'}</p><div className="development-toolbar"><button className="button" onClick={() => onLog(commit)}>创建开发日志草稿</button><button className="button" disabled={!files?.files.length} onClick={() => onSummary({ sha: commit.sha, stage: 'worktree', files: files?.files.filter(f => !f.binary).map(f => f.path) || [], title: commit.subject, commit })}>AI 总结此提交</button></div>{files?.warning && <p className="notice">{files.warning}</p>}<div className="changed-files">{files?.files.map(f => <button key={f.path} onClick={() => void showDiff(f.path, 'worktree')}><span className="file-status">{f.status}</span><span>{f.oldPath ? `${f.oldPath} → ` : ''}{f.path}</span><small>{f.binary ? '二进制 · 行数不适用' : `+${f.additions ?? '?'} −${f.deletions ?? '?'}`}</small></button>)}</div>{files && !files.files.length && <p className="muted">空提交，没有文件变化。</p>}</div>}
    </> : <><div className="development-toolbar"><h2>工作区文件</h2><span>{status.complete ? '' : '至少 '}{status.files.length} 个未提交文件</span><button className="button" disabled={!status.files.length} onClick={() => onSummary({ sha: null, stage, files: status.files.filter(f => !f.untracked && !f.conflicted && (stage === 'index' ? !['.', ' ', '?'].includes(f.indexStatus) : !['.', ' ', '?'].includes(f.worktreeStatus))).map(f => f.path), title: `${stage === 'index' ? '暂存区' : '工作区'}变更` })}>AI 总结所选范围</button><select aria-label="变更比较范围" value={stage === 'untracked' ? 'worktree' : stage} onChange={e => { setStage(e.target.value as DiffStage); setDiff(undefined); setFile(''); onDiffOpen(false); }}><option value="worktree">工作区对暂存区</option><option value="index">暂存区对 HEAD</option></select></div><p className="muted">XY 分别表示暂存区和工作区状态；同一文件多组变化只计一次。只读查看。</p>{status.complete && !status.files.length && <p className="notice">工作区干净</p>}<div className="changed-files">{status.files.map(f => <div key={f.path}><code>{f.indexStatus}{f.worktreeStatus}</code><span>{f.oldPath ? `${f.oldPath} → ` : ''}{f.path}</span><small>{f.conflicted ? '冲突' : f.untracked ? '未跟踪' : '已跟踪'}</small><button className="button" disabled={f.conflicted} onClick={() => void showDiff(f.path, f.untracked ? 'untracked' : stage)}>{f.untracked ? '预览文本' : '查看差异'}</button></div>)}</div></>}
    {busy && <p role="status">读取中…</p>}{error && <p className="error-message" role="alert">{error}</p>}{file && <div className="diff-view"><div className="development-toolbar"><h3>{file}</h3><button className="button" onClick={() => { request.current++; setFile(''); setDiff(undefined); setBusy(false); onDiffOpen(false); }}>关闭差异</button></div>{diff && <>{(diff.warning || diff.truncated) && <p className="notice">{diff.warning || '差异已截断，请在 IDE 查看完整内容。'}</p>}{diff.binary ? <p>二进制文件，不展示文本差异。</p> : <pre tabIndex={0} aria-label="代码差异">{diff.text || '所选范围没有文本差异。'}</pre>}</>}</div>}
  </section>;
}
