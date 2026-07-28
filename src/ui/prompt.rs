//! La tira de abajo: a quién le estás escribiendo y qué se le puede mandar sin
//! teclado.
//!
//! **Aquí había un campo de texto y ya no lo hay.** Se escribía abajo y flow le
//! pasaba la línea entera al panel al pulsar Enter, que es lo que hace un
//! chat y no lo que hace una terminal: no había eco mientras escribías, `Tab` no
//! completaba, las flechas no recorrían el historial y una TUI a pantalla
//! completa no se enteraba de nada. Hoy **lo que escribes va directo al panel
//! con el foco** (ver `crate::keys`), así que un campo aparte solo podía
//! competir por las mismas teclas.
//!
//! Lo que queda es lo que un teclado no da:
//!
//! - **A quién le llega lo que escribas.** Con ocho terminales delante, el borde
//!   del panel con foco lo dice pero se lee de lejos; el nombre, escrito, lo
//!   confirma sin tener que buscarlo.
//! - **Los tres botones de ratón**: `^C` y `ESC`, que son bytes que no se
//!   teclean cómodo, y `KILL` —o `RESTART` si el proceso ya terminó—.

use egui::{Align, Layout, Ui};

use crate::agent::Agent;
use crate::theme;
use crate::ui::{widgets, Action};

pub const HEIGHT: f32 = 30.0;

pub fn show(ui: &mut Ui, agent: Option<&Agent>) -> Option<Action> {
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
                // un `›` a secas no basta para saberlo.
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

                // El estado del teclado, dicho con palabras. No es decoración:
                // es la única forma de enterarse de que se escribe arriba y no
                // aquí, y de por qué ahora mismo no se puede escribir.
                ui.label(
                    egui::RichText::new(match agent {
                        None => "sin sesiones: Ctrl-N para abrir una",
                        Some(_) if alive => "lo que escribas va a este panel",
                        Some(_) => "el proceso terminó",
                    })
                    .font(theme::mono(theme::MONO_SM))
                    .color(theme::pal().text_faint),
                );

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
