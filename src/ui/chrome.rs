//! Barra superior propia y bordes de redimensionado.
//!
//! La ventana va sin decoración del sistema, así que aquí se reimplementa lo
//! que da el SO: arrastrar, maximizar con doble clic, los tres botones y ocho
//! zonas de resize en los bordes. A cambio, la barra superior es parte del
//! diseño —lleva la marca, la tira de agentes y la cuenta— en lugar de un
//! injerto gris de Windows por encima.

use egui::{
    epaint::StrokeKind, pos2, vec2, Align, Color32, Context, CornerRadius, CursorIcon, Id, Layout,
    Rect, ResizeDirection, Response, Sense, Stroke, Ui, ViewportCommand,
};

use crate::session::{Session, MAX_PANES};
use crate::theme;
use crate::ui::{spawn, widgets, Action};

pub const TITLEBAR_H: f32 = 34.0;
/// Anchura de la zona sensible de los bordes, en puntos lógicos.
const GRIP: f32 = 5.0;

#[derive(Clone, Copy, PartialEq)]
enum Btn {
    Minimize,
    Maximize,
    Close,
}

/// Dibuja la barra superior: la marca, el proyecto de la sesión que se está
/// mirando, el `+` que le añade un panel y los botones de ventana.
///
/// Las sesiones ya no viven aquí: se bajaron a la columna de la izquierda, que
/// es donde tienen sitio para crecer. Lo que queda arriba es lo que habla de la
/// **ventana** y de la sesión actual, no de la lista.
pub fn titlebar(ui: &mut Ui, current: Option<&Session>) -> Option<Action> {
    let mut action = None;
    let ctx = ui.ctx().clone();
    let full = current.is_some_and(|s| s.is_full());

    egui::Panel::top("titlebar")
        .exact_size(TITLEBAR_H)
        .resizable(false)
        .show_separator_line(false)
        .frame(
            egui::Frame::new()
                .fill(egui::Color32::TRANSPARENT)
                .inner_margin(egui::Margin::symmetric(theme::GAP as i8 + 2, 0)),
        )
        .show(ui, |ui| {
            // La zona de arrastre se registra primero y en toda la franja; lo
            // que se dibuje encima se queda sus propios clics.
            let barra = ui.max_rect();
            let drag = ui.interact(barra, Id::new("titlebar-drag"), Sense::click_and_drag());
            if drag.drag_started_by(egui::PointerButton::Primary) {
                ctx.send_viewport_cmd(ViewportCommand::StartDrag);
            }
            if drag.double_clicked() {
                let maximized = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
                ctx.send_viewport_cmd(ViewportCommand::Maximized(!maximized));
            }

            ui.horizontal_centered(|ui| {
                ui.spacing_mut().item_spacing.x = 8.0;

                // Solo el logotipo: ya dice cómo se llama la app, y repetirlo en
                // texto al lado es ruido.
                mark(ui, 16.0);

                // El proyecto sobre el que trabaja la sesión que estás mirando.
                // Ocupa el sitio que dejó la tira de sesiones al bajarse a la
                // columna, y responde a la pregunta que se hace uno al volver a
                // la app: de todo lo que hay abierto, ¿dónde estoy? Va en mono
                // porque es una ruta.
                if let Some(session) = current {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    ui.label(
                        egui::RichText::new(crate::projects::name_of(&session.cwd))
                            .font(theme::sans(theme::SANS_SM))
                            .color(theme::pal().text_dim),
                    );
                    ui.label(
                        egui::RichText::new(&session.cwd)
                            .font(theme::mono(theme::MONO_XS))
                            .color(theme::pal().text_faint),
                    )
                    .on_hover_text("el directorio que comparten los paneles de esta sesión");
                }

                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;
                    if window_button(ui, Btn::Close).clicked() {
                        ctx.send_viewport_cmd(ViewportCommand::Close);
                    }
                    if window_button(ui, Btn::Maximize).clicked() {
                        let maximized = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
                        ctx.send_viewport_cmd(ViewportCommand::Maximized(!maximized));
                    }
                    if window_button(ui, Btn::Minimize).clicked() {
                        ctx.send_viewport_cmd(ViewportCommand::Minimized(true));
                    }

                    ui.add_space(10.0);
                    // Cuántos paneles tiene la sesión de los que le caben. Dice
                    // de paso que el límite existe, sin tener que chocarse con
                    // él, y al lado va el `+` que los abre: los dos hablan de la
                    // sesión que estás mirando, no de la app.
                    if let Some(session) = current {
                        ui.label(
                            egui::RichText::new(format!("{}/{}", session.panes.len(), MAX_PANES))
                                .font(theme::mono(theme::MONO_XS))
                                .color(if full {
                                    theme::pal().text_dim
                                } else {
                                    theme::pal().text_faint
                                }),
                        )
                        .on_hover_text(if full {
                            "esta sesión está llena: cierra un panel"
                        } else {
                            "paneles en esta sesión"
                        });

                        ui.add_space(6.0);
                        if !full
                            && widgets::button(ui, "+", theme::pal().text_dim)
                                .on_hover_text(
                                    "Añadir una terminal o un agente a esta sesión (Ctrl-T)",
                                )
                                .clicked()
                        {
                            action = Some(Action::OpenSpawn(spawn::Kind::Pane));
                        }
                    }
                });
            });
        });

    action
}

