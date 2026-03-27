import type { RefObject } from 'react'
import { useWorkbenchShell } from '../hooks/use-workbench-shell'
import { isMacOSDesktop } from '../types'
import {
  ConfirmDeleteCollectionDialog,
  ConfirmDeleteRequestDialog,
  CreateCollectionDialog,
  CreateProjectDialog,
  CreateRequestDialog,
  EditRequestDialog,
} from './dialogs'
import { ProjectSidebar } from './project-sidebar'
import { RequestWorkspace } from './request-workspace'

export function WorkbenchShell(props: { splitContainerRef: RefObject<HTMLDivElement | null> }) {
  const shell = useWorkbenchShell()

  return (
    <main className="relative h-screen overflow-hidden bg-background text-foreground">
      <div
        className="grid h-full min-h-0"
        style={{ gridTemplateColumns: 'clamp(248px, 23vw, 308px) minmax(0, 1fr)' }}
      >
        <ProjectSidebar
          activeProjectId={shell.activeProjectId}
          collapsedCollectionIds={shell.collapsedCollectionIds}
          isMacOSDesktop={isMacOSDesktop}
          openRequestTabs={shell.openRequestTabs}
          projects={shell.workspace?.projects ?? []}
          selectedTreeNode={shell.selectedTreeNode}
          onCreateCollection={shell.openCreateCollectionDialog}
          onCreateProject={shell.openCreateProjectDialog}
          onCreateRequest={shell.openCreateRequestDialog}
          onDeleteCollection={shell.requestDeleteCollection}
          onDeleteRequest={shell.requestDeleteRequest}
          onEditRequest={shell.openEditRequestDialog}
          onOpenRequest={(summary, parentCollectionId) => { void shell.openRequestFromSummary(summary, parentCollectionId) }}
          onProjectChange={shell.setActiveProject}
          onToggleCollection={shell.toggleCollection}
          onTreeSelectionChange={shell.setNodeSelection}
        />

        <RequestWorkspace
          activeDraft={shell.activeDraft}
          activeEditorTab={shell.activeEditorTab}
          activeRequestId={shell.activeRequestId}
          activeRequestIsLoading={shell.activeRequestIsLoading}
          activeResponse={shell.activeResponse}
          dirtyRequestIds={shell.dirtyRequestIds}
          isBusy={shell.isBusy}
          isDraggingSplit={shell.isDraggingSplit}
          isMacOSDesktop={isMacOSDesktop}
          openRequestTabs={shell.openRequestTabs}
          pendingCloseRequestId={shell.pendingCloseRequestId}
          splitContainerRef={props.splitContainerRef}
          splitRatio={shell.splitRatio}
          onActiveEditorTabChange={shell.setActiveEditorTab}
          onChangeDraft={shell.updateRequestDraft}
          onCloseRequestDialogChange={() => shell.clearPendingCloseRequest()}
          onCloseRequestTab={shell.requestCloseRequestTab}
          onConfirmCloseRequestTab={shell.confirmCloseRequestTab}
          onFocusRequestTab={shell.focusRequestTab}
          onSaveRequest={() => { void shell.handleSaveRequest() }}
          onSendRequest={() => { void shell.handleSendRequest() }}
          onStartDraggingSplit={() => shell.setIsDraggingSplit(true)}
        />
      </div>

      <CreateProjectDialog
        description={shell.projectDescriptionDraft}
        name={shell.projectNameDraft}
        open={shell.projectDialogOpen}
        onDescriptionChange={shell.setProjectDescriptionDraft}
        onNameChange={shell.setProjectNameDraft}
        onOpenChange={(open) => {
          if (!open) {
            shell.closeCreateProjectDialog()
          }
        }}
        onSubmit={() => { void shell.handleCreateProject() }}
      />

      <CreateCollectionDialog
        name={shell.collectionNameDraft}
        open={shell.collectionDialogOpen}
        parentLabel={shell.collectionDialogParentLabel}
        onNameChange={shell.setCollectionNameDraft}
        onOpenChange={(open) => {
          if (!open) {
            shell.closeCreateCollectionDialog()
          }
        }}
        onSubmit={() => { void shell.handleCreateCollection() }}
      />

      <CreateRequestDialog
        description={shell.requestDescriptionDraft}
        name={shell.requestNameDraft}
        open={shell.requestDialogOpen}
        parentLabel={shell.requestDialogParentLabel}
        onDescriptionChange={shell.setRequestDescriptionDraft}
        onNameChange={shell.setRequestNameDraft}
        onOpenChange={(open) => {
          if (!open) {
            shell.closeCreateRequestDialog()
          }
        }}
        onSubmit={() => { void shell.handleCreateRequest() }}
      />

      <EditRequestDialog
        description={shell.editRequestDescriptionDraft}
        name={shell.editRequestNameDraft}
        open={shell.editRequestDialogOpen}
        onDescriptionChange={shell.setEditRequestDescriptionDraft}
        onNameChange={shell.setEditRequestNameDraft}
        onOpenChange={(open) => {
          if (!open) {
            shell.closeEditRequestDialog()
          }
        }}
        onSubmit={() => { void shell.handleEditRequest() }}
      />

      <ConfirmDeleteCollectionDialog
        deletion={shell.pendingCollectionDeletion}
        onConfirm={() => { void shell.handleDeleteCollection() }}
        onOpenChange={open => !open && shell.clearPendingCollectionDeletion()}
      />

      <ConfirmDeleteRequestDialog
        deletion={shell.pendingRequestDeletion}
        onConfirm={() => { void shell.handleDeleteRequest() }}
        onOpenChange={open => !open && shell.clearPendingRequestDeletion()}
      />
    </main>
  )
}
