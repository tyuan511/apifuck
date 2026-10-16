use crate::{i18n::tr, icons::AppIcon, surfaces::panel_editor};
use anyhow::{Result, ensure};
use apifuck_core::{
    Collection, NetworkSettings,
    runner::{self, RunEntry, RunOptions},
};
use gpui_kit::component::{
    ActiveTheme, Disableable,
    button::{Button, ButtonVariants},
    input::{EditorState, Input, InputState},
    menu::{DropdownMenu, PopupMenuItem},
    switch::Switch,
};
use gpui_kit::{prelude::FluentBuilder, *};
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

pub enum RunnerEvent {
    Start,
}
impl EventEmitter<RunnerEvent> for RunnerView {}

pub struct RunnerView {
    pub folder: Option<PathBuf>,
    environment: Option<String>,
    environments: Vec<String>,
    iterations: Entity<InputState>,
    delay: Entity<InputState>,
    stop_on_failure: bool,
    running: bool,
    cancel: Arc<AtomicBool>,
    entries: Vec<RunEntry>,
    root: PathBuf,
    data: Vec<serde_json::Value>,
    data_path: Option<PathBuf>,
    loading_data: bool,
    report: Option<Arc<runner::RunReport>>,
    total: usize,
    selected: Option<usize>,
    status: String,
    details: Entity<EditorState>,
    task: Option<Task<()>>,
}
impl RunnerView {
    pub fn new(
        collection: &Collection,
        environment: Option<String>,
        folder: Option<PathBuf>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        Self {
            folder,
            environment,
            environments: collection
                .documents()
                .iter()
                .filter(|d| d.kind() == apifuck_core::DocumentKind::Environment)
                .map(|d| d.name().to_owned())
                .collect(),
            iterations: cx.new(|cx| InputState::new(window, cx).default_value("1")),
            delay: cx.new(|cx| InputState::new(window, cx).default_value("0")),
            stop_on_failure: false,
            running: false,
            cancel: Arc::new(AtomicBool::new(false)),
            entries: vec![],
            root: collection.root().to_owned(),
            data: vec![],
            data_path: None,
            loading_data: false,
            report: None,
            total: 0,
            selected: None,
            status: String::new(),
            details: cx.new(|cx| EditorState::new(window, cx).line_number(false)),
            task: None,
        }
    }
    pub fn is_running(&self) -> bool {
        self.running
    }
    pub fn stop(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
    pub fn start(
        &mut self,
        collection: Collection,
        network: NetworkSettings,
        runtime: &Arc<tokio::runtime::Runtime>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<()> {
        ensure!(!self.running, "{}", tr("集合正在运行"));
        let mut options = parse_options(
            self.environment.clone(),
            self.folder.clone(),
            self.iterations.read(cx).value().as_str(),
            self.delay.read(cx).value().as_str(),
            self.stop_on_failure,
        )?;
        ensure!(!self.loading_data, "{}", tr("正在加载数据文件"));
        options.data = self.data.clone();
        let count = runner::ordered_requests(&collection, options.folder.as_deref()).len();
        ensure!(count > 0, "{}", tr("没有可运行的请求"));
        self.total = count * options.iterations as usize * options.data.len().max(1);
        ensure!(self.total <= 10000, "{}", tr("总执行次数不能超过 10000"));
        self.report = None;
        self.entries.clear();
        self.selected = None;
        self.details
            .update(cx, |editor, cx| editor.set_value("", window, cx));
        self.cancel = Arc::new(AtomicBool::new(false));
        self.running = true;
        self.status = tr("运行中…").into();
        let cancel = self.cancel.clone();
        let (events, mut incoming) = tokio::sync::mpsc::unbounded_channel();
        let job = runtime.spawn(async move {
            runner::run(collection, options, network, cancel, Some(events)).await
        });
        self.task = Some(cx.spawn_in(window, async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(50))
                    .await;
                let batch: Vec<_> = std::iter::from_fn(|| incoming.try_recv().ok()).collect();
                if this
                    .update(cx, |this, cx| {
                        this.entries.extend(batch);
                        cx.notify();
                    })
                    .is_err()
                {
                    break;
                }
                if job.is_finished() {
                    let result = job.await;
                    let _ = this.update(cx, |this, cx| {
                        this.running = false;
                        match result {
                            Ok(Ok(report)) => {
                                this.status = format!(
                                    "{} · {} ms",
                                    tr(if report.cancelled {
                                        "已取消"
                                    } else if report.passed() {
                                        "运行通过"
                                    } else {
                                        "运行失败"
                                    }),
                                    report.elapsed_ms
                                );
                                this.entries.clear();
                                this.report = Some(Arc::new(report));
                            }
                            Ok(Err(error)) => this.status = format!("{error:#}"),
                            Err(error) => this.status = error.to_string(),
                        }
                        cx.notify();
                    });
                    break;
                }
            }
        }));
        cx.notify();
        Ok(())
    }
    fn results(&self) -> &[RunEntry] {
        self.report
            .as_ref()
            .map(|report| report.entries.as_slice())
            .unwrap_or(&self.entries)
    }
    fn choose_data(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.running || self.loading_data {
            return;
        }
        let choice = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some(tr("选择 CSV / JSON 数据文件").into()),
        });
        self.loading_data = true;
        cx.notify();
        cx.spawn_in(window, async move |this, cx| {
            let result = match choice.await {
                Ok(Ok(Some(paths))) if !paths.is_empty() => {
                    let path = paths[0].clone();
                    let load = path.clone();
                    let task = cx.background_spawn(async move { runner::data::read(&load) });
                    task.await.map(|data| Some((path, data)))
                }
                Ok(Err(error)) => Err(error),
                _ => Ok(None),
            };
            let _ = this.update_in(cx, |this, window, cx| {
                this.loading_data = false;
                match result {
                    Ok(Some((path, data))) => {
                        this.data_path = Some(path);
                        this.data = data;
                    }
                    Ok(None) => {}
                    Err(error) => crate::feedback::show(
                        gpui_kit::component::notification::Notification::error(format!(
                            "{error:#}"
                        )),
                        window,
                        cx,
                    ),
                }
                cx.notify();
            });
        })
        .detach();
    }
    fn export_report(
        &mut self,
        format: runner::report::Format,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(report) = self.report.clone() else {
            return;
        };
        let choice = cx.prompt_for_new_path(
            &self.root,
            Some(&format!("run-report.{}", format.extension())),
        );
        cx.spawn_in(window, async move |this, cx| {
            let Ok(Ok(Some(path))) = choice.await else {
                return;
            };
            let task = cx.background_spawn(async move {
                apifuck_core::collection::atomic_write(
                    &path,
                    &runner::report::render(&report, format)?,
                )
            });
            let result = task.await;
            let _ = this.update_in(cx, |_, window, cx| {
                crate::feedback::show(
                    match result {
                        Ok(()) => gpui_kit::component::notification::Notification::success(tr(
                            "运行报告已导出",
                        )),
                        Err(error) => gpui_kit::component::notification::Notification::error(
                            format!("{error:#}"),
                        ),
                    },
                    window,
                    cx,
                )
            });
        })
        .detach();
    }
    fn select(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.selected = Some(index);
        let entry = &self.results()[index];
        let mut text = format!(
            "{}\n{}\n{}: {}\n",
            entry.name,
            entry.path.display(),
            tr("数据行"),
            entry
                .data_row
                .map(|row| row.to_string())
                .unwrap_or_else(|| "-".into())
        );
        if entry.skipped {
            text.push_str(tr("WebSocket 在集合运行中跳过"));
        }
        if let Some(error) = &entry.error {
            text.push_str(error);
        }
        if let Some(response) = &entry.response {
            text.push_str(&format!(
                "HTTP {} · {} ms\n",
                response.status, response.elapsed_ms
            ));
            for test in &response.tests {
                text.push_str(&format!(
                    "\n{} {}",
                    if test.passed { "✓" } else { "✗" },
                    test.name
                ));
            }
            for test in &response.tests {
                if let Some(error) = &test.error {
                    text.push_str(&format!("\n{error}"));
                }
            }
            text.push_str(&format!("\n\n{}", response.body));
            for log in &response.logs {
                text.push_str(&format!("\n{log}"));
            }
        }
        self.details
            .update(cx, |editor, cx| editor.set_value(text, window, cx));
        cx.notify();
    }
}
impl Drop for RunnerView {
    fn drop(&mut self) {
        self.stop();
    }
}
fn parse_options(
    environment: Option<String>,
    folder: Option<PathBuf>,
    iterations: &str,
    delay: &str,
    stop: bool,
) -> Result<RunOptions> {
    let iterations = iterations
        .trim()
        .parse::<u32>()
        .ok()
        .filter(|n| (1..=10000).contains(n));
    let delay = delay.trim().parse::<u64>().ok().filter(|n| *n <= 3_600_000);
    ensure!(
        iterations.is_some() && delay.is_some(),
        "{}",
        tr("次数须为 1–10000，延迟须为 0–3600000 毫秒")
    );
    Ok(RunOptions::new(
        environment,
        folder,
        iterations.unwrap(),
        delay.unwrap(),
        stop,
    ))
}
impl Render for RunnerView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let weak = cx.entity().downgrade();
        let environments = self.environments.clone();
        div()
            .flex()
            .flex_col()
            .size_full()
            .gap_3()
            .p_4()
            .child(div().text_lg().child(tr("集合运行器")))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .flex_wrap()
                    .child(
                        Button::new("runner-data")
                            .label(tr("加载 CSV / JSON"))
                            .disabled(self.running || self.loading_data)
                            .on_click(
                                cx.listener(|this, _, window, cx| this.choose_data(window, cx)),
                            ),
                    )
                    .when(self.data_path.is_some(), |row| {
                        row.child(
                            Button::new("runner-data-clear")
                                .ghost()
                                .label(tr("移除数据文件"))
                                .disabled(self.running || self.loading_data)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.data.clear();
                                    this.data_path = None;
                                    cx.notify();
                                })),
                        )
                    })
                    .child(
                        div().flex_1().min_w_0().truncate().child(
                            self.data_path
                                .as_ref()
                                .map(|path| {
                                    format!(
                                        "{} · {} {}",
                                        path.file_name().unwrap_or_default().to_string_lossy(),
                                        self.data.len(),
                                        tr("行")
                                    )
                                })
                                .unwrap_or_else(|| tr("未加载数据文件").into()),
                        ),
                    )
                    .children(
                        [
                            (runner::report::Format::Json, "JSON"),
                            (runner::report::Format::Junit, "JUnit"),
                            (runner::report::Format::Html, "HTML"),
                        ]
                        .into_iter()
                        .map(|(format, label)| {
                            Button::new(SharedString::from(format!("report-{label}")))
                                .label(format!("{} {label}", tr("导出")))
                                .disabled(self.running || self.report.is_none())
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.export_report(format, window, cx)
                                }))
                        }),
                    ),
            )
            .child(div().text_color(cx.theme().muted_foreground).child(format!(
                    "{} · {}",
                    self.folder
                        .as_ref()
                        .map(|p| p.display().to_string())
                        .unwrap_or_else(|| tr("整个集合").into()),
                    tr("仅执行已保存文档；WebSocket 将跳过")
                )))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .flex_wrap()
                    .child(
                        Button::new("runner-environment")
                            .label(
                                self.environment
                                    .clone()
                                    .unwrap_or_else(|| tr("未选择环境").into()),
                            )
                            .disabled(self.running)
                            .dropdown_menu(move |mut menu, _, _| {
                                for environment in std::iter::once(None)
                                    .chain(environments.iter().cloned().map(Some))
                                {
                                    let weak = weak.clone();
                                    menu = menu.item(
                                        PopupMenuItem::new(
                                            environment
                                                .clone()
                                                .unwrap_or_else(|| tr("未选择环境").into()),
                                        )
                                        .on_click(
                                            move |_, _, cx| {
                                                let _ = weak.update(cx, |this, cx| {
                                                    this.environment = environment.clone();
                                                    cx.notify();
                                                });
                                            },
                                        ),
                                    );
                                }
                                menu
                            }),
                    )
                    .child(
                        div()
                            .w_24()
                            .child(tr("迭代次数"))
                            .child(Input::new(&self.iterations).disabled(self.running)),
                    )
                    .child(
                        div()
                            .w_32()
                            .child(tr("延迟（毫秒）"))
                            .child(Input::new(&self.delay).disabled(self.running)),
                    )
                    .child(
                        Switch::new("runner-stop-on-failure")
                            .label(tr("失败即停止"))
                            .checked(self.stop_on_failure)
                            .disabled(self.running)
                            .on_change(cx.listener(|this, value, _, cx| {
                                this.stop_on_failure = *value;
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("runner-start")
                            .disabled(self.loading_data)
                            .primary()
                            .icon(AppIcon::Play)
                            .label(tr(if self.running { "停止" } else { "运行" }))
                            .on_click(cx.listener(|this, _, _, cx| {
                                if this.running {
                                    this.stop();
                                } else {
                                    cx.emit(RunnerEvent::Start);
                                }
                            })),
                    ),
            )
            .child(div().child(format!(
                "{} · {}/{}",
                self.status,
                self.results().len(),
                self.total
            )))
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h_0()
                    .gap_3()
                    .child(
                        div()
                            .id("runner-results")
                            .w_80()
                            .h_full()
                            .overflow_y_scroll()
                            .children(self.results().iter().enumerate().map(|(index, entry)| {
                                div()
                                    .id(("run-entry", index))
                                    .p_2()
                                    .border_b_1()
                                    .border_color(cx.theme().border)
                                    .cursor_pointer()
                                    .when(self.selected == Some(index), |row| {
                                        row.bg(cx.theme().secondary)
                                    })
                                    .child(format!(
                                        "{} · {} · {}",
                                        entry.iteration,
                                        tr(if entry.skipped {
                                            "已跳过"
                                        } else if entry.passed {
                                            "通过"
                                        } else {
                                            "失败"
                                        }),
                                        entry.name
                                    ))
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(cx.theme().muted_foreground)
                                            .child(
                                                entry
                                                    .response
                                                    .as_ref()
                                                    .map(|r| {
                                                        format!(
                                                            "HTTP {} · {} ms",
                                                            r.status, r.elapsed_ms
                                                        )
                                                    })
                                                    .unwrap_or_default(),
                                            ),
                                    )
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        this.select(index, window, cx)
                                    }))
                            })),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(panel_editor(&self.details).readonly(true)),
                    ),
            )
    }
}
#[cfg(test)]
mod tests {
    use super::parse_options;
    #[test]
    fn validates_run_limits_without_silently_replacing_invalid_input() {
        for (iterations, delay) in [
            ("0", "0"),
            ("10001", "0"),
            ("abc", "0"),
            ("1", "-1"),
            ("1", "3600001"),
        ] {
            assert!(parse_options(None, None, iterations, delay, false).is_err());
        }
        let options =
            parse_options(Some("dev".into()), Some("users".into()), " 2 ", "25", true).unwrap();
        assert_eq!(options.iterations, 2);
        assert_eq!(options.delay_ms, 25);
        assert!(options.stop_on_failure);
        assert_eq!(options.environment.as_deref(), Some("dev"));
    }
}
