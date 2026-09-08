use crate::{
    collab::{
        Phase,
        workflows::{ChatState, Draft, MemberDraft},
    },
    state::Session,
};
use egui::{Color32, RichText, Ui, vec2};

pub enum Action {
    New,
    Select(usize),
    Send,
    Cancel,
    Handoff(String),
    Close,
}

fn phase_name(p: &Phase) -> &'static str {
    match p {
        Phase::Development => "开发",
        Phase::Review => "审查",
        Phase::Fix => "修复",
        Phase::Completed => "完成",
    }
}

fn tool_button(ui: &mut Ui, text: &str, tip: &str) -> bool {
    let dark = ui.visuals().dark_mode;
    let (rect, response) = ui.allocate_exact_size(vec2(32.0, 30.0), egui::Sense::click());
    let hover = ui.ctx().animate_bool(response.id.with("hover"), response.hovered() && ui.is_enabled());
    let alpha = (5.0 + 9.0 * hover) as u8;
    let fill = if dark { Color32::from_white_alpha(alpha) } else { Color32::from_black_alpha(alpha) };
    let border = if dark { Color32::from_white_alpha(8) } else { Color32::from_black_alpha(10) };
    let painter = ui.painter();
    painter.rect_filled(rect.translate(vec2(0.0, 1.5)), 6.0, Color32::from_black_alpha(if dark { 60 } else { 15 }));
    painter.rect_filled(rect, 6.0, fill);
    painter.rect_stroke(rect, 6.0, egui::Stroke::new(0.5, border), egui::StrokeKind::Inside);
    let fg = if ui.is_enabled() { ui.visuals().text_color() } else { ui.visuals().weak_text_color() };
    painter.text(rect.center(), egui::Align2::CENTER_CENTER, text, egui::FontId::proportional(14.0), fg);
    response.on_hover_text(tip).clicked()
}

