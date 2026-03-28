import type { PendingCollectionDeletion, PendingRequestDeletion } from '../types'
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
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { Input } from '@/components/ui/input'
import { Textarea } from '@/components/ui/textarea'
import { LabelBlock } from './shared'

export function CreateProjectDialog(props: {
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
          <DialogTitle>新建项目</DialogTitle>
          <DialogDescription>项目会显示在左侧选择器中，并拥有独立的请求树。</DialogDescription>
        </DialogHeader>
        <div className="space-y-3">
          <LabelBlock label="项目名称">
            <Input value={props.name} onChange={event => props.onNameChange(event.target.value)} placeholder="核心接口" />
          </LabelBlock>
          <LabelBlock label="描述">
            <Textarea value={props.description} onChange={event => props.onDescriptionChange(event.target.value)} placeholder="可选的项目说明。" />
          </LabelBlock>
        </div>
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
          <DialogTitle>编辑目录</DialogTitle>
          <DialogDescription>修改目录名称和描述。</DialogDescription>
        </DialogHeader>
        <div className="space-y-3">
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
        </div>
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
            <Input value={props.name} onChange={event => props.onNameChange(event.target.value)} placeholder="健康检查" />
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
            <Input value={props.name} onChange={event => props.onNameChange(event.target.value)} placeholder="健康检查" />
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
              ? `删除“${props.deletion.name}”后，下面的所有子目录和请求都会被一并删除，此操作不可撤销。`
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
              ? `删除“${props.deletion.name}”后将无法恢复。`
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
