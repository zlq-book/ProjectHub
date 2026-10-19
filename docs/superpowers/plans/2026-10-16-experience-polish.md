# ProjectHub 整体体验优化实施计划

按用户确认的整体优化范围执行，参考同日 experience-polish-design。当前目录无 Git，保留现有改动，不初始化、提交或重构无关后端。

- [x] 1. 先增加 App 场景测试：独立首页入口，功能页不展示无效搜索，Ctrl+K 返回项目检索，跨导航恢复筛选。修改 `src/App.tsx` 及测试，新增轻量首页入口组件。
- [x] 2. 增加周报项目检索／选中保持、材料检索、保存快捷键和历史版本切换交互测试。修改 `src/components/weekly-report/WeeklyReportWorkspace.tsx`，拆出检索纯函数至 `src/services/weeklyReport.ts`；保持既有后台接口。
- [x] 3. 统一体验样式和信息层级，折叠低频设置；修改 `src/components/SettingsDialog.tsx`、`src/components/development/DevelopmentCenter.tsx`、新增 `src/styles/experience.css` 并在 main 注册。不删除真实统计或功能。
- [x] 4. 前端全量测试、生产构建、四宽度双主题页面检查；按发现的问题回归修复，写验收记录。此次不自动覆盖之前已生成的 1.5.0 安装包。

验证命令：`npm.cmd test -- --maxWorkers=2`、`npm.cmd run build`。交互使用已有 Testing Library／Vitest，布局脚本使用工作区提供的 Playwright 和已安装 Edge，不依赖下载浏览器。

完成结果：前端 20 个文件、107 项测试通过；生产构建通过；32 个页面视图检查通过。验收记录见 docs/2026-10-16-整体体验优化验收记录.md。未重新打包或执行原生安装验收。

后续交付：用户追加要求重新生成安装包，已于 2026-10-16 17:03 完成；版本 1.5.0，校验与产物记录见整体体验优化验收记录及 Windows打包记录。
