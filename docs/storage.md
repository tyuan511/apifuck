# 项目格式与本机状态

一个项目目录对应一个 OpenCollection 根集合。根文件为 `opencollection.yml` 或 `opencollection.yaml`，使用 `schema/` 内固定的 OpenCollection 1.0.0 schema；来源与校验信息保存在该目录。当前不支持 bundled 集合，也没有旧 JSON 项目的自动迁移层。

## 文件布局

```text
project/
├── opencollection.yml       # 名称、项目级变量/请求头/认证/脚本
├── environments/
│   └── local.yml            # 环境及其继承配置
├── users/
│   ├── folder.yml           # 目录默认配置及排序
│   └── get-users.yml        # 请求
└── .apifuck/                # 本机状态，应在 .gitignore 中忽略
    ├── workspace.json      # 标签、草稿、搜索、环境选择、展开状态等
    ├── variables.json      # 脚本写回的环境覆盖
    ├── variables.lock      # 并发变量写入锁
    └── trash/              # 已回收文档，可手动恢复
```

新建项目会添加 `.apifuck/` 忽略项；现有项目需自行确认忽略配置。共享文档不应混入窗口状态、草稿或本机变量覆盖。

## 文档身份

项目显示名称来自 `info.name`，可与磁盘目录名不同。请求类型使用 `info.type`，如 `http`、`graphql`、`websocket`，协议内容分别位于同名字段。目录和请求排序使用 `info.seq`，文档定位依赖相对路径或唯一名称。

桌面新建请求生成 `api_<UUID>.yml`，新建目录生成 `folder_<UUID>/folder.yml`，新建环境生成 `environments/env_<UUID>.yml`。请求直接打开新标签，目录和环境只需输入显示名称；环境名在同一项目内唯一。修改显示名称只改元数据，移动或文件重命名使用专门操作。

桌面可将文档移入项目内部 `.apifuck/trash/`。核心还保留整项目回收 API，但当前桌面没有“删除项目”菜单，不能据此描述为用户可用的项目删除入口。

## 示例

```yaml
info:
  name: 获取用户
  type: http
  seq: 1
http:
  method: GET
  url: '{{base_url}}/users'
  auth: inherit
  params:
    - name: limit
      value: '10'
      type: query
```

环境文件：

```yaml
name: local
variables:
  - name: base_url
    value: http://127.0.0.1:8787
```

完整场景见 [Playground](../examples/playground/README.md)。变量、请求头、认证和脚本的继承规则，以及 `extensions.apifuck` 的配置格式见 [execution.md](execution.md)。

## 草稿与共享保存

输入变化保存到 `.apifuck/workspace.json`，包括名称、URL、参数、请求头、认证、正文、脚本、环境、原始 Source 和 WebSocket 待发送内容。草稿保留无效/未完成文本、空行、禁用行和不同 Body/Auth 模式的输入；搜索与对话框输入也在本机恢复。

关闭标签不会丢弃对应草稿，重新打开同一文档可继续编辑；启动恢复上次打开的标签、当前项和环境。原始草稿与已校验文档分别保存，自动恢复不能用干净快照覆盖磁盘上的新内容。

显式保存共享 YAML 时，核心校验 schema，比较上次读取原文和当前磁盘内容，遇到外部变化拒绝覆盖。源码保存保留原文；结构化编辑通过语法树更新指定节点，保留其他区域注释与格式，被替换的列表或脚本块可能重新格式化。

写入通过同目录临时文件加原子替换完成。草稿变化保存之外还有周期和关闭阶段的补充持久化；失败会在界面提示。实现位于 `collection.rs`、`editing.rs`、`settings.rs` 和桌面的 `workbench.rs`、`document_tab.rs`。

## 本机环境覆盖

共享环境文件保存团队可读配置；脚本成功执行后的环境写回保存到 `.apifuck/variables.json`，不改写共享 YAML。JSON 按 `env:<环境名>` 区分环境，`global` 表示未选择环境时的覆盖，不是跨所有项目共享的变量库。

写入使用 `variables.lock` 协调并发读改写，避免不同请求丢失对方的变量。环境继承先合并共享值，再叠加本机覆盖与请求作用域。Secret 声明本身不提供明文值，可由本机覆盖供给；完整 Secret 编辑和外部 Secret 服务尚未实现。

## 设备级设置

| 文件 | 内容 |
| --- | --- |
| `~/.apifuck-native/preferences.json` | 已应用的语言、主题、强调色、默认请求布局和网络设置 |
| `~/.apifuck-native/settings-draft.json` | 设置页面未提交的原始输入 |
| `~/.apifuck-native/mcp.json` | MCP 即时开关、端口和访问令牌，独立于设置草稿 |
| `~/.apifuck-native/recent.json` | 最近项目的显示名称和路径，去重后最多 10 项 |

外观与网络等设置草稿跨分类、返回工作区和重启恢复，点击保存后才应用。MCP 开关单独立即生效。首次没有设备偏好时可沿用旧工作区中的配置，保存设备偏好后优先使用设备配置；请求明确指定的网络选项仍按执行规则覆盖默认值。

最近项目兼容读取旧版单路径记录，启动恢复最近一次成功打开的项目。用户目录名称保持稳定，不随源码目录或项目移动而改变。

程序化读写使用 `Collection::open`、`Document::parse` 等核心 API。原生 MCP 通过同一核心 API 操作保存的 YAML，见 [mcp.md](mcp.md)。当前没有独立 CLI 产品；无界面验证使用 Cargo 测试和 `cargo demo --check`。
