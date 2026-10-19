# AI 项目解析实施计划

依据：[功能设计](../specs/2026-10-14-ai-project-analysis.md)。用户已选择自定义兼容接口。

技术：React/TypeScript 表单、Tauri Rust 后端、Chat Completions JSON 请求、Windows DPAPI。

- [x] 测试先覆盖结果预览、五字段回填、不自动保存和失败保留表单。
- [x] 新建独立 AI 类型及前端服务，新增配置界面；旧设置不变，密钥不进入浏览器存储。
- [x] 新建后端配置存储、Windows 密钥保护、有限上下文读取与解析服务；增加 Tauri 命令。
- [x] 测试输出校验、资料范围、密钥往返与模拟 HTTP 请求／错误；前端与 Rust 全量回归。
- [x] 独立代码审查、生产构建、更新使用说明和验证范围。

验证记录：`npm.cmd test -- --maxWorkers=1` 8 个文件 51 项通过；`cargo test --offline --manifest-path src-tauri/Cargo.toml` 29 项通过。前端初次并行执行遇到 worker 启动超时，单进程重跑通过。独立只读代码审查未发现阻断问题。浏览器验证侧栏配置入口、独立配置窗口、新增项目 AI 按钮与未配置提示。真实模型服务与真实桌面窗口尚未联调；没有重新生成安装包。

不调用真实模型、不写项目源文件、不自动保存项目、不生成新安装包、不执行 Git 提交。
