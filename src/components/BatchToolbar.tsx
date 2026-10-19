import { CheckSquare, FolderInput, LoaderCircle, Star, Trash2, X } from 'lucide-react';

export default function BatchToolbar({ count, hiddenCount, allVisibleSelected, busy, onSelectAll, onClear, onFavorite, onCategory, onRemove, onExit }: {
  count: number; hiddenCount: number; allVisibleSelected: boolean; busy: boolean;
  onSelectAll: () => void; onClear: () => void; onFavorite: (favorite: boolean) => void;
  onCategory: () => void; onRemove: () => void; onExit: () => void;
}) {
  return <div className="batch-toolbar" aria-label="批量操作">
    <div className="batch-summary"><CheckSquare size={18} /><strong>已选 {count} 个项目</strong>{hiddenCount > 0 && <span>含 {hiddenCount} 个筛选外项目</span>}</div>
    <div className="batch-actions">
      <button className="button" disabled={busy} onClick={onSelectAll}>{allVisibleSelected ? '取消当前结果选择' : '全选当前结果'}</button>
      <button className="button" disabled={busy || !count} onClick={onClear}>清空选择</button>
      <button className="button" disabled={busy || !count} onClick={() => onFavorite(true)}><Star size={15} />收藏</button>
      <button className="button" disabled={busy || !count} onClick={() => onFavorite(false)}>取消收藏</button>
      <button className="button" disabled={busy || !count} onClick={onCategory}><FolderInput size={15} />调整分类</button>
      <button className="button batch-danger" disabled={busy || !count} onClick={onRemove}><Trash2 size={15} />移除索引</button>
      <button className="icon-button" disabled={busy} aria-label="退出批量管理" onClick={onExit}>{busy ? <LoaderCircle size={17} className="spin" /> : <X size={17} />}</button>
    </div>
    <p>选择跨筛选保留，操作将应用于全部已选项目。</p>
  </div>;
}
