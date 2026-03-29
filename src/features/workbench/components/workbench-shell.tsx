import { useWorkbenchShell } from '../hooks/use-workbench-shell'
import { isMacOSDesktop } from '../types'
import {
  ConfirmDeleteCollectionDialog,
  ConfirmDeleteEnvironmentDialog,
  ConfirmDeleteRequestDialog,
  ConfirmRemoveRecentProjectDialog,
  CreateCollectionDialog,
  CreateProjectDialog,
  CreateRequestDialog,
  EditCollectionDialog,
  EditRequestDialog,
  EnvironmentDialog,
} from './dialogs'
import { ProjectSidebar } from './project-sidebar'
import { RequestPane } from './request-pane'

export function WorkbenchShell() {
  const shell = useWorkbenchShell()

  return (
    <main className="relative h-screen overflow-hidden bg-background text-foreground">
      <div
        className="grid h-full min-h-0"
        style={{ gridTemplateColumns: 'clamp(248px, 23vw, 308px) minmax(0, 1fr)' }}
      >
        <ProjectSidebar
          collapsedCollectionIds={shell.collapsedCollectionIds}
          environments={shell.environments}
          activeEnvironmentId={shell.activeEnvironmentId}
          isMacOSDesktop={isMacOSDesktop}
          openRequestTabs={shell.openRequestTabs}
          project={shell.project}
          projectPath={shell.projectPath}
          recentProjectPaths={shell.recentProjectPaths}
          selectedTreeNode={shell.selectedTreeNode}
          onCreateCollection={shell.openCreateCollectionDialog}
          onCreateEnvironment={shell.openCreateEnvironmentDialog}
          onCreateProject={shell.openCreateProjectDialog}
          onCreateRequest={shell.openCreateRequestDialog}
          onDeleteCollection={shell.requestDeleteCollection}
          onDeleteEnvironment={shell.requestDeleteEnvironment}
          onDeleteRequest={shell.requestDeleteRequest}
          onEditCollection={shell.openEditCollectionDialog}
          onEditEnvironment={shell.openEditEnvironmentDialog}
          onEditRequest={shell.openEditRequestDialog}
          onMoveTreeNode={shell.moveTreeNode}
          onOpenExistingProject={() => { void shell.handleOpenExistingProject() }}
          onRemoveRecentProject={shell.requestRemoveRecentProject}
          onSelectProject={(path) => { void shell.handleSelectProject(path) }}
          onOpenRequest={(summary, parentCollectionId) => { void shell.openRequestFromSummary(summary, parentCollectionId) }}
          onSetActiveEnvironment={shell.handleSetActiveEnvironment}
          onToggleCollection={shell.toggleCollection}
          onTreeSelectionChange={shell.setNodeSelection}
        />

        <RequestPane
          activeDraft={shell.activeDraft}
          activeEditorTab={shell.activeEditorTab}
          activeRequestId={shell.activeRequestId}
          activeRequestIsLoading={shell.activeRequestIsLoading}
          activeResponse={shell.activeResponse}
          dirtyRequestIds={shell.dirtyRequestIds}
          isBusy={shell.isBusy}
          isMacOSDesktop={isMacOSDesktop}
          openRequestTabs={shell.openRequestTabs}
          pendingCloseRequestId={shell.pendingCloseRequestId}
          splitRatio={shell.splitRatio}
          onActiveEditorTabChange={shell.setActiveEditorTab}
          onChangeDraft={shell.updateRequestDraft}
          onCloseRequestDialogChange={() => shell.clearPendingCloseRequest()}
          onCloseRequestTab={shell.requestCloseRequestTab}
          onConfirmCloseRequestTab={shell.confirmCloseRequestTab}
          onFocusRequestTab={shell.focusRequestTab}
          onReorderRequestTabs={shell.reorderRequestTabs}
          onSaveRequest={() => { void shell.handleSaveRequest() }}
          onSendRequest={() => { void shell.handleSendRequest() }}
          onSplitRatioChange={shell.setSplitRatio}
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

      <EditCollectionDialog
        description={shell.editCollectionDescriptionDraft}
        name={shell.editCollectionNameDraft}
        open={shell.editCollectionDialogOpen}
        onDescriptionChange={shell.setEditCollectionDescriptionDraft}
        onNameChange={shell.setEditCollectionNameDraft}
        onOpenChange={(open) => {
          if (!open) {
            shell.closeEditCollectionDialog()
          }
        }}
        onSubmit={() => { void shell.handleEditCollection() }}
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

      <ConfirmDeleteEnvironmentDialog
        deletion={shell.pendingEnvironmentDeletion}
        onConfirm={() => { void shell.handleDeleteEnvironment() }}
        onOpenChange={open => !open && shell.clearPendingEnvironmentDeletion()}
      />

      <ConfirmRemoveRecentProjectDialog
        removal={shell.pendingRecentProjectRemoval}
        onDeleteLocalFilesChange={shell.setDeleteLocalFilesForPendingRecentProjectRemoval}
        onConfirm={() => { void shell.handleRemoveRecentProject() }}
        onOpenChange={open => !open && shell.clearPendingRecentProjectRemoval()}
      />

      <EnvironmentDialog
        name={shell.environmentNameDraft}
        baseUrl={shell.environmentBaseUrlDraft}
        variables={shell.environmentVariablesDraft}
        open={shell.environmentDialogOpen}
        isEditing={Boolean(shell.editingEnvironmentId)}
        onNameChange={shell.setEnvironmentNameDraft}
        onBaseUrlChange={shell.setEnvironmentBaseUrlDraft}
        onVariablesChange={shell.setEnvironmentVariablesDraft}
        onOpenChange={(open) => {
          if (!open) {
            shell.closeEnvironmentDialog()
          }
        }}
        onSubmit={() => {
          if (shell.editingEnvironmentId) {
            void shell.handleEditEnvironment()
          }
          else {
            void shell.handleCreateEnvironment()
          }
        }}
        onDelete={shell.editingEnvironmentId ? () => { shell.requestDeleteEnvironment(shell.editingEnvironmentId!) } : undefined}
      />
    </main>
  )
}
