import { useEffect, useRef, useState } from 'react';
import { native } from '../../services/projects';
import { statusLabel, useGitStatus } from './useGitStatus';
export default function GitBadge({ projectId }: { projectId: string }) {
  const ref = useRef<HTMLSpanElement>(null);
  const [visible, setVisible] = useState(false);
  useEffect(() => {
    if (!native || !ref.current) return;
    if (typeof IntersectionObserver === 'undefined') { setVisible(true); return; }
    const observer = new IntersectionObserver(([entry]) => setVisible(entry.isIntersecting));
    observer.observe(ref.current); return () => observer.disconnect();
  }, []);
  const { data, error } = useGitStatus(projectId, 0, visible);
  return <span ref={ref} className={`git-badge ${error ? 'git-warning' : ''}`} title={error || data?.warning || 'Git 工作区快照'}>{native ? statusLabel(data, error) : 'Git · 桌面版读取'}</span>;
}