pub fn show(ui: &mut Ui, state: &mut ChatState) -> Option<Action> {
    let mut action = None;
    let weak = ui.visuals().weak_text_color();
    let running = state.is_running();
    ui.spacing_mut().item_spacing = vec2(8.0, 6.0);
    ui.spacing_mut().button_padding = vec2(12.0, 7.0);
    ui.horizontal(|ui| {
        ui.label(RichText::new("Agent Chat").size(18.0).strong());
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if tool_button(ui, "+", "新建工作流") { action = Some(Action::New); }
            if tool_button(ui, "↩", "返回终端") { action = Some(Action::Close); }
        });
    });
    if state.rounds.is_empty() {
        ui.centered_and_justified(|ui| { ui.label("还没有工作流"); });
        return action;
    }
    let active = state.active.unwrap_or(0);
    let round = state.rounds[active].1.clone();
    ui.horizontal_wrapped(|ui| {
        egui::ComboBox::from_id_salt("chat_workflow").width(150.0).selected_text(&round.title).show_ui(ui, |ui| {
            for (i, (_, r)) in state.rounds.iter().enumerate() {
                if ui.selectable_label(i == active, &r.title).clicked() { action = Some(Action::Select(i)); }
            }
        });
        for p in round.participants.iter().filter(|p| p.id != "user") {
            ui.label(RichText::new(format!("{} · {}", p.name, p.role)).color(weak));
        }
    });
    ui.horizontal(|ui| {
        ui.add(egui::Label::new(RichText::new(&round.goal).small()).truncate()).on_hover_text(&round.goal);
        ui.label(RichText::new(phase_name(&round.phase)).small().color(weak));
    });
    if let Some(error) = &state.error {
        ui.add(egui::Label::new(RichText::new(error).color(Color32::from_rgb(220, 100, 90))).truncate()).on_hover_text(error);
    }
    let bounds = ui.available_rect_before_wrap();
    let footer_h = 120.0_f32.min(bounds.height() * 0.4);
    let history_h = if state.chat_expanded { (bounds.height() * 0.25).min(180.0) } else { 0.0 };
    let term_bottom = bounds.max.y - footer_h - history_h;
    let term_rect = egui::Rect::from_min_max(bounds.min, egui::pos2(bounds.max.x, term_bottom));
    let members: Vec<_> = round.participants.iter().filter(|p| p.id != "user").collect();
    let mut terminal_ui = ui.new_child(egui::UiBuilder::new().id_salt(("agents", &round.id)).max_rect(term_rect));
    let n = members.len().max(1);
    egui::ScrollArea::horizontal().id_salt(("terminal_columns", &round.id)).auto_shrink([false, false]).show(&mut terminal_ui, |ui| {
        ui.horizontal(|ui| {
            let width = ((term_rect.width() - 8.0 * (n - 1) as f32) / n as f32).max(280.0);
            for member in members {
                ui.allocate_ui_with_layout(vec2(width, term_rect.height()), egui::Layout::top_down(egui::Align::Min), |ui| {
                    ui.set_min_width(width);
                    ui.push_id((&round.id, &member.id), |ui| {
                        let run = state.native_runs.iter_mut().find(|r| r.round == round.id && r.agent == member.id);
                        ui.horizontal(|ui| {
                            ui.strong(&member.name);
                            if let Some(r) = &run { ui.label(RichText::new(&r.status).small().color(weak)); }
                            else { ui.label(RichText::new("等待任务").small().color(weak)); }
                        });
                        if let Some(run) = run {
                            crate::ui::terminal::show_embedded(ui, &mut run.session, state.draft.is_none(), &state.theme);
                        } else {
                            egui::Frame::NONE.fill(state.theme.background).show(ui, |ui| {
                                ui.set_min_size(vec2(width - 4.0, (term_rect.height() - 32.0).max(0.0)));
                                ui.label(RichText::new(&member.role).color(weak));
                            });
                        }
                    });
                    #[cfg(test)] assert!(ui.min_rect().max.y <= term_rect.max.y + 1.0, "terminal overlaps conversation");
                });
            }
        });
    });
    let history_rect = egui::Rect::from_min_max(egui::pos2(bounds.min.x, term_bottom), egui::pos2(bounds.max.x, bounds.max.y - footer_h));
    if state.chat_expanded {
        let mut history_ui = ui.new_child(egui::UiBuilder::new().id_salt("conversation").max_rect(history_rect));
        egui::ScrollArea::vertical().id_salt(("messages", &round.id)).max_height(history_h).auto_shrink([false, false]).stick_to_bottom(true).show(&mut history_ui, |ui| {
            for m in &state.messages {
                let name = |id: &str| if id == "user" { "我".to_owned() } else { round.participants.iter().find(|p| p.id == id).map(|p| p.name.clone()).unwrap_or_else(|| id.into()) };
                ui.push_id(&m.id, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        ui.strong(format!("{} → {}", name(&m.from), name(&m.to)));
                        if tool_button(ui, "↩", "回复") { state.reply = Some(m.id.clone()); state.recipient = if m.from == "user" { m.to.clone() } else { m.from.clone() }; }
                        if m.from != "user" && m.to == "user" {
                            ui.add_enabled_ui(!running, |ui| {
                                if tool_button(ui, "→", "交给其他 AI") { action = Some(Action::Handoff(m.id.clone())); }
                            });
                        }
                    });
                    let text = m.body.lines().filter(|line| line.trim() != "[WORKFLOW_COMPLETE]").collect::<Vec<_>>().join("\n");
                    ui.add(egui::Label::new(text).wrap().selectable(true));
                    ui.separator();
                });
            }
        });
    }
    let footer_rect = egui::Rect::from_min_max(egui::pos2(bounds.min.x, bounds.max.y - footer_h), bounds.max);
    let mut footer = ui.new_child(egui::UiBuilder::new().id_salt("composer").max_rect(footer_rect));
    footer.horizontal(|ui| {
        if tool_button(ui, if state.chat_expanded { "−" } else { "+" }, "展开或收起聊天") { state.chat_expanded = !state.chat_expanded; }
        ui.label("聊天");
        ui.add(egui::Label::new(RichText::new(&state.execution_status).small().color(weak)).truncate()).on_hover_text(&state.execution_status);
        if state.is_running() && tool_button(ui, "■", "取消协作") { action = Some(Action::Cancel); }
    });
    footer.horizontal(|ui| {
        egui::ComboBox::from_id_salt("first_agent").width(100.0).selected_text(round.participants.iter().find(|p| p.id == state.recipient).map(|p| p.name.as_str()).unwrap_or("首位执行")).show_ui(ui, |ui| {
            for p in round.participants.iter().filter(|p| p.id != "user") { ui.selectable_value(&mut state.recipient, p.id.clone(), &p.name); }
        });
        if state.reply.is_some() && tool_button(ui, "×", "取消回复") { state.reply = None; }
        let width = (ui.available_width() - 48.0).max(40.0);
        ui.add_sized(vec2(width, 46.0), egui::TextEdit::multiline(&mut state.body).hint_text("输入任务…"));
        ui.add_enabled_ui(!state.is_running() && !state.body.trim().is_empty(), |ui| {
            if tool_button(ui, "↑", "发送任务") { action = Some(Action::Send); }
        });
    });
    ui.allocate_rect(bounds, egui::Sense::hover());
    action
}
pub fn begin_draft(sessions: &[Session], selected: usize) -> Draft {
    Draft {
        title: String::new(),
        goal: String::new(),
        repo: sessions
            .get(selected)
            .map(|s| s.cwd.display().to_string())
            .unwrap_or_default(),
        members: sessions
            .iter()
            .map(|s| MemberDraft {
                id: format!("workspace-{}", s.id),
                name: s.name.clone(),
                command: s.command.clone(),
                cwd: s.cwd.clone(),
                selected: false,
                role: "开发".into(),
            })
            .collect(),
    }
}

