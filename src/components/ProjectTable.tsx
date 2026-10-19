import { Star, TriangleAlert } from 'lucide-react';
import type { Project } from '../types';
import FolderIcon from './FolderIcon';
import { folderColors, relativeTime, Tags } from './ProjectCard';
import GitBadge from './development/GitBadge';

type Props = {
  projects: Project[]; selectedId: string | null; batchMode: boolean; checkedIds: Set<string>; busy: boolean;
  onSelect: (project: Project) => void; onOpen: (project: Project) => void;
  onCheck: (id: string) => void; onFavorite: (project: Project) => void;
};

export default function ProjectTable({ projects, selectedId, batchMode, checkedIds, busy, onSelect, onOpen, onCheck, onFavorite }: Props) {
  return <div className="project-table-scroll"><table className="project-table" aria-label="项目列表">
    <colgroup><col className="name-column" /><col className="description-column" /><col className="stack-column" /><col className="path-column" /><col className="time-column" /></colgroup>
    <thead><tr><th scope="col">项目名称</th><th scope="col">简介</th><th scope="col">技术栈</th><th scope="col">路径</th><th scope="col">资料更新</th></tr></thead>
    <tbody>{projects.map(p => {
      const selected = batchMode ? checkedIds.has(p.id) : selectedId === p.id;
      return <tr key={p.id} className={selected ? 'selected' : ''}
        onClick={() => { if (!busy) batchMode ? onCheck(p.id) : onSelect(p); }}
        onDoubleClick={() => { if (!busy && !batchMode) onOpen(p); }}>
        <td><div className="table-name">
          {batchMode ? <input type="checkbox" aria-label={`选择 ${p.name}`} checked={checkedIds.has(p.id)} disabled={busy} onClick={e => e.stopPropagation()} onChange={() => onCheck(p.id)} /> : <FolderIcon color={folderColors[p.category] || 'blue'} small />}
          <button className="project-name" disabled={busy} aria-label={`${batchMode ? '切换选择' : '查看'} ${p.name}`} aria-pressed={selected} title={p.name}>{p.name}</button>
          {!batchMode && <button className={`icon-button table-star ${p.favorite ? 'is-favorite' : ''}`} aria-label={`${p.favorite ? '取消收藏' : '收藏'} ${p.name}`} onDoubleClick={e => e.stopPropagation()} onClick={e => { e.stopPropagation(); onFavorite(p); }}><Star size={15} fill={p.favorite ? 'currentColor' : 'none'} /></button>}
        </div><GitBadge projectId={p.id} /></td>
        <td title={p.description}><span className="table-text">{p.description || '暂无简介'}</span></td>
        <td><Tags values={p.techStack} /></td>
        <td title={p.path}><span className="table-text table-path">{p.path}</span></td>
        <td title={new Date(p.updatedAt).toLocaleString('zh-CN')}><span className="table-updated">{!p.available && <TriangleAlert size={14} aria-label="路径不可用" />}{relativeTime(p.updatedAt)}</span></td>
      </tr>;
    })}</tbody>
  </table></div>;
}
