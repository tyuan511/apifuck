use crate::{i18n::tr, icons::AppIcon, surfaces::panel_editor};
use apifuck_core::mcp::{Service, guides::Agent};
use gpui_kit::component::{
    ActiveTheme, Disableable, Sizable,
    button::{Button, ButtonVariants},
    input::EditorState,
    switch::Switch,
    tab::{Tab, TabBar},
};
use gpui_kit::{prelude::FluentBuilder, *};
use std::sync::Arc;

pub struct McpView {
    service: Option<Arc<Service>>,
    error: Option<String>,
    selected: usize,
    busy: bool,
    editor: Entity<EditorState>,
}
impl McpView {
    pub fn new(
        service: Option<Arc<Service>>,
        error: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let editor = cx.new(|cx| EditorState::new(window, cx).line_number(false));
        let mut view = Self {
            service,
            error,
            selected: 0,
            busy: false,
            editor,
        };
        view.select(0, window, cx);
        view
    }
    fn config(&self) -> String {
        self.service
            .as_ref()
            .map(|service| {
                let status = service.status();
                Agent::ALL[self.selected].config(&status.config.endpoint(), &status.config.token)
            })
            .unwrap_or_default()
    }
    fn select(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.selected = index;
        let code = self.config();
        self.editor.update(cx, |editor, cx| {
            editor.set_highlighter(
                match Agent::ALL[index] {
                    Agent::ClaudeCode => "bash",
                    Agent::Codex => "toml",
                    _ => "json",
                },
                cx,
            );
            editor.set_value(code, window, cx);
        });
        cx.notify();
    }
    fn enable(&mut self, enabled: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        let Some(service) = &self.service else {
            return;
        };
        self.busy = true;
        let task = service.configure(enabled);
        cx.spawn_in(window, async move |this, cx| {
            let result = task
                .await
                .map_err(anyhow::Error::from)
                .and_then(|result| result);
            let _ = this.update_in(cx, |this, window, cx| {
                this.busy = false;
                this.error = result.err().map(|e| format!("{e:#}"));
                if let Some(error) = &this.error {
                    crate::feedback::show(
                        gpui_kit::component::notification::Notification::error(error.clone()),
                        window,
                        cx,
                    );
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
}
impl Render for McpView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let status = self.service.as_ref().map(|service| service.status());
        let error = self
            .error
            .clone()
            .or_else(|| status.as_ref().and_then(|s| s.error.clone()));
        let running = status.as_ref().is_some_and(|s| s.running);
        div().flex().flex_col().gap_4().w_full()
            .child(div().flex().items_center().justify_between().gap_4()
                .child(div().flex().flex_col().gap_1().child(tr("启用 MCP 服务")).child(div().text_sm().text_color(cx.theme().muted_foreground).child(tr("开关立即生效，默认关闭；启用后随应用启动，退出应用即停止。"))))
                .child(Switch::new("mcp-enabled").checked(status.as_ref().is_some_and(|s|s.config.enabled)).disabled(self.service.is_none()).on_click(cx.listener(|this,enabled,window,cx|this.enable(*enabled,window,cx)))))
            .child(div().flex().items_center().gap_3()
                .child(if running {tr("MCP 正在运行")}else{tr("MCP 未运行")})
                .when(!running && self.service.is_some(),|view|view.child(Button::new("mcp-retry").disabled(self.busy).small().outline().label(tr("启动 / 重试")).on_click(cx.listener(|this,_,window,cx|this.enable(true,window,cx))))))
            .when_some(error,|view,error|view.child(div().text_sm().text_color(cx.theme().danger).child(error)))
            .when_some(status.as_ref(),|view,status|view.child(div().text_sm().child(status.config.endpoint())))
            .child(div().text_sm().text_color(cx.theme().muted_foreground).child(tr("仅监听本机并使用令牌鉴权。使用时保持应用运行；配置含访问令牌，只交给可信的 Agent。")))
            .child(div().font_weight(FontWeight::SEMIBOLD).child(tr("可用操作")))
            .child(div().text_sm().child(tr("项目、请求、目录和环境的增删改查，移动排序、HTTP / GraphQL / SSE、WebSocket、取消任务、集合运行、cURL 与 Bru 转换及代码导出。")))
            .child(div().text_sm().text_color(cx.theme().muted_foreground).child(tr("使用已保存的 YAML 和原生请求引擎；存在未保存草稿或运行中请求时拒绝修改工作区。关闭服务会取消 MCP 任务。")))
            .child(div().font_weight(FontWeight::SEMIBOLD).child(tr("添加到 Agent")))
            .child(TabBar::new("mcp-agents").selected_index(self.selected).children(Agent::ALL.into_iter().map(|agent|Tab::new().label(agent.name()))).on_click(cx.listener(|this,index,window,cx|this.select(*index,window,cx))))
            .child(div().text_sm().child(tr(Agent::ALL[self.selected].instruction())))
            .child(div().h(px(260.)).border_1().border_color(cx.theme().border).child(panel_editor(&self.editor).readonly(true)))
            .child(div().flex().gap_3()
                .child(Button::new("mcp-copy").small().primary().icon(AppIcon::Copy).label(tr("复制配置")).disabled(self.service.is_none()).on_click(cx.listener(|this,_,window,cx|{
                    cx.write_to_clipboard(ClipboardItem::new_string(this.config()));
                    crate::feedback::show(gpui_kit::component::notification::Notification::success(tr("已复制 MCP 配置").to_owned()),window,cx);
                })))
                .child(Button::new("mcp-docs").small().outline().label(tr("官方教程")).on_click(cx.listener(|this,_,_,cx|cx.open_url(Agent::ALL[this.selected].docs())))))
            .child(div().text_sm().text_color(cx.theme().muted_foreground).child(tr("合并配置时保留其他服务。连接后让 Agent 调用 get_workspace；云端 Agent 无法直接访问本机地址。端口占用时关闭占用程序，或退出应用后修改 ~/.apifuck-native/mcp.json。")))
    }
}
