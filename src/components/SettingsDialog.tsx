import { useEffect, useState } from 'react';
import { Database, Download, Upload, Monitor, Moon, Sun, ShieldCheck } from 'lucide-react';
import Modal from './Modal';
import { categories, type Settings } from '../types';
import { errorMessage, repository } from '../services/projects';

export default function SettingsDialog({ settings, onClose, onSave, onImported }: {
  settings: Settings; onClose: () => void; onSave: (settings: Settings) => Promise<void>; onImported: () => Promise<void>;
}) {
  const [form, setForm] = useState(structuredClone(settings));
  const [path, setPath] = useState('读取中…');
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState('');
  const [error, setError] = useState('');
  const [importConfirm, setImportConfirm] = useState(false);
  useEffect(() => { repository.databasePath().then(setPath).catch(e => setError(errorMessage(e))); }, []);
  async function backup(action: 'export' | 'import') {
    setBusy(true); setError(''); setMessage('');
    try { const result = await repository.backup(action); if (result) { setMessage(result); if (action === 'import') await onImported(); } }
    catch (e) { setError(errorMessage(e)); }
    finally { setBusy(false); setImportConfirm(false); }
  }
  return <Modal closable={!busy} title="设置" subtitle="按照你的习惯，整理自己的工作台。" onClose={onClose}>
    <form className="project-form" onSubmit={async e => { e.preventDefault(); setBusy(true); try { await onSave(form); onClose(); } catch (err) { setError(errorMessage(err)); } finally { setBusy(false); } }}>
      <fieldset className="form-fields" disabled={busy}>
      <h3 className="form-section-title">外观与偏好</h3>
      <div className="theme-options">{([['light', '浅色', Sun], ['dark', '深色', Moon], ['system', '跟随系统', Monitor]] as const).map(([theme, name, Icon]) => <button key={theme} type="button" aria-pressed={form.theme===theme} className={`theme-choice ${form.theme === theme ? 'active' : ''}`} onClick={() => setForm({ ...form, theme })}><Icon size={20} />{name}</button>)}</div>
      <div className="form-columns"><label>新项目默认分类<select value={form.defaultCategory} onChange={e => setForm({ ...form, defaultCategory: e.target.value as Settings['defaultCategory'] })}>{Object.entries(categories).map(([key, name]) => <option key={key} value={key}>{name}</option>)}</select></label><label>项目展示方式<select value={form.view} onChange={e => setForm({ ...form, view: e.target.value as Settings['view'] })}><option value="grid">卡片视图</option><option value="list">列表视图</option></select></label></div>
      <h3 className="form-section-title">项目打开工具</h3>
      <label>全局默认工具<select value={form.defaultIde} onChange={e => setForm({ ...form, defaultIde: e.target.value as Settings['defaultIde'] })}><option value="system">文件资源管理器</option><option value="code">VS Code</option><option value="idea">IntelliJ IDEA</option><option value="cursor">Cursor</option><option value="custom">自定义程序</option></select></label>
      <details className="settings-advanced"><summary>高级配置：IDE 可执行程序路径</summary>
      {([['code', 'VS Code'], ['idea', 'IntelliJ IDEA'], ['cursor', 'Cursor'], ['custom', '自定义程序']] as const).map(([key, name]) => <label key={key}>{name} 可执行程序路径<input value={form.idePaths[key] || ''} onChange={e => setForm({ ...form, idePaths: { ...form.idePaths, [key]: e.target.value } })} placeholder={key === 'custom' ? 'C:\Tools\editor.exe' : '留空则在系统 PATH 中查找'} /><small>填写 .exe 或 .com 完整路径，不包含参数；目录作为独立参数传入。</small></label>)}
      </details>
      <h3 className="form-section-title"><Database size={17} />本地数据</h3><div className="database-location">{path}</div>
      <div className="backup-actions"><button type="button" className="button" onClick={() => backup('export')} disabled={busy}><Download size={16} />导出 SQLite 备份</button><button type="button" className="button" onClick={() => setImportConfirm(true)} disabled={busy}><Upload size={16} />导入备份</button></div>
      {importConfirm && <div className="notice warning">导入将替换当前项目库、设置、全部开发记录及周报历史。版本 1 备份没有日志和任务，版本 1／2 备份没有周报历史，恢复后对应记录为空。软件会先校验备份，再自动完整备份当前数据库。<div className="backup-actions"><button type="button" className="button" onClick={() => setImportConfirm(false)}>取消</button><button type="button" className="button primary" disabled={busy} onClick={() => backup('import')}>确认并选择备份</button></div></div>}
      {message && <p role="status" className="success-message">{message}</p>}{error && <p role="alert" className="error-message">{error}</p>}
      <footer className="modal-footer"><span><ShieldCheck size={15} />离线存储，数据由你掌控</span><button type="button" className="button" disabled={busy} onClick={onClose}>取消</button><button className="button primary" disabled={busy}>保存设置</button></footer>
      </fieldset>
    </form>
  </Modal>;
}

