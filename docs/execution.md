# 请求执行与脚本

本页描述当前 Rust 核心的执行语义。桌面发送、MCP、代码导出、集合运行器和无界面测试复用同一核心；执行入口见 [架构说明](architecture.md)。

## 执行规则

1. 环境变量先读取父环境，再读取当前环境和本机环境写入；之后叠加项目、外层目录、内层目录、请求变量。脚本运行变量最后覆盖。模板使用 `{{name}}`，支持递归展开，未定义变量或循环会报错。
2. Header 按大小写不敏感的名称覆盖父层。保留同一层的重复条目和顺序；禁用同名条目会屏蔽继承项。Query 保留重复项，禁用项不参与模板展开。
3. `auth: inherit` 显式继承；请求中不写 `auth` 表示无认证。Base URL 编辑器维护标准 `base_url` 变量，按环境 → 项目 → 外层目录 → 内层目录 → 请求覆盖，留空继承。URL 可以写 `{{base_url}}/users`，或直接写 `/users`、`?query=1`；`/users` 保留 Base URL 的服务路径前缀，绝对地址独立解析。发送与代码导出共用解析规则。
4. 前置、后置脚本均按项目 → 外层目录 → 内层目录 → 请求执行。每段脚本拥有独立词法作用域，通过运行变量传递数据。发送前运行前置脚本，然后展开模板；完整响应结束后执行响应脚本和 tests。
5. 请求 `settings` 覆盖继承设置；`inherit` 保留父值。SSE 请求的 Accept 为 `text/event-stream` 时不使用整请求超时；连接超时与取消仍有效。默认响应大小上限 32 MiB，可在网络设置修改。
6. 文件路径相对于请求文件所在目录解析，也支持绝对路径。路径参数支持完整的 `:name` 路径段，以及可嵌入路径段的 `{name}`（如 `/reports/{id}.json`），值按路径段编码；替换只扫描原始路径一次，不替换 Query，也不再次展开参数值中的花括号。
7. 脚本环境写入仅在执行成功后写入 `.apifuck/variables.json`；不会自动修改共享环境文件。JSON 中以 `env:<环境名>` 和 `global` 区分环境与无环境运行。桌面与集成测试共用该规则。

核心通过 `Event::Request` 返回本次发送的方法、URL 和目标请求头，包括继承、变量展开、前置脚本、认证，以及正文和 HTTP 客户端补充的字段。同名重复头保留多行。HTTP 每个重定向分别记录，WebSocket 记录握手头；收到该次响应头后才发布记录，未得到响应的连接失败不冒充已发送结果。记录不包含代理 CONNECT 握手或 TLS 数据。

HTTP 重定向默认不向其他 origin（协议、主机或端口不同）转发 `Authorization` 或认证配置生成的自定义 Header，包括 OAuth 自定义令牌头和 Header API Key。认证头的识别在整个重定向链内保留，同域跳转后再跨域也会剥离，回到原域时不会自动恢复。显式启用 `forwardAuthorizationHeader` 可允许认证头跨域转发；Cookie 等敏感头仍遵守独立的剥离规则。

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

`runner::run` 在核心库中提供顺序执行、目录筛选、迭代、延迟、取消和失败停止，并在请求间共享运行变量。它执行 HTTP/GraphQL，将 WebSocket 记为 skipped；可通过 MCP 的 `run_collection` 调用；桌面通过集合标题旁的运行按钮或文件夹右键「运行集合」使用同一接口。运行前要求保存或放弃所有未保存文档（含关闭标签的草稿），重新读取磁盘后执行；运行中可返回文档，重新打开运行器查看进度，但不能切换项目。结果的通过/失败以执行错误及断言为准，HTTP 错误码本身不自动等于断言失败。

已支持 Basic、Bearer、API Key、OAuth 2.0 客户端凭据和授权码 + PKCE，以及显式无认证与继承。Digest、NTLM、WSSE、OAuth 1、OAuth 隐式/密码授权、AWS SigV4 尚未实现执行支持；表单或导入器能保留某种配置，不代表可以发送。

