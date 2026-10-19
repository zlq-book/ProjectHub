# ProjectHub AI Assistant 实施计划

> 执行方式：使用 subagent-driven-development 分工实现并审查；在本会话持续执行。当前目录无 Git 元数据，不创建工作树、不暂存或提交文件。

目标：实现已确认设计的自然语言检索、单项目简介与分类整理，右侧详情／助手共用一栏。

架构：React 负责会话、资料读取确认与建议预览；Rust 负责严格意图协议、真实项目查询、证据校验、限额、取消、建议存储；SQLite 在确认时原子应用字段补丁。复用当前模型配置，浏览器不发真实模型请求。

技术：React 19、TypeScript、Tauri 2、Rust、rusqlite、reqwest；不增加运行时依赖。

## 全局约束

- 仅三个首版场景，不接入聊天启动 IDE、模块问答或对比。
- 所有 UI、文档、分析与审查使用中文。项目名、协议字段保留原文。
- 每轮最多 100 个项目、4 次模型请求、8 次工具调用、90 秒；每次模型请求最多 60 秒，元数据每批最多 50 个且序列化不超过 64 KiB。
- 用户主动补充资料时最多 3 个项目；每文件最多 32 KiB、每个项目总计 64 KiB。
- 建议有效期 10 分钟；取消、清空、重启、导入或 API 配置变化使相关建议失效。
- 简介只写 description；分类只写 category。写入前快照校验，批量全成或全败。

## 共享接口

`assistant_run({input})` 的 input：

```ts
type AssistantRequest = {
  sessionId: string; requestId: string; prompt: string; scopeIds: string[];
  referenceTime: string; history: {role: 'user' | 'assistant'; content: string}[];
  contextIds: string[];
};
type AssistantItem = {
  project: Project; evidence: {source: string; quote: string}[];
  certainty: 'confirmed' | 'uncertain'; description?: string; category?: Category;
};
type AssistantResult = {
  kind: 'search' | 'description' | 'categories' | 'readContext' | 'notice';
  text: string; items: AssistantItem[]; proposalId?: string;
  toolSteps: string[]; scopeCount: number;
};
```

`apply_assistant_proposal({input: {sessionId, proposalId, changes: {id, value}[]}})` 返回 `Project[]`。模型协议没有此工具。

`cancel_assistant({sessionId, requestId})` 取消指定任务；`clear_assistant({sessionId})` 清空该会话的任务与建议。requestId 使用 UUID，每轮不复用。contextIds 只能来自本轮 scopeIds，必须由明确的读取按钮提交，初次发送为空。

## 任务 1：后端工具与一致性

文件：新建 `src-tauri/src/services/assistant/{mod.rs,protocol.rs,transport.rs,tests.rs}` 和 `src-tauri/src/db/assistant.rs`；修改 commands、lib、services/mod 与 AI context 的共享可见性。

- [x] 写测试：伪造项目 ID 与证据拒绝；搜索语义匹配；缺省读取授权返回 readContext；简介只写目标字段；逐项目分类原子回滚；重放与取消；导入／配置失效。
- [x] 运行 `cargo test --manifest-path src-tauri/Cargo.toml assistant`，确认新增行为缺失导致失败。
- [x] 实现严格 JSON 意图与候选响应，后端枚举调度、实际项目对象回填、时间条件确定性过滤及限额。形如 `{"kind":"search","...":"..."}` 的意图只选择固定工具，不能传任意命令。
- [x] 实现后端内存建议与事务字段补丁，形式为 `UPDATE projects SET description=?2,updated_at=?3 WHERE id=?1` 或 category 对应语句；在同一事务内先比较全部快照，任何冲突不写。
- [x] 注册共享命令并运行 Rust 测试；记录模拟模型与真实文件系统验证边界。

## 任务 2：前端助手与右栏接入

文件：新建 `src/types/assistant.ts`、`src/services/assistant.ts`、`src/components/assistant/AssistantPanel.tsx`、`AssistantResults.tsx`、测试与 `src/styles/assistant.css`；修改 App、ProjectDetail 及 main 样式引用。

- [x] 先在 `src/App.test.tsx` 加入口与无选中项目的失败测试；组件测试覆盖发送、中文输入法、读取确认、建议编辑和确认保存。
- [x] 运行 `npm.cmd test -- src/App.test.tsx src/components/assistant`，确认缺少入口／组件导致预期失败。
- [x] 按共享接口新增服务；浏览器模式明确提示桌面需求，不模拟 AI。
- [x] 面板消息保存在 App 持久挂载的组件中；隐藏不卸载；新会话调用 clear。使用任务令牌阻止取消后的晚到结果。
- [x] 引入 `panelMode`，with-detail 条件为面板打开且助手模式或详情项目存在。详情组件接收可选页签，独立测试保持兼容。结果查看主动切换详情，保留输入及会话。
- [x] 简介 textarea 与分类行 select 都在显式保存按钮提交；UI 操作锁与应用现有状态协调。
- [x] 全量前端测试和生产构建，检查长文本与响应式。

## 任务 3：集成审查与交付

文件：README 与 `docs/2026-10-15-AI助手验收记录.md`。

- [x] 审查共享接口、预算、资料范围、版本冲突、取消及所有首版验收项；修复重要问题后复审。
- [x] 执行 `npm.cmd test`、`npm.cmd run build`、完整 Rust 测试，确认输出。
- [x] 使用可用浏览器自动化检查真实 UI 布局与右栏切换；使用 Rust 临时目录／SQLite／模拟模型覆盖真实读取与写入，不调用收费模型。
- [x] 更新说明，准确区分自动化、浏览器、桌面、供应商与安装包验证。按实际完成项更新本计划勾选。


## 最终执行结果（2026-10-16）

前端70项、Rust54项测试和生产构建通过；已完成浏览器布局检查与代码复审。否定时间、外围排除关系和Windows README重复预算问题的回归已通过。详见 docs/2026-10-15-AI助手验收记录.md。真实模型、原生GUI和新安装包尚未验证；未执行Git暂存或提交。

