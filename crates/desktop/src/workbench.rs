use crate::document_tab::DocumentTab;
use crate::i18n::tr;
use crate::icons::AppIcon;
use crate::settings_view::{SettingsEvent, SettingsView};
use crate::update_view::{AppUpdate, UpdateEvent};
use anyhow::{Context as _, Result};
use apifuck_core::settings::{AppPreferences, RecentProjects};
use apifuck_core::{
    Collection, collection,
    settings::{self, WorkspaceState},
};
use gpui_kit::component::{
    ActiveTheme, Disableable, Icon, IconName, Selectable, Sizable, WindowExt,
    button::{Button, ButtonVariants},
    dialog::DialogButtonProps,
    input::{Input, InputEvent, InputState},
    list::ListItem,
    menu::{ContextMenuExt, DropdownMenu, PopupMenuItem},
    notification::Notification,
    resizable::{h_resizable, resizable_panel},
    tab::{Tab, TabBar},
    tooltip::Tooltip,
    tree::{Tree, TreeEvent, TreeItem, TreeState},
};
use gpui_kit::{prelude::FluentBuilder, *};
use std::{cell::Cell, path::PathBuf, rc::Rc, sync::Arc, time::Duration};

gpui_kit::actions!(
    workbench,
    [
        OpenProject,
        NewProject,
        Save,
        Send,
        CloseTab,
        NewRequest,
        Refresh,
        Settings,
        Quit
    ]
);

