# 请求执行与脚本

本页描述当前 Rust 核心的执行语义。桌面发送、MCP、代码导出、集合运行器和无界面测试复用同一核心；执行入口见 [架构说明](architecture.md)。

## 执行规则

1. 环境变量先读取父环境，再读取当前环境和本机环境写入；之后叠加项目、外层目录、内层目录、请求变量。脚本运行变量最后覆盖。模板使用 `{{name}}`，支持递归展开，未定义变量或循环会报错。
2. Header 按大小写不敏感的名称覆盖父层。保留同一层的重复条目和顺序；禁用同名条目会屏蔽继承项。Query 保留重复项，禁用项不参与模板展开。
3. `auth: inherit` 显式继承；请求中不写 `auth` 表示无认证。Base URL 编辑器维护标准 `base_url` 变量，按环境 → 项目 → 外层目录 → 内层目录 → 请求覆盖，留空继承。URL 可以写 `{{base_url}}/users`，或直接写 `/users`、`?query=1`；`/users` 保留 Base URL 的服务路径前缀，绝对地址独立解析。发送与代码导出共用解析规则。
4. 前置、后置脚本均按项目 → 外层目录 → 内层目录 → 请求执行。每段脚本拥有独立词法作用域，通过运行变量传递数据。发送前运行前置脚本，然后展开模板；完整响应结束后执行响应脚本和 tests。
5. 请求 `settings` 覆盖继承设置；`inherit` 保留父值。SSE 请求的 Accept 为 `text/event-stream` 时不使用整请求超时；连接超时与取消仍有效。默认响应大小上限 32 MiB，可在网络设置修改。
6. 文件路径相对于请求文件所在目录解析，也支持绝对路径。路径参数支持完整的 `:name` 或 `{name}` 路径段，值按路径段编码。
7. 脚本环境写入仅在执行成功后写入 `.apifuck/variables.json`；不会自动修改共享环境文件。JSON 中以 `env:<环境名>` 和 `global` 区分环境与无环境运行。桌面与集成测试共用该规则。

核心通过 `Event::Request` 返回本次发送的方法、URL 和目标请求头，包括继承、变量展开、前置脚本、认证，以及正文和 HTTP 客户端补充的字段。同名重复头保留多行。HTTP 每个重定向分别记录，WebSocket 记录握手头；收到该次响应头后才发布记录，未得到响应的连接失败不冒充已发送结果。记录不包含代理 CONNECT 握手或 TLS 数据。

## 脚本 API

```js
// before-request
api.vars.set('trace_id', 'example');
api.env.set('token', 'new-token');
api.request.url = '{{base_url}}/users';
req.setHeader('X-Trace-Id', api.vars.get('trace_id'));

// after-response
console.log(api.response.status);
api.env.set('last_user_id', api.response.json().id);
api.response.json('$.users[0].name');
```

`api.config` 与 `api.request` 指向同一个 OpenCollection 请求对象。`fuck` 是 `api` 的别名。也提供 `req.getUrl/setUrl/getMethod/setMethod/getHeader/setHeader/setBody` 和 `bru.getEnvVar/setEnvVar/getVar/setVar` 的小范围适配。

每段脚本限制 2 秒、32 MiB 内存；不提供 Node.js 模块、网络/文件系统接口或浏览器全局变量，不承诺完整 Bruno 脚本兼容。支持真实 Chai `expect`/`assert` 和 AJV JSON Schema 断言；`test(name, callback)` 收集各项成功/失败而不中断后续测试。异步测试会明确报失败。旧格式和旧脚本的数据结构没有迁移层。

内嵌 Chai/AJV 等公共运行库在本进程首次使用时编译为 QuickJS 字节码，后续脚本复用这份只读字节码，避免重复解析整个库。字节码不落盘、不读取外部缓存；每段脚本仍创建独立的 VM 和上下文，重新初始化库对象，不共享全局变量、原型修改或用户脚本状态。2 秒和 32 MiB 限制继续生效，失败或超时的上下文不会复用。

桌面 Script（前置/后置）和 Tests 编辑器提供离线补全：输入标识符或 `.` 自动提示，也可按 `Ctrl+Space` 手动唤起，用方向键选择、Enter 插入、Esc 关闭。支持 `fuck` / `api`、`req`、`res`、`bru`、常用 JavaScript 关键字及内置对象；候选列表和悬停显示函数参数/返回值说明。响应成员仅在后置和测试阶段提示。补全描述当前原生请求结构，不提供完整 JavaScript 类型检查或浏览器/Node API 提示。

## ApiFuck 扩展

仅使用集合根部 `extensions.apifuck`，不向标准请求对象添加字段：

```yaml
extensions:
  apifuck:
    version: 1
    requests:
      users/upload.yml:
        connectTimeoutMs: 10000
        allowInvalidCertificates: false
        multipartFileNames:
          '0': upload.bin
```

`multipartFileNames` 用表单项在 `body.data` 中的索引指定上传文件名。此扩展在其他 OpenCollection 工具中不保证生效。

## 变量展示与执行的区别

桌面输入框在非编辑状态把 `{{name}}` 显示为 badge，悬停先显示当前启用环境（无环境时显示“未选择环境”），再显示 `变量名 = 解析值`。缺失变量显示“未设置”，空字符串显示“空字符串”，循环引用或无效配置显示“无法解析”。切换环境会刷新预览；点击输入框后编辑原始模板文本。

输入 `{{` 后，候选变量出现在浮动下拉面板中，面板不参与表格行高计算。候选项也可以悬停查看值。补全和 badge 仅改变展示，保存和复制仍使用原始 `{{name}}` 文本。

HTTP JSON Body 和 GraphQL Variables JSON 编辑器对完整模板使用主题强调色文字与浅色背景，缺失或解析失败时使用错误色，并提供相同的环境与值悬停。装饰和悬停保留原始文本、焦点、选区及撤销历史；文本、环境或模式变化会清除旧悬停，环境名也会在变量值不变时更新。

以上预览调用 `engine::preview_variables` 和 `engine::interpolate`，包含当前请求草稿、环境继承、本机覆盖、项目和目录默认值。预览不会执行前置脚本，因此脚本在发送时生成或改写的值可能与预览不同。显示的环境名表示当前启用环境，不表示该变量一定来自环境文件；项目、目录和请求作用域仍按核心规则覆盖。变量预览与代码编辑器中的脚本补全分别实现。

## 集合运行器与执行边界

`runner::run` 在核心库中提供顺序执行、目录筛选、迭代、延迟、取消和失败停止，并在请求间共享运行变量。它执行 HTTP/GraphQL，将 WebSocket 记为 skipped；可通过 MCP 的 `run_collection` 调用；桌面目前没有独立的集合运行器面板。

已支持 Basic、Bearer、API Key，以及显式无认证与继承。Digest、NTLM、WSSE、OAuth1/2、AWS SigV4 尚未实现执行支持；表单或导入器能保留某种配置，不代表可以发送。

目前不支持 gRPC、完整异步脚本、Node 模块、外部 Secret、dotenv、mTLS、自定义客户端证书、bundled 集合。WebSocket 不支持代理和跳过 TLS 校验。声明式 actions 只支持 after-response 阶段的 jsonq 变量提取。已识别但不支持且影响执行的配置会报错。

Python requests 不能无损表达重复 Header；这类请求可使用 cURL/Go 导出。JavaScript fetch 不支持 GET/HEAD 正文，导出时会提示。Bruno 兼容范围见 [兼容性记录](bruno-parity.md)。
