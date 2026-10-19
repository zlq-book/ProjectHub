import type {CommitLink} from './development';
export interface AuthorRule {
    kind: 'email' | 'name';
    value: string;
}
export interface ReportRange {
    startDate: string;
    endDate: string;
    timeZone: string;
    startUtc: string;
    endExclusiveUtc: string;
}
export interface ReportHints {
    difficulties: string;
    plans: string;
    reflection: string;
}
export interface Outcome {
    id: string;
    projectIds: string[];
    text: string;
    sourceIds: string[];
    origin: 'ai' | 'manual';
}
export interface ReportContent {
    department: string;
    reporter: string;
    range: ReportRange;
    outcomes: Outcome[];
    selectedOutcomeIds: string[];
    difficulties: string[];
    plans: string[];
    reflection: string;
}
export interface ReportPreferences {
    department: string;
    reporter: string;
    authors: AuthorRule[];
}
export interface ReportSource {
    id: string;
    kind: 'commit' | 'log';
    projectIds: string[];
    projectName: string;
    title: string;
    body: string;
    repositoryId: string | null;
    sha: string | null;
    author: string;
    authorEmail: string;
    authorTime: string;
    commitTime: string;
    logId: string | null;
    logDate?:string;
    logCommits?:CommitLink[];
    revision: number | null;
    files: string[];
    renames?:[string,string][];
}
export interface ProjectEvidence {
    projectId: string;
    projectName: string;
    path: string;
    repositoryId: string | null;
    head: string | null;
    branch: string | null;
    complete: boolean;
    warning: string | null;
}
export interface Collection {
    id: string;
    range: ReportRange;
    authors: AuthorRule[];
    projects: ProjectEvidence[];
    sources: ReportSource[];
    warnings: string[];
}
export interface ReportVersion {
    context?: ReportContext;
    id: string;
    reportId: string;
    parentVersionId: string | null;
    revision: number;
    content: ReportContent;
    sources: ReportSource[];
    hints: ReportHints;
    createdAt: string;
    updatedAt: string;
}
export interface ReportGroup {
    id: string;
    adoptedVersionId: string;
    versions: ReportVersion[];
    createdAt: string;
}
export interface ReportPreview {
    token: string;
    model: string;
    baseUrl: string;
    text: string;
    bytes: number;
    expiresAt: string;
    warnings: string[];
}
export interface GeneratedDraft {
    outcomes: Outcome[];
    difficulties: string[];
    plans: string[];
    reflection: string;
}
export interface PreviewInput {
    collectionId: string;
    sourceIds: string[];
    diffs: {
        sourceId: string;
        path: string;
    }[];
    hints: ReportHints;
}

export interface ReportContext { authors: AuthorRule[]; projects: ProjectEvidence[]; diffs: {sourceId:string;path:string}[]; model:string; baseUrl:string; warnings:string[] }