#[derive(Clone, Copy, PartialEq)]
enum SidebarView {
    Collections,
    Environments,
}
pub struct Workbench {
    collection: Option<Collection>,
    tabs: Vec<Entity<DocumentTab>>,
    active: usize,
    tab_scroll: ScrollHandle,
    state: WorkspaceState,
    runtime: Arc<tokio::runtime::Runtime>,
    mcp: Option<Arc<apifuck_core::mcp::Service>>,
    mcp_error: Option<String>,
    _mcp_task: Option<Task<()>>,
    project_loading: bool,
    focus: FocusHandle,
    search: Entity<InputState>,
    environment_search: Entity<InputState>,
    environment_scroll: ScrollHandle,
    tree: Entity<TreeState>,
    tree_selection: String,
    sidebar_view: SidebarView,
    show_settings: bool,
    settings_view: Option<Entity<SettingsView>>,
    settings_subscription: Option<Subscription>,
    preferences: Option<AppPreferences>,
    recent_projects: RecentProjects,
    last_autosave_error: Option<String>,
    updates: Entity<AppUpdate>,
    saved_state: String,
    saved_settings_draft: String,
    tab_subscriptions: std::collections::HashMap<EntityId, Subscription>,
    _subscriptions: Vec<Subscription>,
    _poll: Task<()>,
}
impl Workbench {
    pub fn new(
        path: Option<PathBuf>,
        runtime: Arc<tokio::runtime::Runtime>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let search = cx.new(|cx| InputState::new(window, cx).placeholder(tr("搜索集合")));
        let environment_search =
            cx.new(|cx| InputState::new(window, cx).placeholder(tr("搜索环境")));
        let environment_search_subscription = cx.subscribe_in(
            &environment_search,
            window,
            |this, _, event: &InputEvent, window, cx| {
                if matches!(event, InputEvent::Change) {
                    this.autosave(window, cx);
                }
                cx.notify();
            },
        );
        let tree = cx.new(|cx| TreeState::new(cx));
        let subscription = cx.subscribe_in(
            &search,
            window,
            |this, _, event: &InputEvent, window, cx| {
                if matches!(event, InputEvent::Change) {
                    this.autosave(window, cx);
                }
                this.rebuild_tree(cx);
                cx.notify();
            },
        );
        let selection = cx.observe_in(&tree, window, |this, tree, window, cx| {
            let path = tree
                .read(cx)
                .selected_item()
                .map(|item| item.id.to_string())
                .unwrap_or_default();
            if path != this.tree_selection {
                this.tree_selection = path.clone();
                if this
                    .collection
                    .as_ref()
                    .is_some_and(|c| c.find(&path).is_ok())
                {
                    this.open_document(&path, window, cx);
                }
            }
        });
        let expansion = cx.subscribe(&tree, |this, _, event: &TreeEvent, cx| {
            match event {
                TreeEvent::Expanded(id) => {
                    this.state.collapsed_folders.remove(id.as_ref());
                }
                TreeEvent::Collapsed(id) => {
                    this.state.collapsed_folders.insert(id.to_string());
                }
            }
            cx.notify();
        });
        let close_handle = cx.entity().downgrade();
        window.on_window_should_close(cx, move |window, cx| {
            close_handle
                .update(cx, |this, cx| match this.persist(cx) {
                    Ok(()) => true,
                    Err(e) => {
                        this.notify_error(
                            crate::i18n::format(
                                tr("无法保存草稿：{e:#}"),
                                &[("e:#", format!("{e:#}"))],
                            ),
                            window,
                            cx,
                        );
                        cx.notify();
                        false
                    }
                })
                .unwrap_or(true)
        });
        let quit = cx.on_app_quit(|this, cx| {
            if let Some(mcp) = &this.mcp {
                mcp.shutdown();
            }
            if let Err(e) = this.persist(cx) {
                eprintln!(
                    "{}",
                    crate::i18n::format(tr("无法保存草稿：{e:#}"), &[("e:#", format!("{e:#}"))])
                );
            }
            async {}
        });
        let appearance = cx.observe_window_appearance(window, |this, window, cx| {
            if this.state.appearance.is_empty() || this.state.appearance == "system" {
                crate::appearance::apply("system", &this.state.accent, window, cx);
            }
        });
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        let poll = cx.spawn_in(window, async move |this, cx| {
            loop {
                cx.background_executor().timer(Duration::from_secs(1)).await;
                if this
                    .update_in(cx, |this, window, cx| {
                        this.autosave(window, cx);
                        cx.notify();
                    })
                    .is_err()
                {
                    break;
                }
            }
        });
        let (preferences, preferences_error) =
            match AppPreferences::path().and_then(|p| AppPreferences::load_from(&p)) {
                Ok(preferences) => (preferences, None),
                Err(error) => (
                    None,
                    Some(crate::i18n::format(
                        tr("读取设置失败：{error:#}"),
                        &[("error:#", format!("{error:#}"))],
                    )),
                ),
            };
        crate::i18n::set_language(
            preferences
                .as_ref()
                .map(|p| p.language.as_str())
                .unwrap_or("zh-CN"),
        );
        let (recent_projects, recent_error) =
            match settings::app_state_path().and_then(|path| RecentProjects::load_from(&path)) {
                Ok(recent) => (recent, None),
                Err(error) => (
                    RecentProjects::default(),
                    Some(crate::i18n::format(
                        tr("读取最近项目失败：{error:#}"),
                        &[("error:#", format!("{error:#}"))],
                    )),
                ),
            };
        crate::refresh_menus(cx);
        search.update(cx, |search, cx| {
            search.set_placeholder(tr("搜索集合"), window, cx)
        });
        environment_search.update(cx, |search, cx| {
            search.set_placeholder(tr("搜索环境"), window, cx)
        });
        let mut state = WorkspaceState::default();
        if let Some(preferences) = &preferences {
            preferences.apply_to(&mut state);
        }
        crate::appearance::apply(&state.appearance, &state.accent, window, cx);
        let updates = cx.new(|_| AppUpdate::new(runtime.clone()));
        let update_observer = cx.observe(&updates, |_, _, cx| cx.notify());
        let update_subscription =
            cx.subscribe_in(&updates, window, |this, updates, event, window, cx| {
                if let Err(error) = this.persist(cx) {
                    this.notify_error(format!("{error:#}"), window, cx);
                    return;
                }
                match event {
                    UpdateEvent::InstallRequested => {
                        updates.update(cx, |updates, cx| updates.install(window, cx))
                    }
                    UpdateEvent::Installed => cx.restart(),
                }
            });
        let (mcp, mut mcp_receiver, mcp_error) = match apifuck_core::mcp::Config::path()
            .and_then(|path| apifuck_core::mcp::Service::new(path, runtime.handle().clone()))
        {
            Ok((service, receiver)) => (Some(service), Some(receiver), None),
            Err(error) => (None, None, Some(format!("{error:#}"))),
        };
        let mut this = Self {
            mcp,
            mcp_error,
            _mcp_task: None,
            project_loading: false,
            collection: None,
            tabs: vec![],
            active: 0,
            tab_scroll: ScrollHandle::new(),
            state,
            runtime,
            focus,
            search,
            environment_search,
            environment_scroll: ScrollHandle::new(),
            tree,
            tree_selection: String::new(),
            sidebar_view: SidebarView::Collections,
            show_settings: false,
            settings_view: None,
            settings_subscription: None,
            preferences,
            recent_projects,
            last_autosave_error: None,
            updates,
            saved_state: String::new(),
            saved_settings_draft: String::new(),
            tab_subscriptions: Default::default(),
            _subscriptions: vec![
                subscription,
                update_observer,
                update_subscription,
                environment_search_subscription,
                quit,
                appearance,
                selection,
                expansion,
            ],
            _poll: poll,
        };
        if let Some(mut receiver) = mcp_receiver.take() {
            this._mcp_task = Some(cx.spawn_in(window, async move |this, cx| {
                while let Some(call) = receiver.recv().await {
                    if this
                        .update_in(cx, |this, window, cx| this.handle_mcp(call, window, cx))
                        .is_err()
                    {
                        break;
                    }
                }
            }));
        }
        if let Some(service) = &this.mcp
            && service.status().config.enabled
        {
            drop(service.configure(true));
        }
        for error in preferences_error.into_iter().chain(recent_error) {
            this.notify_error(error, window, cx);
        }
        if let Some(path) = path {
            this.load(path, window, cx);
        }
        let auto_check = this
            .preferences
            .as_ref()
            .is_none_or(|preferences| preferences.auto_check_updates);
        this.updates.update(cx, |updates, cx| {
            updates.set_auto_check(auto_check, window, cx)
        });
        this
    }
    fn load(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        if let Err(e) = self.persist(cx) {
            self.notify_error(
                crate::i18n::format(tr("无法保存当前草稿：{e:#}"), &[("e:#", format!("{e:#}"))]),
                window,
                cx,
            );
            return;
        }
        crate::feedback::show(
            Notification::info(tr("正在读取项目…").to_owned()),
            window,
            cx,
        );
        self.project_loading = true;
        let task = cx.background_spawn(async move {
            let collection = Collection::open(path)?;
            let state = WorkspaceState::load(collection.root())?;
            Ok::<_, anyhow::Error>((collection, state))
        });
        cx.spawn_in(window, async move |this, cx| {
            let result = task.await;
            let _ = this.update_in(cx, |this, window, cx| {
                this.project_loading = false;
                match result {
                    Ok((collection, state)) => {
                        if let Err(error) = this.install_collection(collection, state, window, cx) {
                            this.notify_error(format!("{error:#}"), window, cx);
                        }
                    }
                    Err(e) => {
                        this.notify_error(format!("{e:#}"), window, cx);
                        cx.notify();
                    }
                }
            });
        })
        .detach();
    }
    fn install_collection(
        &mut self,
        collection: Collection,
        mut state: WorkspaceState,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<()> {
        self.persist(cx)?;
        if self
            .collection
            .as_ref()
            .is_some_and(|current| current.root() == collection.root())
        {
            state = self.state.clone();
        }
        self.tab_subscriptions.clear();
        self.tabs.clear();
        self.tab_scroll = ScrollHandle::new();
        self.active = 0;
        self.state = state;
        if let Some(preferences) = &self.preferences {
            preferences.apply_to(&mut self.state);
        }
        self.show_settings = false;
        for (input, key) in [
            (&self.search, "search"),
            (&self.environment_search, "environmentSearch"),
        ] {
            let value = self
                .state
                .ui_drafts
                .get(key)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_owned();
            input.update(cx, |input, cx| input.set_value(value, window, cx));
        }

        self.saved_state.clear();
        crate::appearance::apply(&self.state.appearance, &self.state.accent, window, cx);
        self.collection = Some(collection);
        self.tree_selection.clear();
        self.rebuild_tree(cx);
        let paths = self.state.tabs.clone();
        for path in paths {
            self.open_document_inner(&path, false, window, cx);
        }
        if let Some(active) = &self.state.active
            && let Some(ix) = self
                .tabs
                .iter()
                .position(|t| t.read(cx).document().path().to_string_lossy() == active.as_str())
        {
            self.active = ix;
            self.tab_scroll.scroll_to_item(ix);
        }
        self.sync_active_sidebar(window, cx);
        let project = self.collection.as_ref().unwrap();
        self.recent_projects.record(
            project.root().to_owned(),
            project.project().name().to_owned(),
        );
        if let Err(e) =
            settings::app_state_path().and_then(|path| self.recent_projects.save_to(&path))
        {
            self.notify_error(e.to_string(), window, cx);
        }
        cx.notify();
        Ok(())
    }
    fn handle_mcp(
        &mut self,
        call: apifuck_core::mcp::Call,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let result = (|| -> Result<serde_json::Value> {
            let service = self.mcp.clone().context("MCP is unavailable")?;
            anyhow::ensure!(
                service.accepts(&call),
                "MCP call expired or service stopped"
            );
            anyhow::ensure!(
                !self.project_loading,
                "Desktop is opening a project; retry after it finishes"
            );
            let mutates = apifuck_core::mcp::mutates_workspace(&call.name);
            if mutates {
                self.persist(cx)?;
            }
            let mut context = apifuck_core::mcp::Workspace {
                root: self.collection.as_ref().map(|c| c.root().to_owned()),
                environment: self.state.environment.clone(),
                network: self.state.network.clone(),
                has_drafts: !self.state.drafts.is_empty()
                    || self
                        .state
                        .editor_drafts
                        .values()
                        .any(|v| v["dirty"].as_bool().unwrap_or(false))
                    || self.tabs.iter().any(|t| t.read(cx).is_dirty()),
                busy: self.tabs.iter().any(|t| t.read(cx).is_running()),
            };
            let old_root = context.root.clone();
            let moved = if call.name == "move_document" {
                let collection = self.collection.as_ref().context("No project")?;
                let doc = collection.find(call.arguments["selector"].as_str().unwrap_or(""))?;
                let target = PathBuf::from(call.arguments["target"].as_str().unwrap_or(""));
                Some(if doc.kind() == apifuck_core::DocumentKind::Folder {
                    (
                        doc.path().parent().unwrap().to_owned(),
                        target.parent().context("Invalid target")?.to_owned(),
                    )
                } else {
                    (doc.path().to_owned(), target)
                })
            } else {
                None
            };
            let result = service.call(&call.name, &call.arguments, &mut context)?;
            if old_root != context.root {
                let collection = Collection::open(context.root.context("No project")?)?;
                let state = WorkspaceState::load(collection.root())?;
                self.install_collection(collection, state, window, cx)?;
            } else if mutates {
                let collection = Collection::open(context.root.context("No project")?)?;
                self.state.environment = context
                    .environment
                    .filter(|name| collection.environment_names().contains(name));
                if let Some((from, to)) = &moved {
                    let remap = |path: &str| {
                        PathBuf::from(path)
                            .strip_prefix(from)
                            .ok()
                            .map(|suffix| collection::portable_path(&to.join(suffix)))
                            .unwrap_or_else(|| path.into())
                    };
                    self.state.drafts = std::mem::take(&mut self.state.drafts)
                        .into_iter()
                        .map(|(path, draft)| (remap(&path), draft))
                        .collect();
                    self.state.editor_drafts = std::mem::take(&mut self.state.editor_drafts)
                        .into_iter()
                        .map(|(path, draft)| (remap(&path), draft))
                        .collect();
                    self.state.collapsed_folders =
                        std::mem::take(&mut self.state.collapsed_folders)
                            .into_iter()
                            .map(|path| remap(&path))
                            .collect();
                }
                let mut retained = Vec::new();
                let mut removed = Vec::new();
                for (index, tab) in self.tabs.iter().enumerate() {
                    let previous = tab.read(cx).document();
                    let path = moved
                        .as_ref()
                        .and_then(|(from, to)| {
                            previous
                                .path()
                                .strip_prefix(from)
                                .ok()
                                .map(|suffix| to.join(suffix))
                        })
                        .unwrap_or_else(|| previous.path().to_owned());
                    if let Ok(document) = collection.find(&collection::portable_path(&path)) {
                        if document.source() != previous.source()
                            || document.path() != previous.path()
                        {
                            tab.update(cx, |tab, cx| tab.reload(document.clone(), window, cx));
                        }
                        retained.push(tab.clone());
                    } else {
                        removed.push(index);
                        self.tab_subscriptions.remove(&tab.entity_id());
                        self.state
                            .drafts
                            .remove(&collection::portable_path(previous.path()));
                        self.state
                            .editor_drafts
                            .remove(&collection::portable_path(previous.path()));
                    }
                }
                let mut remaining = self.tabs.len();
                for index in removed.into_iter().rev() {
                    remaining -= 1;
                    self.active = active_after_close(self.active, index, remaining);
                }
                self.tabs = retained;
                self.collection = Some(collection);
                self.rebuild_tree(cx);
                self.sync_active_sidebar(window, cx);
                self.persist(cx)?;
                cx.notify();
            }
            Ok(result)
        })();
        let _ = call
            .reply
            .send(result.map_err(|error| format!("{error:#}")));
    }
    fn choose_project(&mut self, create: bool, window: &mut Window, cx: &mut Context<Self>) {
        let choice = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some(
                if create {
                    tr("选择新项目目录")
                } else {
                    tr("打开项目")
                }
                .into(),
            ),
        });
        cx.spawn_in(window, async move |this, cx| match choice.await {
            Ok(Ok(Some(paths))) => {
                if let Some(path) = paths.first() {
                    let path = path.clone();
                    let _ = this.update_in(cx, |this, window, cx| {
                        if create {
                            let name = path.file_name().unwrap_or_default().to_string_lossy();
                            if let Err(e) = Collection::create(&path, &name) {
                                this.notify_error(format!("{e:#}"), window, cx);
                                cx.notify();
                                return;
                            }
                        }
                        this.load(path, window, cx);
                    });
                }
            }
            Ok(Err(e)) => {
                let _ = this.update_in(cx, |this, window, cx| {
                    this.notify_error(e.to_string(), window, cx);
                    cx.notify();
                });
            }
            _ => {}
        })
        .detach();
    }
    fn render_environment_sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let query = self.environment_search.read(cx).value().to_lowercase();
        let mut environments = environment_rows(self.collection.as_ref());
        let total = environments.len();
        environments.retain(|(_, name)| name.to_lowercase().contains(&query));
        let editing = self.active().and_then(|tab| {
            let doc = tab.read(cx).document();
            (doc.kind() == apifuck_core::DocumentKind::Environment).then(|| doc.path().to_owned())
        });
        div()
            .flex()
            .flex_col()
            .size_full()
            .min_h_0()
            .bg(cx.theme().sidebar)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .px_3()
                    .pt_3()
                    .pb_3()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(tr("环境")),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(total.to_string()),
                    )
                    .child(div().flex_1())
                    .child(
                        Button::new("new-environment")
                            .ghost()
                            .small()
                            .icon(AppIcon::Plus)
                            .tooltip(tr("新建环境"))
                            .accessibility_label(tr("新建环境"))
                            .disabled(self.collection.is_none())
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.new_document("environment", window, cx)
                            })),
                    )
                    .child(
                        Button::new("refresh-environments")
                            .ghost()
                            .small()
                            .icon(AppIcon::RefreshCw)
                            .tooltip(tr("刷新环境"))
                            .accessibility_label(tr("刷新环境"))
                            .disabled(self.collection.is_none())
                            .on_click(cx.listener(|this, _, window, cx| this.refresh(window, cx))),
                    ),
            )
            .child(
                div().px_3().pb_3().child(
                    Input::new(&self.environment_search)
                        .prefix(Icon::new(AppIcon::Search).small())
                        .small()
                        .aria_label(tr("搜索环境")),
                ),
            )
            .child(
                div()
                    .id("environment-list")
                    .track_scroll(&self.environment_scroll)
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .px_2()
                    .when(environments.is_empty(), |list| {
                        list.child(
                            div()
                                .p_3()
                                .text_sm()
                                .text_color(cx.theme().muted_foreground)
                                .child(if self.collection.is_none() {
                                    tr("请先打开项目")
                                } else if total == 0 {
                                    tr("此项目还没有环境")
                                } else {
                                    tr("没有匹配的环境")
                                }),
                        )
                    })
                    .children(environments.into_iter().map(|(path, name)| {
                        let is_editing = editing.as_ref() == Some(&path);
                        let is_enabled = self.state.environment.as_deref() == Some(name.as_str());
                        let tooltip = format!("{}\n{}", name, path.display());
                        Button::new(SharedString::from(format!(
                            "environment:{}",
                            path.display()
                        )))
                        .ghost()
                        .w_full()
                        .h_9()
                        .selected(is_editing)
                        .accessibility_label(name.clone())
                        .tooltip(tooltip)
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .w_full()
                                .min_w_0()
                                .gap_2()
                                .child(
                                    Icon::new(AppIcon::Globe)
                                        .size_4()
                                        .flex_shrink_0()
                                        .text_color(cx.theme().muted_foreground),
                                )
                                .child(div().flex_1().min_w_0().truncate().child(name))
                                .when(is_editing, |row| {
                                    row.child(
                                        div()
                                            .id("editing-indicator")
                                            .flex()
                                            .items_center()
                                            .flex_shrink_0()
                                            .text_color(cx.theme().muted_foreground)
                                            .tooltip(|window, cx| {
                                                Tooltip::new(tr("正在编辑")).build(window, cx)
                                            })
                                            .child(Icon::new(AppIcon::Pencil).size_3()),
                                    )
                                })
                                .when(is_enabled, |row| {
                                    row.child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .flex_shrink_0()
                                            .gap_1()
                                            .text_xs()
                                            .whitespace_nowrap()
                                            .text_color(cx.theme().primary)
                                            .child(Icon::new(AppIcon::Check).size_3())
                                            .child(tr("当前启用")),
                                    )
                                }),
                        )
                        .on_click(cx.listener(
                            move |this, _, window, cx| {
                                this.open_document(&path.to_string_lossy(), window, cx)
                            },
                        ))
                    })),
            )
            .child(
                div()
                    .px_3()
                    .py_3()
                    .border_t_1()
                    .border_color(cx.theme().border)
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(tr("点击环境编辑变量；右上角选择请求使用的环境。")),
            )
    }
    fn rebuild_tree(&mut self, cx: &mut Context<Self>) {
        let Some(collection) = &self.collection else {
            return;
        };
        #[derive(Default)]
        struct Node {
            label: String,
            id: String,
            seq: f64,
            children: std::collections::BTreeMap<String, Node>,
        }
        let mut root = Node::default();
        let query = self.search.read(cx).value().to_lowercase();
        for doc in collection.documents() {
            if matches!(
                doc.kind(),
                apifuck_core::DocumentKind::Collection | apifuck_core::DocumentKind::Environment
            ) {
                continue;
            }
            let path = apifuck_core::collection::portable_path(doc.path());
            if !query.is_empty()
                && !doc.name().to_lowercase().contains(&query)
                && !path.to_lowercase().contains(&query)
            {
                continue;
            }
            let mut node = &mut root;
            let parent = doc.path().parent().unwrap_or(std::path::Path::new(""));
            let mut current = PathBuf::new();
            for part in parent.components() {
                let name = part.as_os_str().to_string_lossy().into_owned();
                current.push(&name);
                node = node.children.entry(name.clone()).or_insert_with(|| Node {
                    label: name,
                    id: format!("folder:{}", current.display()),
                    ..Default::default()
                });
            }
            if doc.kind() == apifuck_core::DocumentKind::Folder {
                node.label = doc.name().into();
                node.id = path;
                node.seq = doc.value()["info"]["seq"].as_f64().unwrap_or(0.);
            } else {
                node.children.insert(
                    path.clone(),
                    Node {
                        label: format!(
                            "{}  {}",
                            if doc.kind() == apifuck_core::DocumentKind::Graphql {
                                "GQL"
                            } else {
                                doc.method()
                            },
                            doc.name()
                        ),
                        id: path,
                        seq: doc.value()["info"]["seq"].as_f64().unwrap_or(0.),
                        ..Default::default()
                    },
                );
            }
        }
        fn items(node: Node, collapsed: &std::collections::BTreeSet<String>) -> Vec<TreeItem> {
            let mut children: Vec<_> = node.children.into_values().collect();
            children.sort_by(|a, b| a.seq.total_cmp(&b.seq).then(a.label.cmp(&b.label)));
            children
                .into_iter()
                .map(|node| {
                    let label = node.label.clone();
                    let id = node.id.clone();
                    TreeItem::new(id.clone(), label)
                        .expanded(!collapsed.contains(&id))
                        .children(items(node, collapsed))
                })
                .collect()
        }
        let selected: SharedString = self.tree_selection.clone().into();
        self.tree.update(cx, |tree, cx| {
            tree.set_items(items(root, &self.state.collapsed_folders), cx);
            tree.set_selected_index(tree.index_of(&selected), cx);
        });
    }
    fn open_document(&mut self, path: &str, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(folder) = path.strip_prefix("folder:") {
            let result = (|| -> Result<String> {
                let collection = self.collection.as_ref().context(tr("请先打开项目"))?;
                anyhow::ensure!(
                    collection.safe_path(std::path::Path::new(folder))?.is_dir(),
                    "目录不存在"
                );
                let target = std::path::Path::new(folder).join("folder.yml");
                let name = std::path::Path::new(folder)
                    .file_name()
                    .context(tr("目录无效"))?
                    .to_string_lossy();
                collection.add(&target, &collection::template("folder", &name)?)?;
                Ok(target.to_string_lossy().into_owned())
            })();
            match result {
                Ok(path) => {
                    self.refresh(window, cx);
                    self.open_document_inner(&path, true, window, cx);
                }
                Err(error) => self.notify_error(format!("{error:#}"), window, cx),
            }
            return;
        }
        self.open_document_inner(path, true, window, cx);
    }
    fn open_document_inner(
        &mut self,
        path: &str,
        reveal: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.show_settings = false;
        if let Some(ix) = self
            .tabs
            .iter()
            .position(|t| t.read(cx).document().path().to_string_lossy() == path)
        {
            self.active = ix;
            self.tab_scroll.scroll_to_item(ix);
            if reveal {
                self.sync_active_sidebar(window, cx);
            }
            cx.notify();
            return;
        }
        let Some(collection) = &self.collection else {
            return;
        };
        match collection.find(path) {
            Ok(document) => {
                let breadcrumb = collection
                    .scopes(document)
                    .iter()
                    .filter_map(|scope| scope["info"]["name"].as_str())
                    .collect::<Vec<_>>()
                    .join(" / ");
                let document = document.clone();
                let draft = self.state.drafts.get(path).cloned();
                let layout = self
                    .preferences
                    .as_ref()
                    .map(|p| p.request_layout)
                    .unwrap_or_default();
                let tab = cx.new(|cx| DocumentTab::new(document, draft, layout, window, cx));
                tab.update(cx, |tab, cx| tab.set_breadcrumb(breadcrumb, cx));
                if let Some(state) = self.state.editor_drafts.get(path) {
                    tab.update(cx, |t, cx| t.restore_editor(state, window, cx));
                }
                self.watch_tab(&tab, window, cx);
                self.tabs.push(tab);
                self.active = self.tabs.len() - 1;
                self.tab_scroll.scroll_to_item(self.active);
            }
            Err(e) => self.notify_error(e.to_string(), window, cx),
        };
        if reveal {
            self.sync_active_sidebar(window, cx);
        }
        cx.notify();
    }
    /// Programmatic selection must not feed back into the tree's open-document observer.
    fn sync_active_sidebar(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(tab) = self.active() else {
            self.tree_selection.clear();
            self.tree
                .update(cx, |tree, cx| tree.set_selected_index(None, cx));
            return;
        };
        let document = tab.read(cx).document();
        let path = document.path().to_owned();
        let name = document.name().to_owned();
        let kind = document.kind();
        self.show_settings = false;
        if kind == apifuck_core::DocumentKind::Environment {
            self.sidebar_view = SidebarView::Environments;
            self.tree_selection.clear();
            self.tree
                .update(cx, |tree, cx| tree.set_selected_index(None, cx));
            let query = self.environment_search.read(cx).value().to_lowercase();
            if !name.to_lowercase().contains(&query) {
                self.environment_search
                    .update(cx, |input, cx| input.set_value("", window, cx));
            }
            let query = self.environment_search.read(cx).value().to_lowercase();
            let rows = environment_rows(self.collection.as_ref());
            if let Some(index) = rows
                .iter()
                .filter(|(_, name)| name.to_lowercase().contains(&query))
                .position(|(row_path, _)| *row_path == path)
            {
                self.environment_scroll.scroll_to_item(index);
            }
            return;
        }
        self.sidebar_view = SidebarView::Collections;
        if kind == apifuck_core::DocumentKind::Collection {
            self.tree_selection.clear();
            self.tree
                .update(cx, |tree, cx| tree.set_selected_index(None, cx));
            return;
        }
        let id: SharedString = collection::portable_path(&path).into();
        self.tree_selection = id.to_string();
        let found = self.reveal_tree_selection(&id, cx);
        if !found && !self.search.read(cx).value().is_empty() {
            self.search
                .update(cx, |input, cx| input.set_value("", window, cx));
            self.rebuild_tree(cx);
            self.reveal_tree_selection(&id, cx);
        }
    }
    fn reveal_tree_selection(&mut self, id: &SharedString, cx: &mut Context<Self>) -> bool {
        let (found, expanded) = self.tree.update(cx, |tree, cx| {
            let ancestors = navigation_ancestors(
                (0..)
                    .map_while(|index| tree.entry(index))
                    .filter(|entry| entry.is_root())
                    .map(|entry| entry.item()),
                id,
            );
            tree.reveal_item(id, ScrollStrategy::Nearest, cx);
            let selected = tree.index_of(id);
            tree.set_selected_index(selected, cx);
            (selected.is_some(), ancestors)
        });
        // Input events can rebuild the tree before the queued Expanded events arrive.
        // Keep persistence in sync now so that rebuild cannot collapse the revealed path.
        for folder in expanded {
            self.state.collapsed_folders.remove(&folder);
        }
        found
    }
    fn active(&self) -> Option<Entity<DocumentTab>> {
        self.tabs.get(self.active).cloned()
    }
    fn watch_tab(
        &mut self,
        tab: &Entity<DocumentTab>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let subscription = cx.observe_in(tab, window, |this, _, window, cx| {
            this.autosave(window, cx);
            cx.notify();
        });
        self.tab_subscriptions.insert(tab.entity_id(), subscription);
    }
    fn autosave(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let error = self.persist(cx).err().map(|error| {
            crate::i18n::format(tr("草稿保存失败：{e:#}"), &[("e:#", format!("{error:#}"))])
        });
        if error != self.last_autosave_error {
            if let Some(message) = &error {
                self.notify_error(message.clone(), window, cx);
            }
            self.last_autosave_error = error;
        }
    }
    fn draft_input(
        &mut self,
        key: String,
        initial: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<InputState> {
        let value = self
            .state
            .ui_drafts
            .get(&key)
            .and_then(|v| v.as_str())
            .unwrap_or(initial)
            .to_owned();
        let input = cx.new(|cx| InputState::new(window, cx).default_value(value));
        let root = self.collection.as_ref().map(|c| c.root().to_owned());
        cx.subscribe_in(
            &input,
            window,
            move |this, input, event: &InputEvent, window, cx| {
                if matches!(event, InputEvent::Change)
                    && this.collection.as_ref().map(|c| c.root()) == root.as_deref()
                {
                    this.state.ui_drafts.insert(
                        key.clone(),
                        serde_json::json!(input.read(cx).value().to_string()),
                    );
                    this.autosave(window, cx);
                }
            },
        )
        .detach();
        input
    }
    fn persist_settings_draft(&mut self, cx: &App) -> Result<()> {
        if let Some(view) = &self.settings_view {
            let draft = view.read(cx).snapshot(cx);
            let serialized = serde_json::to_string(&draft)?;
            if serialized != self.saved_settings_draft {
                settings::save_raw_draft(&AppPreferences::draft_path()?, &draft)?;
                self.saved_settings_draft = serialized;
            }
        }
        Ok(())
    }
    fn persist(&mut self, cx: &App) -> Result<()> {
        // Failure in the global settings location must not block project recovery.
        let settings_result = self.persist_settings_draft(cx);
        let Some(collection) = &self.collection else {
            return settings_result;
        };
        for (input, key) in [
            (&self.search, "search"),
            (&self.environment_search, "environmentSearch"),
        ] {
            self.state.ui_drafts.insert(
                key.into(),
                serde_json::json!(input.read(cx).value().to_string()),
            );
        }
        self.state.tabs = self
            .tabs
            .iter()
            .map(|t| t.read(cx).document().path().to_string_lossy().into_owned())
            .collect();
        self.state.active = self
            .active()
            .map(|t| t.read(cx).document().path().to_string_lossy().into_owned());
        for tab in &self.tabs {
            let tab = tab.read(cx);
            let path = tab.document().path().to_string_lossy().into_owned();
            // UI-only drafts (for example a WebSocket message) also need recovery.
            self.state
                .editor_drafts
                .insert(path.clone(), tab.editor_snapshot(cx));
            if tab.is_dirty() {
                self.state.drafts.insert(path, tab.raw_source(cx));
            } else {
                self.state.drafts.remove(&path);
            }
        }
        let snapshot = serde_json::to_string(&self.state)?;
        if snapshot != self.saved_state {
            self.state.save(collection.root())?;
            self.saved_state = snapshot;
        }
        settings_result
    }
    fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let (Some(collection), Some(tab)) = (&self.collection, self.active()) {
            match tab.update(cx, |t, cx| t.save(collection, window, cx)) {
                Ok(()) => {
                    self.notify_success(tr("已保存").into(), window, cx);
                    self.refresh(window, cx);
                }
                Err(e) => self.notify_error(format!("{e:#}"), window, cx),
            }
        }
        cx.notify();
    }
    fn notify_success(&mut self, message: String, window: &mut Window, cx: &mut Context<Self>) {
        crate::feedback::show(Notification::success(message), window, cx);
        cx.notify();
    }
    fn notify_error(&mut self, message: String, window: &mut Window, cx: &mut Context<Self>) {
        crate::feedback::show(Notification::error(message), window, cx);
        cx.notify();
    }
    fn refresh(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(c) = &self.collection {
            match Collection::open(c.root()) {
                Ok(c) => {
                    self.collection = Some(c);
                    self.rebuild_tree(cx);
                }
                Err(e) => self.notify_error(format!("{e:#}"), window, cx),
            }
        }
        cx.notify();
    }
    fn send(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let (Some(c), Some(tab)) = (&self.collection, self.active()) {
            if tab.read(cx).is_running() {
                tab.update(cx, |t, cx| t.stop(cx));
                return;
            }
            let result = tab.update(cx, |t, cx| {
                t.send(
                    c,
                    self.state.environment.as_deref(),
                    self.state.network.clone(),
                    &self.runtime,
                    window,
                    cx,
                )
            });
            if let Err(e) = result {
                tab.update(cx, |t, cx| t.set_error(format!("{e:#}"), window, cx));
            }
        }
    }
    fn close_tab(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.close_tab_at(self.active, window, cx);
    }
    fn close_tab_at(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.close_tabs(Some(index), TabCloseAction::Current, window, cx);
    }
    fn close_tabs(
        &mut self,
        target: Option<usize>,
        action: TabCloseAction,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let indices = action.indices(target, self.tabs.len());
        if indices.is_empty() {
            return;
        }
        // Persist every draft before dropping any document, including invalid YAML.
        if let Err(e) = self.persist(cx) {
            self.notify_error(e.to_string(), window, cx);
            cx.notify();
            return;
        }
        for index in indices.into_iter().rev() {
            self.active = active_after_close(self.active, index, self.tabs.len());
            let tab = self.tabs.remove(index);
            self.tab_subscriptions.remove(&tab.entity_id());
        }
        if !self.tabs.is_empty() {
            self.tab_scroll.scroll_to_item(self.active);
        }
        self.sync_active_sidebar(window, cx);
        // Save the final open-tab list immediately, keeping the closed drafts.
        if let Err(e) = self.persist(cx) {
            self.notify_error(e.to_string(), window, cx);
        }
        cx.notify();
    }
    fn creation_parent(&self, cx: &App) -> PathBuf {
        self.active()
            .and_then(|tab| {
                let document = tab.read(cx).document();
                (document.is_request() || document.kind() == apifuck_core::DocumentKind::Folder)
                    .then(|| {
                        document
                            .path()
                            .parent()
                            .unwrap_or(std::path::Path::new(""))
                            .to_owned()
                    })
            })
            .unwrap_or_default()
    }
    fn new_request_tab(&mut self, kind: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(collection) = &self.collection else {
            return;
        };
        let parent = self.creation_parent(cx);
        let base = match kind {
            "graphql" => tr("未命名 GraphQL 请求"),
            "websocket" => tr("未命名 WebSocket 请求"),
            _ => tr("未命名 HTTP 请求"),
        };
        let name = (1..)
            .map(|index| format!("{base} {index}"))
            .find(|name| {
                !collection
                    .documents()
                    .iter()
                    .any(|document| document.name() == name)
                    && !self.tabs.iter().any(|tab| tab.read(cx).name(cx) == *name)
            })
            .unwrap();
        match collection.create_request(&parent, kind, &name) {
            Ok(document) => {
                self.refresh(window, cx);
                self.open_document(&document.path().to_string_lossy(), window, cx);
                if let Some(tab) = self.active() {
                    tab.update(cx, |tab, cx| tab.focus_name(window, cx));
                }
                self.autosave(window, cx);
            }
            Err(error) => self.notify_error(format!("{error:#}"), window, cx),
        }
        cx.notify();
    }
    fn new_document(&mut self, kind: &'static str, window: &mut Window, cx: &mut Context<Self>) {
        if matches!(kind, "http" | "graphql" | "websocket") {
            self.new_request_tab(kind, window, cx);
            return;
        }
        if self.collection.is_none() || !matches!(kind, "folder" | "environment") {
            return;
        }
        let parent = self.creation_parent(cx);
        // Name drafts are separate from the old file-path input drafts.
        let draft_key = format!("new-name:{kind}");
        let name = self.draft_input(draft_key.clone(), "", window, cx);
        let label = if kind == "environment" {
            tr("环境名称")
        } else {
            tr("目录名称")
        };
        name.update(cx, |input, cx| input.set_placeholder(label, window, cx));
        let handle = cx.entity().downgrade();
        window.open_dialog(cx, move |dialog, _, _| {
            let input = name.clone();
            let handle = handle.clone();
            let draft_key = draft_key.clone();
            let parent = parent.clone();
            dialog
                .title(if kind == "environment" {
                    tr("新建环境")
                } else {
                    tr("新建目录")
                })
                .child(label)
                .child(Input::new(&name).aria_label(label))
                .button_props(
                    DialogButtonProps::default()
                        .ok_text(tr("创建"))
                        .cancel_text(tr("取消"))
                        .show_cancel(true),
                )
                .on_ok(move |_, window, cx| {
                    let name = input.read(cx).value().trim().to_owned();
                    handle
                        .update(cx, |this, cx| {
                            let result = (|| {
                                let collection =
                                    this.collection.as_ref().context(tr("请先打开项目"))?;
                                if kind == "environment" {
                                    collection.create_environment(&name)
                                } else {
                                    collection.create_folder(&parent, &name)
                                }
                            })();
                            match result {
                                Ok(document) => {
                                    this.state.ui_drafts.remove(&draft_key);
                                    this.state.ui_drafts.remove(&format!("new:{kind}"));
                                    this.refresh(window, cx);
                                    if kind == "environment" {
                                        this.sidebar_view = SidebarView::Environments;
                                        this.environment_search.update(cx, |search, cx| {
                                            search.set_value("", window, cx)
                                        });
                                    }
                                    this.open_document(
                                        &document.path().to_string_lossy(),
                                        window,
                                        cx,
                                    );
                                    this.autosave(window, cx);
                                    true
                                }
                                Err(error) => {
                                    this.notify_error(
                                        tr(&format!("{error:#}")).to_owned(),
                                        window,
                                        cx,
                                    );
                                    false
                                }
                            }
                        })
                        .unwrap_or(true)
                })
        });
    }
    fn rename(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(tab) = self.active() else {
            return;
        };
        let doc = tab.read(cx).document().clone();
        let draft_key = format!("rename:{}", doc.path().display());
        let path = self.draft_input(draft_key.clone(), &doc.path().to_string_lossy(), window, cx);
        let handle = cx.entity().downgrade();
        window.open_dialog(cx, move |dialog, _, _| {
            let input = path.clone();
            let handle = handle.clone();
            let doc = doc.clone();
            let draft_key = draft_key.clone();
            dialog
                .title(tr("移动或重命名文档"))
                .child(Input::new(&path))
                .button_props(
                    DialogButtonProps::default()
                        .ok_text(tr("移动"))
                        .cancel_text(tr("取消"))
                        .show_cancel(true),
                )
                .on_ok(move |_, window, cx| {
                    let target = PathBuf::from(input.read(cx).value().as_ref());
                    handle
                        .update(cx, |this, cx| {
                            let result = (|| -> Result<()> {
                                let c = this.collection.as_ref().context(tr("项目未打开"))?;
                                let folder = doc.kind() == apifuck_core::DocumentKind::Folder;
                                let from = if folder {
                                    doc.path().parent().unwrap()
                                } else {
                                    doc.path()
                                };
                                let to = if folder {
                                    target.parent().context(tr("目标目录无效"))?
                                } else {
                                    target.as_path()
                                };
                                let snapshots: Vec<_> = this
                                    .tabs
                                    .iter()
                                    .enumerate()
                                    .filter_map(|(ix, t)| {
                                        let tab = t.read(cx);
                                        tab.document().path().strip_prefix(from).ok().map(
                                            |suffix| (ix, to.join(suffix), tab.editor_snapshot(cx)),
                                        )
                                    })
                                    .collect();
                                c.rename(&doc, &target)?;
                                let c = Collection::open(c.root())?;
                                for (ix, path, snapshot) in snapshots {
                                    let renamed = c.find(&path.to_string_lossy())?.clone();
                                    let layout = this
                                        .preferences
                                        .as_ref()
                                        .map(|p| p.request_layout)
                                        .unwrap_or_default();
                                    let entity = cx.new(|cx| {
                                        DocumentTab::new(renamed, None, layout, window, cx)
                                    });
                                    entity.update(cx, |t, cx| {
                                        t.restore_editor(&snapshot, window, cx)
                                    });
                                    this.tab_subscriptions.remove(&this.tabs[ix].entity_id());
                                    this.watch_tab(&entity, window, cx);
                                    this.tabs[ix] = entity;
                                }
                                this.collection = Some(c);
                                this.state.ui_drafts.remove(&draft_key);
                                this.persist(cx)?;
                                Ok(())
                            })();
                            match result {
                                Ok(()) => {
                                    cx.notify();
                                    true
                                }
                                Err(e) => {
                                    this.notify_error(format!("{e:#}"), window, cx);
                                    cx.notify();
                                    false
                                }
                            }
                        })
                        .unwrap_or(true)
                })
        });
    }
    fn duplicate(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let (Some(c), Some(tab)) = (&self.collection, self.active()) {
            let doc = tab.read(cx).document();
            let mut path = doc.path().to_owned();
            let stem = path
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned();
            let mut n = 1;
            loop {
                path.set_file_name(format!("{stem}-copy-{n}.yml"));
                if !c.root().join(&path).exists() {
                    break;
                }
                n += 1;
            }
            let result = tab
                .read(cx)
                .draft(cx)
                .and_then(|source| c.add(&path, &source));
            match result {
                Ok(_) => {
                    self.refresh(window, cx);
                    self.open_document(&path.to_string_lossy(), window, cx);
                    self.notify_success(tr("已创建副本").into(), window, cx);
                }
                Err(e) => self.notify_error(format!("{e:#}"), window, cx),
            }
        }
        cx.notify();
    }
    fn trash(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let (Some(c), Some(tab)) = (&self.collection, self.active()) {
            let path = tab
                .read(cx)
                .document()
                .path()
                .to_string_lossy()
                .into_owned();
            match c.trash(tab.read(cx).document()) {
                Ok(()) => {
                    let prefix =
                        if tab.read(cx).document().kind() == apifuck_core::DocumentKind::Folder {
                            tab.read(cx).document().path().parent().unwrap().to_owned()
                        } else {
                            PathBuf::from(&path)
                        };
                    self.tabs
                        .retain(|t| !t.read(cx).document().path().starts_with(&prefix));
                    self.active = self.active.min(self.tabs.len().saturating_sub(1));
                    self.state
                        .drafts
                        .retain(|key, _| !std::path::Path::new(key).starts_with(&prefix));
                    self.state
                        .editor_drafts
                        .retain(|key, _| !std::path::Path::new(key).starts_with(&prefix));
                    self.notify_success(
                        tr("文档已移至项目 .apifuck/trash，可从文件夹恢复").into(),
                        window,
                        cx,
                    );
                    self.refresh(window, cx);
                    self.sync_active_sidebar(window, cx);
                }
                Err(e) => self.notify_error(format!("{e:#}"), window, cx),
            }
        }
        cx.notify();
    }
    fn reorder_tree(
        &mut self,
        source: &std::path::Path,
        target: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let result = (|| -> Result<()> {
            let collection = self.collection.as_ref().context(tr("请先打开项目"))?;
            for tab in &self.tabs {
                serde_yaml::from_str::<serde_json::Value>(&tab.read(cx).raw_source(cx))?;
            }
            collection.reorder_before(
                collection.find(&source.to_string_lossy())?,
                collection.find(target)?,
            )?;
            let collection = Collection::open(collection.root())?;
            for tab in &self.tabs {
                let path = tab
                    .read(cx)
                    .document()
                    .path()
                    .to_string_lossy()
                    .into_owned();
                if let Ok(document) = collection.find(&path) {
                    tab.update(cx, |tab, cx| tab.reordered(document.clone(), window, cx))?;
                }
            }
            self.collection = Some(collection);
            self.rebuild_tree(cx);
            self.autosave(window, cx);
            Ok(())
        })();
        if let Err(error) = result {
            self.notify_error(format!("{error:#}"), window, cx);
        }
    }
    fn export(&mut self, language: &str, window: &mut Window, cx: &mut Context<Self>) {
        if let (Some(collection), Some(tab)) = (&self.collection, self.active()) {
            let codes: Vec<(&'static str, Result<String, String>)> =
                ["curl", "javascript", "python", "go"]
                    .into_iter()
                    .map(|language| {
                        let code = tab
                            .update(cx, |tab, cx| {
                                tab.export(
                                    collection,
                                    self.state.environment.as_deref(),
                                    self.state.network.clone(),
                                    language,
                                    window,
                                    cx,
                                )
                            })
                            .map_err(|error| format!("{error:#}"));
                        (language, code)
                    })
                    .collect();
            if let Some((_, Ok(code))) = codes.iter().find(|(name, _)| *name == language) {
                cx.write_to_clipboard(ClipboardItem::new_string(code.clone()));
            }
            let view =
                cx.new(|cx| crate::export_view::ExportView::new(codes, language, window, cx));
            window.open_dialog(cx, move |dialog, _, _| {
                dialog.title(tr("生成代码")).w(px(800.)).child(view.clone())
            });
        }
    }
    fn import_bru(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(collection) = self.collection.clone() else {
            return;
        };
        let choice = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some(tr("导入 .bru 文件").into()),
        });
        cx.spawn_in(window, async move |this, cx| {
            let Ok(Ok(Some(paths))) = choice.await else {
                return;
            };
            let Some(path) = paths.into_iter().next() else {
                return;
            };
            let task = cx.background_spawn(async move {
                let name = path
                    .file_stem()
                    .and_then(|n| n.to_str())
                    .unwrap_or("imported");
                let kind = if name == "folder" {
                    "folder"
                } else if path
                    .parent()
                    .and_then(|p| p.file_name())
                    .is_some_and(|p| p == "environments")
                {
                    "environment"
                } else {
                    "request"
                };
                let source =
                    apifuck_core::bru::convert(&std::fs::read_to_string(&path)?, kind, "import")?;
                let mut value: serde_json::Value = serde_yaml::from_str(&source)?;
                if kind == "environment" {
                    value["name"] = serde_json::json!(name);
                }
                let mut target = if kind == "folder" {
                    PathBuf::from("imported/folder.yml")
                } else if kind == "environment" {
                    PathBuf::from(format!("environments/{name}.yml"))
                } else {
                    PathBuf::from(format!("{name}.yml"))
                };
                let mut suffix = 1;
                while collection.root().join(&target).exists() {
                    if kind == "folder" {
                        target = PathBuf::from(format!("imported-{suffix}/folder.yml"));
                    } else {
                        target.set_file_name(format!("{name}-{suffix}.yml"));
                    }
                    suffix += 1;
                }
                collection.add(&target, &serde_yaml::to_string(&value)?)?;
                Ok::<_, anyhow::Error>(target)
            });
            let result = task.await;
            let _ = this.update_in(cx, |this, window, cx| match result {
                Ok(path) => {
                    this.refresh(window, cx);
                    this.open_document(&path.to_string_lossy(), window, cx);
                    this.notify_success(tr("已导入 Bru 文件").into(), window, cx);
                }
                Err(e) => {
                    this.notify_error(format!("{e:#}"), window, cx);
                }
            });
        })
        .detach();
    }
    fn export_bru(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(tab) = self.active() else {
            return;
        };
        let source = match tab.read(cx).draft(cx) {
            Ok(source) => source,
            Err(e) => {
                self.notify_error(e.to_string(), window, cx);
                return;
            }
        };
        let kind = match tab.read(cx).document().kind() {
            apifuck_core::DocumentKind::Environment => "environment",
            apifuck_core::DocumentKind::Folder => "folder",
            _ => "request",
        };
        let task =
            cx.background_spawn(async move { apifuck_core::bru::convert(&source, kind, "export") });
        cx.spawn_in(window, async move |this, cx| {
            let result = task.await;
            let _ = this.update_in(cx, |this, window, cx| match result {
                Ok(source) => {
                    cx.write_to_clipboard(ClipboardItem::new_string(source));
                    this.notify_success(tr("已复制 Bru 源码").into(), window, cx);
                }
                Err(e) => {
                    this.notify_error(format!("{e:#}"), window, cx);
                }
            });
        })
        .detach();
    }
    fn settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.settings_view.is_none() {
            let view = cx.new(|cx| {
                SettingsView::new(
                    self.preferences
                        .clone()
                        .unwrap_or_else(|| AppPreferences::from_workspace(&self.state)),
                    self.mcp.clone(),
                    self.mcp_error.clone(),
                    window,
                    cx,
                )
            });
            self.settings_subscription = Some(cx.subscribe_in(
                &view,
                window,
                |this, _, event, window, cx| {
                    match event {
                        SettingsEvent::Save(preferences) => {
                            let result =
                                AppPreferences::path().and_then(|p| preferences.save_to(&p));
                            match result {
                                Ok(()) => {
                                    preferences.apply_to(&mut this.state);
                                    crate::i18n::set_language(&preferences.language);
                                    crate::refresh_menus(cx);
                                    this.search.update(cx, |search, cx| {
                                        search.set_placeholder(tr("搜索集合"), window, cx)
                                    });
                                    this.environment_search.update(cx, |search, cx| {
                                        search.set_placeholder(tr("搜索环境"), window, cx)
                                    });
                                    for tab in &this.tabs {
                                        tab.update(cx, |tab, cx| {
                                            tab.set_request_layout(preferences.request_layout, cx);
                                            tab.refresh_locale(window, cx);
                                        });
                                    }
                                    cx.refresh_windows();
                                    this.preferences = Some(preferences.clone());
                                    this.updates.update(cx, |updates, cx| {
                                        updates.set_auto_check(
                                            preferences.auto_check_updates,
                                            window,
                                            cx,
                                        )
                                    });
                                    crate::appearance::apply(
                                        &this.state.appearance,
                                        &this.state.accent,
                                        window,
                                        cx,
                                    );
                                    this.notify_success(
                                        tr("设置已保存并生效。").into(),
                                        window,
                                        cx,
                                    );
                                }
                                Err(error) => {
                                    let message = crate::i18n::format(
                                        tr("保存失败：{error:#}"),
                                        &[("error:#", format!("{error:#}"))],
                                    );
                                    this.notify_error(message, window, cx);
                                }
                            }
                        }
                        SettingsEvent::CheckUpdates => this
                            .updates
                            .update(cx, |updates, cx| updates.open(window, cx)),
                    }
                    cx.notify();
                },
            ));
            cx.observe_in(&view, window, |this, _, window, cx| {
                this.autosave(window, cx)
            })
            .detach();
            self.settings_view = Some(view);
        }
        let notes_view = self.settings_view.as_ref().unwrap().downgrade();
        let task = self.runtime.spawn(apifuck_core::update::release_notes(env!(
            "CARGO_PKG_VERSION"
        )));
        cx.spawn_in(window, async move |_, cx| {
            let notes = match task.await {
                Ok(Ok(Some(notes))) => notes,
                Ok(Ok(None)) => tr("当前版本尚未发布版本介绍。").into(),
                _ => {
                    let _ = cx.update(|window, cx| {
                        crate::feedback::show(
                            Notification::error(
                                tr("无法加载版本介绍，请检查网络后重新打开设置。").to_owned(),
                            ),
                            window,
                            cx,
                        );
                    });
                    String::new()
                }
            };
            let _ = notes_view.update(cx, |view, cx| view.set_notes(notes, cx));
        })
        .detach();
        self.show_settings = true;
        cx.notify();
    }
    fn reload_tab(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.refresh(window, cx);
        if let (Some(c), Some(tab)) = (&self.collection, self.active()) {
            let path = tab
                .read(cx)
                .document()
                .path()
                .to_string_lossy()
                .into_owned();
            let Ok(document) = c.find(&path).cloned() else {
                return;
            };
            if tab.read(cx).is_dirty() {
                window.open_dialog(cx, move |dialog, _, _| {
                    let tab = tab.clone();
                    let document = document.clone();
                    dialog
                        .title(tr("重新载入并放弃未保存的编辑？"))
                        .child(tr("磁盘上的文件将替换当前标签页的草稿。"))
                        .button_props(
                            DialogButtonProps::default()
                                .ok_text(tr("放弃编辑并载入"))
                                .cancel_text(tr("取消"))
                                .show_cancel(true),
                        )
                        .on_ok(move |_, window, cx| {
                            tab.update(cx, |t, cx| t.reload(document.clone(), window, cx));
                            true
                        })
                });
            } else {
                tab.update(cx, |t, cx| t.reload(document, window, cx));
            }
        }
    }
}
impl Render for Workbench {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let weak = cx.entity().downgrade();
        let env_weak = weak.clone();
        let selected_environment = self.state.environment.clone();
        let more_weak = weak.clone();
        let export_weak = weak.clone();
        let environments = self
            .collection
            .as_ref()
            .map(Collection::environment_names)
            .unwrap_or_default();
        let title = self
            .collection
            .as_ref()
            .map(|c| c.project().name().to_owned())
            .unwrap_or_else(|| "ApiFuck".into());
        let active = self.active();
        if let (Some(collection), Some(tab)) = (&self.collection, &active) {
            let mut names = std::collections::BTreeSet::new();
            if let Ok(env) = collection.environment(self.state.environment.as_deref()) {
                names.extend(
                    env.as_object()
                        .into_iter()
                        .flatten()
                        .map(|(name, _)| name.clone()),
                );
            }
            if let Ok(env) = settings::overlay(collection.root(), self.state.environment.as_deref())
            {
                names.extend(
                    env.as_object()
                        .into_iter()
                        .flatten()
                        .map(|(name, _)| name.clone()),
                );
            }
            for scope in collection.scopes(tab.read(cx).document()) {
                names.extend(
                    scope["request"]["variables"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter_map(|v| v["name"].as_str().map(str::to_owned)),
                );
            }
            if let Ok(source) = tab.read(cx).draft(cx)
                && let Ok(value) = serde_yaml::from_str::<serde_json::Value>(&source)
            {
                for scope in ["runtime", "request"] {
                    names.extend(
                        value[scope]["variables"]
                            .as_array()
                            .into_iter()
                            .flatten()
                            .filter_map(|v| v["name"].as_str().map(str::to_owned)),
                    );
                }
            }
            let values = tab.read(cx).draft(cx).and_then(|source| {
                let document = apifuck_core::Document::parse(
                    tab.read(cx).document().path().to_owned(),
                    source,
                )?;
                apifuck_core::engine::preview_variables(
                    collection,
                    &document,
                    self.state.environment.as_deref(),
                )
            });
            let variables = crate::variable_input::Variables::new(
                names.into_iter().collect(),
                values,
                self.state.environment.clone(),
            );
            tab.update(cx, |tab, cx| tab.set_variables(variables, cx));
        }
        let request = active.as_ref().is_some_and(|t| {
            matches!(
                t.read(cx).document().kind(),
                apifuck_core::DocumentKind::Http | apifuck_core::DocumentKind::Graphql
            )
        });
        let project_selected = active.as_ref().is_some_and(|tab| {
            tab.read(cx).document().kind() == apifuck_core::DocumentKind::Collection
        });
        let folder_ids = self
            .collection
            .as_ref()
            .map(|collection| {
                collection
                    .documents()
                    .iter()
                    .filter(|doc| doc.kind() == apifuck_core::DocumentKind::Folder)
                    .map(|doc| doc.path().to_string_lossy().into_owned())
                    .collect::<std::collections::BTreeSet<_>>()
            })
            .unwrap_or_default();
        let tree_handle = cx.entity().downgrade();
        let tree_menu_handle = tree_handle.clone();
        let sidebar = div()
            .flex()
            .flex_col()
            .size_full()
            .min_h_0()
            .bg(cx.theme().sidebar)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .px_3()
                    .pt_3()
                    .pb_2()
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(tr("集合")),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(
                                self.collection
                                    .as_ref()
                                    .map(|c| {
                                        c.documents().iter().filter(|d| d.is_request()).count()
                                    })
                                    .unwrap_or(0)
                                    .to_string(),
                            ),
                    )
                    .child(div().flex_1())
                    .child(
                        Button::new("collection-settings")
                            .small()
                            .ghost()
                            .selected(project_selected)
                            .icon(AppIcon::Settings)
                            .tooltip(tr("集合配置"))
                            .accessibility_label(tr("集合配置"))
                            .disabled(self.collection.is_none())
                            .on_click(cx.listener(|this, _, window, cx| {
                                if let Some(collection) = &this.collection {
                                    let path =
                                        collection.project().path().to_string_lossy().into_owned();
                                    this.open_document(&path, window, cx);
                                }
                            })),
                    )
                    .child(
                        Button::new("new-document")
                            .small()
                            .ghost()
                            .flex_shrink_0()
                            .icon(IconName::Plus)
                            .tooltip(tr("新建请求、目录或环境"))
                            .accessibility_label(tr("新建"))
                            .disabled(self.collection.is_none())
                            .dropdown_menu(move |menu, _, _| {
                                let mut menu = menu;
                                for (label, kind) in [
                                    (tr("HTTP 请求…"), "http"),
                                    ("GraphQL…", "graphql"),
                                    ("WebSocket…", "websocket"),
                                    (tr("目录…"), "folder"),
                                    (tr("环境…"), "environment"),
                                ] {
                                    let weak = weak.clone();
                                    menu = menu.item(
                                        PopupMenuItem::new(label)
                                            .icon(crate::icons::document_kind(kind))
                                            .on_click(move |_, window, cx| {
                                                let _ = weak.update(cx, |this, cx| {
                                                    this.new_document(kind, window, cx)
                                                });
                                            }),
                                    );
                                }
                                menu
                            }),
                    )
                    .child(
                        Button::new("refresh")
                            .small()
                            .ghost()
                            .icon(IconName::RefreshCw)
                            .tooltip(tr("刷新集合"))
                            .accessibility_label(tr("刷新集合"))
                            .on_click(cx.listener(|this, _, window, cx| this.refresh(window, cx))),
                    ),
            )
            .child(
                div().px_3().pb_3().child(
                    Input::new(&self.search)
                        .prefix(Icon::new(AppIcon::Search).small())
                        .small()
                        .aria_label(tr("搜索集合")),
                ),
            )
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .px_2()
                    .pt_1()
                    .pb_2()
                    .capture_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                        if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                            if let Some(item) = this.tree.read(cx).selected_item() {
                                let path = item.id.to_string();
                                this.open_document(&path, window, cx);
                            }
                            cx.stop_propagation();
                        }
                    }))
                    .child(
                        Tree::new(&self.tree, move |_, entry, _, _, cx| {
                            let label = entry.item().label.to_string();
                            let tooltip = label.clone();
                            let row_path = entry.item().id.to_string();
                            let click_path = row_path.clone();
                            let drop_path = row_path.clone();
                            let click_handle = tree_handle.clone();
                            let drop_handle = tree_handle.clone();
                            let toggle_handle = tree_handle.clone();
                            let toggle_path = row_path.clone();
                            let expanded = entry.is_expanded();
                            let drag = crate::tab_drag::DraggedRequest {
                                path: PathBuf::from(&row_path),
                                title: label.clone(),
                            };
                            let folder = entry.is_folder()
                                || folder_ids.contains(entry.item().id.as_ref())
                                || entry.item().id.starts_with("folder:");
                            ListItem::new(SharedString::from(format!("tree:{}", entry.item().id)))
                                // The underlying Tree toggles on mouse-down, before click.
                                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                                .on_click(move |_, window, cx| {
                                    cx.stop_propagation();
                                    let _ = click_handle.update(cx, |this, cx| {
                                        this.tree_selection = click_path.clone();
                                        this.open_document(&click_path, window, cx);
                                        let selected = this.tree_selection.clone();
                                        this.tree.update(cx, |tree, cx| {
                                            tree.set_selected_index(
                                                tree.index_of(&selected.into()),
                                                cx,
                                            );
                                            tree.focus(window, cx);
                                        });
                                    });
                                })
                                .on_drag(drag, |drag, _, _, cx| {
                                    cx.new(|_| crate::tab_drag::Preview {
                                        title: drag.title.clone(),
                                    })
                                })
                                .on_drop(
                                    move |drag: &crate::tab_drag::DraggedRequest, window, cx| {
                                        let _ = drop_handle.update(cx, |this, cx| {
                                            this.reorder_tree(&drag.path, &drop_path, window, cx)
                                        });
                                    },
                                )
                                .accessibility_label(label.clone())
                                .pl(rems(entry.depth() as f32 * 0.75 + 0.5))
                                .pr_3()
                                .py_0()
                                .h_9()
                                .w_full()
                                .min_w_0()
                                .overflow_hidden()
                                .rounded_md()
                                .child(
                                    div()
                                        .id(SharedString::from(format!(
                                            "tree-label:{}",
                                            entry.item().id
                                        )))
                                        .flex()
                                        .items_center()
                                        .flex_1()
                                        .min_w_0()
                                        .gap_2()
                                        .text_sm()
                                        .tooltip(move |window, cx| {
                                            Tooltip::new(tooltip.clone()).build(window, cx)
                                        })
                                        .when(folder, |row| {
                                            row.child(
                                                Button::new(SharedString::from(format!(
                                                    "toggle:{toggle_path}"
                                                )))
                                                .ghost()
                                                .xsmall()
                                                .icon(if expanded {
                                                    AppIcon::ChevronDown
                                                } else {
                                                    AppIcon::ChevronRight
                                                })
                                                .on_click(move |_, _, cx| {
                                                    cx.stop_propagation();
                                                    let _ = toggle_handle.update(cx, |this, cx| {
                                                        if expanded {
                                                            this.state
                                                                .collapsed_folders
                                                                .insert(toggle_path.clone());
                                                        } else {
                                                            this.state
                                                                .collapsed_folders
                                                                .remove(&toggle_path);
                                                        }
                                                        this.rebuild_tree(cx);
                                                    });
                                                }),
                                            )
                                            .child(
                                                Icon::new(
                                                    if entry.is_folder() && entry.is_expanded() {
                                                        gpui_kit::assets::IconName::FolderOpen
                                                    } else {
                                                        gpui_kit::assets::IconName::Folder
                                                    },
                                                )
                                                .size_4()
                                                .flex_shrink_0()
                                                .text_color(cx.theme().muted_foreground),
                                            )
                                        })
                                        .child({
                                            if !folder
                                                && let Some((method, name)) = label.split_once("  ")
                                            {
                                                let method_color =
                                                    crate::appearance::method_color(method, cx);
                                                div()
                                                    .flex()
                                                    .items_center()
                                                    .gap(px(6.))
                                                    .flex_1()
                                                    .min_w_0()
                                                    .child(
                                                        div()
                                                            .flex()
                                                            .items_center()
                                                            .justify_center()
                                                            .w(rems(3.25))
                                                            .h(px(20.))
                                                            .px(px(3.))
                                                            .flex_shrink_0()
                                                            .rounded(px(4.))
                                                            .bg(method_color.opacity(
                                                                if cx.theme().is_dark() {
                                                                    0.14
                                                                } else {
                                                                    0.09
                                                                },
                                                            ))
                                                            .text_size(px(11.))
                                                            .font_weight(FontWeight::SEMIBOLD)
                                                            .text_color(method_color)
                                                            .child(
                                                                div()
                                                                    .truncate()
                                                                    .child(method.to_owned()),
                                                            ),
                                                    )
                                                    .child(
                                                        div()
                                                            .flex_1()
                                                            .min_w_0()
                                                            .truncate()
                                                            .child(name.to_owned()),
                                                    )
                                                    .into_any_element()
                                            } else {
                                                div()
                                                    .flex_1()
                                                    .min_w_0()
                                                    .truncate()
                                                    .font_weight(FontWeight::MEDIUM)
                                                    .child(label)
                                                    .into_any_element()
                                            }
                                        }),
                                )
                        })
                        .context_menu(move |_, entry, mut menu, _, cx| {
                            let path = entry.item().id.to_string();
                            let handle = tree_menu_handle.clone();
                            if handle.upgrade().is_some_and(|view| {
                                view.read(cx).collection.as_ref().is_some_and(|collection| {
                                    collection.find(&path).is_ok_and(|doc| {
                                        matches!(
                                            doc.kind(),
                                            apifuck_core::DocumentKind::Http
                                                | apifuck_core::DocumentKind::Graphql
                                        )
                                    })
                                })
                            }) {
                                for language in ["curl", "javascript", "python", "go"] {
                                    let path = path.clone();
                                    let handle = handle.clone();
                                    menu = menu.item(
                                        PopupMenuItem::new(format!("{} {language}", tr("复制为")))
                                            .on_click(move |_, window, cx| {
                                                let _ = handle.update(cx, |this, cx| {
                                                    this.open_document(&path, window, cx);
                                                    this.export(language, window, cx);
                                                });
                                            }),
                                    );
                                }
                            }
                            menu
                        }),
                    ),
            );
        let sidebar = if self.sidebar_view == SidebarView::Environments {
            self.render_environment_sidebar(cx).into_any_element()
        } else {
            sidebar.into_any_element()
        };
        let workspace_weak = cx.entity().downgrade();
        let recent_projects = self.recent_projects.entries().to_vec();
        let current_project_path = self
            .collection
            .as_ref()
            .map(|project| project.root().to_owned());
        let toolbar = div()
            .flex()
            .items_center()
            .h_12()
            .px_4()
            .gap_2()
            .border_b_1()
            .border_color(cx.theme().border)
            .child(
                img(crate::brand::LOGO)
                    .size_7()
                    .object_fit(ObjectFit::Contain),
            )
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("ApiFuck"),
            )
            .child(
                Button::new("workspace-menu")
                    .ghost()
                    .small()
                    .max_w(px(240.))
                    .min_w_0()
                    .child(div().truncate().child(if self.collection.is_some() {
                        title.clone()
                    } else {
                        tr("项目").into()
                    }))
                    .when_some(self.collection.as_ref(), |button, collection| {
                        button.tooltip(collection.root().display().to_string())
                    })
                    .icon(AppIcon::Layers)
                    .dropdown_caret(true)
                    .dropdown_menu(move |menu, window, cx| {
                        let open = workspace_weak.clone();
                        let new = workspace_weak.clone();
                        let import = workspace_weak.clone();
                        let recent = workspace_weak.clone();
                        let projects = recent_projects.clone();
                        let active_path = current_project_path.clone();
                        menu.item(
                            PopupMenuItem::new(tr("打开项目…"))
                                .icon(AppIcon::FolderOpen)
                                .on_click(move |_, window, cx| {
                                    let _ = open.update(cx, |this, cx| {
                                        this.choose_project(false, window, cx)
                                    });
                                }),
                        )
                        .submenu(tr("最近打开"), window, cx, move |mut menu, _, _| {
                            if projects.is_empty() {
                                return menu
                                    .item(PopupMenuItem::new(tr("暂无最近项目")).disabled(true));
                            }
                            for project in &projects {
                                let path = project.path.clone();
                                let name = project.name.clone();
                                let display_path = path.display().to_string();
                                let tooltip = format!("{name}\n{display_path}");
                                let handle = recent.clone();
                                let checked = active_path.as_ref() == Some(&path);
                                menu = menu.item(
                                    PopupMenuItem::element(move |_, cx| {
                                        let tooltip = tooltip.clone();
                                        div()
                                            .id(SharedString::from(format!(
                                                "recent-project:{display_path}"
                                            )))
                                            .flex()
                                            .flex_col()
                                            .gap_1()
                                            .w(px(280.))
                                            .min_w_0()
                                            .child(div().text_sm().truncate().child(name.clone()))
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .text_color(cx.theme().muted_foreground)
                                                    .truncate()
                                                    .child(display_path.clone()),
                                            )
                                            .tooltip(move |window, cx| {
                                                Tooltip::new(tooltip.clone()).build(window, cx)
                                            })
                                    })
                                    .checked(checked)
                                    .on_click(
                                        move |_, window, cx| {
                                            let _ = handle.update(cx, |this, cx| {
                                                this.load(path.clone(), window, cx);
                                            });
                                        },
                                    ),
                                );
                            }
                            menu
                        })
                        .item(
                            PopupMenuItem::new(tr("新建项目…"))
                                .icon(AppIcon::FolderPlus)
                                .on_click(move |_, window, cx| {
                                    let _ = new.update(cx, |this, cx| {
                                        this.choose_project(true, window, cx)
                                    });
                                }),
                        )
                        .item(
                            PopupMenuItem::new(tr("导入 .bru…"))
                                .icon(AppIcon::FileInput)
                                .on_click(move |_, window, cx| {
                                    let _ =
                                        import.update(cx, |this, cx| this.import_bru(window, cx));
                                }),
                        )
                    }),
            )
            .child(div().flex_1())
            .child(
                Button::new("environment")
                    .outline()
                    .small()
                    .label(
                        self.state
                            .environment
                            .clone()
                            .unwrap_or_else(|| tr("未选择环境").into()),
                    )
                    .icon(AppIcon::Globe)
                    .dropdown_caret(true)
                    .dropdown_menu(move |menu, _, _| {
                        let mut menu = menu;
                        for env in
                            std::iter::once(None).chain(environments.iter().cloned().map(Some))
                        {
                            let handle = env_weak.clone();
                            let label = env.clone().unwrap_or_else(|| tr("未选择环境").into());
                            menu = menu.item(
                                PopupMenuItem::new(label)
                                    .icon(AppIcon::Globe)
                                    .checked(selected_environment == env)
                                    .on_click(move |_, _, cx| {
                                        let _ = handle.update(cx, |this, cx| {
                                            this.state.environment = env.clone();
                                            cx.notify();
                                        });
                                    }),
                            );
                        }
                        menu
                    }),
            );
        // Share one row with the native traffic lights instead of stacking a
        // system title bar above our toolbar. TitleBar owns dragging/double-clicks.
        #[cfg(target_os = "macos")]
        let toolbar = gpui_kit::component::TitleBar::new()
            .h(px(48.))
            .bg(cx.theme().background)
            .border_color(cx.theme().border)
            .child(toolbar.h_full().w_full().min_w_0().border_b_0());
        // TabBar requires Tab children, so attach the context menu to its wrapper.
        // The tab records its identity during pointer dispatch; the menu builder
        // runs afterwards. Identity remains valid if tabs are reordered meanwhile.
        let context_target = Rc::new(Cell::new(None));
        let tabs = TabBar::new("documents")
            .track_scroll(&self.tab_scroll)
            .max_width(rems(14.).to_pixels(window.rem_size()))
            .selected_index(self.active)
            .menu(true)
            .children(self.tabs.iter().map(|t| {
                let path = t.read(cx).document().path().to_owned();
                let target = path.clone();
                let tab_id = t.entity_id();
                let context_target = context_target.clone();
                let drag = crate::tab_drag::DraggedRequest {
                    path: path.clone(),
                    title: t.read(cx).name(cx),
                };
                let tooltip = format!("{}\n{}", t.read(cx).name(cx), path.display());
                Tab::new()
                    .flex_shrink_0()
                    .on_mouse_down(MouseButton::Right, move |_, _, _| {
                        context_target.set(Some(tab_id));
                    })
                    .label(t.read(cx).name(cx))
                    .when(t.read(cx).is_dirty(), |tab| {
                        tab.child(
                            div()
                                .ml_2()
                                .size(px(6.))
                                .flex_shrink_0()
                                .rounded_full()
                                .bg(cx.theme().warning),
                        )
                    })
                    .tooltip(move |window, cx| Tooltip::new(tooltip.clone()).build(window, cx))
                    .on_drag(drag, |drag, _, _, cx| {
                        cx.new(|_| crate::tab_drag::Preview {
                            title: drag.title.clone(),
                        })
                    })
                    .drag_over::<crate::tab_drag::DraggedRequest>(|tab, _, _, cx| {
                        tab.border_l_2().border_color(cx.theme().primary)
                    })
                    .on_drop(cx.listener(
                        move |this, drag: &crate::tab_drag::DraggedRequest, _, cx| {
                            if drag.path == target {
                                return;
                            }
                            let active = this.active();
                            if let Some(from) = this
                                .tabs
                                .iter()
                                .position(|t| t.read(cx).document().path() == drag.path)
                            {
                                let moved = this.tabs.remove(from);
                                let to = this
                                    .tabs
                                    .iter()
                                    .position(|t| t.read(cx).document().path() == target)
                                    .unwrap_or(this.tabs.len());
                                this.tabs.insert(to, moved);
                                if let Some(active) = active {
                                    this.active =
                                        this.tabs.iter().position(|t| *t == active).unwrap_or(0);
                                }
                                this.tab_scroll.scroll_to_item(this.active);
                                cx.notify();
                            }
                        },
                    ))
                    .suffix(
                        Button::new(SharedString::from(format!("close:{}", path.display())))
                            .ghost()
                            .xsmall()
                            .icon(IconName::Close)
                            .accessibility_label(tr("关闭标签"))
                            .on_click(cx.listener(move |this, _, window, cx| {
                                cx.stop_propagation();
                                if let Some(ix) = this
                                    .tabs
                                    .iter()
                                    .position(|t| t.read(cx).document().path() == path)
                                {
                                    this.close_tab_at(ix, window, cx);
                                }
                            })),
                    )
            }))
            .on_click(cx.listener(|this, ix, window, cx| {
                this.active = *ix;
                this.sync_active_sidebar(window, cx);
                cx.notify();
            }));
        let menu_handle = cx.entity().downgrade();
        let tabs = div()
            .id("document-tabs-context-menu")
            .flex_1()
            .min_w_0()
            .child(tabs)
            .context_menu(move |mut menu, _, cx| {
                let target_id = context_target.take();
                let Some(workbench) = menu_handle.upgrade() else {
                    return menu;
                };
                let workbench = workbench.read(cx);
                let target = workbench
                    .tabs
                    .iter()
                    .position(|tab| Some(tab.entity_id()) == target_id);
                let count = workbench.tabs.len();
                for (action, label) in [
                    (TabCloseAction::Current, tr("关闭当前")),
                    (TabCloseAction::Others, tr("关闭其他")),
                    (TabCloseAction::Left, tr("关闭左侧")),
                    (TabCloseAction::Right, tr("关闭右侧")),
                    (TabCloseAction::All, tr("关闭全部")),
                ] {
                    if matches!(action, TabCloseAction::All) {
                        menu = menu.separator();
                    }
                    let handle = menu_handle.clone();
                    menu = menu.item(
                        PopupMenuItem::new(label)
                            .disabled(action.indices(target, count).is_empty())
                            .on_click(move |_, window, cx| {
                                let _ = handle.update(cx, |this, cx| {
                                    let target = this
                                        .tabs
                                        .iter()
                                        .position(|tab| Some(tab.entity_id()) == target_id);
                                    this.close_tabs(target, action, window, cx);
                                });
                            }),
                    );
                }
                menu
            });
        let commands = div()
            .flex()
            .flex_shrink_0()
            .h_full()
            .border_b_1()
            .border_color(cx.theme().border)
            .items_center()
            .px_3()
            .py_1()
            .gap_2()
            .child(
                Button::new("export")
                    .icon(AppIcon::Code)
                    .dropdown_caret(true)
                    .ghost()
                    .small()
                    .label(tr("生成代码"))
                    .disabled(!request)
                    .dropdown_menu(move |menu, _, _| {
                        let mut menu = menu;
                        for lang in ["curl", "javascript", "python", "go"] {
                            let handle = export_weak.clone();
                            menu = menu.item(
                                PopupMenuItem::new(lang).icon(AppIcon::FileCode).on_click(
                                    move |_, window, cx| {
                                        let _ = handle
                                            .update(cx, |this, cx| this.export(lang, window, cx));
                                    },
                                ),
                            );
                        }
                        menu
                    }),
            )
            .child(
                Button::new("more")
                    .icon(AppIcon::Ellipsis)
                    .small()
                    .ghost()
                    .label(tr("更多"))
                    .disabled(active.is_none())
                    .dropdown_menu(move |menu, _, _| {
                        let a = more_weak.clone();
                        let b = more_weak.clone();
                        let c = more_weak.clone();
                        let d = more_weak.clone();
                        let e = more_weak.clone();
                        let import = more_weak.clone();
                        let bru = more_weak.clone();
                        menu.item(
                            PopupMenuItem::new(tr("复制为 .bru"))
                                .icon(AppIcon::Copy)
                                .on_click(move |_, window, cx| {
                                    let _ = bru.update(cx, |this, cx| this.export_bru(window, cx));
                                }),
                        )
                        .item(
                            PopupMenuItem::new(tr("从地址栏导入 cURL"))
                                .icon(AppIcon::FileInput)
                                .on_click(move |_, window, cx| {
                                    let _ = import.update(cx, |this, cx| {
                                        if let Some(tab) = this.active() {
                                            match tab.update(cx, |t, cx| t.import_curl(window, cx))
                                            {
                                                Ok(()) => this.notify_success(
                                                    tr("已导入 cURL 请求").into(),
                                                    window,
                                                    cx,
                                                ),
                                                Err(e) => {
                                                    this.notify_error(e.to_string(), window, cx)
                                                }
                                            }
                                        }
                                    });
                                }),
                        )
                        .item(
                            PopupMenuItem::new(tr("创建副本"))
                                .icon(AppIcon::Copy)
                                .on_click(move |_, window, cx| {
                                    let _ = a.update(cx, |this, cx| this.duplicate(window, cx));
                                }),
                        )
                        .item(
                            PopupMenuItem::new(tr("移动或重命名…"))
                                .icon(AppIcon::Pencil)
                                .on_click(move |_, window, cx| {
                                    let _ = b.update(cx, |this, cx| this.rename(window, cx));
                                }),
                        )
                        .item(
                            PopupMenuItem::new(tr("重新载入"))
                                .icon(AppIcon::RefreshCw)
                                .on_click(move |_, window, cx| {
                                    let _ = c.update(cx, |this, cx| this.reload_tab(window, cx));
                                }),
                        )
                        .item(
                            PopupMenuItem::new(tr("移到废纸篓"))
                                .icon(AppIcon::Trash)
                                .on_click(move |_, window, cx| {
                                    let _ = d.update(cx, |this, cx| this.trash(window, cx));
                                }),
                        )
                        .item(
                            PopupMenuItem::new(tr("向左移动标签"))
                                .icon(AppIcon::ArrowLeft)
                                .on_click(move |_, _, cx| {
                                    let _ = e.update(cx, |this, cx| {
                                        if this.active > 0 {
                                            this.tabs.swap(this.active, this.active - 1);
                                            this.active -= 1;
                                            this.tab_scroll.scroll_to_item(this.active);
                                            cx.notify();
                                        }
                                    });
                                }),
                        )
                    }),
            );
        let content = div()
            .flex()
            .flex_col()
            .size_full()
            .min_w_0()
            .min_h_0()
            .child(
                div()
                    .flex()
                    .items_center()
                    .flex_shrink_0()
                    .h_8()
                    .bg(cx.theme().tab_bar)
                    .child(div().flex_1().min_w_0().overflow_hidden().child(tabs))
                    .child(commands),
            )
            .when_some(active, |d, tab| {
                d.child(div().flex_1().min_h_0().child(tab))
            })
            .when(self.tabs.is_empty(), |d| {
                d.child(
                    div()
                        .flex_1()
                        .flex()
                        .flex_col()
                        .justify_center()
                        .items_center()
                        .gap_3()
                        .child(tr("选择请求，或创建新的 HTTP / WebSocket 请求"))
                        .child(tr("⌘/Ctrl+O 打开项目 · ⌘/Ctrl+S 保存 · ⌘/Ctrl+Enter 发送")),
                )
            });
        let rail = div()
            .flex()
            .flex_col()
            .items_center()
            .w_16()
            .flex_shrink_0()
            .h_full()
            .py_3()
            .gap_2()
            .bg(cx.theme().sidebar)
            .border_r_1()
            .border_color(cx.theme().border)
            .child(
                Button::new("nav-collections")
                    .ghost()
                    .selected(!self.show_settings && self.sidebar_view == SidebarView::Collections)
                    .w_12()
                    .h_12()
                    .accessibility_label(tr("集合"))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .items_center()
                            .gap_1()
                            .child(Icon::new(IconName::BookOpen))
                            .child(div().text_xs().child(tr("集合"))),
                    )
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.show_settings = false;
                        this.sidebar_view = SidebarView::Collections;
                        this.rebuild_tree(cx);
                        cx.notify();
                    })),
            )
            .child(
                Button::new("nav-environments")
                    .ghost()
                    .selected(!self.show_settings && self.sidebar_view == SidebarView::Environments)
                    .w_12()
                    .h_12()
                    .accessibility_label(tr("环境"))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .items_center()
                            .gap_1()
                            .child(Icon::new(AppIcon::Globe))
                            .child(div().text_xs().child(tr("环境"))),
                    )
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.show_settings = false;
                        this.sidebar_view = SidebarView::Environments;
                        cx.notify();
                    })),
            )
            .child(div().flex_1())
            .when_some(self.updates.read(cx).sidebar_label(), |rail, label| {
                rail.child(
                    Button::new("nav-update")
                        .small()
                        .primary()
                        .label(label)
                        .tooltip(tr("软件更新"))
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.updates
                                .update(cx, |updates, cx| updates.activate(window, cx));
                        })),
                )
            })
            .child(
                Button::new("nav-settings")
                    .ghost()
                    .selected(self.show_settings)
                    .w_12()
                    .h_12()
                    .accessibility_label(tr("设置"))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .items_center()
                            .gap_1()
                            .child(Icon::new(AppIcon::Settings))
                            .child(div().text_xs().child(tr("设置"))),
                    )
                    .on_click(cx.listener(|this, _, window, cx| this.settings(window, cx))),
            );
        div()
            .key_context("Workbench")
            .track_focus(&self.focus)
            .flex()
            .flex_col()
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .text_sm()
            .on_action(cx.listener(|this, _: &OpenProject, window, cx| {
                this.choose_project(false, window, cx)
            }))
            .on_action(
                cx.listener(|this, _: &NewProject, window, cx| {
                    this.choose_project(true, window, cx)
                }),
            )
            .on_action(cx.listener(|this, _: &Save, window, cx| {
                if this.show_settings {
                    if let Some(view) = &this.settings_view {
                        view.update(cx, |view, cx| view.save(window, cx));
                    }
                } else {
                    this.save(window, cx);
                }
            }))
            .on_action(cx.listener(|this, _: &Send, window, cx| {
                if !this.show_settings {
                    this.send(window, cx);
                }
            }))
            .on_action(cx.listener(|this, _: &CloseTab, window, cx| {
                if this.show_settings {
                    this.show_settings = false;
                    cx.notify();
                } else {
                    this.close_tab(window, cx);
                }
            }))
            .on_action(
                cx.listener(|this, _: &NewRequest, window, cx| {
                    this.new_document("http", window, cx)
                }),
            )
            .on_action(cx.listener(|this, _: &Refresh, window, cx| this.refresh(window, cx)))
            .on_action(cx.listener(|this, _: &Settings, window, cx| this.settings(window, cx)))
            .on_action(cx.listener(|this, _: &Quit, window, cx| {
                if let Err(e) = this.persist(cx) {
                    this.notify_error(e.to_string(), window, cx);
                    cx.notify();
                } else {
                    cx.quit();
                }
            }))
            .child(toolbar)
            .child(
                div()
                    .flex()
                    .items_stretch()
                    .flex_1()
                    .min_h_0()
                    .child(rail)
                    .child(if self.show_settings {
                        div()
                            .flex_1()
                            .min_w_0()
                            .min_h_0()
                            .children(self.settings_view.clone())
                            .into_any_element()
                    } else {
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(
                                h_resizable("workbench")
                                    .child(
                                        resizable_panel()
                                            .size(rems(16.).to_pixels(cx.theme().font_size))
                                            .size_range(
                                                rems(12.).to_pixels(cx.theme().font_size)
                                                    ..rems(24.).to_pixels(cx.theme().font_size),
                                            )
                                            .child(sidebar),
                                    )
                                    .child(resizable_panel().child(content)),
                            )
                            .into_any_element()
                    }),
            )
    }
}

