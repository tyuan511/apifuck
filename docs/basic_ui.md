# 正式工作台 UI 规划

## Summary
桌面端正式 UI 采用“左侧资源树 + 右侧多标签编辑器”的双栏工作台，整体气质走偏 Linear / Postman 的高密度专业界面，而不是卡片化仪表盘。

视觉与交互方向：
- 视觉 thesis：冷静、精确、低装饰，强调可扫描性和持续工作感。
- 内容 plan：左侧负责定位与切换，右侧负责编辑、发送、观察响应。
- interaction thesis：树节点选中高亮、请求 tab 未保存标记、右侧编辑区/响应区可拖拽分割。

第一版范围明确为：
- 左侧：项目选择器 + collection/API 树浏览 + 新建入口
- 右侧：固定打开即保留的请求 tabs、多区块请求编辑器、可拖拽响应面板
- 响应视图：自动切换 `JSON` 和 `文本`
- 不做：左侧重命名、删除、拖拽排序；响应图片/HTML/二进制专用视图；环境与鉴权入口塞进第一排

## Key Changes
### 1. 工作台布局
整体页面改为三层结构：
- 顶层 `WorkbenchShell`
- 左栏 `SidebarPane`
- 右栏 `RequestWorkspace`

桌面默认布局：
- 左栏固定宽度 `300-340px`
- 右栏占剩余宽度
- 整体高度为窗口可视高度
- 面板之间使用细分隔线，不做厚重卡片外框
- 背景保持纯净，用当前主题 token，避免营销式大面积渐变

窗口缩窄时：
- 宽度足够时保持双栏
- 窄于约 `960px` 时左栏收窄到 `260px`
- 再窄时允许树区域横向挤压，但不改成移动端堆叠布局；这是桌面工作台，不做页面式重排

### 2. 左侧栏信息架构
左侧从上到下固定为：
- 项目选择器
- 项目内快捷操作条
- collection/API 文件树

项目选择器：
- 顶部一行显示当前项目名、slug、展开箭头
- 展开后为项目列表
- 同区块保留“新建项目”按钮
- 不显示工作空间路径编辑器作为主入口；路径属于应用级设置，不放进正式主工作区

快捷操作条：
- `New Collection`
- `New Request`
- 可选一个轻量搜索框占位，但第一版不要求实现过滤逻辑

树节点规则：
- collection 节点：可展开/折叠，显示名称和子项数量
- API 节点：显示方法 badge + 名称
- 当前选中节点高亮
- 点击 API：若右侧未打开该请求则开新 tab，已打开则切到对应 tab
- 点击 collection：仅聚焦树，不在右侧打开 tab

左侧第一版行为边界：
- 支持浏览、展开、切换、新建
- 不支持重命名、删除、拖拽排序
- 这些动作在实现时不要偷偷塞进上下文菜单，避免半成品交互

### 3. 右侧请求工作区
右侧区域从上到下分四层：
- 请求 tab 栏
- 当前请求头部工具栏
- 请求参数编辑区
- 响应展示区

请求 tab 栏：
- 一个 API 对应一个 tab
- 点击树节点后打开并保留，不自动替换
- tab 显示：方法、名称、脏标记
- 支持关闭 tab
- 若关闭当前 tab，自动切到最近聚焦 tab
- 若同一个请求再次打开，只激活已有 tab
- 新建请求后立即打开对应 tab

头部工具栏：
- `Method Select`
- `URL Input`
- `Send`
- `Save`
- 保存状态文本：`Saved` / `Unsaved`
- 不在第一排放环境切换、鉴权入口、更多高级选项

布局细节：
- `Method` 宽度固定
- `URL` 占满剩余空间
- `Send` 为主按钮
- `Save` 为次按钮
- 脏状态在最右侧，以小字或 badge 形式出现

### 4. 请求编辑区
请求编辑区位于头部工具栏下方，采用 tabs：
- `Query`
- `Headers`
- `Body`
- `Docs`
- `Mock`

各 tab 第一版职责：
- `Query`：键值表单，支持启用/禁用
- `Headers`：键值表单，支持启用/禁用
- `Body`：先支持 `none/raw/json`
- `Docs`：编辑 `summary/description/operationId/groupName/deprecated`
- `Mock`：编辑 `enabled/status/latency/body/contentType`

