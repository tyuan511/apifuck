import type { RequestExportLanguage, RequestExportSnippet } from '../request-export'
import type { PendingCollectionDeletion, PendingEnvironmentDeletion, PendingRecentProjectRemoval, PendingRequestDeletion } from '../types'
import type { RequestScopeConfig } from '@/lib/project'
import { CopyIcon } from 'lucide-react'
import * as React from 'react'
import { toast } from 'sonner'
import { MonacoCodeEditor } from '@/components/monaco-editor'
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
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { Input } from '@/components/ui/input'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { Textarea } from '@/components/ui/textarea'
import { RequestScopeConfigEditor } from './request-pane'
import { LabelBlock } from './shared'

export function CreateProjectDialog(props: {
  description: string
  name: string
  requestConfig: RequestScopeConfig
  open: boolean
  onDescriptionChange: (value: string) => void
  onNameChange: (value: string) => void
  onRequestConfigChange: (value: RequestScopeConfig) => void
  onOpenChange: (open: boolean) => void
  onSubmit: () => void
}) {
  return (
    <Dialog open={props.open} onOpenChange={props.onOpenChange}>
      <DialogContent className="flex h-[min(85vh,820px)] max-h-[85vh] max-w-5xl flex-col overflow-hidden">
        <DialogHeader>
          <DialogTitle>新建项目</DialogTitle>
          <DialogDescription>选择一个本地目录并在其中初始化项目，请求树和环境会直接保存在该目录下。</DialogDescription>
        </DialogHeader>
        <Tabs defaultValue="basic" className="min-h-0 flex-1 overflow-hidden">
          <TabsList>
            <TabsTrigger value="basic">基础信息</TabsTrigger>
            <TabsTrigger value="request">请求设置</TabsTrigger>
          </TabsList>
          <TabsContent value="basic" className="space-y-3 overflow-auto pr-1">
            <LabelBlock label="项目名称">
              <Input value={props.name} onChange={event => props.onNameChange(event.target.value)} placeholder="核心接口" />
            </LabelBlock>
            <LabelBlock label="描述">
              <Textarea value={props.description} onChange={event => props.onDescriptionChange(event.target.value)} placeholder="可选的项目说明。" />
            </LabelBlock>
          </TabsContent>
          <TabsContent value="request" className="min-h-0 flex-1 overflow-hidden">
            <RequestScopeConfigEditor
              config={props.requestConfig}
              onChange={props.onRequestConfigChange}
              title="项目级默认请求设置"
              description="会被这个项目下所有目录和请求继承，请求自身可以覆盖。"
            />
          </TabsContent>
        </Tabs>
        <DialogFooter>
          <Button variant="outline" onClick={() => props.onOpenChange(false)}>取消</Button>
          <Button onClick={props.onSubmit}>创建项目</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}

export function CreateCollectionDialog(props: {
  name: string
  open: boolean
  parentLabel: string
  onNameChange: (value: string) => void
  onOpenChange: (open: boolean) => void
  onSubmit: () => void
}) {
  return (
    <Dialog open={props.open} onOpenChange={props.onOpenChange}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>新建集合</DialogTitle>
          <DialogDescription>
            新集合将创建到
            {' '}
            <span className="font-medium text-foreground">{props.parentLabel}</span>
            下。
          </DialogDescription>
        </DialogHeader>
        <LabelBlock label="集合名称">
          <Input value={props.name} onChange={event => props.onNameChange(event.target.value)} placeholder="鉴权" />
        </LabelBlock>
        <DialogFooter>
          <Button variant="outline" onClick={() => props.onOpenChange(false)}>取消</Button>
          <Button onClick={props.onSubmit}>创建集合</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}

export function EditCollectionDialog(props: {
  description: string
  name: string
  requestConfig: RequestScopeConfig
  open: boolean
  onDescriptionChange: (value: string) => void
  onNameChange: (value: string) => void
  onRequestConfigChange: (value: RequestScopeConfig) => void
  onOpenChange: (open: boolean) => void
  onSubmit: () => void
}) {
  return (
    <Dialog open={props.open} onOpenChange={props.onOpenChange}>
      <DialogContent className="flex h-[min(85vh,820px)] max-h-[85vh] max-w-5xl flex-col overflow-hidden">
        <DialogHeader>
          <DialogTitle>编辑目录</DialogTitle>
          <DialogDescription>修改目录名称和描述。</DialogDescription>
        </DialogHeader>
        <Tabs defaultValue="basic" className="min-h-0 flex-1 overflow-hidden">
          <TabsList>
            <TabsTrigger value="basic">基础信息</TabsTrigger>
            <TabsTrigger value="request">请求设置</TabsTrigger>
          </TabsList>
          <TabsContent value="basic" className="space-y-3 overflow-auto pr-1">
            <LabelBlock label="目录名称">
              <Input value={props.name} onChange={event => props.onNameChange(event.target.value)} placeholder="鉴权" />
            </LabelBlock>
            <LabelBlock label="描述（可选）">
              <Textarea
                value={props.description}
                onChange={event => props.onDescriptionChange(event.target.value)}
                placeholder="补充这个目录的用途或备注。"
              />
            </LabelBlock>
          </TabsContent>
          <TabsContent value="request" className="min-h-0 flex-1 overflow-hidden">
            <RequestScopeConfigEditor
              config={props.requestConfig}
              onChange={props.onRequestConfigChange}
              title="目录级默认请求设置"
              description="会叠加在项目级设置之上，并被当前目录下的请求继承。"
            />
          </TabsContent>
        </Tabs>
        <DialogFooter>
          <Button variant="outline" onClick={() => props.onOpenChange(false)}>取消</Button>
          <Button onClick={props.onSubmit}>保存修改</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}

export function CreateRequestDialog(props: {
  description: string
  name: string
  open: boolean
  parentLabel: string
  onDescriptionChange: (value: string) => void
  onNameChange: (value: string) => void
  onOpenChange: (open: boolean) => void
  onSubmit: () => void
}) {
  return (
    <Dialog open={props.open} onOpenChange={props.onOpenChange}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>新建请求</DialogTitle>
          <DialogDescription>
            请求将创建到
            {' '}
            <span className="font-medium text-foreground">{props.parentLabel}</span>
            下。
          </DialogDescription>
        </DialogHeader>
        <div className="space-y-3">
          <LabelBlock label="请求名称">
            <Input value={props.name} onChange={event => props.onNameChange(event.target.value)} placeholder="请求名称" />
          </LabelBlock>
          <LabelBlock label="描述（可选）">
            <Textarea
              value={props.description}
              onChange={event => props.onDescriptionChange(event.target.value)}
              placeholder="补充这个请求的用途或备注。"
            />
          </LabelBlock>
        </div>
        <DialogFooter>
          <Button variant="outline" onClick={() => props.onOpenChange(false)}>取消</Button>
          <Button onClick={props.onSubmit}>创建请求</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}

export function EditRequestDialog(props: {
  description: string
  name: string
  open: boolean
  onDescriptionChange: (value: string) => void
  onNameChange: (value: string) => void
  onOpenChange: (open: boolean) => void
  onSubmit: () => void
}) {
  return (
    <Dialog open={props.open} onOpenChange={props.onOpenChange}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>编辑请求</DialogTitle>
          <DialogDescription>修改请求名称和描述。</DialogDescription>
        </DialogHeader>
        <div className="space-y-3">
          <LabelBlock label="请求名称">
            <Input value={props.name} onChange={event => props.onNameChange(event.target.value)} placeholder="请求名称" />
          </LabelBlock>
          <LabelBlock label="描述（可选）">
            <Textarea
              value={props.description}
              onChange={event => props.onDescriptionChange(event.target.value)}
              placeholder="补充这个请求的用途或备注。"
            />
          </LabelBlock>
        </div>
        <DialogFooter>
          <Button variant="outline" onClick={() => props.onOpenChange(false)}>取消</Button>
          <Button onClick={props.onSubmit}>保存修改</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}

export function ConfirmDeleteCollectionDialog(props: {
  deletion: PendingCollectionDeletion | null
  onConfirm: () => void
  onOpenChange: (open: boolean) => void
}) {
  return (
    <AlertDialog
      open={Boolean(props.deletion)}
      onOpenChange={open => props.onOpenChange(open)}
    >
      <AlertDialogContent>
        <AlertDialogHeader>
          <AlertDialogTitle>删除目录？</AlertDialogTitle>
          <AlertDialogDescription>
            {props.deletion
              ? `删除"${props.deletion.name}"后，下面的所有子目录和请求都会被一并删除，此操作不可撤销。`
              : '删除目录后，下面的所有子目录和请求都会被一并删除。'}
          </AlertDialogDescription>
        </AlertDialogHeader>
        <AlertDialogFooter>
          <AlertDialogCancel onClick={() => props.onOpenChange(false)}>
            取消
          </AlertDialogCancel>
          <AlertDialogAction variant="destructive" onClick={props.onConfirm}>
            删除目录
          </AlertDialogAction>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  )
}

export function ConfirmDeleteRequestDialog(props: {
  deletion: PendingRequestDeletion | null
  onConfirm: () => void
  onOpenChange: (open: boolean) => void
}) {
  return (
    <AlertDialog
      open={Boolean(props.deletion)}
      onOpenChange={open => props.onOpenChange(open)}
    >
      <AlertDialogContent>
        <AlertDialogHeader>
          <AlertDialogTitle>删除请求？</AlertDialogTitle>
          <AlertDialogDescription>
            {props.deletion
              ? `删除"${props.deletion.name}"后将无法恢复。`
              : '删除请求后将无法恢复。'}
          </AlertDialogDescription>
        </AlertDialogHeader>
        <AlertDialogFooter>
          <AlertDialogCancel onClick={() => props.onOpenChange(false)}>
            取消
          </AlertDialogCancel>
          <AlertDialogAction variant="destructive" onClick={props.onConfirm}>
            删除请求
          </AlertDialogAction>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  )
}

export function ConfirmDeleteEnvironmentDialog(props: {
  deletion: PendingEnvironmentDeletion | null
  onConfirm: () => void
  onOpenChange: (open: boolean) => void
}) {
  return (
    <AlertDialog
      open={Boolean(props.deletion)}
      onOpenChange={open => props.onOpenChange(open)}
    >
      <AlertDialogContent>
        <AlertDialogHeader>
          <AlertDialogTitle>删除环境？</AlertDialogTitle>
          <AlertDialogDescription>
            {props.deletion
              ? `确认删除环境 ${props.deletion.name} 吗？`
              : '确认删除这个环境吗？'}
          </AlertDialogDescription>
        </AlertDialogHeader>
        <AlertDialogFooter>
          <AlertDialogCancel onClick={() => props.onOpenChange(false)}>
            取消
          </AlertDialogCancel>
          <AlertDialogAction variant="destructive" onClick={props.onConfirm}>
            删除
          </AlertDialogAction>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  )
}

export function ConfirmRemoveRecentProjectDialog(props: {
  removal: PendingRecentProjectRemoval | null
  onDeleteLocalFilesChange: (checked: boolean) => void
  onConfirm: () => void
  onOpenChange: (open: boolean) => void
}) {
  return (
    <AlertDialog
      open={Boolean(props.removal)}
      onOpenChange={open => props.onOpenChange(open)}
    >
      <AlertDialogContent>
        <AlertDialogHeader>
          <AlertDialogTitle>确认删除</AlertDialogTitle>
          <AlertDialogDescription>
            {props.removal
              ? `删除后不可恢复，确认删除项目 ${props.removal.name} 吗？`
              : '删除后不可恢复，确认删除这个项目吗？'}
          </AlertDialogDescription>
        </AlertDialogHeader>
        <label className="flex items-center gap-3 px-1">
          <Checkbox
            checked={props.removal?.deleteLocalFiles ?? false}
            onCheckedChange={checked => props.onDeleteLocalFilesChange(Boolean(checked))}
          />
          <span className="block text-sm font-medium text-foreground">同时删除本地文件</span>
        </label>
        <AlertDialogFooter>
          <AlertDialogCancel onClick={() => props.onOpenChange(false)}>
            取消
          </AlertDialogCancel>
          <AlertDialogAction variant="destructive" onClick={props.onConfirm}>
            确定
          </AlertDialogAction>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  )
}

export function EnvironmentDialog(props: {
  name: string
  baseUrl: string
  variables: Array<{ id: string, key: string, value: string, enabled: boolean, description: string }>
  open: boolean
  isEditing: boolean
  onNameChange: (value: string) => void
  onBaseUrlChange: (value: string) => void
  onVariablesChange: (variables: Array<{ id: string, key: string, value: string, enabled: boolean, description: string }>) => void
  onOpenChange: (open: boolean) => void
  onSubmit: () => void
  onDelete?: () => void
}) {
  return (
    <Dialog open={props.open} onOpenChange={props.onOpenChange}>
      <DialogContent className="max-w-2xl">
        <DialogHeader>
          <DialogTitle>{props.isEditing ? '编辑环境' : '新建环境'}</DialogTitle>
          <DialogDescription>
            {props.isEditing
              // eslint-disable-next-line no-template-curly-in-string
              ? '修改环境配置，变量使用 ${VARIABLE_NAME} 语法替换。'
              // eslint-disable-next-line no-template-curly-in-string
              : '创建新的环境配置，变量使用 ${VARIABLE_NAME} 语法替换。'}
          </DialogDescription>
        </DialogHeader>
        <div className="space-y-3">
          <LabelBlock label="环境名称">
            <Input
              value={props.name}
              onChange={event => props.onNameChange(event.target.value)}
              placeholder="开发环境"
            />
          </LabelBlock>
          <LabelBlock label="Base URL">
            <Input
              value={props.baseUrl}
              onChange={event => props.onBaseUrlChange(event.target.value)}
              placeholder="https://api.example.com"
            />
          </LabelBlock>
          <LabelBlock label="变量">
            <div className="space-y-2">
              {props.variables.map((variable, index) => (
                <div key={variable.id} className="flex items-center gap-2">
                  <input
                    type="checkbox"
                    checked={variable.enabled}
                    onChange={(e) => {
                      const newVars = [...props.variables]
                      newVars[index] = { ...variable, enabled: e.target.checked }
                      props.onVariablesChange(newVars)
                    }}
                    className="w-4 h-4"
                  />
                  <Input
                    value={variable.key}
                    onChange={(e) => {
                      const newVars = [...props.variables]
                      newVars[index] = { ...variable, key: e.target.value }
                      props.onVariablesChange(newVars)
                    }}
                    placeholder="变量名"
                    className="w-32"
                  />
                  <Input
                    value={variable.value}
                    onChange={(e) => {
                      const newVars = [...props.variables]
                      newVars[index] = { ...variable, value: e.target.value }
                      props.onVariablesChange(newVars)
                    }}
                    placeholder="变量值"
                    className="flex-1"
                  />
                  <Button
                    variant="ghost"
                    size="icon"
                    onClick={() => {
                      const newVars = props.variables.filter((_, i) => i !== index)
                      props.onVariablesChange(newVars)
                    }}
                  >
                    <span className="text-destructive">×</span>
                  </Button>
                </div>
              ))}
              <Button
                variant="outline"
                size="sm"
                onClick={() => {
                  const newVars = [
                    ...props.variables,
                    { id: crypto.randomUUID(), key: '', value: '', enabled: true, description: '' },
                  ]
                  props.onVariablesChange(newVars)
                }}
              >
                + 添加变量
              </Button>
            </div>
          </LabelBlock>
        </div>
        <DialogFooter className="gap-2">
          {props.isEditing && props.onDelete && (
            <Button variant="destructive" onClick={props.onDelete}>
              删除
            </Button>
          )}
          <div className="flex-1" />
          <Button variant="outline" onClick={() => props.onOpenChange(false)}>取消</Button>
          <Button onClick={props.onSubmit}>{props.isEditing ? '保存' : '创建'}</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}

export function CopyRequestDialog(props: {
  open: boolean
  requestName: string
  snippets: Record<RequestExportLanguage, RequestExportSnippet> | null
  onOpenChange: (open: boolean) => void
}) {
  const [activeTab, setActiveTab] = React.useState<RequestExportLanguage>('curl')

  React.useEffect(() => {
    if (props.open) {
      setActiveTab('curl')
    }
  }, [props.open, props.requestName])

  async function handleCopy() {
    if (!props.snippets) {
      return
    }

    try {
      await navigator.clipboard.writeText(props.snippets[activeTab].code)
      toast.success(`已复制 ${props.snippets[activeTab].title} 代码`)
    }
    catch (error) {
      toast.error(error instanceof Error ? error.message : '复制失败')
    }
  }

  return (
    <Dialog open={props.open} onOpenChange={props.onOpenChange}>
      <DialogContent className="flex h-[min(82vh,760px)] max-h-[82vh] max-w-5xl flex-col overflow-hidden">
        <DialogHeader>
          <DialogTitle>复制为...</DialogTitle>
          <DialogDescription>
            导出
            {' '}
            <span className="font-medium text-foreground">{props.requestName || '当前接口'}</span>
            {' '}
            的完整请求代码。
          </DialogDescription>
        </DialogHeader>

        {props.snippets && (
          <Tabs value={activeTab} onValueChange={value => setActiveTab(value as RequestExportLanguage)} className="flex min-h-0 flex-1 flex-col overflow-hidden">
            <div className="flex items-center justify-between gap-3 border-b border-border/70 pb-3">
              <TabsList>
                <TabsTrigger value="curl">cURL</TabsTrigger>
                <TabsTrigger value="javascript">JavaScript</TabsTrigger>
                <TabsTrigger value="python">Python</TabsTrigger>
                <TabsTrigger value="go">Go</TabsTrigger>
              </TabsList>
              <Button size="sm" onClick={() => { void handleCopy() }}>
                <CopyIcon />
                复制代码
              </Button>
            </div>

            {(['curl', 'javascript', 'python', 'go'] as RequestExportLanguage[]).map(language => (
              <TabsContent key={language} value={language} className="min-h-0 flex-1 overflow-hidden pt-3">
                <MonacoCodeEditor
                  className="h-full"
                  language={props.snippets[language].language}
                  lineNumbers="on"
                  modelUri={`file:///request-export-${language}.${language === 'curl' ? 'sh' : language === 'javascript' ? 'js' : language === 'python' ? 'py' : 'go'}`}
                  readOnly
                  value={props.snippets[language].code}
                  wordWrap="on"
                />
              </TabsContent>
            ))}
          </Tabs>
        )}
      </DialogContent>
    </Dialog>
  )
}
