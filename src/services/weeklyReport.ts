import { invoke } from '@tauri-apps/api/core';
import { native } from './projects';
export function matchesReportSearch(query:string,values:(string|null|undefined)[]):boolean {const words=query.trim().toLocaleLowerCase().split(/\s+/).filter(Boolean);const text=values.filter(Boolean).join(' ').toLocaleLowerCase();return words.every(word=>text.includes(word));}
import type { ReportContext, Collection, GeneratedDraft, PreviewInput, ReportContent, ReportGroup, ReportHints, ReportPreferences, ReportPreview, ReportRange, ReportSource, ReportVersion } from '../types/weeklyReport';
const dateText = (d: Date) => `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`;
export function makeRange(startDate: string, endDate: string): ReportRange {
    const start = new Date(`${startDate}T00:00:00`), end = new Date(`${endDate}T00:00:00`);
    end.setDate(end.getDate() + 1);
    return { startDate, endDate, timeZone: Intl.DateTimeFormat().resolvedOptions().timeZone, startUtc: start.toISOString(), endExclusiveUtc: end.toISOString() };
}
export function defaultRange(now = new Date()): ReportRange { const monday = new Date(now.getFullYear(), now.getMonth(), now.getDate()); monday.setDate(monday.getDate() - (monday.getDay() + 6) % 7); const friday = new Date(monday); friday.setDate(friday.getDate() + 4); return makeRange(dateText(monday), dateText(friday)); }
export function validateContent(c: ReportContent, exporting = false): string {
    if (c.selectedOutcomeIds.length > 4)
        return '主要完成工作最多选择 4 条';
    if (c.difficulties.length > 3)
        return '困难最多 3 条';
    if (c.plans.length > 4)
        return '下周计划最多 4 条';
    if (new Set(c.selectedOutcomeIds).size !== c.selectedOutcomeIds.length || c.selectedOutcomeIds.some(id => !c.outcomes.some(o => o.id === id)))
        return '成果选择无效';
    if (c.range.startDate > c.range.endDate)
        return '开始日期不能晚于结束日期';
    if (c.outcomes.some(o => o.text.length > 4000) || [...c.difficulties, ...c.plans].some(s => s.length > 4000) || c.reflection.length > 8000)
        return '正文超过长度限制';
    if (exporting && (!c.department.trim() || !c.reporter.trim()))
        return '请填写部门和填报人';
    if (exporting && !c.selectedOutcomeIds.length && !c.difficulties.some(s => s.trim()) && !c.plans.some(s => s.trim()) && !c.reflection.trim())
        return '请至少填写一项正文';
    return '';
}
function call<T>(name: string, args: Record<string, unknown> = {}): Promise<T> { return native ? invoke<T>(`weekly_report_${name}`, args) : Promise.reject(new Error('请在 ProjectHub 桌面版中读取、生成和保存真实周报。')); }
export const weeklyReportApi = {
    authors: (projectIds: string[]) => call<ReportPreferences['authors']>('authors', { projectIds }),
    preferences: () => call<ReportPreferences>('preferences'), savePreferences: (input: ReportPreferences) => call<void>('save_preferences', { input }),
    collect: (input: {
        projectIds: string[];
        range: ReportRange;
        authors: ReportPreferences['authors'];
        requestId: string;
    }) => call<Collection>('collect', { input }),
    cancelCollection: (id: string) => call<void>('cancel_collection', { id }),
    preview: (input: PreviewInput) => call<ReportPreview>('preview', { input }), generate: (token: string) => call<GeneratedDraft>('generate', { token }), cancel: (token: string) => call<void>('cancel', { token }),
    list: () => call<ReportGroup[]>('list'), get: (id: string) => call<ReportGroup>('get', { id }),
    save: (input: {
        reportId: string | null;
        versionId: string | null;
        parentVersionId: string | null;
        revision: number | null;
        content: ReportContent;
        sources: ReportSource[];
        hints: ReportHints;
        context?: ReportContext;
    }) => call<ReportGroup>('save_version', { input }),
    adopt: (reportId: string, versionId: string) => call<ReportGroup>('adopt_version', { reportId, versionId }),
    export: (version: ReportVersion, format: 'xlsx' | 'md') => call<{
        status: 'saved' | 'cancelled';
        path?: string;
    }>('export', { id: version.id, revision: version.revision, format }),
};
