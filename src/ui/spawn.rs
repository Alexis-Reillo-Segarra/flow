//! Formulario de lanzamiento.
//!
//! Modal propio en vez de `egui::Window`: hace falta control total sobre el
//! borde (1 px duro, sin sombra ni esquinas redondeadas) y sobre el orden de
//! capas del velo de fondo.

use egui::{
    epaint::StrokeKind, vec2, Align, Color32, Context, CornerRadius, Id, Key, Layout, Rect, Sense,
    Stroke, Ui,
};

use crate::presets;
use crate::theme;
use crate::ui::{widgets, Action};

/// Ancho al que aspira el cuadro. Si no cabe, se encoge.
const BOX_W: f32 = 440.0;
/// Lo que ocupan el título y el pie, que van fuera del scroll. Es lo que hay
/// que descontarle al alto disponible para saber cuánto le queda al centro.
const RESERVED_H: f32 = 120.0;
const AGENTS_TITLE: &str = "AGENTES";

/// Qué se está lanzando.
///
/// Es el mismo formulario porque es la misma pregunta —qué comando, dónde— pero
/// no lanza lo mismo: una sesión nueva empieza de cero en el directorio que le
/// digas, y un panel nace dentro de la sesión que estás mirando y hereda su
/// directorio.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Session,
    Pane,
}

pub struct Form {
    pub open: bool,
    pub kind: Kind,
    pub name: String,
    pub cmd: String,
    pub cwd: String,
    pub error: Option<String>,
    /// Pone el foco en el campo de comando el primer frame tras abrirse.
    focus_pending: bool,
}

impl Form {
    pub fn new(cwd: String) -> Self {
        Self {
            open: false,
            kind: Kind::Session,
            name: String::new(),
            cmd: String::new(),
            cwd,
            error: None,
            focus_pending: false,
        }
    }

    /// Abre el formulario.
    ///
    /// `cwd` impone un directorio —el de la sesión, al añadirle un panel, para
    /// que nazca ya mirando donde trabaja el agente—. A `None` se conserva el
    /// último que se escribió: abrir dos sesiones seguidas en el mismo sitio es
    /// lo normal, y volver a teclear la ruta, un peaje.
    ///
    /// El comando llega precargado con el shell solo al añadir un panel: es lo
    /// que uno quiere nueve de cada diez veces —abrir algo para mirar— y deja
    /// el lanzamiento en un Enter. Para un subagente basta con pulsar el suyo
    /// en la lista, que lo sobrescribe.
    pub fn show(&mut self, kind: Kind, cwd: Option<String>) {
        self.open = true;
        self.kind = kind;
        self.error = None;
        self.focus_pending = true;
        if let Some(cwd) = cwd {
            self.cwd = cwd;
        }
        self.name.clear();
        self.cmd = match kind {
            Kind::Session => String::new(),
            Kind::Pane => shell().to_owned(),
        };
    }

    pub fn close(&mut self) {
        self.open = false;
        self.name.clear();
        self.cmd.clear();
        self.error = None;
    }

    /// Nombre efectivo del panel: el que se escribió, o el que se deduce del
    /// comando.
    pub fn effective_name(&self) -> String {
        let name = self.name.trim();
        if name.is_empty() {
            name_of(&self.cmd)
        } else {
            name.to_owned()
        }
    }
}

/// El nombre que se le pone a un panel a partir de su comando: el primer token,
/// sin ruta ni comillas, que casi siempre es lo que uno diría de viva voz ("el
/// claude", "el cargo"). También lo usa lo que llega por el buzón, donde nadie
/// escribe un nombre.
pub fn name_of(cmd: &str) -> String {
    cmd.split_whitespace()
        .next()
        .unwrap_or("agent")
        .trim_matches(|c| c == '"' || c == '\'')
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or("agent")
        .to_owned()
}

/// ¿La ruta escrita apunta a un sitio que todavía no existe?
///
/// Solo tiene sentido al abrir sesión: un panel hereda el directorio de la suya,
/// que por definición ya existe. Una ruta vacía tampoco cuenta —eso es no haber
/// escrito nada, no haber pedido una carpeta nueva—.
pub fn missing_dir(form: &Form) -> bool {
    let cwd = form.cwd.trim();
    form.kind == Kind::Session && !cwd.is_empty() && !std::path::Path::new(cwd).is_dir()
}

/// El shell del sistema, que es con lo que llega precargado un panel nuevo.
pub fn shell() -> &'static str {
    if cfg!(windows) {
        "cmd"
    } else {
        "bash -i"
    }
}

