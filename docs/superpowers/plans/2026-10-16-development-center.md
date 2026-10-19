# 开发状态中心实施计划

**目标：** 按已确认方案建立跨项目开发状态中心及五标签项目详情。
**架构：** 新增纯统计服务和汇总组件，复用现有 Git、ActivityPanel、RecordsPanel；DevelopmentWorkspace 负责导航。
**技术栈：** React / TypeScript / Vitest / CSS，现有 Tauri 接口。

## 全局约束
真实本地数据；当前分支全部作者；本地自然日及周一周界；未知不当零；不提交、不初始化 Git，不改数据库。

## 任务
- [x] 统计：新增 src/services/developmentCenter.ts 及测试。以固定日期覆盖本周/最近七天、未来时间排除和不完整读取；先运行失败测试，再实现统计函数。
- [x] 汇总：新增 DevelopmentCenter.tsx，双工作任务读取各项目状态和活动，取消过期结果；提供刷新、搜索、状态筛选、三个指标、七日趋势及项目表格。
- [x] 详情：修改 DevelopmentWorkspace.tsx 和测试，默认概览、五个标签及原有提交定位/草稿入口；概览复用活动组件和任务计数。
- [x] 样式：development.css 增加响应式指标卡、七日柱状图、统计提示及表格布局。
- [x] 验证：npm.cmd test -- --maxWorkers=1 --pool=threads；npm.cmd run build；检查读取错误、非 Git、空项目、部分统计与刷新。

本轮直接执行已获授权的实现；目录无 Git 元数据，不创建工作树和提交。
