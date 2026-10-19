import { useEffect, useRef, useState } from 'react';
import { developmentApi } from '../../services/development';
import { errorMessage } from '../../services/projects';
import type { Project } from '../../types';
import type { SummaryPreview } from '../../types/development';
import type { SummarySelection } from './GitPanel';
import type { DevelopmentDraft } from './DevelopmentWorkspace';
import Modal from '../Modal';
export default function SummaryDialog({ project, selection, onClose, onSave }: { project: Project; selection: SummarySelection; onClose: () => void; onSave: (draft: DevelopmentDraft) => void }) {
  const [files, setFiles] = useState(selection.files);
  const [preview, setPreview] = useState<SummaryPreview>();
  const [error, setError] = useState('');
  const [busy, setBusy] = useState(false);
  const [result, setResult] = useState('');
  const [generated,setGenerated]=useState(false);
  const alive=useRef(true);
  useEffect(()=>{alive.current=true;return()=>{alive.current=false;};},[]);
  function close(){if(preview)void developmentApi.summaryCancel(preview.token).catch(()=>{});onClose();}
  useEffect(() => { let active = true; let token:string|undefined; setPreview(undefined); setError(''); setBusy(true); developmentApi.summaryPreview(project.id, selection.sha, selection.stage, files).then(value => { token=value.token; if (active) setPreview(value); else void developmentApi.summaryCancel(value.token).catch(()=>{}); }, e => { if (active) setError(errorMessage(e)); }).finally(() => { if (active) setBusy(false); }); return () => { active = false; if(token)void developmentApi.summaryCancel(token).catch(()=>{}); }; }, [project.id, selection, files]);
  async function send() {
    if (!preview || busy) return; setBusy(true); setError('');
    try { const text=await developmentApi.summaryGenerate(preview.token); if(alive.current){setResult(text);setGenerated(true);} }
    catch (e) { if(alive.current)setError(errorMessage(e)); } finally { if(alive.current)setBusy(false); }
  }
  return <Modal title="AI 变更摘要" subtitle={`${project.name} · ${selection.sha?.slice(0, 8) || selection.title}`} wide onClose={close}><div className="summary-dialog-body">{!generated ? <><p className="notice">确认后，以下文本会发送到所选模型服务。代码可能含敏感信息，请检查发送范围。此前的项目资料授权不包含本次 Git 差异。</p><fieldset disabled={busy}><legend>选择文件</legend>{selection.files.map(path => <label className="commit-choice" key={path}><input type="checkbox" checked={files.includes(path)} onChange={e => setFiles(current => e.target.checked ? [...current, path] : current.filter(p => p !== path))} />{path}</label>)}</fieldset>{preview && <><p>模型：{preview.model} · {preview.baseUrl}</p><p>完整请求 {preview.bytes} / 65536 字节；包含 {preview.files.length} 个文件。</p>{preview.excluded.length > 0 && <p className="muted">已排除：{preview.excluded.join('、')}</p>}<pre className="summary-preview" tabIndex={0}>{preview.text}</pre></>}{!files.length && <p className="muted">没有可发送文件，请关闭后选择其他范围。</p>}</> : <><p className="notice">这是 AI 生成的变更摘要，请核对引用和风险。保存后才会成为开发日志。</p><label className="record-editor">摘要正文<textarea rows={16} maxLength={16000} value={result} onChange={e => setResult(e.target.value)} /></label></>}{error && <p role="alert" className="error-message">{error}</p>}{busy && <p role="status">{preview ? '正在生成摘要…' : '准备发送预览…'}</p>}</div><footer className="modal-footer"><button className="button" onClick={close}>{busy ? '取消本次摘要' : '关闭'}</button>{generated ? <button className="button primary" disabled={!result.trim()} onClick={() => onSave({ title: selection.title.slice(0, 200) || '开发变更摘要', body: result, commits: selection.commit ? [selection.commit] : [] })}>编辑并保存为开发日志</button> : <button className="button primary" disabled={busy || !preview || !preview.files.length} onClick={() => void send()}>确认发送并生成</button>}</footer></Modal>;
}
