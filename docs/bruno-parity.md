# Bruno 互操作与功能边界

ApiFuck 保持 OpenCollection YAML 为主存储，复用固定版本的 Bruno 转换代码实现 Bru 导入/导出，并提供部分同步脚本 API 适配。桌面使用自己的 GPUI Kit 组件；当前工作区布局见 [工作区交互](postman-layout.md)。本页区分实际实现与待实现能力，不表示完整兼容 Bruno。

## 转换基线

转换 bundle 基于 usebruno/bruno 提交 `23fcb63383afdfa16df0da61476a6051930ada5b`，来源、许可和重建步骤见 [JavaScript 资源说明](../crates/core/resources/javascript/NOTICE.md)。该基线是固定快照，不代表上游最新版本或当前商业产品的能力范围。

仓库不保存 Bruno 上游源码快照；`scripts/javascript/bridge.ts` 提供转换入口，重建脚本按固定提交临时获取解析和序列化源码，生成的 bundle 由 QuickJS 执行。应用没有复制 Bruno UI/品牌，也没有给转换器暴露网络或文件系统接口；导入文档不会执行其脚本。

本地修补保留响应变量的 request 作用域；缺省 seq 按 Bruno 的 1 归一化，显式 seq（包括 0）保留。公开 fixture 测试覆盖 request、annotations、custom-method、settings、complex-auth 等 Bru → YAML → Bru → YAML 语义往返。

每个调用线程复用一个拥有 16 MiB 原生栈的专用转换线程，QuickJS 栈限制为 8 MiB，给 Rust/FFI 调用和异常处理留出余量。QuickJS 上下文只在该线程创建、执行和销毁，避免 Windows 或后台任务的小栈在初始化时溢出。调用间仅传递文本和结果；调用线程结束后，缓存转换线程随通道关闭而退出。

## 当前状态

| 范围 | 已实现 | 仍未实现或有限制 |
| --- | --- | --- |
| 工作区 | 原生集合树、搜索、导航联动、标签重排与批量关闭、两种请求/响应分栏 | 多集合并列工作区、标签固定、完整命令面板 |
| 请求编辑 | Params/Headers/Vars/Assert 表格、认证/正文表单、Script/Tests/Docs/Source | 批量编辑等完整交互覆盖 |
| HTTP/GraphQL | HTTP 执行，GraphQL Query/Variables 与 HTTP 传输 | GraphQL introspection 和 schema explorer |
| 实时协议 | SSE 增量响应、取消、WebSocket 连接与消息 | gRPC；WebSocket 代理与跳过 TLS 校验 |
| 认证 | Basic、Bearer、API Key、OAuth 2.0 客户端凭据/授权码 PKCE、显式无认证/继承 | Digest、NTLM、WSSE、OAuth 1、OAuth 隐式/密码授权、AWS SigV4 的执行 |
| 变量 | 环境继承、本机覆盖、项目/目录/请求变量、静态 badge/tooltip、浮动补全 | 跨项目全局变量库、完整 Secret 编辑、外部 Secret、dotenv、动态变量体系 |
| 脚本 | 同步 QuickJS Hook，`api`/`fuck` 和部分 `bru`/`req`/`res` 适配 | 完整 API、异步请求、Node/第三方模块、Bruno sandwich 顺序 |
| 测试 | Chai test/expect/assert、AJV Draft-07、声明式断言和响应变量提取 | 上游全部断言和 actions 组合 |
| 集合运行 | 核心与桌面数据驱动/迭代/延迟/取消/失败停止、运行变量和 JSON/JUnit/HTML 报告 | 并行运行、独立 CLI、断点调试；WebSocket 在 runner 中跳过 |
| 转换与导出 | Bru 请求/环境/目录转换，OpenAPI/Postman 本地集合导入，cURL 导入，四种语言代码导出 | OpenAPI/Postman 导出，Insomnia/HAR/WSDL 导入；Postman 脚本需手动迁移 |
| 编辑器 | 高亮、后台格式化、内置 LSP 语法诊断、Hook API 补全 | JS 类型检查、远程 schema 校验、完整 IDE 能力 |
| 网络和记录 | 超时、响应大小限制、TLS 开关、基础代理配置、自定义 CA、PEM mTLS、按项目/环境的 Cookie jar、持久化执行历史 | PKCS#12/加密私钥、代理绕过、响应比较 |

可导入、编辑或往返保留某字段，不代表执行引擎支持它。实际执行以 [execution.md](execution.md) 和 `crates/core/` 为准；不能将表单选项或上游文件存在当成功能已完成的证据。

## 验证与维护

核心契约测试覆盖变量/认证/请求头继承、GraphQL、脚本/断言、响应变量、集合运行、重定向、WebSocket 和 SSE。转换器 fixture 测试检查固定上游样例的语义往返；Playground 用真实回环服务串联执行。

调整转换 bundle 时运行 Bru fixture 和脚本测试，保留许可文件。调整执行语义时同步 [执行规则](execution.md)，避免桌面预览、代码导出与发送行为分叉。

自动测试与源码核对不等于视觉/交互验收。禁止 Computer Use；人工检查入口见 [工作区交互](postman-layout.md)，自动检查命令见 [开发指南](development.md)。