/// La marca de flow en la barra de título.
///
/// Se rasteriza al número exacto de píxeles físicos que va a ocupar y se sube
/// con filtrado `NEAREST`: a 16 pt las barras miden dos o tres píxeles y el
/// hueco entre ellas uno, así que escalar una imagen para llegar a ese tamaño
/// convertiría la marca en una mancha gris. La textura se cachea por tamaño y
/// por color, así que solo se rasteriza al arrancar, al cambiar la escala del
/// sistema y al cambiar de tema.
///
/// La marca **no es cuadrada**, y el ancho lo dice `logo::caja()` en vez de
/// suponerse: la marca cambia de proporciones con el tamaño —a 16 px engorda la
/// barra y abre la separación— así que darle una caja cuadrada, o la de otro
/// tamaño, le corta la última barra.
fn mark(ui: &mut Ui, alto_pt: f32) {
    let ctx = ui.ctx().clone();
    let ppp = ctx.pixels_per_point();
    // La cara clara del acento y no `accent`, que es la de trazo. La marca ya
    // trae dentro su propia caída —la barra más apagada se queda en un tercio de
    // la tinta— así que arrancarla en la cara oscura la apagaría dos veces y las
    // dos últimas barras desaparecerían sobre el negro. Es además el sitio que
    // le da el sistema de color: el blanco puro es de la marca (ver `theme`).
    let ink = theme::pal().accent_text;
    let alto_px = (alto_pt * ppp).round().max(8.0) as usize;
    let (ancho_px, alto_px) = crate::logo::caja(alto_px);
    let id = Id::new(("logo", alto_px, ink.to_array()));

    let texture = match ctx.data(|d| d.get_temp::<egui::TextureHandle>(id)) {
        Some(t) => t,
        None => {
            // Fuera de `data_mut`: cargar una textura vuelve a tomar el
            // cerrojo del contexto y se quedaría bloqueado.
            let t = ctx.load_texture(
                format!("flow-mark-{alto_px}"),
                crate::logo::color_image(ancho_px, alto_px, ink),
                egui::TextureOptions::NEAREST,
            );
            ctx.data_mut(|d| d.insert_temp(id, t.clone()));
            t
        }
    };

    // La caja en puntos sale de la de píxeles y no al revés: así el sitio
    // reservado es exactamente el de la textura y la marca no se reescala.
    let caja_pt = vec2(ancho_px as f32 / ppp, alto_px as f32 / ppp);
    let (rect, resp) = ui.allocate_exact_size(caja_pt, Sense::hover());
    ui.painter().image(
        texture.id(),
        rect,
        Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
        Color32::WHITE,
    );
    // La barra de título no dice "flow" en ningún sitio: lo dice la marca, y una
    // imagen dibujada a mano no existe para un lector de pantalla si no se le
    // declara. Es lo único que nombra la aplicación.
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Image, true, "flow"));
}

fn window_button(ui: &mut Ui, kind: Btn) -> Response {
    let size = vec2(34.0, TITLEBAR_H);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());

    let hover_bg = if kind == Btn::Close {
        theme::pal().red.gamma_multiply(0.85)
    } else {
        theme::pal().sel
    };
    if resp.hovered() {
        ui.painter().rect_filled(rect, CornerRadius::ZERO, hover_bg);
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }

    let fg = if resp.hovered() {
        if kind == Btn::Close {
            Color32::WHITE
        } else {
            theme::pal().text_hi
        }
    } else {
        theme::pal().text_dim
    };
    let stroke = Stroke::new(1.0, fg);
    // Centro redondeado al píxel: si cae en medio, las líneas de 1 px se ven
    // grises en vez de nítidas.
    let c = pos2(rect.center().x.round(), rect.center().y.round());
    let painter = ui.painter();

    match kind {
        Btn::Minimize => {
            painter.rect_filled(
                Rect::from_min_size(pos2(c.x - 4.0, c.y), vec2(8.0, 1.0)),
                CornerRadius::ZERO,
                fg,
            );
        }
        Btn::Maximize => {
            painter.rect_stroke(
                Rect::from_center_size(c, vec2(8.0, 8.0)),
                CornerRadius::ZERO,
                stroke,
                StrokeKind::Inside,
            );
        }
        Btn::Close => widgets::draw_cross(painter, c, 4.0, stroke),
    }

    // Los glifos van dibujados con líneas, no con texto, así que el nombre hay
    // que dárselo explícitamente a AccessKit.
    let label = match kind {
        Btn::Minimize => "Minimizar",
        Btn::Maximize => "Maximizar",
        Btn::Close => "Cerrar",
    };
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label));

    resp
}