目前不支持 gRPC、完整异步脚本、Node 模块、外部 Secret、dotenv、PKCS#12 和加密客户端私钥、bundled 集合。WebSocket 不支持代理和跳过 TLS 校验。声明式 actions 只支持 after-response 阶段的 jsonq 变量提取。已识别但不支持且影响执行的配置会报错。

Python requests 不能无损表达重复 Header；这类请求可使用 cURL/Go 导出。JavaScript fetch 不支持 GET/HEAD 正文，导出时会提示。Bruno 兼容范围见 [兼容性记录](bruno-parity.md)。


## 本机历史与 Cookie jar

HTTP/GraphQL/SSE 的完成、失败和取消，以及 WebSocket 会话结束，统一写入项目 `.apifuck/history.json`，桌面、集合运行和 MCP 共用。准备阶段就失败而未进入执行的请求不产生历史。最多保留 200 条，文本响应最多预览 32 KiB，二进制正文不存预览；不保存请求头、请求正文及日志。URL 移除用户名、密码、片段并隐藏所有查询值；路径、响应预览及错误文本仍可能包含业务数据，历史不是脱敏日志。历史查看提供 UTC 时间、环境、状态、耗时和正文；「打开原请求」打开当前文档，不重放旧请求，文档已移动或删除时提示错误。持久化失败不把已完成的网络操作改成失败，桌面以通知、响应日志反馈。历史不是自动重试队列。

Cookie jar 默认开启，可在应用网络设置关闭；关闭后既不自动发送，也不自动存储 Cookie，手写 Cookie 头仍生效。每个项目和环境各有独立存储，不启用环境使用独立桶。根据 Domain、Path、Secure 和有效期匹配，拒绝公共后缀及其他域的 Cookie，自动处理重定向和 WebSocket 握手。显式 Cookie 头覆盖自动 Cookie；实际发送头包含自动 Cookie。Cookie 会话值也会保存至本机并跨重启保留，可从侧栏 Cookie jar 查看、单条删除或清空当前环境。运行中的响应可以重新写入 Cookie，彻底清理前应先停止请求。不会自动把 Cookie 写入代码导出。

## OAuth 2.0

认证表单提供客户端凭据与授权码 + PKCE 两种流程，客户端 ID/密钥使用标准 `credentials` 嵌套结构，凭据位置支持请求正文或 Basic Authorization。配置可使用变量，正常发送在前置脚本之后解析；「获取 OAuth 令牌」只解析静态变量，不执行脚本或发送资源请求。

客户端凭据可在首次发送时自动获取令牌；授权码须先点击「获取 OAuth 令牌」，应用打开系统浏览器，使用随机 state、PKCE S256 和 `http://127.0.0.1:<port>/<path>` 回调。默认端口 0 自动分配，供应商要求固定回调地址时填写注册的端口和路径。回调只监听回环地址；拒绝不匹配的 state、路径和重复 code。等待授权最多 5 分钟，停止请求或关闭标签会取消。授权服务地址要求 HTTPS，回环测试服务可用 HTTP。令牌请求不跟随重定向，最长 30 秒，响应上限 1 MiB。

仅使用 Bearer `access_token`，支持标准 `tokenConfig.placement` 的 header/query 位置。缓存按项目、环境和已解析认证配置隔离，不写共享 YAML。已知到期时间提前 10 秒视为过期；有 refresh token 时自动刷新，支持刷新令牌轮换；`autoRefreshToken: false` 禁用自动刷新。客户端凭据没有可用刷新令牌时可重新获取，`autoFetchToken: false` 禁用自动获取。服务未提供 expires_in 时缓存令牌保留至重新获取；资源服务返回 401 不自动重放请求，需手动重新获取令牌。附加参数支持标准 header/query/body 位置，但不能覆盖 OAuth 协议字段或认证/传输头。固定 state、禁用 PKCE、plain PKCE、id_token 及其他 grant 均明确拒绝。

OAuth 的代码导出目前明确拒绝，不在导出过程中获取令牌。MCP 和集合运行器可复用桌面授权后的本机缓存；不自动弹出浏览器发起授权码登录。缓存是本机明文文件，应保持 `.apifuck/` 不进入 Git；手动删除 `.apifuck/oauth/` 可清除缓存（先停止相关请求）。