/// `full` es "ya no caben más paneles en esta sesión": el formulario se abre
/// igual —para que el atajo nunca parezca roto— pero dice por qué no va a
/// lanzar nada.
pub fn show(
    ctx: &Context,
    form: &mut Form,
    installed: &presets::Installed,
    projects: &crate::projects::Projects,
    full: bool,
) -> Option<Action> {
    if !form.open {
        return None;
    }
    let mut action = None;

    // Velo: apaga el fondo sin ocultarlo del todo, para no perder el contexto
    // de qué agentes ya hay corriendo.
    ctx.layer_painter(egui::LayerId::new(
        egui::Order::Middle,
        Id::new("spawn-dim"),
    ))
    .rect_filled(
        ctx.content_rect(),
        CornerRadius::ZERO,
        Color32::from_black_alpha(190),
    );

    if ctx.input(|i| i.key_pressed(Key::Escape)) {
        return Some(Action::CancelSpawn);
    }

    // El cuadro se ajusta a la ventana en vez de imponerle un tamaño: se
    // encoge de ancho si no cabe, se centra, y el alto lo tiene limitado a lo
    // que haya. Antes era un rectángulo fijo colocado a mano al 22% de la
    // altura, y en una ventana pequeña —o con muchos agentes detectados, que
    // añaden filas de botones— se salía por abajo llevándose consigo LAUNCH y
    // CANCEL: el formulario se abría y no había forma de lanzar nada.
    let screen = ctx.content_rect();
    let margin = theme::GAP * 2.0;
    let box_w = (screen.width() - margin * 2.0).clamp(200.0, BOX_W);
    let box_h = (screen.height() - margin * 2.0).max(120.0);

    egui::Area::new(Id::new("spawn-form"))
        .order(egui::Order::Foreground)
        .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
        .show(ctx, |ui| {
            egui::Frame::new()
                .fill(theme::RAISED)
                .stroke(Stroke::new(1.0, theme::LINE_HI))
                .corner_radius(CornerRadius::same(theme::RADIUS))
                .inner_margin(egui::Margin::same(14))
                .show(ui, |ui| {
                    ui.set_width(box_w - 28.0);
                    ui.spacing_mut().item_spacing.y = 6.0;

                    ui.label(
                        egui::RichText::new(match form.kind {
                            Kind::Session => "NEW SESSION",
                            Kind::Pane => "ADD TO SESSION",
                        })
                        .font(theme::sans(theme::SANS_MD))
                        .color(theme::ACCENT_TEXT),
                    );
                    widgets::hline(ui, theme::LINE);
                    ui.add_space(2.0);

                    // Al añadir a una sesión, lo primero es decir a cuál: el
                    // directorio es lo que comparten sus paneles y lo que hace
                    // que la terminal nueva sirva para mirar lo que hace el
                    // agente en vez de para perderse.
                    if form.kind == Kind::Pane {
                        widgets::mono_label(ui, &form.cwd, theme::TEXT_DIM);
                        ui.add_space(4.0);
                    }

                    // Lo de en medio va en un scroll y el pie se queda fuera:
                    // pase lo que pase con el alto de la ventana o con cuántos
                    // agentes haya detectados, LAUNCH y CANCEL siguen ahí.
                    egui::ScrollArea::vertical()
                        .max_height((box_h - RESERVED_H).max(80.0))
                        .auto_shrink([false, true])
                        .show(ui, |ui| {
                            ui.spacing_mut().item_spacing.y = 6.0;

                            // Comando: el único campo obligatorio, y por eso el primero.
                            let cmd_resp = field(ui, "COMMAND", &mut form.cmd, "cargo test");
                            if form.focus_pending {
                                cmd_resp.request_focus();
                                form.focus_pending = false;
                            }

                            ui.add_space(6.0);

                            // Solo se ofrecen los que están instalados de verdad: una
                            // lista de agentes que no tienes es peor que ninguna lista.
                            let mut row =
                                |ui: &mut Ui, title: &str, items: Vec<&presets::Preset>| {
                                    if items.is_empty() {
                                        return;
                                    }
                                    ui.label(
                                        egui::RichText::new(title)
                                            .font(theme::sans(theme::SANS_SM))
                                            .color(theme::TEXT_FAINT),
                                    );
                                    ui.add_space(3.0);
                                    ui.horizontal_wrapped(|ui| {
                                        ui.spacing_mut().item_spacing = vec2(4.0, 4.0);
                                        for p in items {
                                            let accent = if std::ptr::eq(title, AGENTS_TITLE) {
                                                theme::ACCENT_TEXT
                                            } else {
                                                theme::TEXT_DIM
                                            };
                                            if widgets::agent_button(ui, p.label, p.mark, accent)
                                                .on_hover_text(p.about)
                                                .clicked()
                                            {
                                                form.cmd = p.command.to_owned();
                                            }
                                        }
                                    });
                                    ui.add_space(6.0);
                                };

                            row(ui, AGENTS_TITLE, installed.agents().collect());
                            row(ui, "HERRAMIENTAS", installed.tools().collect());

                            if installed.agent_count() == 0 {
                                widgets::mono_label(
                                    ui,
                                    "no se detectó ningún agente en el PATH",
                                    theme::TEXT_FAINT,
                                );
                                ui.add_space(6.0);
                            }

                            ui.add_space(2.0);
                            field(ui, "NAME", &mut form.name, "(opcional)");
                            // El directorio solo se elige al abrir sesión. Un panel
                            // hereda el de la suya y no se puede desviar: si pudiera,
                            // la sesión dejaría de significar "todo esto trabaja sobre
                            // lo mismo", que es justo para lo que sirve.
                            if form.kind == Kind::Session {
                                ui.add_space(6.0);

                                // Los proyectos que ya has usado, para no volver
                                // a teclear la ruta. Es el mismo gesto que la
                                // fila de agentes de arriba: pulsas el nombre y
                                // rellena el campo, que sigue ahí debajo por si
                                // el sitio es otro.
                                if !projects.dirs().is_empty() {
                                    ui.label(
                                        egui::RichText::new("PROYECTO")
                                            .font(theme::sans(theme::SANS_SM))
                                            .color(theme::TEXT_FAINT),
                                    );
                                    ui.add_space(3.0);
                                    ui.horizontal_wrapped(|ui| {
                                        ui.spacing_mut().item_spacing = vec2(4.0, 4.0);
                                        for dir in projects.dirs() {
                                            let chosen = form.cwd.trim() == dir;
                                            let accent = if chosen {
                                                theme::ACCENT_TEXT
                                            } else {
                                                theme::TEXT_DIM
                                            };
                                            if widgets::button(
                                                ui,
                                                &crate::projects::name_of(dir),
                                                accent,
                                            )
                                            .on_hover_text(dir)
                                            .clicked()
                                            {
                                                form.cwd = dir.clone();
                                            }
                                        }
                                    });
                                    ui.add_space(6.0);
                                }

                                field(ui, "DIR", &mut form.cwd, "");
                                // Que la ruta no exista no es un error todavía:
                                // escribir una carpeta nueva es una forma
                                // legítima de empezar un proyecto. Se avisa aquí
                                // y el botón de lanzar cambia de nombre, para
                                // que crear un directorio en tu disco no ocurra
                                // nunca sin que lo hayas leído antes.
                                if missing_dir(form) {
                                    ui.add_space(3.0);
                                    widgets::mono_label(ui, "no existe todavía", theme::AMBER);
                                }
                            }

                            if let Some(err) = &form.error {
                                ui.add_space(4.0);
                                widgets::mono_label(ui, err, theme::RED);
                            }
                        });

                    ui.add_space(10.0);
                    widgets::hline(ui, theme::LINE);
                    ui.add_space(8.0);

                    ui.horizontal(|ui| {
                        widgets::mono_label(
                            ui,
                            if full {
                                "esta sesión está llena: cierra un panel"
                            } else {
                                "Esc cancela · Enter lanza"
                            },
                            if full {
                                theme::AMBER
                            } else {
                                theme::TEXT_FAINT
                            },
                        );
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.spacing_mut().item_spacing.x = 6.0;
                            let launch = if missing_dir(form) {
                                "CREAR Y LANZAR"
                            } else {
                                "LAUNCH"
                            };
                            if !full && widgets::button(ui, launch, theme::ACCENT_TEXT).clicked() {
                                action = Some(Action::ConfirmSpawn);
                            }
                            if widgets::button(ui, "CANCEL", theme::TEXT_DIM).clicked() {
                                action = Some(Action::CancelSpawn);
                            }
                        });
                    });

                    if !full
                        && ui.input(|i| i.key_pressed(Key::Enter))
                        && !form.cmd.trim().is_empty()
                    {
                        action = Some(Action::ConfirmSpawn);
                    }
                });
        });

    action
}

