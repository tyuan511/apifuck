use crate::i18n::tr;
mod appearance;
mod brand;
mod document_tab;
mod editor_tools;
mod export_view;
mod feedback;
mod i18n;
mod icons;
mod key_value_editor;
mod language_server;
mod languages;
mod mcp_view;
mod network_editor;
mod request_form;
mod response_view;
mod script_completion;
mod settings_view;
mod surfaces;
mod syntax;
mod tab_drag;
mod update_view;
mod variable_input;
mod workbench;
use gpui_kit::*;
use std::{path::PathBuf, sync::Arc};
use workbench::*;
fn main() {
    let runtime = Arc::new(
        tokio::runtime::Runtime::new()
            .unwrap_or_else(|e| panic!("{}: {e}", tr("无法启动网络运行时"))),
    );
    let path = std::env::args_os().nth(1).map(PathBuf::from).or_else(|| {
        apifuck_core::settings::app_state_path()
            .ok()
            .and_then(|path| apifuck_core::settings::RecentProjects::load_from(&path).ok())
            .and_then(|recent| recent.last_path().map(std::path::Path::to_path_buf))
    });
    gpui_kit::application()
        .with_assets(brand::BrandAssets)
        .run(move |cx| {
            gpui_kit::init(cx);
            syntax::init();
            gpui_kit::component::set_locale("zh-CN");
            cx.on_window_closed(|cx, _| {
                if cx.windows().is_empty() {
                    cx.quit();
                }
            })
            .detach();
            let key = if cfg!(target_os = "macos") {
                "cmd"
            } else {
                "ctrl"
            };
            cx.bind_keys([
                KeyBinding::new(
                    "ctrl-space",
                    gpui_kit::component::input::ShowCompletions,
                    Some("Input && mode == editor"),
                ),
                KeyBinding::new(&format!("{key}-o"), OpenProject, Some("Workbench")),
                KeyBinding::new(&format!("{key}-s"), Save, Some("Workbench")),
                KeyBinding::new(&format!("{key}-enter"), Send, Some("Workbench")),
                KeyBinding::new(&format!("{key}-w"), CloseTab, Some("Workbench")),
                KeyBinding::new(&format!("{key}-n"), NewRequest, Some("Workbench")),
                KeyBinding::new(&format!("{key}-q"), Quit, Some("Workbench")),
            ]);
            refresh_menus(cx);
            // Physical pixel values only at the platform window boundary.
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                    None,
                    size(px(1280.), px(840.)),
                    cx,
                ))),
                window_min_size: Some(size(px(900.), px(620.))),
                titlebar: Some(TitlebarOptions {
                    title: Some("ApiFuck".into()),
                    appears_transparent: cfg!(target_os = "macos"),
                    traffic_light_position: cfg!(target_os = "macos")
                        .then(|| point(px(16.), px(17.))),
                }),
                #[cfg(target_os = "macos")]
                app_owns_titlebar_drag: true,
                ..Default::default()
            };
            gpui_kit::open_window(options, cx, move |window, cx| {
                cx.new(|cx| Workbench::new(path, runtime, window, cx))
            })
            .unwrap_or_else(|e| panic!("{}: {e}", tr("无法创建窗口")));
            cx.activate(true);
        });
}

pub(crate) fn refresh_menus(cx: &App) {
    cx.set_menus(vec![
        Menu {
            disabled: false,
            name: "ApiFuck".into(),
            items: vec![
                MenuItem::action(tr("设置…"), Settings),
                MenuItem::separator(),
                MenuItem::action(tr("退出"), Quit),
            ],
        },
        Menu {
            disabled: false,
            name: tr("文件").into(),
            items: vec![
                MenuItem::action(tr("打开项目…"), OpenProject),
                MenuItem::action(tr("新建项目…"), NewProject),
                MenuItem::action(tr("新建请求…"), NewRequest),
                MenuItem::action(tr("保存"), Save),
                MenuItem::action(tr("关闭标签"), CloseTab),
            ],
        },
        Menu {
            disabled: false,
            name: tr("请求").into(),
            items: vec![
                MenuItem::action(tr("发送 / 停止"), Send),
                MenuItem::action(tr("刷新项目"), Refresh),
            ],
        },
    ]);
}