实现约束：
- 编辑区直接绑定当前 tab 的本地 draft，不直接写磁盘
- `Save` 时统一提交到 `update_api`
- 切换请求 tab 时保留未保存草稿
- 若请求尚未保存且关闭 tab，需要二次确认或阻止关闭；默认采用确认弹窗

请求表单状态模型新增：
- `openRequestTabs: OpenRequestTab[]`
- `activeRequestId: string | null`
- `requestDrafts: Record<requestId, RequestEditorDraft>`
- `dirtyRequestIds: Set<string>`
- `requestResponses: Record<requestId, ResponseState>`

### 5. 响应展示区
响应展示区位于底部，与编辑区之间加入可拖拽 splitter。

默认行为：
- 初始比例约 `55 / 45`
- 拖拽后仅保存在内存状态；第一版不需要持久化分割比例
- 未发送前展示空状态
- 发送中展示 loading 与基本请求状态
- 发送完成后显示响应元信息条 + 内容区

响应元信息条内容：
- `status`
- `duration`
- `size`
- `content-type`

响应视图切换规则：
- `application/json` 或可解析 JSON：进入 `JSON` 视图
- 其他文本类型：进入 `Text` 视图
- 暂不做 HTML 渲染、图片预览、文件下载专区
- 无法识别但有文本内容时降级到 `Text`

JSON 视图：
- 格式化缩进显示
- 支持折叠层级可作为后续增强，但第一版不强制
- 错误 JSON 回退为文本显示

Text 视图：
- 等宽字体
- 保留换行
- 支持长内容滚动
- 不自动高亮代码语言

## Public Interfaces / State Additions
前端需要新增或重组的核心类型：
- `WorkbenchState`
- `OpenRequestTab { requestId, title, method, dirty, lastFocusedAt }`
- `RequestEditorDraft`
- `ResponseState { status, durationMs, sizeBytes, contentType, responseType, body, isLoading, error }`
- `ResponseViewMode = 'json' | 'text'`

Tauri / 数据层沿用现有接口为主：
- `read_api(id)`
- `update_api(input)`
- 发送请求时在现有或后续 `send_request` 命令返回中补齐
  - `status`
  - `headers`
  - `durationMs`
  - `sizeBytes`
  - `contentType`
  - `responseType`
  - `body`

组件分层建议：
- `WorkbenchShell`
- `ProjectSidebar`
- `RequestTree`
- `RequestTabsBar`
- `RequestHeaderBar`
- `RequestEditorTabs`
- `ResponsePane`
- `JsonResponseView`
- `TextResponseView`

优先复用现有 [src/components/ui/tabs.tsx](/Users/yuantang/code/fuckapi/src/components/ui/tabs.tsx)、[src/components/ui/select.tsx](/Users/yuantang/code/fuckapi/src/components/ui/select.tsx)、[src/components/ui/input.tsx](/Users/yuantang/code/fuckapi/src/components/ui/input.tsx)、[src/components/ui/textarea.tsx](/Users/yuantang/code/fuckapi/src/components/ui/textarea.tsx)。

## Test Plan
需要覆盖的实现与验收场景：
- 启动后左栏显示当前项目，树可展开，点击 API 会打开右侧 tab
- 同一请求重复点击不会产生重复 tab
- 多个请求 tab 可同时保留，切换不丢草稿
- 编辑 URL / Query / Header / Body 后 tab 出现未保存标记
- 点击 `Save` 后脏标记消失，重新读取数据仍一致
- 关闭未保存 tab 时出现确认流程
- `Send` 后响应区显示 loading，再展示状态、耗时、大小
- JSON 响应自动进入 JSON 视图，普通文本进入 Text 视图
- splitter 可拖拽改变上下区域高度，拖拽后编辑区与响应区均保持可滚动
- 左侧第一版只出现浏览和新建入口，不混入未实现的管理动作

## Assumptions
- 正式 UI 基于桌面 Tauri 场景，优先服务宽屏高效操作，不做移动端式布局重排。
- 第一版响应展示只支持 `JSON + 文本`，其他类型统一降级为文本或空状态提示。
- 左侧树的高级管理动作会在后续版本补齐，本次只规划浏览和新建，避免把交互做半套。
- 应用级配置如主题、上次工作区仍保存在 `~/.apifuck/config.json`，不会放回工作空间内。
