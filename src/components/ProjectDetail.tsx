import { useEffect, useState, type ReactNode } from 'react';
import { Clipboard, Code2, ExternalLink, FileText, Folder, GitBranch, Pencil, Star, Terminal, Trash2, X, Clock3, Layers, CalendarDays, StickyNote, TriangleAlert, CircleCheck } from 'lucide-react';
import { categories, type Project, type GitInfo } from '../types';
import { repository } from '../services/projects';
import FolderIcon from './FolderIcon';
import { folderColors, Tags } from './ProjectCard';

export default function ProjectDetail({ project: p, onClose, onEdit, onRemove, onFavorite, onOpen, onCopy, onTag, panelHeader, onDevelopment }: {
  project: Project; onClose: () => void; onEdit: () => void; onRemove: () => void; onFavorite: () => void;
  onOpen: (action: string) => void; onCopy: () => void; onTag: (tag: string) => void; panelHeader?: ReactNode; onDevelopment?: () => void;
}) {
  const [git, setGit] = useState<GitInfo | null>(null);
  const [gitError, setGitError] = useState('');
  useEffect(() => {
    let active = true;
    setGit(null); setGitError('');
    repository.gitInfo(p.id).then(info => { if (active) setGit(info); }, error => { if (active) setGitError(error instanceof Error ? error.message : String(error)); });
    return () => { active = false; };
  }, [p.id, p.path, p.updatedAt]);
  const date = (v: string | null) => v ? new Date(v).toLocaleString('zh-CN', { year: 'numeric', month: '2-digit', day: '2-digit', hour: '2-digit', minute: '2-digit' }) : '尚未打开';
  return <aside className="detail-panel" aria-label="项目详情">
    <div className="detail-caption">{panelHeader || <span>项目详情</span>}<button className="icon-button" aria-label="收起项目详情" onClick={onClose}><X size={17} /></button></div>
    <div className="detail-title"><FolderIcon color={folderColors[p.category] || 'blue'} /><h2 title={p.name}>{p.name}</h2><button className={`icon-button ${p.favorite ? 'is-favorite' : ''}`} aria-label={p.favorite ? '取消当前项目收藏' : '收藏当前项目'} onClick={onFavorite}><Star size={23} fill={p.favorite ? 'currentColor' : 'none'} /></button></div>
    <p className="detail-description">{p.description || '为项目添加简介，让下一次回到这里更轻松。'}</p>
    <Tags values={p.techStack} onClick={onTag} />
    {!p.available && <div className="notice warning"><TriangleAlert size={16} />目录暂时不可用，索引已保留。<button onClick={onEdit}>重新定位</button></div>}
    <dl className="detail-meta">
      <div><dt><Folder size={16} />项目路径</dt><dd className="meta-path" title={p.path}>{p.path}</dd><button className="icon-button" aria-label="复制项目路径" onClick={onCopy}><Clipboard size={16} /></button></div>
      <div><dt><Layers size={16} />项目类型</dt><dd>{categories[p.category]}</dd></div>
      <div><dt><Layers size={16} />Git 仓库</dt><dd className="git-remote">{gitError ? '读取不可用' : !git ? '读取中…' : !git.repository ? '未初始化 Git' : git.remote || '未配置 origin 远程'}</dd></div>
      <div><dt><GitBranch size={16} />当前分支</dt><dd>{git?.branch || (git?.repository ? '分离 HEAD' : '—')}</dd></div>
      <div><dt><CircleCheck size={16} />项目状态</dt><dd><span className={`project-availability ${p.available ? '' : 'offline'}`} />{p.available ? '路径可用' : '路径不可用'}</dd></div>
      <div><dt><CalendarDays size={16} />添加时间</dt><dd>{date(p.createdAt)}</dd></div>
      <div><dt><Clock3 size={16} />资料更新</dt><dd>{date(p.updatedAt)}</dd></div>
      <div><dt><Clock3 size={16} />最近打开</dt><dd>{date(p.lastOpenedAt)}</dd></div>
      <div><dt><Code2 size={16} />打开工具</dt><dd>{({ system: '跟随全局设置', code: 'VS Code', idea: 'IntelliJ IDEA', cursor: 'Cursor', custom: '自定义程序' })[p.defaultIde]}</dd></div>
    </dl>
    {onDevelopment && <button className="button development-cta" onClick={onDevelopment}><GitBranch size={16} />查看开发进度</button>}
    {gitError && <p className="muted git-message" role="status">{gitError}</p>}
    <section className="detail-section"><div className="section-heading"><h3>项目简介</h3><button className="icon-button" aria-label="编辑项目简介" onClick={onEdit}><Pencil size={16} /></button></div><p className="full-description">{p.description || '暂无简介'}</p></section>
    <section className="detail-section"><h3>快捷操作</h3><div className="quick-actions">
      <button className="button primary idea-action" onClick={() => onOpen('idea')}><Code2 size={17} />打开 IDEA</button>
      <button className="button" onClick={() => onOpen('terminal')}><Terminal size={17} />打开终端</button>
      <button className="button" onClick={() => onOpen('explorer')}><Folder size={17} />打开目录</button>
    </div><details className="more-project-actions"><summary>更多操作</summary><div className="quick-actions">
      <button className="button" onClick={() => onOpen('system')}><Code2 size={17} />默认工具打开</button>
      <button className="button" onClick={() => onOpen('readme')}><FileText size={17} />查看 README</button>
      <button className="button" onClick={onEdit}><Pencil size={16} />编辑资料</button>
      <button className="button" onClick={onCopy}><Clipboard size={16} />复制路径</button>
    </div><div className="other-tools"><span>其他工具</span>{[['code', 'VS Code'], ['cursor', 'Cursor']].map(([action, label]) => <button key={action} onClick={() => onOpen(action)}>{label}<ExternalLink size={12} /></button>)}</div></details></section>
    <section className="detail-section"><div className="section-heading"><h3>相关标签</h3><button className="icon-button" aria-label="编辑项目标签" onClick={onEdit}><Pencil size={16} /></button></div>{p.tags.length ? <Tags values={p.tags} onClick={onTag} /> : <p className="muted">暂无标签</p>}</section>
    {p.notes && <section className="detail-section"><h3><StickyNote size={16} />项目备注</h3><p className="full-description">{p.notes}</p></section>}
    <button className="remove-project" onClick={onRemove}><Trash2 size={15} />从项目库移除</button><p className="safety-note">仅移除索引，本地项目文件始终保留。</p>
  </aside>;
}