#[derive(Clone, Copy)]
enum TabCloseAction {
    Current,
    Others,
    Left,
    Right,
    All,
}

impl TabCloseAction {
    fn indices(self, target: Option<usize>, count: usize) -> Vec<usize> {
        if matches!(self, Self::All) {
            return (0..count).collect();
        }
        let Some(target) = target.filter(|index| *index < count) else {
            return Vec::new();
        };
        (0..count)
            .filter(|&index| match self {
                Self::Current => index == target,
                Self::Others => index != target,
                Self::Left => index < target,
                Self::Right => index > target,
                Self::All => true,
            })
            .collect()
    }
}

// Closing a background tab must keep the current document selected.
fn active_after_close(active: usize, closed: usize, count: usize) -> usize {
    if closed < active {
        active - 1
    } else {
        active.min(count.saturating_sub(2))
    }
}

#[cfg(test)]
mod tests {
    use super::{TabCloseAction, active_after_close};

    #[test]
    fn context_menu_targets_the_clicked_tab_and_respects_boundaries() {
        use TabCloseAction::*;
        assert_eq!(Current.indices(Some(2), 5), vec![2]);
        assert_eq!(Others.indices(Some(2), 5), vec![0, 1, 3, 4]);
        assert_eq!(Left.indices(Some(2), 5), vec![0, 1]);
        assert_eq!(Right.indices(Some(2), 5), vec![3, 4]);
        assert_eq!(All.indices(Some(2), 5), vec![0, 1, 2, 3, 4]);
        assert!(Left.indices(Some(0), 5).is_empty());
        assert!(Right.indices(Some(4), 5).is_empty());
        assert!(Others.indices(Some(0), 1).is_empty());
        for action in [Current, Others, Left, Right] {
            assert!(action.indices(None, 5).is_empty());
            assert!(action.indices(Some(5), 5).is_empty());
        }
        assert!(All.indices(None, 0).is_empty());
    }