协议依据：[OAuth 2.0](https://www.rfc-editor.org/rfc/rfc6749)、[原生应用授权](https://www.rfc-editor.org/rfc/rfc8252)、[PKCE](https://www.rfc-editor.org/rfc/rfc7636)。


## 数据驱动运行与报告

运行器可加载本地 CSV 或 JSON 文件，也可通过 MCP `run_collection` 的 `data` 对象数组传入数据。CSV 第一行为唯一且非空的变量名，支持 BOM、引号、换行和 Unicode，值保留为字符串；JSON 必须是对象数组，保留数字、布尔和对象类型。文件上限 8 MiB、10000 行、每行最多 256 列。桌面加载的是文件快照，修改原数据文件后需要重新加载；关闭项目不保存数据文件选择。

一次迭代按数据行顺序运行整个选定集合/文件夹，再进入下一次迭代；没有数据文件时保持原有顺序执行。数据变量覆盖环境和请求变量，前置脚本仍可覆盖数据值；同一行内脚本运行变量可在请求间传递，换行和下次有数据迭代时重置。显式 `api.env.set` 的本机环境写回继续生效，不属于行内隔离。所有 HTTP/GraphQL/SSE 执行继续复用核心；WebSocket 记为 skipped。次数 × 数据行数 × 请求数不得超过 10000，延迟上限一小时，失败停止与取消作用于整次运行。

完成或取消后可导出 JSON、JUnit XML 和独立 HTML 报告，包含迭代号、数据行号、相对文档路径、结果、耗时、HTTP 状态和测试错误。报告不包含原始数据行、请求认证、正文、响应头、响应正文及控制台日志，但名称及测试错误本身可能包含业务值。JUnit 将执行错误与断言失败区分，跳过协议生成 skipped；取消另产生一个 error 用例，避免 CI 把未完成运行判成成功。HTML 对用户数据转义，无外部脚本或资源。报告默认用英文，桌面入口按当前语言显示。不提供独立 CLI、并行运行或自动上传报告。

## 自定义 CA 与客户端证书

应用网络设置可选择自定义 CA（PEM 证书链或 DER），并添加多个按域名匹配的客户端证书。客户端身份目前支持 PEM 证书链与未加密 PEM 私钥；不支持 PKCS#12/PFX 或加密私钥。路径和开关随应用设置保存，未完成输入随设置草稿恢复。自定义 CA 追加到内置信任根，仍检查证书链和主机名，不等于关闭 TLS 校验。

共享集合可用标准 `config.clientCertificates` 的 `type: pem`、`domain`、`certificateFilePath`、`privateKeyFilePath` 和 `disabled` 配置。环境及其父环境也可使用标准 `clientCertificates`。匹配顺序为启用环境 → 父环境 → 集合 → 应用，按每组数组顺序选首个匹配项；禁用条目不参与。域名支持精确主机、可选端口和 `*.example.com`（匹配子域名，不匹配根域）。集合/环境路径相对于项目根目录；应用路径必须为绝对路径。集合/环境的域名和文件路径使用静态环境/作用域变量解析，不运行脚本，也不使用数据行变量。

集合的 `extensions.apifuck.customCaCertificate` 可覆盖应用 CA 路径，支持相对于项目根目录的路径和静态变量，空字符串表示不添加自定义 CA：

```yaml
config:
  clientCertificates:
    - domain: api.internal.example
      type: pem
      certificateFilePath: certs/client.pem
      privateKeyFilePath: certs/client-key.pem
extensions:
  apifuck:
    version: 1
    customCaCertificate: certs/root-ca.pem
```

证书和私钥按匹配目标加载，每个文件最大 4 MiB；配置不把私钥内容写入 YAML 或偏好。HTTP 每个重定向重新按目标选择身份，WSS 使用相同的 CA 与客户端身份规则，OAuth 令牌请求也按自身目标选择证书。WSS 的代理和跳过 TLS 校验限制仍保留。错误 CA、缺少必需客户端身份、无效证书和密钥不匹配会导致连接失败。证书配置不会自动写入代码导出，导出的客户端代码需单独配置 TLS。
