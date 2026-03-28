import type { ReactNode } from 'react'
import { cn } from '@/lib/utils'
import { getMethodBadgeTone } from '../utils'

export function MethodBadge(props: { method: string, subtle?: boolean }) {
  const tone = getMethodBadgeTone(props.method)
  return (
    <span
      className={cn(
        'inline-flex h-5 items-center rounded-sm px-1.5 text-[10px] font-semibold uppercase',
        props.subtle ? tone.subtle : tone.solid,
      )}
    >
      {props.method}
    </span>
  )
}

export function ResponseMetaBadge(props: { label: string, value: string }) {
  return (
    <span className="inline-flex items-center gap-1 rounded-full border border-border/70 bg-muted/40 px-2 py-0.5">
      <span className="text-muted-foreground">
        {props.label}
        :
      </span>
      <span className="font-medium text-foreground">{props.value}</span>
    </span>
  )
}

export function LabelBlock(props: { children: ReactNode, className?: string, label: string }) {
  return (
    <div className={cn('space-y-1.5', props.className)}>
      <label className="text-xs font-medium uppercase tracking-[0.18em] text-muted-foreground">
        {props.label}
      </label>
      {props.children}
    </div>
  )
}

export function SidebarEmptyState(props: { title: string, body: string }) {
  return (
    <div className="rounded-xl border border-dashed border-border/70 px-3 py-5 text-center">
      <p className="font-medium">{props.title}</p>
      <p className="mt-1.5 text-sm leading-6 text-muted-foreground">{props.body}</p>
    </div>
  )
}

export function WorkbenchLoadingScreen() {
  return (
    <main className="relative h-screen overflow-hidden bg-background text-foreground">
      <div className="grid h-full place-items-center">
        <div className="flex items-center gap-3 text-sm text-muted-foreground">
          <span className="size-4 animate-spin rounded-full border-2 border-border border-t-primary" />
          正在加载工作台...
        </div>
      </div>
    </main>
  )
}
