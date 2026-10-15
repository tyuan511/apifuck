//! One update state shared by the sidebar and the manual update panel.
use crate::{feedback, i18n::tr, icons::AppIcon};
use apifuck_core::update::{self, DownloadProgress, DownloadedUpdate, Release};
use gpui_kit::component::{
    ActiveTheme, WindowExt,
    button::{Button, ButtonVariants},
    notification::Notification,
    progress::Progress,
    spinner::Spinner,
    text::TextView,
};
use gpui_kit::*;
use std::{
    sync::Arc,
    time::{Duration, Instant},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    Idle,
    Checking,
    Current,
    Available,
    Downloading,
    Downloaded,
    Installing,
    Installed,
    Error,
}
impl Phase {
    fn busy(self) -> bool {
        matches!(self, Self::Checking | Self::Downloading | Self::Installing)
    }
    fn holds_download(self) -> bool {
        matches!(self, Self::Downloaded | Self::Installing | Self::Installed)
    }
    fn can_check(self, has_download: bool) -> bool {
        !self.busy() && !self.holds_download() && !has_download
    }
}
pub enum UpdateEvent {
    InstallRequested,
    Installed,
}
pub struct AppUpdate {
    runtime: Arc<tokio::runtime::Runtime>,
    phase: Phase,
    release: Option<Release>,
    downloaded: Option<Arc<DownloadedUpdate>>,
    progress: Option<DownloadProgress>,
    checked_at: Option<Instant>,
    auto_check: bool,
    schedule: Option<Task<()>>,
}
impl EventEmitter<UpdateEvent> for AppUpdate {}
impl AppUpdate {
    pub fn new(runtime: Arc<tokio::runtime::Runtime>) -> Self {
        Self {
            runtime,
            phase: Phase::Idle,
            release: None,
            downloaded: None,
            progress: None,
            checked_at: None,
            auto_check: false,
            schedule: None,
        }
    }
    pub fn set_auto_check(&mut self, enabled: bool, window: &mut Window, cx: &mut Context<Self>) {
        if enabled == self.auto_check {
            return;
        }
        self.auto_check = enabled;
        self.schedule = None;
        if !enabled || cfg!(debug_assertions) {
            return;
        }
        self.schedule = Some(cx.spawn_in(window, async move |this, cx| {
            cx.background_executor().timer(Duration::from_secs(3)).await;
            loop {
                if this
                    .update_in(cx, |this, window, cx| this.check(false, None, window, cx))
                    .is_err()
                {
                    break;
                }
                cx.background_executor()
                    .timer(Duration::from_secs(600))
                    .await;
            }
        }));
    }
    pub fn open(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if matches!(self.phase, Phase::Idle | Phase::Current | Phase::Error)
            && self.downloaded.is_none()
        {
            self.check(true, None, window, cx);
        }
        self.show_panel(window, cx);
    }
    fn show_panel(&self, window: &mut Window, cx: &mut Context<Self>) {
        let view = cx.entity();
        window.open_dialog(cx, move |dialog, _, _| {
            dialog
                .title(tr("软件更新"))
                .width(px(520.))
                .child(view.clone())
        });
    }
    fn fail(&mut self, error: impl std::fmt::Display, window: &mut Window, cx: &mut Context<Self>) {
        self.phase = Phase::Error;
        self.progress = None;
        feedback::show(Notification::error(error.to_string()), window, cx);
        cx.notify();
    }
    fn check(
        &mut self,
        manual: bool,
        resume: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.phase.can_check(self.downloaded.is_some()) {
            return;
        }
        let previous = self.phase;
        self.phase = Phase::Checking;
        cx.notify();
        let task = self.runtime.spawn(update::check());
        cx.spawn_in(window, async move |this, cx| {
            let result = task.await;
            let _ = this.update_in(cx, |this, window, cx| {
                match result {
                    Ok(Ok(release)) => {
                        this.checked_at = Some(Instant::now());
                        this.phase = if release.is_some() {
                            Phase::Available
                        } else {
                            Phase::Current
                        };
                        this.release = release;
                        if let Some(expected) = resume {
                            if resume_download(
                                &expected,
                                this.release.as_ref().map(Release::version),
                            ) {
                                this.download(window, cx);
                            } else {
                                this.show_panel(window, cx);
                            }
                        }
                    }
                    Ok(Err(error)) if manual => this.fail(error, window, cx),
                    Err(error) if manual => this.fail(error, window, cx),
                    _ => this.phase = previous,
                }
                cx.notify();
            });
        })
        .detach();
    }
    fn download(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.phase.can_check(self.downloaded.is_some()) {
            return;
        }
        let Some(release) = self.release.clone() else {
            self.check(true, None, window, cx);
            return;
        };
        if self
            .checked_at
            .is_none_or(|at| at.elapsed() >= Duration::from_secs(60))
        {
            self.check(true, Some(release.version().to_owned()), window, cx);
            return;
        }
        self.phase = Phase::Downloading;
        self.progress = None;
        cx.notify();
        let (progress, mut receiver) = tokio::sync::watch::channel(None);
        cx.spawn_in(window, async move |this, cx| {
            while receiver.changed().await.is_ok() {
                let value = *receiver.borrow_and_update();
                if this
                    .update_in(cx, |this, _, cx| {
                        if this.phase == Phase::Downloading {
                            this.progress = value;
                            cx.notify();
                        }
                    })
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();
        let task = self.runtime.spawn(async move {
            let directory = apifuck_core::settings::app_state_path()?.with_file_name("updates");
            update::download(&release, &directory, Some(progress)).await
        });
        cx.spawn_in(window, async move |this, cx| {
            let result = task.await;
            let _ = this.update_in(cx, |this, window, cx| {
                match result {
                    Ok(Ok(downloaded)) => {
                        this.downloaded = Some(Arc::new(downloaded));
                        this.phase = Phase::Downloaded;
                        this.progress = None;
                        feedback::show(
                            Notification::success(tr("更新已下载，可稍后重启安装。").to_owned()),
                            window,
                            cx,
                        );
                    }
                    Ok(Err(error)) => this.fail(error, window, cx),
                    Err(error) => this.fail(error, window, cx),
                }
                cx.notify();
            });
        })
        .detach();
    }
    /// The workbench saves all drafts before calling this method.
    pub fn install(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.phase == Phase::Installed {
            cx.emit(UpdateEvent::Installed);
            return;
        }
        if self.phase.busy() {
            return;
        }
        let Some(downloaded) = self.downloaded.clone() else {
            return;
        };
        // Linux may report a "(deleted)" /proc/self/exe after replacement. Capture
        // the original launch target before installing so restart uses the new file.
        let executable = match std::env::current_exe() {
            Ok(path) => path,
            Err(error) => {
                self.fail(error, window, cx);
                return;
            }
        };
        #[cfg(target_os = "macos")]
        let executable = executable
            .ancestors()
            .nth(3)
            .filter(|path| path.extension().is_some_and(|ext| ext == "app"))
            .map(std::path::Path::to_path_buf)
            .unwrap_or(executable);
        cx.set_restart_path(executable);
        self.phase = Phase::Installing;
        cx.notify();
        let task = self
            .runtime
            .spawn_blocking(move || update::install(&downloaded));
        cx.spawn_in(window, async move |this, cx| {
            let result = task.await;
            let _ = this.update_in(cx, |this, window, cx| {
                match result {
                    Ok(Ok(())) => {
                        this.phase = Phase::Installed;
                        this.downloaded = None;
                        cx.emit(UpdateEvent::Installed);
                    }
                    Ok(Err(error)) => this.fail(error, window, cx),
                    Err(error) => this.fail(error, window, cx),
                }
                cx.notify();
            });
        })
        .detach();
    }
    pub fn sidebar_label(&self) -> Option<String> {
        match self.phase {
            Phase::Available => Some(tr("更新").into()),
            Phase::Checking if self.release.is_some() => Some(tr("检查中").into()),
            Phase::Downloading => Some(
                self.progress
                    .map(|p| format!("{}%", (100 * p.received / p.total).min(100)))
                    .unwrap_or_else(|| tr("下载中").into()),
            ),
            Phase::Downloaded | Phase::Installed => Some(tr("重启").into()),
            Phase::Installing => Some(tr("安装中").into()),
            Phase::Error => Some(tr("重试").into()),
            _ => None,
        }
    }
    pub fn activate(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match self.phase {
            Phase::Available => self.download(window, cx),
            Phase::Downloaded | Phase::Installed => cx.emit(UpdateEvent::InstallRequested),
            _ => self.open(window, cx),
        }
    }
}
fn resume_download(confirmed: &str, available: Option<&str>) -> bool {
    available == Some(confirmed)
}
impl Render for AppUpdate {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let version = self.release.as_ref().map(Release::version).unwrap_or("");
        let description = match self.phase {
            Phase::Idle | Phase::Checking => tr("正在检查更新…").to_owned(),
            Phase::Current => tr("当前没有适用于此平台的新版本").to_owned(),
            Phase::Available => {
                crate::i18n::format(tr("发现新版本 {}"), &[("0", version.to_owned())])
            }
            Phase::Downloading => tr("正在下载更新…").to_owned(),
            Phase::Downloaded => tr("更新已下载，可稍后重启安装。").to_owned(),
            Phase::Installing => tr("正在安装更新…").to_owned(),
            Phase::Installed => tr("更新已安装，重新启动后生效").to_owned(),
            Phase::Error => tr("更新未完成，请重试。").to_owned(),
        };
        let mut body = div()
            .flex()
            .flex_col()
            .gap_4()
            .min_w_0()
            .child(div().text_sm().child(description));
        if self.phase == Phase::Checking
            || self.phase == Phase::Installing
            || (self.phase == Phase::Downloading && self.progress.is_none())
        {
            body = body.child(div().flex().justify_center().py_4().child(Spinner::new()));
        }
        if self.phase == Phase::Available {
            let notes = self.release.as_ref().map(Release::notes).unwrap_or("");
            body = body
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::MEDIUM)
                        .child(tr("版本介绍")),
                )
                .child(
                    div()
                        .id("update-notes")
                        .max_h(px(260.))
                        .overflow_y_scroll()
                        .p_3()
                        .rounded_md()
                        .border_1()
                        .border_color(cx.theme().border)
                        .child(TextView::markdown(
                            "available-release-notes",
                            if notes.trim().is_empty() {
                                tr("此版本暂无发布说明。").to_owned()
                            } else {
                                notes.to_owned()
                            },
                        )),
                );
        }
        if self.phase == Phase::Downloading
            && let Some(progress) = self.progress
        {
            let percent = (progress.received as f32 / progress.total as f32 * 100.).min(100.);
            body = body
                .child(Progress::new("update-progress").value(percent))
                .child(
                    div()
                        .flex()
                        .justify_between()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(format!("{percent:.0}%"))
                        .child(format!(
                            "{:.1} / {:.1} MiB",
                            progress.received as f64 / 1048576.,
                            progress.total as f64 / 1048576.
                        )),
                );
        }
        let mut footer = div().flex().justify_end().gap_2().child(
            Button::new("close-update")
                .outline()
                .label(tr("稍后"))
                .on_click(|_, window, cx| window.close_dialog(cx)),
        );
        match self.phase {
            Phase::Available => {
                footer = footer.child(
                    Button::new("download-update")
                        .primary()
                        .icon(AppIcon::Download)
                        .label(tr("立即更新"))
                        .on_click(cx.listener(|this, _, window, cx| this.download(window, cx))),
                )
            }
            Phase::Downloaded | Phase::Installed => {
                footer = footer.child(
                    Button::new("restart-update")
                        .primary()
                        .icon(AppIcon::RefreshCw)
                        .label(tr("重启并安装"))
                        .on_click(
                            cx.listener(|_, _, _, cx| cx.emit(UpdateEvent::InstallRequested)),
                        ),
                )
            }
            Phase::Error => {
                footer = footer.child(
                    Button::new("retry-update")
                        .primary()
                        .label(tr("重试"))
                        .on_click(cx.listener(|this, _, window, cx| {
                            if this.downloaded.is_some() {
                                cx.emit(UpdateEvent::InstallRequested);
                            } else {
                                this.check(true, None, window, cx);
                            }
                        })),
                )
            }
            _ => {}
        }
        body.child(footer)
    }
}
#[cfg(test)]
mod tests {
    use super::{Phase, resume_download};
    #[test]
    fn stale_notices_require_confirmation_when_the_release_changes_or_disappears() {
        assert!(resume_download("0.2.0", Some("0.2.0")));
        assert!(!resume_download("0.2.0", Some("0.3.0")));
        assert!(!resume_download("0.2.0", None));
    }
    #[test]
    fn checks_do_not_replace_downloads_in_progress_or_ready_to_install() {
        for phase in [
            Phase::Checking,
            Phase::Downloading,
            Phase::Downloaded,
            Phase::Installing,
            Phase::Installed,
        ] {
            assert!(phase.busy() || phase.holds_download());
        }
        assert!(!Phase::Available.busy() && !Phase::Available.holds_download());
        // A failed installation still owns the verified download for a retry.
        assert!(!Phase::Error.can_check(true));
        assert!(Phase::Error.can_check(false));
    }
}
