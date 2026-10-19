export const categories = {
  backend: '后端项目', frontend: '前端项目', ai: 'AI 项目',
  tool: '工具项目', learning: '学习项目', other: '其他项目',
} as const;
export type Category = keyof typeof categories;
export type Ide = 'system' | 'code' | 'idea' | 'cursor' | 'custom';
export type ProjectInput = {
  name: string; path: string; description: string; category: Category;
  tags: string[]; techStack: string[]; favorite: boolean; notes: string; defaultIde: Ide;
};
export type Project = ProjectInput & {
  id: string; createdAt: string; updatedAt: string; lastOpenedAt: string | null; available: boolean;
};
export type Detection = { path: string; name: string; description: string; techStack: string[] };
export type GitInfo = { repository: boolean; branch: string | null; remote: string | null };
export type AiSettings = { baseUrl: string; model: string; hasApiKey: boolean };
export type AiSettingsInput = { baseUrl: string; model: string; apiKey: string; clearApiKey: boolean };
export type AiConnectionResult = { model: string; latencyMs: number };
export type AiAnalysis = Pick<ProjectInput, 'name' | 'description' | 'category' | 'techStack' | 'tags'>;
export type Settings = {
  defaultIde: Ide; defaultCategory: Category; theme: 'light' | 'dark' | 'system';
  view: 'grid' | 'list'; idePaths: Record<string, string>;
};
export const defaultSettings: Settings = {
  defaultIde: 'code', defaultCategory: 'other', theme: 'light', view: 'list',
  idePaths: { code: '', idea: '', cursor: '', custom: '' },
};
export type TagSource = 'tags' | 'techStack';
export type Filters = { query: string; category: string; scope: string; status: string; tag: string; sort: string; tagSource?: TagSource };
export type BatchPatch = { favorite?: boolean; category?: Category };
export type ImportResult = { projects: Project[]; skipped: string[] };
export type ScanCandidate = Detection & { existing: boolean };
export type ScanResult = { projects: ScanCandidate[]; warnings: string[]; truncated: boolean; scannedDirectories: number };