pub fn new_round_modal(ui: &mut Ui, state: &mut ChatState) {
    let Some(draft) = &mut state.draft else {
        return;
    };
    let mut create = false;
    let mut cancel = false;
    let screen = ui
        .ctx()
        .input(|i| i.raw.screen_rect)
        .unwrap_or(ui.max_rect());
    let frame = egui::Frame::popup(ui.style())
        .corner_radius(8)
        .inner_margin(20.0);
    let response = egui::Modal::new(egui::Id::new("new_workflow"))
        .frame(frame)
        .show(ui.ctx(), |ui| {
            ui.set_width((screen.width() - 70.0).clamp(260.0, 560.0));
            ui.spacing_mut().item_spacing = vec2(10.0, 10.0);
            ui.spacing_mut().button_padding = vec2(14.0, 9.0);
            ui.label(RichText::new("新建工作流").size(18.0).strong());
            egui::ScrollArea::vertical()
                .max_height((screen.height() - 170.0).max(160.0))
                .show(ui, |ui| {
                    ui.label("名称");
                    ui.add(
                        egui::TextEdit::singleline(&mut draft.title).desired_width(f32::INFINITY),
                    );
                    ui.label("目标与约束");
                    ui.add(
                        egui::TextEdit::multiline(&mut draft.goal)
                            .desired_rows(3)
                            .desired_width(f32::INFINITY),
                    );
                    ui.label("工作目录");
                    ui.horizontal(|ui| {
                        ui.add_sized(
                            vec2((ui.available_width() - 44.0).max(120.0), 26.0),
                            egui::TextEdit::singleline(&mut draft.repo),
                        );
                        if ui.button("…").on_hover_text("选择目录").clicked() {
                            if let Some(path) = rfd::FileDialog::new().pick_folder() {
                                draft.repo = path.display().to_string();
                            }
                        }
                    });
                    ui.separator();
                    ui.strong("参与 AI 与职责");
                    for m in &mut draft.members {
                        ui.push_id(&m.id, |ui| {
                            ui.horizontal(|ui| {
                                let supported = crate::collab::runner::detect(&m.command).is_ok();
                                ui.add_enabled(supported, egui::Checkbox::new(&mut m.selected, &m.name));
                                if !supported { ui.label(RichText::new("暂未适配").small()); }
                                if m.selected {
                                    ui.add(egui::TextEdit::singleline(&mut m.role).desired_width(180.0).hint_text("职责"));
                                }
                            });
                            if m.selected {
                                ui.horizontal(|ui| {
                                    egui::ComboBox::from_id_salt("role")
                                        .width(110.0)
                                        .selected_text("职责预设")
                                        .show_ui(ui, |ui| {
                                            for role in [
                                                "主开发",
                                                "代码审查",
                                                "测试验证",
                                                "方案设计",
                                                "协调者",
                                            ] {
                                                if ui
                                                    .selectable_label(m.role == role, role)
                                                    .clicked()
                                                {
                                                    m.role = role.into();
                                                }
                                            }
                                        });
                                });
                                ui.label(
                                    RichText::new(&m.command)
                                        .small()
                                        .color(ui.visuals().weak_text_color()),
                                );
                            }
                        });
                    }
                });
            if let Some(error) = &state.error {
                ui.colored_label(Color32::from_rgb(210, 90, 80), error);
            }
            ui.separator();
            ui.horizontal(|ui| {
                cancel = ui.button("取消").clicked();
                let valid = !draft.title.trim().is_empty()
                    && !draft.goal.trim().is_empty()
                    && draft.members.iter().any(|m| m.selected)
                    && draft
                        .members
                        .iter()
                        .filter(|m| m.selected)
                        .all(|m| !m.role.trim().is_empty());
                create = ui
                    .add_enabled(valid, egui::Button::new("创建工作流"))
                    .clicked();
            });
        });
    if cancel || response.should_close() {
        state.draft = None;
        state.error = None;
    } else if create {
        state.error = state.create().err().map(|e| e.to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collab::Store;

    #[test]
    fn chat_and_creation_dialog_render_at_supported_window_sizes() {
        let root = std::env::temp_dir().join(Store::new_id("chat-layout"));
        let mut state = ChatState::new(root.clone());
        let sessions = vec![
            Session::new(3, "Codex", "codex", root.clone()),
            Session::new(8, "Claude Code", "claude", root.clone()),
        ];
        let mut draft = begin_draft(&sessions, 0);
        draft.title = "Development and review".into();
        draft.goal = "Review changes".into();
        draft.members[0].selected = true;
        draft.members[1].selected = true;
        state.draft = Some(draft);
        state.create().unwrap();
        state.body = "测试消息：请审查当前工作流的修改。".repeat(4);
        state.send().unwrap();
        state.execution_status = "工作流 · Codex 正在执行".into();
        for p in state.rounds[0].1.participants.iter().filter(|p| p.id != "user") {
            state.native_runs.push(crate::collab::native::NativeRun::preview(&state.rounds[0].1.id, &p.id, &p.name));
        }
        for (w, h) in [(760.0, 480.0), (1120.0, 720.0), (1600.0, 900.0)] {
            let ctx = egui::Context::default();
            ctx.set_visuals(crate::fonts::app_visuals(true));
            crate::fonts::setup_fonts(&ctx);
            for pass in 0..4 {
                state.chat_expanded = pass % 2 == 0;
                let input = egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, vec2(w, h))),
                    ..Default::default()
                };
                let output = ctx.run_ui(input, |ui| {
                    egui::Panel::left("test_sidebar")
                        .exact_size(232.0)
                        .show(ui, |_| {});
                    egui::CentralPanel::default_margins().show(ui, |ui| {
                        show(ui, &mut state);
                        assert!(ui.min_rect().max.x <= w + 1.0, "horizontal overflow at {w}");
                        assert!(
                            ui.min_rect().max.y <= h + 1.0,
                            "vertical overflow at {h}: {:?}",
                            ui.min_rect()
                        );
                    });
                });
                assert!(!output.shapes.is_empty());
            }
            state.draft = Some(begin_draft(&sessions, 0));
            let output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, vec2(w, h))),
                    ..Default::default()
                },
                |ui| new_round_modal(ui, &mut state),
            );
            assert!(!output.shapes.is_empty());
            state.draft = None;
        }
        std::fs::remove_dir_all(root).unwrap();
    }
}