/// Campo etiquetado: rótulo en sans encima, caja de 1 px con el valor en mono.
/// El valor va monoespaciado porque casi siempre es una ruta o un comando, y
/// ahí importa distinguir `l` de `1` y `O` de `0`.
fn field(ui: &mut Ui, label: &str, value: &mut String, hint: &str) -> egui::Response {
    ui.label(
        egui::RichText::new(label)
            .font(theme::sans(theme::SANS_SM))
            .color(theme::TEXT_FAINT),
    );

    let width = ui.available_width();
    let height = 22.0;
    let (rect, _) = ui.allocate_exact_size(vec2(width, height), Sense::hover());

    ui.painter()
        .rect_filled(rect, CornerRadius::ZERO, theme::BG);

    let inner = Rect::from_min_max(rect.min + vec2(6.0, 4.0), rect.max - vec2(6.0, 3.0));
    let resp = ui
        .scope_builder(egui::UiBuilder::new().max_rect(inner), |ui| {
            ui.add(
                egui::TextEdit::singleline(value)
                    .font(theme::mono(theme::MONO_SM))
                    .text_color(theme::TEXT_HI)
                    .frame(egui::Frame::NONE)
                    .desired_width(inner.width())
                    .hint_text(
                        egui::RichText::new(hint)
                            .font(theme::mono(theme::MONO_SM))
                            .color(theme::TEXT_FAINT),
                    ),
            )
        })
        .inner;

    let border = if resp.has_focus() {
        theme::ACCENT
    } else {
        theme::LINE
    };
    ui.painter().rect_stroke(
        rect,
        CornerRadius::ZERO,
        Stroke::new(1.0, border),
        StrokeKind::Inside,
    );

    resp
}
