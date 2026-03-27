import type { RefObject } from 'react'
import type {
  EditorPanelTab,
  OpenRequestTab,
  RequestEditorDraft,
  ResponseState,
} from '../types'
import type { KeyValue } from '@/lib/workspace'
import { Loader2Icon, PlusIcon, SaveIcon, SendHorizonalIcon, XIcon } from 'lucide-react'
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from '@/components/ui/alert-dialog'
import { Button } from '@/components/ui/button'
import { Checkbox } from '@/components/ui/checkbox'
import { Input } from '@/components/ui/input'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { Textarea } from '@/components/ui/textarea'
import { cn } from '@/lib/utils'
import {
  editorTabs,
  macOSWindowChromeHeightClassName,
  methodOptions,
} from '../types'
import {
  createKeyValueDraft,
  formatBytes,
  hasVisibleResponse,
  startWindowDragging,
} from '../utils'
import { MethodBadge, ResponseMetaBadge } from './shared'

interface RequestWorkspaceProps {
  activeDraft: RequestEditorDraft | null
  activeEditorTab: EditorPanelTab
  activeRequestId: string | null
  activeRequestIsLoading: boolean
  activeResponse: ResponseState | null
  dirtyRequestIds: Set<string>
  isBusy: boolean
  isDraggingSplit: boolean
  isMacOSDesktop: boolean
  openRequestTabs: OpenRequestTab[]
  pendingCloseRequestId: string | null
  splitContainerRef: RefObject<HTMLDivElement | null>
  splitRatio: number
  onActiveEditorTabChange: (tab: EditorPanelTab) => void
  onChangeDraft: (updater: (draft: RequestEditorDraft) => RequestEditorDraft) => void
  onCloseRequestDialogChange: (requestId: string | null) => void
  onCloseRequestTab: (requestId: string) => void
  onConfirmCloseRequestTab: () => void
  onFocusRequestTab: (requestId: string) => void
  onSaveRequest: () => void
  onSendRequest: () => void
  onStartDraggingSplit: () => void
}

