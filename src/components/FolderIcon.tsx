export default function FolderIcon({ color = 'blue', small = false }: { color?: string; small?: boolean }) {
  return <span className={`folder-icon folder-${color} ${small ? 'folder-small' : ''}`} aria-hidden="true"><span /></span>;
}