    #[test]
    fn batch_close_preserves_surviving_active_tabs_and_handles_empty_list() {
        use TabCloseAction::*;
        for (action, target, active, expected_tabs, expected_active) in [
            (Current, 1, 3, vec![0, 2, 3, 4], 2),
            (Others, 2, 4, vec![2], 0),
            (Left, 2, 4, vec![2, 3, 4], 2),
            (Right, 2, 1, vec![0, 1, 2], 1),
            (Right, 2, 4, vec![0, 1, 2], 2),
            (All, 2, 3, vec![], 0),
        ] {
            let mut tabs: Vec<_> = (0..5).collect();
            let mut active = active;
            for index in action.indices(Some(target), tabs.len()).into_iter().rev() {
                active = active_after_close(active, index, tabs.len());
                tabs.remove(index);
            }
            assert_eq!(tabs, expected_tabs);
            assert_eq!(active, expected_active);
        }
    }

    #[test]
    fn closing_tabs_preserves_selection_or_selects_a_neighbor() {
        assert_eq!(active_after_close(2, 0, 4), 1);
        assert_eq!(active_after_close(1, 3, 4), 1);
        assert_eq!(active_after_close(1, 1, 4), 1);
        assert_eq!(active_after_close(3, 3, 4), 2);
        assert_eq!(active_after_close(0, 0, 1), 0);
    }
}