export function RequestWorkspace(props: RequestWorkspaceProps) {
  const hasUnsavedChanges = Boolean(props.activeRequestId && props.dirtyRequestIds.has(props.activeRequestId))
  const shouldShowResponsePane = props.activeResponse ? hasVisibleResponse(props.activeResponse) : false

  return (
    <section className="flex min-h-0 flex-col bg-background">
      <RequestTabsBar
        activeRequestId={props.activeRequestId}
        isMacOSDesktop={props.isMacOSDesktop}
        openRequestTabs={props.openRequestTabs}
        onCloseRequestTab={props.onCloseRequestTab}
        onFocusRequestTab={props.onFocusRequestTab}
      />

      {props.activeRequestIsLoading && !props.activeDraft
        ? (
            <div className="grid flex-1 place-items-center">
              <div className="flex items-center gap-3 text-sm text-muted-foreground">
                <Loader2Icon className="size-4 animate-spin" />
                正在读取请求详情...
              </div>
            </div>
          )
        : props.activeDraft
          ? (
              <>
                <RequestHeaderBar
                  draft={props.activeDraft}
                  hasUnsavedChanges={hasUnsavedChanges}
                  isBusy={props.isBusy}
                  onChangeDraft={props.onChangeDraft}
                  onSaveRequest={props.onSaveRequest}
                  onSendRequest={props.onSendRequest}
                />

                {shouldShowResponsePane
                  ? (
                      <div
                        ref={props.splitContainerRef}
                        className="grid min-h-0 flex-1"
                        style={{
                          gridTemplateRows: `minmax(250px, ${props.splitRatio}fr) 8px minmax(200px, ${1 - props.splitRatio}fr)`,
                        }}
                      >
                        <RequestEditorTabs
                          activeTab={props.activeEditorTab}
                          draft={props.activeDraft}
                          onActiveTabChange={props.onActiveEditorTabChange}
                          onChangeDraft={props.onChangeDraft}
                        />

                        <button
                          type="button"
                          aria-label="调整请求区与响应区高度"
                          onMouseDown={props.onStartDraggingSplit}
                          className={cn(
                            'group relative mx-3 my-0.5 rounded-full bg-transparent transition',
                            props.isDraggingSplit ? 'cursor-row-resize' : 'cursor-row-resize hover:bg-accent/50',
                          )}
                        >
                          <span className="absolute inset-x-0 top-1/2 h-px -translate-y-1/2 bg-border" />
                          <span className="absolute left-1/2 top-1/2 h-1.5 w-14 -translate-x-1/2 -translate-y-1/2 rounded-full bg-border transition group-hover:bg-primary/40" />
                        </button>

                        <ResponsePane response={props.activeResponse} />
                      </div>
                    )
                  : (
                      <div className="min-h-0 flex-1">
                        <RequestEditorTabs
                          activeTab={props.activeEditorTab}
                          draft={props.activeDraft}
                          onActiveTabChange={props.onActiveEditorTabChange}
                          onChangeDraft={props.onChangeDraft}
                        />
                      </div>
                    )}
              </>
            )
          : (
              <div className="grid flex-1 place-items-center">
                <div className="max-w-md text-center">
                  <p className="text-base font-medium">打开一个请求开始工作</p>
                  <p className="mt-1.5 text-sm leading-6 text-muted-foreground">
                    左侧点击请求节点会在这里打开一个持久化标签页。编辑后的草稿会保留在本地内存，保存后再写回工作空间文件。
                  </p>
                </div>
              </div>
            )}

      <AlertDialog open={Boolean(props.pendingCloseRequestId)} onOpenChange={open => !open && props.onCloseRequestDialogChange(null)}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>关闭未保存的请求？</AlertDialogTitle>
            <AlertDialogDescription>
              这个请求标签还有未保存的修改。确认关闭后，本次草稿会被丢弃。
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel onClick={() => props.onCloseRequestDialogChange(null)}>
              继续编辑
            </AlertDialogCancel>
            <AlertDialogAction variant="destructive" onClick={props.onConfirmCloseRequestTab}>
              关闭标签
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </section>
  )
}

interface RequestTabsBarProps {
  activeRequestId: string | null
  isMacOSDesktop: boolean
  openRequestTabs: OpenRequestTab[]
  onCloseRequestTab: (requestId: string) => void
  onFocusRequestTab: (requestId: string) => void
}

function RequestTabsBar(props: RequestTabsBarProps) {
  return (
    <div
      className={cn('relative border-b border-border/70', props.isMacOSDesktop && macOSWindowChromeHeightClassName)}
      onMouseDown={props.isMacOSDesktop ? startWindowDragging : undefined}
    >
      <div
        className={cn(
          'relative z-10 flex items-center gap-1.5 overflow-x-auto overflow-y-hidden px-3',
          props.isMacOSDesktop ? 'h-full min-h-full py-2' : 'min-h-10 py-1.5',
        )}
      >
        {props.openRequestTabs.length > 0
          ? props.openRequestTabs.map(tab => (
              <div
                key={tab.requestId}
                data-no-window-drag
                className={cn(
                  'group flex min-w-[168px] items-center gap-1.5 rounded-lg border transition',
                  props.isMacOSDesktop ? 'h-8 px-2.5' : 'px-2.5 py-1.5',
                  props.activeRequestId === tab.requestId
                    ? 'border-primary/40 bg-primary/5'
                    : 'border-transparent bg-muted/50 hover:border-border/80 hover:bg-accent',
                )}
              >
                <button
                  type="button"
                  onClick={() => props.onFocusRequestTab(tab.requestId)}
                  className="flex min-w-0 flex-1 items-center gap-1.5 text-left"
                >
                  <MethodBadge method={tab.method} subtle />
                  <span className="min-w-0 truncate text-[13px] font-medium">{tab.title}</span>
                  {tab.dirty && <span className="size-2 rounded-full bg-amber-500" />}
                </button>
                <button
                  type="button"
                  onClick={() => props.onCloseRequestTab(tab.requestId)}
                  className="rounded-md p-1 text-muted-foreground opacity-0 transition group-hover:opacity-100 hover:bg-background hover:text-foreground"
                >
                  <XIcon className="size-4" />
                </button>
              </div>
            ))
          : <span className="text-[13px] text-muted-foreground">暂无已打开的请求标签</span>}
      </div>
    </div>
  )
}

