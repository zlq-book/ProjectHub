import { useCallback, useEffect, useRef, useState, type ReactNode } from 'react';
import { LoaderCircle, MessageSquarePlus, Send, Sparkles, Square, X } from 'lucide-react';
import type { AiSettings, Project } from '../../types';
import type { AssistantApply, AssistantRequest, AssistantResult } from '../../types/assistant';
import { errorMessage, native } from '../../services/projects';
import { assistantApi } from '../../services/assistant';
import AssistantResults from './AssistantResults';

type Message = { id: string; role: 'user' | 'assistant'; text: string; status?: 'running' | 'done' | 'error' | 'cancelled'; result?: AssistantResult; request?: AssistantRequest; scopeLabel?: string };
export default function AssistantPanel({ visible, projects, filteredProjects, selectedProject, header, workspaceVersion, onConfigure, onClose, onViewProject, onViewDevelopment, onShowResults, onApply }: {
  visible: boolean; projects: Project[]; filteredProjects: Project[]; selectedProject?: Project; header: ReactNode; workspaceVersion: number;
  onConfigure: () => void; onClose: () => void; onViewProject: (project: Project) => void; onViewDevelopment?: (project: Project, sha?:string) => void; onShowResults: (ids: string[]) => void;
  onApply: (input: AssistantApply) => Promise<Project[]>;
}) {
  const [sessionId, setSessionId] = useState(() => crypto.randomUUID());
  const [messages, setMessages] = useState<Message[]>([]);
  const [draft, setDraft] = useState('');
  const [scope, setScope] = useState('all');
  const [settings, setSettings] = useState<AiSettings | null>(null);
  const [configError, setConfigError] = useState('');
  const [consent, setConsent] = useState(false);
  const [running, setRunning] = useState(false);
  const [saving, setSaving] = useState(false);
  const [clearing, setClearing] = useState(false);
  const active = useRef<{ sessionId: string; requestId: string } | null>(null);
  const composing = useRef(false);
  const logRef = useRef<HTMLDivElement>(null);
  const nearBottom = useRef(true);
  const inputRef = useRef<HTMLTextAreaElement>(null);
  const currentSession = useRef(sessionId);
  currentSession.current = sessionId;
  const previousVersion = useRef(workspaceVersion);
  const configured = !!settings?.baseUrl && !!settings.model;
  const range = scope === 'current' ? selectedProject ? [selectedProject] : [] : scope === 'filtered' ? filteredProjects : projects;
  const scopeLabel = scope === 'current' ? `当前项目：${selectedProject?.name || '未选择'}` : scope === 'filtered' ? '当前筛选结果' : '全部项目';

  useEffect(() => {
    if (!visible) return;
    let live = true; setConfigError('');
    assistantApi.settings().then(config => { if (live) setSettings(config); }, e => { if (live) setConfigError(errorMessage(e)); });
    return () => { live = false; };
  }, [visible, workspaceVersion]);
  useEffect(() => {
    if (previousVersion.current === workspaceVersion) return;
    previousVersion.current = workspaceVersion;
    const previousSession = currentSession.current;
    active.current = null; setRunning(false); setMessages([]); setConsent(false); setSettings(null); setSessionId(crypto.randomUUID());
    void assistantApi.clear(previousSession).catch(() => {});
  }, [workspaceVersion]);
  useEffect(() => () => { active.current = null; void assistantApi.clear(currentSession.current).catch(() => {}); }, []);
  useEffect(() => { if (nearBottom.current && logRef.current) logRef.current.scrollTop = logRef.current.scrollHeight; }, [messages, running]);

  const cancel = useCallback(() => {
    const request = active.current; if (!request) return;
    active.current = null; setRunning(false);
    setMessages(list => list.map(message => message.id === request.requestId ? { ...message, status: 'cancelled', text: '已取消，可修改输入后重试。' } : message));
    void assistantApi.cancel(request.sessionId, request.requestId).catch(e => setConfigError(`本地取消未确认：${errorMessage(e)}。界面已忽略该任务结果。`));
  }, []);
  async function execute(request: AssistantRequest, label: string, showUser = true) {
    if (active.current || saving || clearing) return;
    const requestId = crypto.randomUUID(); const next = { ...request, requestId };
    active.current = { sessionId: next.sessionId, requestId }; setRunning(true); nearBottom.current = true;
    setMessages(list => [...list, ...(showUser ? [{ id: crypto.randomUUID(), role: 'user' as const, text: next.prompt }] : []), { id: requestId, role: 'assistant', text: next.contextIds.length ? '正在读取资料并生成结果…' : '正在理解需求并检索项目…', status: 'running', request: next, scopeLabel: label }]);
    try {
      const result = await assistantApi.run(next);
      if (active.current?.requestId !== requestId) return;
      setMessages(list => list.map(message => message.id === requestId ? { ...message, status: 'done', text: result.text, result } : message));
      setDraft(current => current === next.prompt ? '' : current);
    } catch (e) {
      if (active.current?.requestId !== requestId) return;
      setMessages(list => list.map(message => message.id === requestId ? { ...message, status: 'error', text: errorMessage(e) } : message));
    } finally { if (active.current?.requestId === requestId) { active.current = null; setRunning(false); } }
  }
  function send() {
    const prompt = draft.trim();
    if (!prompt || prompt.length > 4000 || !native || !configured || !consent || !range.length || range.length > 100) return;
    const history = messages.filter(message => message.status === 'done' || message.role === 'user').slice(-4).map(message => ({ role: message.role, content: message.text.slice(0, 2000) }));
    void execute({ sessionId, requestId: '', prompt, scopeIds: range.map(project => project.id), referenceTime: new Date().toISOString(), history, contextIds: [] }, `${scopeLabel}（${range.length} 个）`);
  }
  async function newConversation() {
    if (running || saving || clearing) return;
    setClearing(true); setConfigError('');
    try { await assistantApi.clear(sessionId); setSessionId(crypto.randomUUID()); setMessages([]); setDraft(''); setConsent(false); }
    catch (e) { setConfigError(errorMessage(e)); }
    finally { setClearing(false); }
  }
  function read(request: AssistantRequest, ids: string[]) {
    if (running || saving) return;
    if (!ids.length) {
      const result: AssistantResult = { kind: 'readContext', text: '选择需要补充资料的项目。读取前请核对发送范围。', items: request.scopeIds.flatMap(id => { const project = projects.find(p => p.id === id); return project ? [{ project, certainty: 'uncertain' as const, evidence: [] }] : []; }), toolSteps: [], scopeCount: request.scopeIds.length };
      setMessages(list => [...list, { id: crypto.randomUUID(), role: 'assistant', status: 'done', text: result.text, result, request, scopeLabel: `原任务范围（${request.scopeIds.length} 个）` }]);
      return;
    }
    void execute({ ...request, contextIds: ids }, `本轮授权读取 ${ids.length} 个项目`, false);
  }

  return <aside className="detail-panel assistant-panel" hidden={!visible} aria-label="AI 助手">
    <div className="assistant-header">{header}<button className="icon-button" aria-label="收起 AI 助手" onClick={onClose}><X size={17} /></button></div>
    <div className="assistant-title"><Sparkles size={20} /><h2>ProjectHub Assistant</h2><button className="icon-button" aria-label="新建会话" title="新建会话" disabled={running || saving || clearing} onClick={() => void newConversation()}><MessageSquarePlus size={17} /></button></div>
    <div className="assistant-log" ref={logRef} role="log" aria-label="助手对话记录" aria-live="polite" onScroll={e => { const el = e.currentTarget; nearBottom.current = el.scrollHeight - el.scrollTop - el.clientHeight < 60; }}>
      <div className="assistant-welcome"><p>你好，我可以帮你查找项目、生成简介和整理分类。</p><div className="assistant-prompts">{['查找 Spring Boot 项目', '为当前项目生成简介', '整理项目分类'].map((prompt, index) => <button key={prompt} disabled={running || saving || index === 1 && !selectedProject} onClick={() => { if (index === 1) setScope('current'); setDraft(prompt); inputRef.current?.focus(); }}>{prompt}</button>)}</div></div>
      {!native ? <div className="assistant-notice">浏览器示例预览。请使用桌面版配置模型并处理本地项目资料。</div> : !settings ? <div className="assistant-notice">{configError || '正在读取模型配置…'}</div> : !configured ? <div className="assistant-notice">尚未配置模型。<button className="assistant-text-button" onClick={onConfigure}>配置 API</button></div> : !consent ? <div className="assistant-notice"><strong>使用 {settings.model}</strong><p className="assistant-service-url">{settings.baseUrl}</p><p>发送后，当前范围的项目名称、简介、分类、技术栈、标签及必要时间会交给此模型服务。README／依赖需另行确认读取。会话只在本次运行中保留。</p><button className="button" onClick={() => setConsent(true)}>开始使用 AI 助手</button></div> : null}
      {projects.length === 0 && <p className="assistant-notice">项目库为空，请先添加项目。</p>}
      {messages.map(message => <div className={`assistant-message assistant-message-${message.role}`} key={message.id}><div className="assistant-message-author">{message.role === 'user' ? '你' : <><Sparkles size={13} />助手</>}{message.status === 'running' && <LoaderCircle className="spin" size={13} />}</div><p className="assistant-message-text" role={message.status === 'error' ? 'alert' : undefined}>{message.text}</p>{message.scopeLabel && <small className="assistant-message-scope">{message.scopeLabel}</small>}{message.result && message.request && <>{message.result.toolSteps.length > 0 && <details className="assistant-tools"><summary>本次工具记录（{message.result.toolSteps.length}）</summary>{message.result.toolSteps.map((step, index) => <p key={index}>{step}</p>)}</details>}<AssistantResults result={message.result} request={message.request} blocked={running || saving || clearing} projects={projects} onApply={onApply} onRead={read} onViewProject={onViewProject} onViewDevelopment={onViewDevelopment} onShowResults={onShowResults} onSaving={setSaving} /></>}{message.status === 'error' && message.request && <button className="assistant-text-button" disabled={running || saving} onClick={() => void execute(message.request!, message.scopeLabel || '原任务范围', false)}>重试本次请求</button>}</div>)}
    </div>
    <form className="assistant-composer" onSubmit={e => { e.preventDefault(); send(); }}><label className="sr-only" htmlFor="assistant-input">发送给 AI 助手</label><textarea ref={inputRef} id="assistant-input" aria-label="发送给 AI 助手" placeholder="描述你要查找或整理的项目…" rows={3} maxLength={4000} value={draft} onChange={e => setDraft(e.target.value)} onCompositionStart={() => { composing.current = true; }} onCompositionEnd={() => { composing.current = false; }} onKeyDown={e => { if (e.key === 'Enter' && !e.shiftKey && !composing.current && !e.nativeEvent.isComposing && e.keyCode !== 229) { e.preventDefault(); send(); } }} /><div className="assistant-composer-footer"><label>当前范围<select aria-label="助手操作范围" value={scope} disabled={running || saving} onChange={e => setScope(e.target.value)}><option value="all">全部项目（{projects.length}）</option><option value="filtered">当前筛选结果（{filteredProjects.length}）</option><option value="current" disabled={!selectedProject}>当前项目{selectedProject ? `：${selectedProject.name}` : '：未选择'}</option></select></label>{running ? <button className="button" type="button" onClick={cancel}><Square size={13} />停止生成</button> : <button className="button primary" type="submit" disabled={!native || !configured || !consent || !draft.trim() || !range.length || range.length > 100 || saving || clearing}><Send size={15} />发送</button>}</div>{range.length > 100 && <p className="assistant-range-warning">每轮最多处理 100 个项目，请切换到较小的筛选范围。</p>}{configError && <p className="error-message" role="alert">{configError}</p>}</form>
  </aside>;
}
