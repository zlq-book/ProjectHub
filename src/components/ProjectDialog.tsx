import { useEffect, useRef, useState } from 'react';
import { FolderOpen, ScanLine, LoaderCircle, Star, ShieldCheck, Sparkles } from 'lucide-react';
import Modal from './Modal';
import { categories, type AiAnalysis, type AiSettings, type Project, type ProjectInput, type Settings } from '../types';
import { errorMessage, native, repository } from '../services/projects';

const split = (value: string) => [...new Set(value.split(/[,，]/).map(v => v.trim()).filter(Boolean))];
export default function ProjectDialog({ project, settings, onClose, onSave }: {
  project?: Project; settings: Settings; onClose: () => void; onSave: (project: Project) => void;
}) {
  const [form, setForm] = useState<ProjectInput>(project || { name: '', path: '', description: '', category: settings.defaultCategory,
    tags: [], techStack: [], favorite: false, notes: '', defaultIde: 'system' });
  const [tags, setTags] = useState(form.tags.join(', '));
  const [stack, setStack] = useState(form.techStack.join(', '));
  const [busy, setBusy] = useState('');
  const [error, setError] = useState('');
  const [hint, setHint] = useState('');
  const request = useRef(0);
  const [aiSettings, setAiSettings] = useState<AiSettings | null>(null);
  const [analysis, setAnalysis] = useState<AiAnalysis | null>(null);
  useEffect(() => {
    let active = true;
    repository.aiSettings().then(value => { if (active) setAiSettings(value); }).catch(e => { if (active) setError(`无法读取 AI 配置：${errorMessage(e)}`); });
    return () => { active = false; request.current++; };
  }, []);
  function field<K extends keyof ProjectInput>(key: K, value: ProjectInput[K]) { setForm(f => ({ ...f, [key]: value })); setError(''); if (key === 'path') setAnalysis(null); }
  async function analyze() {
    if (!aiSettings?.baseUrl || !aiSettings.model) { setError('请先通过左侧“配置 API”填写接口地址与模型名称。'); return; }
    const current = ++request.current; setBusy('ai'); setError(''); setHint(''); setAnalysis(null);
    try { const result = await repository.analyze(form.path, project?.id); if (current === request.current) setAnalysis(result); }
    catch (e) { if (current === request.current) setError(errorMessage(e)); }
    finally { if (current === request.current) setBusy(''); }
  }
  function applyAnalysis() {
    if (!analysis) return;
    setForm(f => ({ ...f, ...analysis })); setStack(analysis.techStack.join(', ')); setTags(analysis.tags.join(', '));
    setAnalysis(null); setHint('AI 解析结果已应用，可继续修改后保存。');
  }
  async function choose() {
    setBusy('choose'); setError('');
    try { const path = await repository.choose(); if (path) { field('path', path); setHint(''); } }
    catch (e) { setError(errorMessage(e)); }
    finally { setBusy(''); }
  }
  async function detect() {
    const current = ++request.current; setBusy('detect'); setError(''); setHint('');
    try {
      const detected = await repository.detect(form.path, project?.id);
      if (current !== request.current) return;
      setForm(f => ({ ...f, ...detected })); setStack(detected.techStack.join(', '));
      setHint('目录有效，识别完成。你可以修改资料后保存。');
    } catch (e) { if (current === request.current) setError(errorMessage(e)); }
    finally { if (current === request.current) setBusy(''); }
  }
  async function save(e: React.FormEvent) {
    e.preventDefault(); setBusy('save'); setError('');
    try {
      const input = { ...form,
        tags: tags === form.tags.join(', ') ? form.tags : split(tags),
        techStack: stack === form.techStack.join(', ') ? form.techStack : split(stack),
      };
      const result = project ? await repository.update(project.id, input) : await repository.create(input);
      onSave(result);
    } catch (e) { setError(errorMessage(e)); setBusy(''); }
  }
  return <Modal closable={!busy} title={project ? '编辑项目' : '添加本地项目'} subtitle="让散落的代码，拥有一个共同的入口。" onClose={() => { request.current++; onClose(); }}>
    <form onSubmit={save} className="project-form">
      <fieldset className="form-fields" disabled={!!busy}>
      {!native && <div className="notice">当前为浏览器示例预览，路径仅作为示例文本保存，不验证或访问本地文件。</div>}
      <label>本地项目路径 <span className="required">*</span><div className="input-action"><input autoFocus required placeholder="D:\Projects\my-project" value={form.path} disabled={!!busy} onChange={e => { field('path', e.target.value); setHint(''); }} /><button type="button" className="button" onClick={choose} disabled={!!busy}><FolderOpen size={16} />浏览</button></div></label>
      <div className="detect-row"><span>识别配置文件和 README，补全项目资料</span><div className="detect-actions"><button type="button" className="button" onClick={detect} disabled={!form.path.trim() || !!busy}>{busy === 'detect' ? <LoaderCircle size={16} className="spin" /> : <ScanLine size={16} />}识别项目</button><button type="button" className="button button-tint" onClick={analyze} disabled={!form.path.trim() || !!busy || !aiSettings}>{busy === 'ai' ? <LoaderCircle size={16} className="spin" /> : <Sparkles size={16} />}{busy === 'ai' ? '解析中…' : 'AI 解析'}</button></div></div>
      <p className="muted ai-data-note">{aiSettings?.baseUrl && aiSettings.model ? `AI 解析将向 ${aiSettings.baseUrl}（${aiSettings.model}）发送 README、依赖清单及有限目录名称；不附带项目绝对路径，不读取源码正文。` : '使用 AI 解析前，请通过左侧“配置 API”填写兼容接口地址、模型名称和可选 API Key。'}</p>
      {analysis && <section className="ai-result" aria-label="AI 解析结果"><h3>AI 解析结果</h3><dl><dt>项目名称</dt><dd>{analysis.name}</dd><dt>项目简介</dt><dd>{analysis.description || '暂无简介'}</dd><dt>项目分类</dt><dd>{categories[analysis.category]}</dd><dt>技术栈</dt><dd>{analysis.techStack.join('、') || '未识别'}</dd><dt>项目标签</dt><dd>{analysis.tags.join('、') || '暂无标签'}</dd></dl><p className="muted">应用后将替换以上五项字段，保存前仍可编辑。</p><div className="detect-actions"><button type="button" className="button" onClick={() => setAnalysis(null)}>放弃结果</button><button type="button" className="button primary" onClick={applyAnalysis}>应用解析结果</button></div></section>}
      <div className="form-columns"><label>项目名称 <span className="required">*</span><input required maxLength={120} value={form.name} onChange={e => field('name', e.target.value)} placeholder="例如 env-agent-backend" /></label><label>项目分类<select value={form.category} onChange={e => field('category', e.target.value as ProjectInput['category'])}>{Object.entries(categories).map(([key, name]) => <option key={key} value={key}>{name}</option>)}</select></label></div>
      <label>项目简介<textarea rows={3} maxLength={4000} value={form.description} onChange={e => field('description', e.target.value)} placeholder="用几句话描述这个项目的用途…" /></label>
      <label>技术栈<input value={stack} onChange={e => setStack(e.target.value)} placeholder="Spring Boot, Java, MySQL（逗号分隔）" /></label>
      <div className="form-columns"><label>项目标签<input value={tags} onChange={e => setTags(e.target.value)} placeholder="AI, 环境监测" /></label><label>默认打开工具<select value={form.defaultIde} onChange={e => field('defaultIde', e.target.value as ProjectInput['defaultIde'])}><option value="system">跟随全局设置</option><option value="code">VS Code</option><option value="idea">IntelliJ IDEA</option><option value="cursor">Cursor</option><option value="custom">自定义程序</option></select></label></div>
      <label>备注<textarea rows={2} maxLength={8000} value={form.notes} onChange={e => field('notes', e.target.value)} placeholder="记录环境、待办或其他需要记住的事" /></label>
      <label className="checkbox-label"><input type="checkbox" checked={form.favorite} onChange={e => field('favorite', e.target.checked)} /><Star size={16} />收藏这个项目</label>
      {hint && <p className="success-message" role="status">{hint}</p>}{error && <p className="error-message" role="alert">{error}</p>}
      <footer className="modal-footer"><span><ShieldCheck size={15} />只保存索引，不改动源文件</span><button type="button" className="button" onClick={onClose} disabled={!!busy}>取消</button><button className="button primary" disabled={!!busy}>{busy === 'save' && <LoaderCircle size={16} className="spin" />}{project ? '保存修改' : '保存项目'}</button></footer>
      </fieldset>
    </form>
  </Modal>;
}