fn environment_rows(collection: Option<&Collection>) -> Vec<(PathBuf, String)> {
    let mut rows = collection
        .map(|collection| {
            collection
                .documents()
                .iter()
                .filter(|doc| doc.kind() == apifuck_core::DocumentKind::Environment)
                .map(|doc| (doc.path().to_owned(), doc.name().to_owned()))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    rows.sort_by(|a, b| {
        a.1.to_lowercase()
            .cmp(&b.1.to_lowercase())
            .then(a.0.cmp(&b.0))
    });
    rows
}

// Tree IDs may point at folder metadata files or virtual folders. Follow the actual
// tree ancestry instead of guessing IDs from path strings.
fn navigation_ancestors<'a>(
    roots: impl IntoIterator<Item = &'a TreeItem>,
    target: &SharedString,
) -> Vec<String> {
    roots
        .into_iter()
        .find_map(|root| root.ancestors(target))
        .unwrap_or_default()
        .into_iter()
        .map(|item| item.id.to_string())
        .collect()
}

#[cfg(test)]
mod navigation_tests {
    use super::navigation_ancestors;
    use gpui_kit::component::tree::TreeItem;

    #[test]
    fn reveals_nested_hidden_parents_without_opening_unrelated_folders() {
        let roots = vec![
            TreeItem::new("api/folder.yml", "API")
                .expanded(false)
                .child(
                    TreeItem::new("folder:api/users", "Users")
                        .expanded(false)
                        .child(TreeItem::new("api/users/get.yml", "Get user")),
                ),
            TreeItem::new("other/folder.yml", "Other").expanded(false),
        ];
        let ancestors = navigation_ancestors(&roots, &"api/users/get.yml".into());
        assert_eq!(ancestors, ["folder:api/users", "api/folder.yml"]);
        let mut collapsed = ["api/folder.yml", "folder:api/users", "other/folder.yml"]
            .into_iter()
            .map(String::from)
            .collect::<std::collections::BTreeSet<_>>();
        for ancestor in ancestors {
            collapsed.remove(&ancestor);
        }
        assert_eq!(
            collapsed.into_iter().collect::<Vec<_>>(),
            ["other/folder.yml"]
        );
    }

    #[test]
    fn folder_tab_reveals_its_parents_and_missing_targets_have_no_ancestors() {
        let roots = vec![
            TreeItem::new("api/folder.yml", "API")
                .expanded(false)
                .child(TreeItem::new("api/users/folder.yml", "Users").expanded(false)),
        ];
        assert_eq!(
            navigation_ancestors(&roots, &"api/users/folder.yml".into()),
            ["api/folder.yml"]
        );
        assert!(navigation_ancestors(&roots, &"filtered-out.yml".into()).is_empty());
    }
}
