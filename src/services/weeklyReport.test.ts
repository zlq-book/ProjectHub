import { expect, it } from 'vitest';
import { matchesReportSearch } from './weeklyReport';
import { defaultRange, validateContent } from './weeklyReport';
import type { ReportContent } from '../types/weeklyReport';
it('本地检索忽略大小写和首尾空格，空关键词显示全部',()=>{
 expect(matchesReportSearch('  API ',['项目甲','api/service'])).toBe(true);
 expect(matchesReportSearch('不存在',['项目甲','api/service'])).toBe(false);
 expect(matchesReportSearch(' ',['项目甲'])).toBe(true);
});
it('本周默认工作日，周五整天包含，跨年保持正确', () => {
 const range=defaultRange(new Date(2026,9,18,12));
 expect(range.startDate).toBe('2026-10-12'); expect(range.endDate).toBe('2026-10-16');
 expect(defaultRange(new Date(2027,0,1,12)).startDate).toBe('2026-12-28');
});
it('草稿允许空白，但导出需要基本信息且超过四条成果被阻止', () => {
 const c:ReportContent={department:'',reporter:'',range:defaultRange(),outcomes:[],selectedOutcomeIds:[],difficulties:[],plans:[],reflection:''};
 expect(validateContent(c,false)).toBe(''); expect(validateContent(c,true)).toContain('部门');
 c.selectedOutcomeIds=['1','2','3','4','5']; expect(validateContent(c,false)).toContain('4 条');
});
