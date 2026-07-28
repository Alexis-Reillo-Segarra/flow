//! La columna de sesiones, a la izquierda.
//!
//! Hace dos cosas: **saltar a una sesión** y **decir de un vistazo cuál reclama
//! atención**. Es la única vista que hay de las sesiones que no estás mirando,
//! así que cada fila lleva a su izquierda una barra con el estado resumido de
//! todos sus paneles: si un `cargo test` revienta en la sesión tres, se ve desde
//! la uno.
//!
//! Estuvo arriba, como una tira de pastillas horizontal, y se bajó aquí porque
//! el sitio que tiene una sesión para crecer es hacia abajo: en horizontal, seis
//! sesiones ya obligaban a quedarse solo con el número, y el nombre es lo único
//! que distingue una de otra. En vertical caben con su nombre las que quieras.
//!
//! No es una barra lateral de navegación: no tiene pestañas, ni iconos de
//! sección, ni se pliega en un menú. Es la columna de espacios de trabajo de un
//! gestor en mosaico —una fila por sesión, el estado a la izquierda— y cuando la
//! ventana se estrecha se queda en los números, que son los mismos del atajo
//! `Ctrl-1`…`9`.
//!
//! **Aquí no se cierra nada.** Hubo una X que salía al pasar por encima, clavada
//! en la esquina de la pastilla: se salía por arriba invadiendo la barra de
//! título, quedaba cortada, y ponía «matar todos los procesos de esta sesión» a
//! un píxel del clic con el que se cambia de sesión. Cerrar es destructivo y no
//! tiene deshacer, así que va por atajo —`Ctrl-Shift-W`— donde hace falta
//! intención para llegar y no basta con que se te vaya el ratón.

use egui::{
    pos2, vec2, Align, Color32, Context, CornerRadius, CursorIcon, Layout, Rect, Sense, Ui,
};

use crate::agent::State;
use crate::session::Session;
use crate::theme;
use crate::ui::{spawn, widgets, Action};

/// Ancho de la columna con los nombres.
const W: f32 = 148.0;
/// Ancho cuando solo caben los números.
const W_NARROW: f32 = 32.0;
/// Por debajo de este ancho de ventana, la columna se queda en los números. Lo
/// que se protege es el terminal: en una ventana estrecha, 148 puntos de nombres
/// de sesión son columnas que la salida de un CLI ya no tiene.
const TIGHT: f32 = 820.0;

/// Alto de una fila.
const ROW_H: f32 = 26.0;
/// Ancho de la barra de estado que lleva cada fila a su izquierda.
const TINT_W: f32 = 2.0;

/// El ancho que va a ocupar la columna. Lo necesita quien reparte la ventana.
pub fn width(ctx: &Context) -> f32 {
    if ctx.content_rect().width() < TIGHT {
        W_NARROW
    } else {
        W
    }
}

pub fn sidebar(
    ui: &mut Ui,
    sessions: &[Session],
    current: Option<u64>,
    time: f64,
) -> Option<Action> {
    let mut action = None;
    let w = width(ui.ctx());
    let narrow = w <= W_NARROW;

    egui::Panel::left("sessions")
        .exact_size(w)
        .resizable(false)
        // La divisoria de 1 px del canto derecho es la que separa la columna de
        // la rejilla: aquí no hay relleno distinto que lo haga, como en el resto
        // de la interfaz. La pinta egui con `noninteractive.bg_stroke`, que el
        // tema ya tiene puesto en `LINE`, y además la dibuja sobre el `Ui` padre
        // —fuera del recorte del panel—, que es justo lo que hace falta para que
        // una línea pegada al borde no se quede a medias.
        .show_separator_line(true)
        .frame(
            egui::Frame::new()
                .fill(egui::Color32::TRANSPARENT)
                .inner_margin(egui::Margin {
                    left: theme::GAP as i8,
                    right: theme::GAP as i8,
                    // Arriba, el mismo hueco que le da la rejilla a su primer
                    // panel: así la primera fila arranca a la altura de la
                    // primera cabecera en vez de un pelo por encima.
                    top: theme::GAP as i8,
                    bottom: theme::GAP as i8,
                }),
        )
        .show(ui, |ui| {
            // El `+` se coloca primero, de abajo arriba, para que se quede
            // clavado al pie pase lo que pase con el número de sesiones: con
            // muchas, la lista scrollea y el botón sigue donde estaba.
            ui.with_layout(Layout::bottom_up(Align::Min), |ui| {
                let label = if narrow { "+" } else { "+  SESIÓN" };
                if widgets::button(ui, label, theme::pal().accent_text)
                    .on_hover_text("Abrir una sesión nueva (Ctrl-N)")
                    .clicked()
                {
                    action = Some(Action::OpenSpawn(spawn::Kind::Session));
                }
                ui.add_space(theme::GAP);

                ui.with_layout(Layout::top_down(Align::Min), |ui| {
                    egui::ScrollArea::vertical()
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            ui.spacing_mut().item_spacing.y = 2.0;
                            for (i, session) in sessions.iter().enumerate() {
                                if let Some(a) = row(ui, i, session, current, time, narrow) {
                                    action = Some(a);
                                }
                            }
                        });
                });
            });
        });

    action
}

