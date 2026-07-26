//! La barra de entrada, abajo del todo.
//!
//! Una sola para toda la rejilla, no una por panel: multiplicar el campo por
//! ocho llenaría la pantalla de cajas iguales y dejaría a cada terminal sin
//! filas. Le habla siempre al panel con el foco, y lo dice —el nombre del
//! destinatario va escrito al lado del cursor.

use egui::{Align, Layout, Ui};

use crate::agent::Agent;
use crate::theme;
use crate::ui::{widgets, Action};

pub const HEIGHT: f32 = 30.0;

pub fn show(
    ui: &mut Ui,
    agent: Option<&Agent>,
    input: &mut String,
    focus_input: &mut bool,
) -> Option<Action> {
    let mut action = None;
    let alive = agent.is_some_and(|a| a.state.is_running());

    egui::Panel::bottom("prompt")
        .exact_size(HEIGHT)
        .resizable(false)
        .show_separator_line(false)
        .frame(
            egui::Frame::new()
                .fill(egui::Color32::TRANSPARENT)
                .inner_margin(egui::Margin {
                    left: theme::GAP as i8 + 4,
                    right: theme::GAP as i8 + 2,
                    top: 4,
                    bottom: 4,
                }),
        )
        .show(ui, |ui| {
            ui.horizontal_centered(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;

                // A quién se le está escribiendo. Con ocho terminales delante,
                // un `>` a secas no basta para saberlo.
                if let Some(a) = agent {
                    ui.label(
                        egui::RichText::new(a.name.to_uppercase())
                            .font(theme::sans(theme::SANS_SM))
                            .color(if alive {
                                theme::pal().text_dim
                            } else {
                                theme::pal().text_faint
                            }),
                    );
                }
                ui.label(
                    egui::RichText::new("›")
                        .font(theme::mono(theme::MONO_SM))
                        .color(if alive {
                            theme::pal().accent_text
                        } else {
                            theme::pal().text_faint
                        }),
                );

                // Los botones van a la derecha, así que se reserva su hueco
                // antes de darle al campo el ancho restante.
                let field_w = (ui.available_width() - 168.0).max(60.0);

                let resp = ui.add(
                    egui::TextEdit::singleline(input)
                        .font(theme::mono(theme::MONO_SM))
                        .text_color(theme::pal().text)
                        .frame(egui::Frame::NONE)
                        .desired_width(field_w)
                        .hint_text(
                            egui::RichText::new(match agent {
                                None => "sin sesiones: Ctrl-N para abrir una",
                                Some(_) if alive => "escribe y Enter para enviar al proceso",
                                Some(_) => "el proceso terminó",
                            })
                            .font(theme::mono(theme::MONO_SM))
                            .color(theme::pal().text_faint),
                        )
                        .interactive(alive),
                );

                // Al cambiar de panel el foco va al campo: lo normal tras
                // seleccionarlo es querer contestarle, sobre todo si está
                // BLOCKED esperando precisamente eso.
                if *focus_input {
                    *focus_input = false;
                    if alive {
                        resp.request_focus();
                    }
                }

                if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    action = Some(Action::Send(std::mem::take(input)));
                    resp.request_focus();
                }

                let Some(agent) = agent else {
                    return;
                };
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    if alive {
                        if widgets::button(ui, "KILL", theme::pal().red)
                            .on_hover_text("Matar el proceso")
                            .clicked()
                        {
                            action = Some(Action::Kill(agent.id));
                        }
                        if widgets::button(ui, "ESC", theme::pal().text_dim).clicked() {
                            action = Some(Action::SendRaw(vec![0x1b]));
                        }
                        if widgets::button(ui, "^C", theme::pal().amber)
                            .on_hover_text("Enviar Ctrl-C")
                            .clicked()
                        {
                            action = Some(Action::SendRaw(vec![0x03]));
                        }
                    } else if widgets::button(ui, "RESTART", theme::pal().accent_text).clicked() {
                        action = Some(Action::Restart(agent.id));
                    }
                });
            });
        });

    action
}
