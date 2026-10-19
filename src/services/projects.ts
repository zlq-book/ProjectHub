import { invoke, isTauri } from '@tauri-apps/api/core';
import { trimTagName } from './tags';
import { categories, defaultSettings, type AiAnalysis, type AiConnectionResult, type AiSettings, type AiSettingsInput, type BatchPatch, type Detection, type Filters, type GitInfo, type ImportResult, type Project, type ProjectInput, type ScanResult, type Settings } from '../types';

export const native = isTauri();
export function errorMessage(error: unknown) { return error instanceof Error ? error.message : String(error); }
const normalized = (path: string) => path.trim().replace(/\//g, '\\').replace(/\\+$/, '').toLowerCase();
export function filterProjects(projects: Project[], filters: Filters) {
  const terms = filters.query.trim().toLocaleLowerCase().split(/\s+/).filter(Boolean);
  const timestamp = (value: string | null) => value ? Date.parse(value) || 0 : 0;
  return projects.filter(p =>
    terms.every(term => [p.name, p.description, p.path, ...p.tags, ...p.techStack].join(' ').toLocaleLowerCase().includes(term)) &&
    (!filters.category || p.category === filters.category) &&
    (filters.scope !== 'favorites' || p.favorite) &&
    (filters.scope !== 'recent' || !!p.lastOpenedAt) &&
    (!filters.tag || (filters.tagSource ? p[filters.tagSource] : [...p.tags, ...p.techStack]).includes(filters.tag)) &&
    (!filters.status || (filters.status === 'available' ? p.available : !p.available))
  ).sort((a, b) => filters.sort === 'name' ? a.name.localeCompare(b.name, 'zh-CN') :
    timestamp(filters.sort === 'opened' || filters.scope === 'recent' ? b.lastOpenedAt : b.updatedAt) -
    timestamp(filters.sort === 'opened' || filters.scope === 'recent' ? a.lastOpenedAt : a.updatedAt) || a.name.localeCompare(b.name, 'zh-CN'));
}

export function previewRepository(initial: Project[], persist?: (projects: Project[]) => void) {
  let projects = structuredClone(initial);
  function write(next: Project[]) { persist?.(structuredClone(next)); projects = next; }
  function check(input: ProjectInput, id?: string, duplicates = true) {
    if (!input.name.trim()) throw new Error('请输入项目名称');
    if ([...input.name.trim()].length > 120) throw new Error('项目名称最多 120 个字符');
    if (!Object.hasOwn(categories, input.category)) throw new Error('项目分类无效');
    if (!['system', 'code', 'idea', 'cursor', 'custom'].includes(input.defaultIde)) throw new Error('打开工具无效');
    if ([...input.description].length > 4000 || [...input.notes].length > 8000) throw new Error('简介或备注过长');
    for (const tags of [input.tags, input.techStack]) {
      if (tags.length > 50 || tags.some(tag => [...tag].length > 80)) throw new Error('最多支持 50 个标签，每个最多 80 个字符');
    }
    if (!/^(?:[a-z]:[\\/]|\\\\[^\\]+\\[^\\]+)/i.test(input.path.trim())) throw new Error('请输入 Windows 绝对目录路径');
    if (duplicates && projects.some(p => p.id !== id && normalized(p.path) === normalized(input.path))) throw new Error('该路径已录入，请查看已有项目');
  }
  function selectedIds(ids: string[]) {
    const unique = new Set(ids);
    if (!unique.size || unique.size > 500) throw new Error('请选择 1–500 个项目');
    if ([...unique].some(id => !projects.some(p => p.id === id))) throw new Error('部分项目不存在，请刷新后重试');
    return unique;
  }
  return {
    async list() { return structuredClone(projects); },
    async create(input: ProjectInput) {
      check(input);
      const project: Project = { ...structuredClone(input), name: input.name.trim(), path: input.path.trim(), id: crypto.randomUUID(),
        createdAt: new Date().toISOString(), updatedAt: new Date().toISOString(), lastOpenedAt: null, available: true };
      write([...projects, project]); return structuredClone(project);
    },
    async update(id: string, input: ProjectInput) {
      check(input, id);
      const index = projects.findIndex(p => p.id === id);
      if (index < 0) throw new Error('项目不存在');
      const updated = { ...projects[index], ...structuredClone(input), name: input.name.trim(), path: input.path.trim(), updatedAt: new Date().toISOString() };
      write(projects.map(p => p.id === id ? updated : p)); return structuredClone(updated);
    },
    async remove(id: string) { write(projects.filter(p => p.id !== id)); },
    async changeTag(ids: string[], source: string, target: string | null) {
      const selected = selectedIds(ids);
      if (!trimTagName(source) || [...source].length > 80) throw new Error('源标签无效');
      const name = target === null ? null : trimTagName(target);
      if (name !== null && (!name || [...name].length > 80)) throw new Error('标签名称须为 1–80 个字符');
      if (name === source) throw new Error('新标签名称与原名称相同');
      if (projects.some(p => selected.has(p.id) && !p.tags.includes(source))) throw new Error('部分项目的标签已变化，请返回标签列表后重试');
      const now = new Date().toISOString();
      const updated = projects.map(p => selected.has(p.id) ? {
        ...p, tags: [...new Set(p.tags.flatMap(tag => tag === source ? (name === null ? [] : [name]) : [tag]))], updatedAt: now,
      } : p);
      write(updated); return structuredClone(updated.filter(p => selected.has(p.id)));
    },
    async batchUpdate(ids: string[], patch: BatchPatch) {
      const selected = selectedIds(ids);
      if (patch.favorite === undefined && patch.category === undefined) throw new Error('没有要更新的资料');
      const changes: BatchPatch = {};
      if (patch.favorite !== undefined) changes.favorite = patch.favorite;
      if (patch.category !== undefined) changes.category = patch.category;
      const updated = projects.map(p => selected.has(p.id) ? { ...p, ...changes, updatedAt: new Date().toISOString() } : p);
      for (const p of updated.filter(p => selected.has(p.id))) check(p, p.id);
      write(updated); return structuredClone(updated.filter(p => selected.has(p.id)));
    },
    async batchRemove(ids: string[]) { const selected = selectedIds(ids); write(projects.filter(p => !selected.has(p.id))); },
    async importProjects(inputs: ProjectInput[]): Promise<ImportResult> {
      if (!inputs.length || inputs.length > 500) throw new Error('请选择 1–500 个候选项目');
      const keys = new Set(projects.map(p => normalized(p.path)));
      const imported: Project[] = []; const skipped: string[] = [];
      for (const item of inputs) {
        check(item, undefined, false);
        const path = normalized(item.path);
        if (keys.has(path)) { skipped.push(`${item.path}：已录入`); continue; }
        keys.add(path);
        const now = new Date().toISOString();
        imported.push({ ...structuredClone(item), name: item.name.trim(), path: item.path.trim(), id: crypto.randomUUID(), createdAt: now, updatedAt: now, lastOpenedAt: null, available: true });
      }
      if (imported.length) write([...projects, ...imported]);
      return { projects: structuredClone(imported), skipped };
    },
  };
}

function sampleProjects(): Project[] {
  const rows: Array<[string, string, Project['category'], string[], string[], boolean]> = [
    ['env-agent-backend', '环境监测 AI 智能体后端服务，基于 Spring Boot + Spring AI Alibaba，提供环境数据分析、预警判断、报告生成功能。', 'backend', ['Spring Boot', 'Java', 'MySQL'], ['AI', '环境监测'], true],
    ['env-agent-frontend', '环境监测 AI 智能体前端项目，基于 Vue3 + Element Plus，提供可视化地图和智能问答。', 'frontend', ['Vue3', 'Element Plus', 'TypeScript'], ['环境监测'], false],
    ['spring-ai-demo', 'Spring AI Alibaba 示例项目，包含 ReactAgent、Graph、Skills 等示例代码。', 'ai', ['Spring Boot', 'AI', 'Java'], ['学习'], false],
    ['air-quality-agent', '基于实时数据的空气质量异动预警智能体，包含数据采集、预警规则、报告生成功能。', 'ai', ['Python', 'AI'], ['大气监测'], false],
    ['data-sync-tool', '多源 SQL Server 数据采集与汇总工具，Python 脚本 + Windows 定时任务。', 'tool', ['Python', 'SQL Server'], ['数据采集'], false],
    ['image-generator', '文生图网站原型，基于 Stable Diffusion + Vue3，实现图片生成与历史记录功能。', 'frontend', ['Vue3', 'Stable Diffusion'], ['图片生成'], true],
    ['sql-import-plugin', 'Excel 导入数据库插件，支持 sensor_code 更新 sensor_name。', 'tool', ['Java', 'MySQL'], ['工具'], false],
    ['desktop-pet', '桌面电子宠物，透明悬浮窗，支持任务系统和道具。', 'learning', ['Tauri', 'TypeScript'], ['桌面应用'], false],
  ];
  return rows.map(([name, description, category, techStack, tags, favorite], i) => ({
    name, description, category, techStack, tags, favorite, id: `demo-${i}`, path: `D:\\Projects\\${name}`, notes: '', defaultIde: 'system',
    createdAt: new Date(Date.now() - 35 * 86400000).toISOString(), updatedAt: new Date(Date.now() - i * 86400000).toISOString(),
    lastOpenedAt: i < 4 ? new Date(Date.now() - (i * 24 + 2) * 3600000).toISOString() : null, available: true,
  }));
}
const previewKey = 'projecthub-preview-v1';
function loadPreview() {
  try { const value = localStorage.getItem(previewKey); return value ? JSON.parse(value) as Project[] : sampleProjects(); }
  catch { return sampleProjects(); }
}
const demo = previewRepository(native ? [] : loadPreview(), p => localStorage.setItem(previewKey, JSON.stringify(p)));
export const repository = {
  async aiModels(input: AiSettingsInput): Promise<string[]> {
    if (!native) throw new Error('获取模型列表需要桌面版，浏览器不发送密钥。');
    return invoke('list_ai_models', { input });
  },
  async testAiConnection(input: AiSettingsInput): Promise<AiConnectionResult> {
    if (!native) throw new Error('测试连接需要桌面版，浏览器不发送密钥。');
    return invoke('test_ai_connection', { input });
  },
  async aiSettings(): Promise<AiSettings> {
    return native ? invoke('get_ai_settings') : { baseUrl: '', model: '', hasApiKey: false };
  },
  async saveAiSettings(input: AiSettingsInput): Promise<AiSettings> {
    if (!native) throw new Error('AI 配置请在桌面版保存，浏览器不存储密钥。');
    return invoke('save_ai_settings', { input });
  },
  async analyze(path: string, id?: string): Promise<AiAnalysis> {
    if (!native) throw new Error('AI 解析需要桌面版读取本地项目资料。');
    return invoke('analyze_project', { path, id: id || null });
  },
  async gitInfo(id: string): Promise<GitInfo> {
    if (!native) throw new Error('Git 信息请在桌面版查看');
    return invoke('get_project_git', { id });
  },
  list: () => native ? invoke<Project[]>('list_projects') : demo.list(),
  create: (input: ProjectInput) => native ? invoke<Project>('create_project', { input }) : demo.create(input),
  update: (id: string, input: ProjectInput) => native ? invoke<Project>('update_project', { id, input }) : demo.update(id, input),
  remove: (id: string) => native ? invoke<void>('remove_project', { id }) : demo.remove(id),
  changeTag: (ids: string[], source: string, target: string | null) => native ? invoke<Project[]>('change_project_tag', { ids, source, target }) : demo.changeTag(ids, source, target),
  batchUpdate: (ids: string[], patch: BatchPatch) => native ? invoke<Project[]>('batch_update_projects', { ids, favorite: patch.favorite ?? null, category: patch.category ?? null }) : demo.batchUpdate(ids, patch),
  batchRemove: (ids: string[]) => native ? invoke<void>('batch_remove_projects', { ids }) : demo.batchRemove(ids),
  importProjects: (inputs: ProjectInput[]) => native ? invoke<ImportResult>('import_projects', { inputs }) : demo.importProjects(inputs),
  async scan(roots: string[], depth: number, requestId: string): Promise<ScanResult> {
    if (!native) throw new Error('目录扫描需要桌面版。浏览器可以使用项目库中的批量管理功能。');
    return invoke('scan_projects', { roots, depth, requestId });
  },
  async cancelScan(requestId: string) { if (native) await invoke('cancel_scan', { requestId }); },
  async detect(path: string, id?: string): Promise<Detection> {
    if (native) return invoke('detect_project', { path, id: id || null });
    throw new Error('浏览器预览无法读取本地目录，请使用桌面版识别项目。预览中可手动填写示例资料。');
  },
  async choose(): Promise<string | null> {
    if (native) return invoke('choose_project_folder');
    throw new Error('浏览器无法选择本地项目目录，请在桌面版使用此功能。');
  },
  async open(id: string, action: string) {
    if (!native) throw new Error('浏览器预览无法启动本地程序，请使用桌面版。');
    const commands: Record<string, string> = { explorer: 'open_in_explorer', terminal: 'open_terminal', readme: 'open_readme' };
    return invoke(commands[action] || 'open_project', { id, ide: commands[action] ? undefined : action });
  },
  async settings(): Promise<Settings> {
    if (native) return invoke('get_settings');
    try { return JSON.parse(localStorage.getItem('projecthub-preview-settings') || 'null') || structuredClone(defaultSettings); }
    catch { return structuredClone(defaultSettings); }
  },
  async saveSettings(settings: Settings) {
    if (native) return invoke<void>('save_settings', { settings });
    localStorage.setItem('projecthub-preview-settings', JSON.stringify(settings));
  },
  async databasePath(): Promise<string> { return native ? invoke('database_path') : '浏览器示例数据 · localStorage（与桌面数据库独立）'; },
  async backup(action: 'export' | 'import'): Promise<string | null> {
    if (!native) throw new Error('SQLite 备份仅支持桌面版。');
    return invoke(action === 'export' ? 'export_backup' : 'import_backup');
  },
};