/// Una fila: barra de estado, número y nombre.
fn row(
    ui: &mut Ui,
    index: usize,
    session: &Session,
    current: Option<u64>,
    time: f64,
    narrow: bool,
) -> Option<Action> {
    let active = current == Some(session.id);
    let state = session.state();

    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), ROW_H), Sense::click());
    let painter = ui.painter();

    // Esquina viva: el redondeo está reservado a los paneles de la rejilla, para
    // que signifique una cosa concreta —esto es una ventana—. Una fila de una
    // lista no lo es.
    if active {
        painter.rect_filled(rect, CornerRadius::ZERO, theme::pal().sel);
    } else if resp.hovered() {
        painter.rect_filled(rect, CornerRadius::ZERO, theme::pal().hover);
    }

    // El estado, en una barra vertical pegada al canto izquierdo. En horizontal
    // esto era un subrayado; en vertical, el canto es el lado que comparten
    // todas las filas y por tanto donde se leen de un vistazo, sin ir buscando.
    painter.rect_filled(
        Rect::from_min_size(
            pos2(rect.left(), rect.top() + 3.0),
            vec2(TINT_W, rect.height() - 6.0),
        ),
        CornerRadius::ZERO,
        tint(ui.ctx(), &state, time),
    );

    let cy = rect.center().y;
    let mono = theme::mono(theme::MONO_XS);
    let num = format!("{}", index + 1);

    if narrow {
        painter.text(
            pos2(rect.center().x + TINT_W * 0.5, cy),
            egui::Align2::CENTER_CENTER,
            &num,
            mono,
            if active {
                theme::pal().text_hi
            } else {
                theme::pal().text_dim
            },
        );
    } else {
        let x = rect.left() + TINT_W + 7.0;
        painter.text(
            pos2(x, cy),
            egui::Align2::LEFT_CENTER,
            &num,
            mono,
            theme::pal().text_faint,
        );
        let name_x = x + 13.0;
        let sans = theme::sans(theme::SANS_SM);
        let color = if active {
            theme::pal().text_hi
        } else {
            theme::pal().text_dim
        };
        let galley = widgets::fit(
            ui,
            &session.name.to_uppercase(),
            sans,
            color,
            (rect.right() - name_x - 4.0).max(0.0),
        );
        ui.painter()
            .galley(pos2(name_x, cy - galley.size().y * 0.5), galley, color);
    }

    if resp.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    let spoken = format!(
        "sesión {}, {}, {} paneles",
        session.name,
        state.label(),
        session.panes.len()
    );
    resp.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::Button, true, active, spoken.clone())
    });
    let clicked = resp.clicked();
    resp.on_hover_text(format!(
        "{} · {} de {} vivos\n{}",
        state.label(),
        session.running(),
        session.panes.len(),
        session.cwd
    ));

    clicked.then_some(Action::Switch(session.id))
}

