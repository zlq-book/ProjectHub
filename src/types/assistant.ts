import type { Category, Project } from './index';

export type AssistantRequest = {
  sessionId: string; requestId: string; prompt: string; scopeIds: string[];
  referenceTime: string; history: { role: 'user' | 'assistant'; content: string }[]; contextIds: string[];
};
export type AssistantItem = {
  project: Project; evidence: { source: string; quote: string }[];
  certainty: 'confirmed' | 'uncertain'; description?: string; category?: Category;
};
export type AssistantResult = {
  kind: 'git' | 'search' | 'description' | 'categories' | 'readContext' | 'notice'; text: string;
  items: AssistantItem[]; proposalId?: string; toolSteps: string[]; scopeCount: number;
};
export type AssistantApply = { sessionId: string; proposalId: string; changes: { id: string; value: string }[] };
