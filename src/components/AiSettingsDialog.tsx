import { useEffect, useId, useState } from 'react';
import Modal from './Modal';
import { repository, errorMessage } from '../services/projects';

export default function AiSettingsDialog({ onClose, onSaved }: { onClose: () => void; onSaved?: () => void }) {
  const [baseUrl, setBaseUrl] = useState('');
  const [model, setModel] = useState('');
  const [apiKey, setApiKey] = useState('');
  const [hasApiKey, setHasApiKey] = useState(false);
  const [clearApiKey, setClearApiKey] = useState(false);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  const [attempt, setAttempt] = useState(0);
  const [checking, setChecking] = useState<'' | 'models' | 'test'>('');
  const [models, setModels] = useState<string[]>([]);
  const [message, setMessage] = useState('');
  const modelListId = useId();
  const locked = busy || !!checking;
  function changed() { setMessage(''); setError(''); setModels([]); }
  async function check(action: 'models' | 'test') {
    if (loading || locked) return;
    setChecking(action); setError(''); setMessage('');
    const input = { baseUrl: baseUrl.trim(), model: model.trim(), apiKey, clearApiKey };
    try {
      if (action === 'models') {
        const list = await repository.aiModels(input); setModels(list);
        setMessage(`已获取 ${list.length} 个模型，可从模型名称的下拉建议中选择，或继续手动填写。`);
      } else {
        const result = await repository.testAiConnection(input);
        setMessage(`连接成功：${result.model}，耗时 ${result.latencyMs} ms。`);
      }
    } catch (e) { setError(errorMessage(e)); }
    finally { setChecking(''); }
  }
  useEffect(() => {
    let active = true; setLoading(true); setError('');
    repository.aiSettings().then(config => {
      if (active) { setBaseUrl(config.baseUrl); setModel(config.model); setHasApiKey(config.hasApiKey); setLoading(false); }
    }).catch(e => { if (active) setError(errorMessage(e)); });
    return () => { active = false; };
  }, [attempt]);
  return <Modal title="配置 API" subtitle="为项目 AI 解析配置兼容接口" onClose={onClose} closable={!locked}>
    <form className="project-form" onSubmit={async e => {
      e.preventDefault(); if (loading || locked) return; setBusy(true); setError(''); setMessage('');
      try { await repository.saveAiSettings({ baseUrl: baseUrl.trim(), model: model.trim(), apiKey, clearApiKey }); setApiKey(''); onSaved?.(); onClose(); }
      catch (e) { setError(errorMessage(e)); } finally { setBusy(false); }
    }}>
      <fieldset className="form-fields" disabled={locked || loading}>
        <label>API 基础地址<input aria-label="API 基础地址" autoFocus type="url" value={baseUrl} onChange={e => { setBaseUrl(e.target.value); changed(); }} placeholder="https://your-provider.example/v1" /><small>兼容 Chat Completions，自动补全 /chat/completions；也可填写完整接口地址。本地服务支持 HTTP。</small></label>
        <label>模型名称<div className="api-model-row"><input aria-label="模型名称" list={modelListId} value={model} onChange={e => { setModel(e.target.value); setMessage(''); setError(''); }} maxLength={200} placeholder="填写或从下拉建议中选择模型 ID" /><button type="button" className="button" disabled={!baseUrl.trim()} onClick={() => check('models')}>{checking === 'models' ? '获取中…' : '获取模型列表'}</button></div><datalist id={modelListId}>{models.map(id => <option key={id} value={id} />)}</datalist></label>
        <label>API Key<input aria-label="API Key" type="password" autoComplete="new-password" value={apiKey} onChange={e => { setApiKey(e.target.value); setClearApiKey(false); changed(); }} maxLength={4096} placeholder={hasApiKey && !clearApiKey ? '已配置，留空保持原密钥' : '可选，本地模型可留空'} /><small>Windows 当前用户加密保存，界面不回显原密钥，也不写入项目库备份。</small></label>
        {hasApiKey && <label className="checkbox-label"><input type="checkbox" checked={clearApiKey} onChange={e => { setClearApiKey(e.target.checked); if (e.target.checked) setApiKey(''); changed(); }} />清除已保存的 API Key</label>}
        <div className="api-test-row"><button type="button" className="button" disabled={!baseUrl.trim() || !model.trim()} onClick={() => check('test')}>{checking === 'test' ? '测试中…' : '测试连接'}</button><small>使用当前填写的配置，不自动保存。测试会发送简短对话，可能产生少量费用，不发送项目资料。</small></div>
        <div className="notice">保存配置不会发起模型请求。主动使用 AI 解析或 AI 助手时，才按界面说明发送项目资料。基础地址和模型同时留空可停用。</div>
      </fieldset>
      {loading && !error && <p className="muted" role="status">正在读取 API 配置…</p>}
      {error && <p className="error-message" role="alert">{error}</p>}
      {message && <p className="success-message" role="status">{message}</p>}
      {loading && error && <button type="button" className="button" onClick={() => setAttempt(n => n + 1)}>重新读取配置</button>}
      <footer className="modal-footer"><button type="button" className="button" disabled={locked} onClick={onClose}>取消</button><button className="button primary" disabled={locked || loading}>{busy ? '保存中…' : '保存 API 配置'}</button></footer>
    </form>
  </Modal>;
}
