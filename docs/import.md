# OpenAPI / Postman 导入

项目菜单中的「导入 OpenAPI / Postman…」可读取本地 JSON/YAML 文件。先预览集合名称、格式、请求数量及转换提示，再选择新集合的父目录。导入器在该目录中生成唯一的 `collection_<随机值>` 子目录，不合并或覆盖现有项目；所有输出文档先经过 OpenCollection schema 校验，写入失败清理本次临时目录。原文件不会被修改。

文件上限 16 MiB，输出最多 10000 个文档。文档和文件夹使用生成路径，源名称只用作显示元数据。导入过程不发送网络请求、不执行脚本，也不加载外部 `$ref` 或 `externalValue`。转换提示保存为新集合根目录的 `import-notes.txt`。

## OpenAPI

支持 Swagger/OpenAPI 2.0、OpenAPI 3.0.x 和 3.1.x。按 paths 生成 HTTP 请求，首个 tag 用作文件夹，保留方法、名称、说明、路径/查询参数、请求头和可转换的请求正文。

- 作用域依次使用 operation、path、根部 servers 的第一个 server，替换 server variable 的默认值。Swagger 2 使用 host/basePath 和首个 scheme。相对 server 或缺少 host 时生成 `{{base_url}}`，需要用户补充集合或环境变量。
- 支持请求所需的文档内 JSON Pointer 引用；外部、缺失或循环引用报错。对象 allOf 合并样例，oneOf/anyOf 使用首个变体；递归 Schema 必须提供可用的显式 example。
- 参数/正文优先使用 example、examples、default、const 或 enum，缺省时生成待编辑的样例。样例生成不等于完整 Schema 校验，也不会把响应 Schema 变成断言。
- 支持简单 path/header（包括 `/reports/{id}.json` 这类嵌入式路径模板），以及 query form 序列化和数组；Swagger 2 支持 csv/multi 数组。对象参数、deepObject 等复杂 style、allowReserved、自定义 requestBody encoding 等会明确报错。
- 支持 JSON、字符串正文、简单 urlencoded/multipart 表单和文件占位项。多种内容类型优先选择 application/json，否则选首个；导入提示中说明该选择。上传文件需手动指定本机文件。
- 支持 Basic、Bearer、header/query API Key，以及可映射的 OAuth 2.0 客户端凭据/授权码配置。凭据生成模板变量，需用户填写。security 有多个 OR 方案时选择首个并提示；AND 认证组合或其他不支持的认证报错。空 security 保持无认证。
- callbacks、webhooks 和保存的响应不会变成主动请求或执行历史；转换提示记录这些边界。

## Postman

支持 Postman Collection 2.0/2.1，保留文件夹、请求顺序、集合/文件夹/请求变量、说明、重复 Header/Query、禁用行、路径变量和可转换的正文。支持 raw、urlencoded、multipart、文件和 GraphQL；multipart 文本及文件部分保留各自的 Content-Type，多文件部分对每个文件应用同一类型。GraphQL 保留为原生 Query/Variables 编辑模式。

支持认证继承、显式 noauth、Basic、Bearer 和 API Key。已有 OAuth accessToken 转为 Bearer 并提示重新配置授权流程；其他无法可靠映射的认证会报错。文件夹 noauth 使继承它的请求保持无认证。

Postman 的 `pm.*`、tests 和 pre-request scripts 原文放入相应文档说明，并在预览提示「不自动执行」；需要用户迁移为本应用脚本后启用。保存的响应示例不转换为历史/断言，protocolProfileBehavior 不自动应用。文件正文及 multipart 的路径只保留引用，不复制本机文件，发送前需要重新选择。

不提供 Postman 环境独立文件、OpenAPI URL、WSDL、Insomnia 或 HAR 导入；Bru 单文档转换入口保持独立。导入成功表示格式转换及目标文档通过校验，不能据此宣称脚本、认证服务和所有接口已经运行成功。