/// El color de la barra de estado. `BLOCKED` parpadea a 1,25 Hz —el mismo ritmo
/// que su marca de estado, muy por debajo del límite de 3 destellos/s de la
/// WCAG— y nunca baja a cero: apagarse del todo haría desaparecer la fila.
fn tint(ctx: &Context, state: &State, time: f64) -> Color32 {
    let color = state.color();
    if *state == State::Blocked {
        ctx.request_repaint_after(std::time::Duration::from_millis(100));
        if (time * 1.25).fract() >= 0.55 {
            return color.gamma_multiply(0.25);
        }
    }
    color
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testkit::{self, Ventana};

    fn sesiones(n: usize) -> Vec<Session> {
        (0..n)
            .map(|i| {
                let id = i as u64 + 1;
                testkit::sesion(
                    id,
                    &format!("sesion-{id}"),
                    vec![testkit::agente(id * 10, "panel", testkit::quieto())],
                )
            })
            .collect()
    }

    /// En una ventana estrecha la columna se queda en los números. Lo que se
    /// protege es el terminal: 148 puntos de nombres de sesión son columnas que
    /// la salida de un CLI ya no tiene.
    #[test]
    fn en_una_ventana_estrecha_la_columna_se_queda_en_los_numeros() {
        // Se pregunta dentro de un frame: fuera de uno el contexto todavía no
        // sabe de qué tamaño es la ventana, y contestaría por el valor de
        // fábrica en vez de por el de verdad.
        let mut ancha = Ventana::de(1480.0, 900.0);
        let mut estrecha = Ventana::de(700.0, 460.0);
        assert_eq!(ancha.frame_ctx(width), W);
        assert_eq!(estrecha.frame_ctx(width), W_NARROW);
        const { assert!(W_NARROW < W) };
    }

    /// Pinchar una pastilla cambia de sesión, y cambia a **esa**: es el fallo
    /// que un test de índices coge y una mirada no.
    #[test]
    fn pinchar_una_pastilla_cambia_a_esa_sesion() {
        let mut v = Ventana::nueva();
        let s = sesiones(3);

        v.calienta(|ui| {
            sidebar(ui, &s, Some(s[0].id), 0.0);
        });

        // Las filas se apilan desde arriba con el hueco del tema por delante.
        // Se recorre la columna de arriba abajo apuntando el orden en que
        // aparecen las sesiones, que es lo que de verdad importa.
        let mut vistas: Vec<u64> = Vec::new();
        for k in 0..8 {
            let y = theme::GAP + ROW_H * (k as f32 + 0.5) + 2.0 * k as f32;
            v.clic(egui::pos2(W / 2.0, y));
            if let Some(Action::Switch(id)) = v.frame(|ui| sidebar(ui, &s, Some(s[0].id), 0.0)) {
                if vistas.last() != Some(&id) {
                    vistas.push(id);
                }
            }
        }
        assert_eq!(
            vistas,
            vec![s[0].id, s[1].id, s[2].id],
            "las pastillas no cambian a la sesión que se pincha, o no en ese orden"
        );
    }

    /// El `+` del pie abre una sesión, y se queda clavado abajo pase lo que pase
    /// con el número de sesiones: con muchas, la lista scrollea y el botón sigue
    /// donde estaba.
    #[test]
    fn el_boton_del_pie_abre_una_sesion() {
        for n in [0, 1, 12] {
            let mut v = Ventana::nueva();
            let s = sesiones(n);
            v.calienta(|ui| {
                sidebar(ui, &s, s.first().map(|x| x.id), 0.0);
            });

            let alto = v.rect().height();
            let mut abrio = false;
            for k in 0..40 {
                let y = alto - theme::GAP - k as f32;
                v.clic(egui::pos2(W / 2.0, y));
                if let Some(Action::OpenSpawn(spawn::Kind::Session)) =
                    v.frame(|ui| sidebar(ui, &s, s.first().map(|x| x.id), 0.0))
                {
                    abrio = true;
                    break;
                }
            }
            assert!(abrio, "con {n} sesiones no se encontró el + del pie");
        }
    }

    /// La columna resume el estado de cada sesión, y lo hace en las dos anchuras
    /// y con sesiones en cualquier estado: es lo que permite que un `cargo test`
    /// que revienta en la tres se vea desde la uno.
    #[test]
    fn la_columna_se_dibuja_con_sesiones_de_todo_tipo() {
        use crate::agent::State;
        let estados = [
            State::Working,
            State::Blocked,
            State::Idle,
            State::Exited(0),
            State::Exited(1),
            State::Failed("no arrancó".to_owned()),
        ];
        let mut s: Vec<Session> = estados
            .iter()
            .enumerate()
            .map(|(i, e)| {
                let id = i as u64 + 1;
                testkit::sesion(
                    id,
                    "una-sesion-con-un-nombre-larguisimo-que-no-cabe",
                    vec![testkit::agente_terminado(id * 10, "panel", e.clone())],
                )
            })
            .collect();
        s[0].panes
            .push(testkit::agente_terminado(99, "otro", State::Blocked));

        for (ancho, alto) in [(1480.0, 900.0), (700.0, 460.0)] {
            let mut v = Ventana::de(ancho, alto);
            for k in 0..3 {
                v.frame(|ui| sidebar(ui, &s, Some(2), k as f64 * 0.4));
            }
            // Y sin ninguna sesión puesta: al cerrar la última no hay actual.
            v.frame(|ui| sidebar(ui, &s, None, 0.0));
        }
    }

    /// Sin sesiones la columna sigue estando, con su botón y sin filas.
    #[test]
    fn sin_sesiones_la_columna_sigue_ahi() {
        let mut v = Ventana::nueva();
        v.frame(|ui| sidebar(ui, &[], None, 0.0));
    }
}
