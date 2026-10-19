# Windows 安装包构建记录

日期：2026-10-14。版本：0.3.0，架构：x64，安装方式：当前用户 NSIS。

## 下载超时的处理

用户运行 `npm.cmd run package:windows`，在下载 Tauri 官方 `nsis-3.11.zip` 时出现 `timeout: global`。本次检查确认应用 release 可执行文件已存在；直接连接 GitHub 仍出现连接超时/重置。

本机系统代理处于启用状态。使用已有代理从官方发布地址下载 NSIS 3.11 和 `nsis_tauri_utils-v0.5.3` 插件；按当前 Tauri CLI 2.12.2 对应源码中的 SHA1 校验，分别为：

- NSIS：`EF7FF767E5CBD9EDD22ADD3A32C9B8F4500BB10D`
- 插件：`75197FEE3C6A814FE035788D1C34EAD39349B860`

启用 `bundle.useLocalToolsDir=true`，将校验后的工具缓存到 `src-tauri/target/.tauri/NSIS`。该目录已被 `.gitignore` 排除。构建进程临时设置 HTTP/HTTPS 代理，并补充 Cargo 的 PATH；没有修改系统代理或持久环境变量。

## 本次验证

- `npm.cmd run package:windows` 退出码 0。
- TypeScript 和 Vite 生产构建通过。
- Rust release 编译通过，生成 `src-tauri/target/release/projecthub.exe`。
- NSIS 完成 1 个安装包：`src-tauri/target/release/bundle/nsis/ProjectHub_0.3.0_x64-setup.exe`。
- 安装包长度：3,060,748 字节（打包日志显示 2.92 MiB）。
- 本次没有重新运行 Rust 单元测试，也没有执行安装器或桌面业务验收；打包成功不等于实际安装和业务验证通过。

## 本地安装与再次打包

双击上述 `ProjectHub_0.3.0_x64-setup.exe`，按向导安装，再从开始菜单打开 ProjectHub。安装版不需要运行 Vite 服务。

再次打包使用 `npm.cmd run package:windows`。工具缓存存在时会复用；删除 target 后、在新机器上或升级 CLI 后可能需要重新下载。若 GitHub 下载再次超时，在当前 PowerShell 设置能访问 GitHub 的实际代理地址，再重新运行命令；Rust 的 RsProxy 镜像设置不会代理 NSIS 的 GitHub 下载。

官方配置说明：https://v2.tauri.app/reference/config/#uselocaltoolsdir

本次校验依据：https://github.com/tauri-apps/tauri/blob/tauri-cli-v2.12.2/crates/tauri-bundler/src/bundle/windows/nsis/mod.rs


## 2026-10-16 · V1.4 最终构建

当前版本为 1.4.0，x64、当前用户 NSIS。使用已有工具缓存，构建时仅设置进程内 `CARGO_NET_OFFLINE=true`，未修改系统代理、未下载其他来源工具。

- 最终 `npm.cmd run package:windows` 退出码 0；生产前端和 Rust release 编译成功。
- 产物：`src-tauri/target/release/bundle/nsis/ProjectHub_1.4.0_x64-setup.exe`，3,851,243 字节，2026-10-16 15:00:41。
- SHA-256：`3FC29DA27C853CA7E781CBE0C2446607838C6F28C24C06E32C503FE42D4CF3A4`；同目录提供 `.exe.sha256`。
- 应用 ProductVersion 与 FileVersion 均为 `1.4.0`；旧 0.3.0 包保留为历史产物。
- 已包含本次 Git 工作区、历史与 Diff、活跃度、受控 AI 摘要、SQLite v2 日志／任务／提交关联及备份升级。
- 已验证：前端全量 82 项、Rust 全量 79 项、最后新增防护的助手专项 24 项。原生 GUI、真实模型、安装器运行及干净机器安装未实测，打包不代表这些验证已经完成。

详细证据见 [V1.4 验收记录](./2026-10-16-V1.4验收记录.md)。

## 2026-10-16 17:03 · V1.5 整体体验优化后重新打包

- 按用户要求使用当前优化后的源码重新生成安装包，版本保持 1.5.0，覆盖同名旧产物。
- 命令 `npm.cmd run package:windows` 退出码 0，TypeScript、Vite、Rust release 与 NSIS 均成功。复用本地 NSIS 缓存，进程内设置 `CARGO_NET_OFFLINE=true` 并补充 Cargo PATH。
- 输出：`src-tauri/target/release/bundle/nsis/ProjectHub_1.5.0_x64-setup.exe`，4,111,331 字节（约 3.92 MiB），修改时间 2026-10-16 17:03:24。
- 安装器与主程序 ProductVersion 均为 `1.5.0`。
- SHA-256：`3308B6C824D4B44F2CE704D340842C0B4A3630E522B549D4FE1DF6F0732E0FE2`；同目录 `.exe.sha256` 已更新并回读核对一致，与优化前旧包哈希不同。
- 包含首页入口、筛选恢复、周报检索／保存交互与整体主题布局优化，详见 [整体体验优化验收记录](./2026-10-16-整体体验优化验收记录.md)。
- 本次没有重新运行测试或执行安装器；紧邻打包前的源码验证为前端 107 项测试、生产构建和 32 个浏览器视图检查通过。原生 GUI、真实供应商与干净机器安装仍未实测。

## 2026-10-16 17:19 · 周报生成失败修复版

- 版本仍为 1.5.0，替换同名旧包，包含空工具数组兼容、官方 DeepSeek 新模型非思考周报请求，以及失败后重新预览再确认的修复。
- `npm.cmd run package:windows` 退出码 0，前端构建、Rust release 和 NSIS 成功。
- 输出：`src-tauri/target/release/bundle/nsis/ProjectHub_1.5.0_x64-setup.exe`，4,120,992 字节，2026-10-16 17:19:17。安装器及主程序 ProductVersion 均为 1.5.0。
- SHA-256：`1E191CBCF9163AD3EA2122A816E03417D4743BD7879D7D54F5AD87AFEECC8797`，同目录校验文件已更新。
- 前端全量 108 项及周报后端专项 8 项通过。Rust 全量首次 85/88 通过，失败三项单独复测两项通过，仍有一项既有开发摘要模拟 HTTP 读取超时，未声称全量通过。
- 没有调用真实 DeepSeek、运行原生 GUI 或安装器。详见 [周报生成失败修复记录](./2026-10-16-周报生成失败修复.md)。