/// Ocho zonas invisibles en los bordes para redimensionar.
///
/// Sin decoración del sistema winit no hace el hit-testing por nosotros, así
/// que hay que declararlas a mano. Van en una capa por encima de todo para que
/// ganen a cualquier widget que llegue hasta el borde.
pub fn resize_handles(ctx: &Context) {
    if ctx.input(|i| i.viewport().maximized.unwrap_or(false)) {
        return; // Maximizada no se redimensiona por los bordes.
    }
    let screen = ctx.content_rect();

    let (l, r, t, b) = (screen.left(), screen.right(), screen.top(), screen.bottom());
    let zones: [(&str, Rect, ResizeDirection, CursorIcon); 8] = [
        (
            "n",
            Rect::from_min_max(pos2(l + GRIP, t), pos2(r - GRIP, t + GRIP)),
            ResizeDirection::North,
            CursorIcon::ResizeNorth,
        ),
        (
            "s",
            Rect::from_min_max(pos2(l + GRIP, b - GRIP), pos2(r - GRIP, b)),
            ResizeDirection::South,
            CursorIcon::ResizeSouth,
        ),
        (
            "w",
            Rect::from_min_max(pos2(l, t + GRIP), pos2(l + GRIP, b - GRIP)),
            ResizeDirection::West,
            CursorIcon::ResizeWest,
        ),
        (
            "e",
            Rect::from_min_max(pos2(r - GRIP, t + GRIP), pos2(r, b - GRIP)),
            ResizeDirection::East,
            CursorIcon::ResizeEast,
        ),
        (
            "nw",
            Rect::from_min_max(pos2(l, t), pos2(l + GRIP, t + GRIP)),
            ResizeDirection::NorthWest,
            CursorIcon::ResizeNorthWest,
        ),
        (
            "ne",
            Rect::from_min_max(pos2(r - GRIP, t), pos2(r, t + GRIP)),
            ResizeDirection::NorthEast,
            CursorIcon::ResizeNorthEast,
        ),
        (
            "sw",
            Rect::from_min_max(pos2(l, b - GRIP), pos2(l + GRIP, b)),
            ResizeDirection::SouthWest,
            CursorIcon::ResizeSouthWest,
        ),
        (
            "se",
            Rect::from_min_max(pos2(r - GRIP, b - GRIP), pos2(r, b)),
            ResizeDirection::SouthEast,
            CursorIcon::ResizeSouthEast,
        ),
    ];

    for (name, rect, dir, cursor) in zones {
        if rect.width() <= 0.0 || rect.height() <= 0.0 {
            continue;
        }
        egui::Area::new(Id::new(("resize", name)))
            .order(egui::Order::Foreground)
            .fixed_pos(rect.min)
            .interactable(true)
            .show(ctx, |ui| {
                let resp = ui.allocate_response(rect.size(), Sense::drag());
                if resp.hovered() || resp.dragged() {
                    ui.ctx().set_cursor_icon(cursor);
                }
                if resp.drag_started() {
                    ui.ctx()
                        .send_viewport_cmd(ViewportCommand::BeginResize(dir));
                }
            });
    }
}

/// Borde exterior de 1 px. Sin él la ventana sin decoración se funde con lo que
/// tenga detrás en un escritorio oscuro. Va a esquina viva aunque los paneles
/// vayan redondeados: es el corte de la ventana, no una superficie flotante.
pub fn window_border(ctx: &Context) {
    ctx.layer_painter(egui::LayerId::new(
        egui::Order::Foreground,
        Id::new("window-border"),
    ))
    .rect_stroke(
        ctx.content_rect(),
        CornerRadius::ZERO,
        Stroke::new(1.0, theme::pal().line_hi),
        StrokeKind::Inside,
    );
}
