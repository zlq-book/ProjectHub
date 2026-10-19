import { describe, expect, it } from 'vitest';
import { filterProjects, previewRepository } from './projects';
import type { ProjectInput } from '../types';

const input: ProjectInput = {
  name: '环境平台', path: 'D:\\Projects\\EnvAgent', description: '环境数据分析', category: 'backend',
  tags: ['监测'], techStack: ['Spring Boot'], favorite: false, notes: '', defaultIde: 'idea',
};
describe('项目检索与索引管理', () => {
  it('标签重命名合并重复并保持顺序，不改变技术栈和其他项目', async () => {
    const repo = previewRepository([]);
    const a = await repo.create({ ...input, tags: ['监测', '平台', '监测'], techStack: ['监测'] });
    const b = await repo.create({ ...input, path: 'E:\\Other' });
    const [changed] = await repo.changeTag([a.id, a.id], '监测', ' 平台 ');
    expect(changed.tags).toEqual(['平台']); expect(changed.techStack).toEqual(['监测']);
    expect((await repo.list()).find(p => p.id === b.id)?.tags).toEqual(['监测']);
  });
  it('移除标签保留项目、技术栈和其他标签', async () => {
    const repo = previewRepository([]);
    const a = await repo.create({ ...input, tags: ['监测', '保留'], techStack: ['监测'] });
    const [changed] = await repo.changeTag([a.id], '监测', null);
    expect(changed.tags).toEqual(['保留']); expect(changed.techStack).toEqual(['监测']);
    expect(await repo.list()).toHaveLength(1);
  });
  it('标签操作发现记录失效时整批回滚', async () => {
    const repo = previewRepository([]);
    const a = await repo.create(input);
    const b = await repo.create({ ...input, path: 'E:\\Other', tags: ['其他'] });
    await expect(repo.changeTag([a.id, b.id], '监测', null)).rejects.toThrow('标签已变化');
    await expect(repo.changeTag([a.id, 'missing'], '监测', null)).rejects.toThrow('不存在');
    expect((await repo.list())[0].tags).toEqual(['监测']);
  });
  it('标签操作存储失败时数据和更新时间均不变', async () => {
    const seed = await previewRepository([]).create(input);
    const repo = previewRepository([seed], () => { throw new Error('存储失败'); });
    await expect(repo.changeTag([seed.id], '监测', '新标签')).rejects.toThrow('存储失败');
    expect(await repo.list()).toEqual([seed]);
  });
  it('标签操作拒绝空白目标、超长目标、相同名称和空操作范围', async () => {
    const repo = previewRepository([]); const a = await repo.create(input);
    for (const target of [' ', '😀'.repeat(81), ' 监测 ']) {
      await expect(repo.changeTag([a.id], '监测', target)).rejects.toThrow();
    }
    await expect(repo.changeTag([], '监测', null)).rejects.toThrow();
    expect((await repo.list())[0].tags).toEqual(['监测']);
  });
  it('标签与技术栈可按各自来源精确筛选', async () => {
    const repo = previewRepository([]);
    const a = await repo.create({ ...input, tags: ['Java'], techStack: ['React'] });
    const b = await repo.create({ ...input, path: 'E:\\Other', tags: ['其他'], techStack: ['Java'] });
    const filters = { query: '', category: '', scope: 'all', status: '', tag: 'Java', sort: 'updated' };
    expect(filterProjects([a, b], { ...filters, tagSource: 'tags' }).map(p => p.id)).toEqual([a.id]);
    expect(filterProjects([a, b], { ...filters, tagSource: 'techStack' }).map(p => p.id)).toEqual([b.id]);
  });
  it('统一处理 NEL 与 BOM 空白，不保存纯空白目标', async () => {
    const repo = previewRepository([]); const a = await repo.create(input);
    await expect(repo.changeTag([a.id], '监测', '\u0085\uFEFF')).rejects.toThrow('1–80');
    const [changed] = await repo.changeTag([a.id], '监测', '\u0085\uFEFF新标签\u0085');
    expect(changed.tags).toEqual(['新标签']);
  });
  it('名称、简介、路径、标签和技术栈均可搜索且忽略大小写', async () => {
    const repo = previewRepository([]);
    const project = await repo.create(input);
    for (const query of ['环境平台', '数据分析', 'd:\\projects', '监测', 'SPRING']) {
      expect(filterProjects([project], { query, category: '', scope: 'all', status: '', tag: '', sort: 'updated' })).toHaveLength(1);
    }
  });
  it('规范化路径阻止大小写、斜杠、尾部分隔符导致的重复录入', async () => {
    const repo = previewRepository([]);
    await repo.create(input);
    await expect(repo.create({ ...input, path: 'd:/projects/ENVAGENT/' })).rejects.toThrow('已录入');
  });
  it('收藏和最近打开筛选只包含对应项目', async () => {
    const repo = previewRepository([]);
    const project = await repo.create(input);
    const filters = { query: '', category: '', scope: 'favorites', status: '', tag: '', sort: 'updated' };
    expect(filterProjects([project], filters)).toHaveLength(0);
    const updated = await repo.update(project.id, { ...input, favorite: true });
    expect(filterProjects([updated], filters)).toHaveLength(1);
    expect(filterProjects([updated], { ...filters, scope: 'recent' })).toHaveLength(0);
  });
  it('更新不可覆盖已有路径，删除只移除索引', async () => {
    const repo = previewRepository([]);
    const first = await repo.create(input);
    const second = await repo.create({ ...input, path: 'E:\\Second' });
    await expect(repo.update(second.id, input)).rejects.toThrow('已录入');
    await repo.remove(first.id);
    expect((await repo.list()).map(p => p.id)).toEqual([second.id]);
  });
  it('写入失败时不改变内存中的项目库', async () => {
    const original = await previewRepository([]).create(input);
    const repo = previewRepository([original], () => { throw new Error('存储空间不足'); });
    await expect(repo.create({ ...input, path: 'E:\\New' })).rejects.toThrow('存储空间不足');
    await expect(repo.update(original.id, { ...input, name: '失败的修改' })).rejects.toThrow('存储空间不足');
    await expect(repo.remove(original.id)).rejects.toThrow('存储空间不足');
    expect(await repo.list()).toEqual([original]);
  });
  it('多个搜索词可以分别匹配名称和技术栈', async () => {
    const project = await previewRepository([]).create(input);
    expect(filterProjects([project], { query: '环境平台 SPRING', category: '', scope: 'all', status: '', tag: '', sort: 'updated' })).toHaveLength(1);
  });
  it('最近打开按真实时间排序而不是带时区的字符串', async () => {
    const project = await previewRepository([]).create(input);
    const older = { ...project, id: 'older', lastOpenedAt: '2026-10-13T10:00:00+08:00' };
    const newer = { ...project, id: 'newer', lastOpenedAt: '2026-10-13T03:00:00Z' };
    expect(filterProjects([older, newer], { query: '', category: '', scope: 'recent', status: '', tag: '', sort: 'updated' }).map(p => p.id)).toEqual(['newer', 'older']);
  });
  it('预览录入也拒绝无效分类和超过上限的名称', async () => {
    const repo = previewRepository([]);
    await expect(repo.create({ ...input, name: '长'.repeat(121) })).rejects.toThrow('120');
    await expect(repo.create({ ...input, category: 'invalid' as ProjectInput['category'] })).rejects.toThrow('分类');
  });
  it('批量更新去重 ID 并只持久化一次', async () => {
    const original = await previewRepository([]).create(input);
    let writes = 0;
    const repo = previewRepository([original], () => { writes++; });
    const updated = await repo.batchUpdate([original.id, original.id], { favorite: true, category: 'ai' });
    expect(updated).toHaveLength(1);
    expect(updated[0]).toMatchObject({ favorite: true, category: 'ai' });
    expect(writes).toBe(1);
  });
  it('批量修改包含不存在的项目时不会部分保存', async () => {
    const original = await previewRepository([]).create(input);
    let writes = 0;
    const repo = previewRepository([original], () => { writes++; });
    await expect(repo.batchUpdate([original.id, 'missing'], { favorite: true })).rejects.toThrow('不存在');
    await expect(repo.batchRemove([original.id, 'missing'])).rejects.toThrow('不存在');
    expect(await repo.list()).toEqual([original]);
    expect(writes).toBe(0);
  });
  it('批量移除写入失败保留全部记录', async () => {
    const initial = previewRepository([]);
    const first = await initial.create(input);
    const second = await initial.create({ ...input, path: 'E:\\Second' });
    const repo = previewRepository([first, second], () => { throw new Error('存储失败'); });
    await expect(repo.batchRemove([first.id, second.id])).rejects.toThrow('存储失败');
    expect(await repo.list()).toEqual([first, second]);
  });
  it('候选批量导入跳过已有路径及批内重复', async () => {
    const repo = previewRepository([]);
    await repo.create(input);
    const result = await repo.importProjects([input, { ...input, path: 'E:\\Second' }, { ...input, path: 'e:/SECOND/' }]);
    expect(result.projects).toHaveLength(1);
    expect(result.skipped).toHaveLength(2);
    expect(await repo.list()).toHaveLength(2);
  });
  it('候选导入包含非法输入时不会部分写入', async () => {
    const repo = previewRepository([]);
    await expect(repo.importProjects([input, { ...input, name: '', path: 'E:\\Bad' }])).rejects.toThrow('名称');
    expect(await repo.list()).toHaveLength(0);
  });
  it('批量更新忽略未指定的可选字段', async () => {
    const repo = previewRepository([]);
    const project = await repo.create({ ...input, favorite: true });
    const [updated] = await repo.batchUpdate([project.id], { category: 'ai', favorite: undefined });
    expect(updated.favorite).toBe(true);
    expect(updated.category).toBe('ai');
  });
  it('候选导入持久化失败时整个项目库保持不变', async () => {
    const repo = previewRepository([], () => { throw new Error('存储失败'); });
    await expect(repo.importProjects([input, { ...input, path: 'E:\\Second' }])).rejects.toThrow('存储失败');
    expect(await repo.list()).toHaveLength(0);
  });
});
