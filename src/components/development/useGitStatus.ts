import { useEffect, useRef, useState } from 'react';
import { developmentApi } from '../../services/development';
import { errorMessage } from '../../services/projects';
import type { GitStatus } from '../../types/development';

export function useElementVisible<T extends Element>() {
  const ref=useRef<T>(null);const [visible,setVisible]=useState(typeof IntersectionObserver==='undefined');
  useEffect(()=>{if(typeof IntersectionObserver==='undefined'||!ref.current)return;const observer=new IntersectionObserver(([entry])=>setVisible(entry.isIntersecting));observer.observe(ref.current);return()=>observer.disconnect();},[]);
  return {ref,visible};
}

export function useGitStatus(projectId: string, version = 0, enabled = true) {
  const [state, setState] = useState<{ id?: string; data?: GitStatus; error?: string; loading: boolean }>({ loading: true });
  useEffect(() => {
    if (!enabled) return;
    let active = true, reading = false;
    setState({ id:projectId, loading: true });
    async function load(refresh = false) {
      if (!active || reading || document.hidden) return;
      reading = true;
      try { const data = await developmentApi.status(projectId, refresh, () => active && !document.hidden); if (active) setState({ id:projectId, data, loading: false }); }
      catch (e) { if (active) setState({ id:projectId, error: errorMessage(e), loading: false }); }
      finally { reading = false; }
    }
    void load(version > 0);
    const timer = setInterval(() => void load(true), 15000);
    const visible = () => { if (!document.hidden) void load(true); };
    document.addEventListener('visibilitychange', visible);
    return () => { active = false; clearInterval(timer); document.removeEventListener('visibilitychange', visible); };
  }, [projectId, version, enabled]);
  return state.id===projectId ? state : { loading:true };
}
export function statusLabel(data?: GitStatus, error?: string) {
  if (error) return '读取失败 · 状态未知';
  if (!data) return 'Git 待读取';
  if (!data.repository) return '未初始化 Git';
  return `${data.branch || (data.head ? '分离 HEAD' : '暂无提交')} · ${data.files.length ? `${data.complete ? '' : '至少'}未提交 ${data.files.length}` : data.complete ? '工作区干净' : '状态未完整读取'}`;
}
