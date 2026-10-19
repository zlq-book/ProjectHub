import { Clock3, MoreHorizontal, Star, TriangleAlert } from 'lucide-react';
import { useEffect, useRef } from 'react';
import FolderIcon from './FolderIcon';
import GitBadge from './development/GitBadge';
import type { Project } from '../types';
export const folderColors = { backend: 'blue', frontend: 'green', ai: 'purple', tool: 'amber', learning: 'coral', other: 'teal' };
const TAG_TONES: Record<string, string> = {
  java: 'amber', 'java 17': 'amber', 'java 1.8': 'amber',
  javascript: 'amber', typescript: 'blue', 'node.js': 'green', node: 'green',
  react: 'cyan', vite: 'purple', vue: 'green', 'vue3': 'green', 'element plus': 'teal',
  python: 'orange', fastapi: 'teal', flask: 'green', django: 'green', pyodbc: 'blue',
  maven: 'blue', gradle: 'blue', 'spring boot': 'green', spring: 'green', 'spring ai alibaba': 'green', 'spring ai': 'green',
  mysql: 'teal', mybatis: 'rose', postgresql: 'blue', redis: 'rose', sqlite: 'blue', 'sql server': 'blue',
  rust: 'orange', go: 'cyan', '.net': 'purple', 'c#': 'purple', 'c++': 'blue',
  docker: 'blue', kubernetes: 'blue', nginx: 'green', linux: 'amber', git: 'orange',
  ai: 'purple', 'stable diffusion': 'purple', tauri: 'orange',
};
export function tagTone(value: string) { return TAG_TONES[value.trim().toLowerCase()] || 'slate'; }
export function relativeTime(value: string | null) {
  if (!value) return '尚未打开';
  const hours = Math.max(0, Math.floor((Date.now() - new Date(value).getTime()) / 3600000));
  if (hours < 1) return '刚刚';
  if (hours < 24) return `${hours} 小时前`;
  const days = Math.floor(hours / 24);
  return days < 7 ? `${days} 天前` : `${Math.floor(days / 7)} 周前`;
}
export function Tags({ values, onClick }: { values: string[]; onClick?: (tag: string) => void }) {
  return <div className="tags">{values.map(tag => onClick ? <button type="button" key={tag} className={`tag tag--${tagTone(tag)}`} onClick={() => onClick(tag)}>{tag}</button> : <span key={tag} className={`tag tag--${tagTone(tag)}`}>{tag}</span>)}</div>;
}
export default function ProjectCard({ project: p, selected, onSelect, onFavorite, onEdit, onOpen, batchMode = false, checked = false, onCheck, busy = false }: {
  project: Project; selected: boolean; onSelect: (showDetail?: boolean) => void; onFavorite: () => void; onEdit: () => void; onOpen: () => void;
  batchMode?: boolean; checked?: boolean; onCheck?: () => void;
  busy?: boolean;
}) {
  const clickTimer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  useEffect(() => () => clearTimeout(clickTimer.current), []);
  useEffect(() => { if (!selected || batchMode) clearTimeout(clickTimer.current); }, [selected, batchMode]);
  return <article className={`project-card ${selected || checked ? 'selected' : ''} ${batchMode ? 'batch-card' : ''} ${!p.available ? 'unavailable' : ''}`}>
    {batchMode && <label className="card-checkbox"><input type="checkbox" aria-label={`选择 ${p.name}`} checked={checked} disabled={busy} onChange={onCheck} /></label>}
    <button type="button" className="card-main" disabled={busy} onClick={event => {
      if (batchMode) { onCheck?.(); return; }
      clearTimeout(clickTimer.current);
      // 键盘即时展开；鼠标先选中，避免第一次点击展开面板后移动双击目标。
      onSelect(event.detail === 0);
      if (event.detail > 0) clickTimer.current = setTimeout(() => onSelect(true), 500);
    }} onDoubleClick={batchMode ? undefined : () => { clearTimeout(clickTimer.current); onOpen(); }} aria-pressed={batchMode ? checked : selected} aria-label={`${batchMode ? '切换选择' : '查看'} ${p.name}`}>
      <div className="card-heading"><FolderIcon color={folderColors[p.category] || 'blue'} /><h3 title={p.name}>{p.name}</h3></div>
      <GitBadge projectId={p.id} />
      <p className="card-description">{p.description || '暂时没有简介，添加一些说明吧。'}</p>
      <Tags values={[...new Set([...p.techStack, ...p.tags])].slice(0, 3)} />
      <div className="card-bottom"><span className="card-path" title={p.path}>{p.path}</span><span className="card-time">{!p.available ? <><TriangleAlert size={14} />路径不可用</> : <><Clock3 size={14} />{relativeTime(p.lastOpenedAt)}</>}</span></div>
    </button>
    {!batchMode && <><button type="button" className={`icon-button card-star ${p.favorite ? 'is-favorite' : ''}`} onClick={onFavorite} aria-label={`${p.favorite ? '取消收藏' : '收藏'} ${p.name}`}><Star size={20} fill={p.favorite ? 'currentColor' : 'none'} /></button>
    <button type="button" className="icon-button card-more" onClick={onEdit} aria-label={`编辑 ${p.name}`} title="编辑项目"><MoreHorizontal size={19} /></button></>}
  </article>;
}