interface RequestHeaderBarProps {
  draft: RequestEditorDraft
  hasUnsavedChanges: boolean
  isBusy: boolean
  onChangeDraft: (updater: (draft: RequestEditorDraft) => RequestEditorDraft) => void
  onSaveRequest: () => void
  onSendRequest: () => void
}

function RequestHeaderBar(props: RequestHeaderBarProps) {
  return (
    <div className="border-b border-border/70 px-3 py-3">
      <div className="mb-2 flex items-center justify-between gap-3">
        <div>
          <p className="text-base font-semibold tracking-tight">{props.draft.name}</p>
          {props.draft.description.trim() && (
            <p className="mt-0.5 text-xs text-muted-foreground">{props.draft.description.trim()}</p>
          )}
        </div>
        <span
          className={cn(
            'rounded-full px-2 py-0.5 text-[11px] font-medium',
            props.hasUnsavedChanges
              ? 'bg-amber-500/10 text-amber-600 dark:text-amber-300'
              : 'bg-emerald-500/10 text-emerald-600 dark:text-emerald-300',
          )}
        >
          {props.hasUnsavedChanges ? '未保存' : '已保存'}
        </span>
      </div>

      <div className="flex flex-wrap items-center gap-2 xl:flex-nowrap">
        <Select
          value={props.draft.method}
          onValueChange={(value) => {
            if (value) {
              props.onChangeDraft(draft => ({ ...draft, method: value }))
            }
          }}
        >
          <SelectTrigger size="sm" className="w-[112px]">
            <SelectValue placeholder="请求方法" />
          </SelectTrigger>
          <SelectContent>
            {methodOptions.map(method => (
              <SelectItem key={method} value={method}>
                {method}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>

        <Input
          className="h-8 min-w-[280px] flex-1 text-sm"
          value={props.draft.url}
          onChange={event => props.onChangeDraft(draft => ({ ...draft, url: event.target.value }))}
          placeholder="请输入请求地址"
        />

        <Button size="sm" disabled={props.isBusy} onClick={props.onSendRequest}>
          <SendHorizonalIcon />
          发送
        </Button>
        <Button size="sm" disabled={props.isBusy || !props.hasUnsavedChanges} variant="outline" onClick={props.onSaveRequest}>
          <SaveIcon />
          保存
        </Button>
      </div>
    </div>
  )
}

interface RequestEditorTabsProps {
  activeTab: EditorPanelTab
  draft: RequestEditorDraft
  onActiveTabChange: (tab: EditorPanelTab) => void
  onChangeDraft: (updater: (draft: RequestEditorDraft) => RequestEditorDraft) => void
}

function RequestEditorTabs(props: RequestEditorTabsProps) {
  return (
    <div className="min-h-0 overflow-hidden border-b border-border/70 px-3 py-3">
      <Tabs value={props.activeTab} onValueChange={value => props.onActiveTabChange(value as EditorPanelTab)} className="h-full">
        <TabsList variant="line" className="mb-3">
          {editorTabs.map(tab => (
            <TabsTrigger key={tab.value} value={tab.value} className="px-1.5 py-0.5 text-[13px]">
              {tab.label}
            </TabsTrigger>
          ))}
        </TabsList>

        <TabsContent value="query" className="min-h-0">
          <KeyValueTable
            emptyLabel="暂无查询参数"
            rows={props.draft.request.query}
            onAdd={() => props.onChangeDraft(draft => ({
              ...draft,
              request: {
                ...draft.request,
                query: [...draft.request.query, createKeyValueDraft()],
              },
            }))}
            onChange={rows => props.onChangeDraft(draft => ({
              ...draft,
              request: { ...draft.request, query: rows },
            }))}
          />
        </TabsContent>

        <TabsContent value="headers" className="min-h-0">
          <KeyValueTable
            emptyLabel="暂无请求头"
            rows={props.draft.request.headers}
            onAdd={() => props.onChangeDraft(draft => ({
              ...draft,
              request: {
                ...draft.request,
                headers: [...draft.request.headers, createKeyValueDraft()],
              },
            }))}
            onChange={rows => props.onChangeDraft(draft => ({
              ...draft,
              request: { ...draft.request, headers: rows },
            }))}
          />
        </TabsContent>

        <TabsContent value="body" className="min-h-0">
          <BodyEditor draft={props.draft} onChangeDraft={props.onChangeDraft} />
        </TabsContent>
      </Tabs>
    </div>
  )
}

function ResponsePane(props: { response: ResponseState | null }) {
  if (!props.response) {
    return (
      <div className="grid min-h-0 place-items-center px-3 py-5">
        <div className="max-w-md text-center">
          <p className="text-sm font-medium">响应会在这里出现</p>
          <p className="mt-1.5 text-sm leading-6 text-muted-foreground">
            发送请求后，这里会展示状态码、耗时、响应大小和自动识别的内容视图。
          </p>
        </div>
      </div>
    )
  }

  return (
    <div className="flex min-h-0 flex-col">
      <div className="flex flex-wrap items-center gap-1.5 border-b border-border/70 px-3 py-2 text-[11px] text-muted-foreground">
        <ResponseMetaBadge label="状态" value={props.response.status !== null ? String(props.response.status) : '等待中'} />
        <ResponseMetaBadge label="耗时" value={`${props.response.durationMs} 毫秒`} />
        <ResponseMetaBadge label="大小" value={formatBytes(props.response.sizeBytes)} />
        <ResponseMetaBadge label="内容类型" value={props.response.contentType || '未知'} />
        {props.response.responseType && <ResponseMetaBadge label="视图" value={props.response.responseType === 'json' ? 'JSON' : '文本'} />}
      </div>

      <div className="min-h-0 flex-1 overflow-auto px-3 py-3">
        {props.response.isLoading
          ? (
              <div className="grid h-full place-items-center">
                <div className="flex items-center gap-3 text-sm text-muted-foreground">
                  <Loader2Icon className="size-4 animate-spin" />
                  请求发送中...
                </div>
              </div>
            )
          : props.response.error
            ? (
                <div className="rounded-xl border border-destructive/20 bg-destructive/5 p-3 text-sm text-destructive">
                  {props.response.error}
                </div>
              )
            : props.response.responseType === 'json'
              ? <JsonResponseView body={props.response.body} />
              : <TextResponseView body={props.response.body} />}
      </div>
    </div>
  )
}

function JsonResponseView(props: { body: string }) {
  return (
    <pre className="overflow-auto rounded-xl border border-border/70 bg-muted/40 p-3 font-mono text-xs leading-5">
      {props.body}
    </pre>
  )
}

function TextResponseView(props: { body: string }) {
  return (
    <pre className="overflow-auto rounded-xl border border-border/70 bg-muted/40 p-3 font-mono text-xs leading-5 whitespace-pre-wrap break-words">
      {props.body || '没有响应体'}
    </pre>
  )
}

interface KeyValueTableProps {
  emptyLabel: string
  rows: KeyValue[]
  onAdd: () => void
  onChange: (rows: KeyValue[]) => void
}

function KeyValueTable(props: KeyValueTableProps) {
  function updateRow(rowId: string, updater: (row: KeyValue) => KeyValue) {
    props.onChange(props.rows.map(row => (row.id === rowId ? updater(row) : row)))
  }

  function removeRow(rowId: string) {
    props.onChange(props.rows.filter(row => row.id !== rowId))
  }

  return (
    <div className="flex h-full min-h-0 flex-col">
      <div className="mb-2 flex items-center justify-between">
        <p className="text-[13px] text-muted-foreground">{props.emptyLabel}</p>
        <Button size="xs" variant="outline" onClick={props.onAdd}>
          <PlusIcon className="size-4" />
          新增
        </Button>
      </div>

      <div className="min-h-0 space-y-1.5 overflow-auto pr-1">
        {props.rows.length > 0
          ? props.rows.map(row => (
              <div
                key={row.id}
                className="grid grid-cols-[28px_minmax(0,_1fr)_minmax(0,_1fr)_32px] items-center gap-1.5 rounded-xl border border-border/70 bg-muted/30 p-1.5"
              >
                <div className="flex items-center justify-center">
                  <Checkbox checked={row.enabled} onCheckedChange={checked => updateRow(row.id, current => ({ ...current, enabled: Boolean(checked) }))} />
                </div>
                <Input
                  value={row.key}
                  onChange={event => updateRow(row.id, current => ({ ...current, key: event.target.value }))}
                  placeholder="键"
                />
                <Input
                  value={row.value}
                  onChange={event => updateRow(row.id, current => ({ ...current, value: event.target.value }))}
                  placeholder="值"
                />
                <button
                  type="button"
                  onClick={() => removeRow(row.id)}
                  className="rounded-md p-1.5 text-muted-foreground transition hover:bg-background hover:text-foreground"
                >
                  <XIcon className="size-4" />
                </button>
              </div>
            ))
          : (
              <div className="grid min-h-[100px] place-items-center rounded-xl border border-dashed border-border/70 text-sm text-muted-foreground">
                {props.emptyLabel}
              </div>
            )}
      </div>
    </div>
  )
}

function BodyEditor(props: {
  draft: RequestEditorDraft
  onChangeDraft: (updater: (draft: RequestEditorDraft) => RequestEditorDraft) => void
}) {
  const body = props.draft.request.body

  return (
    <div className="flex h-full min-h-0 flex-col gap-3">
      <div className="flex items-center gap-2">
        <span className="text-[13px] text-muted-foreground">请求体模式</span>
        <Select
          value={body.mode}
          onValueChange={(value) => {
            if (value) {
              props.onChangeDraft(draft => ({
                ...draft,
                request: {
                  ...draft.request,
                  body: { ...draft.request.body, mode: value as typeof body.mode },
                },
              }))
            }
          }}
        >
          <SelectTrigger size="sm" className="w-[164px]">
            <SelectValue placeholder="选择模式" />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="none">无</SelectItem>
            <SelectItem value="raw">原始文本</SelectItem>
            <SelectItem value="json">JSON</SelectItem>
          </SelectContent>
        </Select>
      </div>

      {body.mode === 'none' && (
        <div className="grid min-h-[140px] place-items-center rounded-xl border border-dashed border-border/70 text-sm text-muted-foreground">
          当前请求不会发送请求体。
        </div>
      )}

      {body.mode === 'raw' && (
        <Textarea
          className="min-h-[220px] flex-1 font-mono text-xs"
          value={body.raw}
          onChange={event => props.onChangeDraft(draft => ({
            ...draft,
            request: {
              ...draft.request,
              body: { ...draft.request.body, raw: event.target.value },
            },
          }))}
          placeholder="请输入原始请求体"
        />
      )}

      {body.mode === 'json' && (
        <Textarea
          className="min-h-[220px] flex-1 font-mono text-xs"
          value={body.json}
          onChange={event => props.onChangeDraft(draft => ({
            ...draft,
            request: {
              ...draft.request,
              body: { ...draft.request.body, json: event.target.value },
            },
          }))}
          placeholder={'{\n  "message": "你好"\n}'}
        />
      )}
    </div>
  )
}
