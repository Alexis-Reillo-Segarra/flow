//! La rejilla de paneles.
//!
//! Todos los agentes están en pantalla a la vez, cada uno en su panel, como las
//! ventanas de un gestor en mosaico. No hay pestañas ni un panel activo que
//! tape a los demás: si tienes cuatro agentes trabajando, ves trabajar a los
//! cuatro. El foco solo decide a quién le habla la barra de entrada.
//!
//! Quién tiene el foco lo dice el marco —degradado de la marca el activo, gris
//! los demás— y, muy de refilón, un 10% de apagado en el contenido de los otros.
//! Nada más: ni fondo propio, ni letra distinta, ni tamaño distinto. Repetir la
//! misma señal cinco veces no la hace más clara, solo deja sin idioma al día que
//! haya algo de verdad que destacar.
//!
//! El reparto no es una rejilla fija de N×M, sino un corte recursivo del
//! rectángulo disponible **por su lado largo**. De ahí sale la única propiedad
//! que se le pide: se adapta sola a la forma de la ventana —maximizada, a media
//! pantalla, en vertical— sin que haya nada que configurar. En horizontal ocho
//! paneles caen en 4×2; estrecha la ventana y esos mismos ocho se reordenan en
//! 2×4 solos.
//!
//! Los paneles no saltan de sitio: cada uno persigue su hueco con un suavizado
//! exponencial. Abrir o cerrar uno reorganiza la rejilla deslizando, que es la
//! diferencia entre entender lo que pasó y encontrarte otra pantalla.

use std::collections::HashMap;

use egui::{
    emath::GuiRounding as _, epaint::StrokeKind, pos2, vec2, Color32, CornerRadius, FontId, Id,
    Painter, Rect, Sense, Stroke, Ui, UiBuilder,
};

use crate::agent::{Agent, State};
use crate::theme;
use crate::ui::{output, widgets, Action, Dir};

/// Alto de la cabecera de cada panel.
const HEADER_H: f32 = 22.0;

/// Constante de tiempo del deslizamiento.
///
/// Un panel recorre el 95% de la distancia en 3τ ≈ 165 ms: se ve el movimiento
/// sin llegar a esperarlo.
const TAU: f32 = 0.055;

/// Cuánto tarda en aparecer del todo un panel recién abierto.
const BORN: f32 = 0.16;

/// Un panel nace a este tamaño respecto del que le toca, y crece hasta el suyo.
const BORN_SCALE: f32 = 0.94;

/// Estado de la rejilla entre frames.
///
/// Vive en `Flow` y no dentro de cada `Agent` a propósito: es información de
/// presentación —dónde está cada panel y cuánto le falta para llegar—, no del
/// proceso. Un agente no tiene por qué saber que lo están dibujando.
#[derive(Default)]
pub struct Tiling {
    panes: HashMap<u64, Pane>,
    /// Dónde acabó cada panel en el último frame. Es lo que permite mover el
    /// foco por dirección: sin geometría, "el de la derecha" no significa nada.
    rects: Vec<(u64, Rect)>,
}

#[derive(Clone, Copy)]
struct Pane {
    rect: Rect,
    /// 0 recién abierto, 1 asentado.
    born: f32,
}

impl Tiling {
    /// El vecino de `from` hacia `dir`, o nada si por ese lado no hay panel.
    pub fn neighbour(&self, from: u64, dir: Dir) -> Option<u64> {
        let origin = self.rects.iter().find(|(id, _)| *id == from)?.1.center();
        self.rects
            .iter()
            .filter(|(id, _)| *id != from)
            .filter_map(|(id, rect)| {
                let c = rect.center();
                let (along, across) = match dir {
                    Dir::Left => (origin.x - c.x, (origin.y - c.y).abs()),
                    Dir::Right => (c.x - origin.x, (origin.y - c.y).abs()),
                    Dir::Up => (origin.y - c.y, (origin.x - c.x).abs()),
                    Dir::Down => (c.y - origin.y, (origin.x - c.x).abs()),
                };
                // Solo cuentan los que de verdad están hacia ese lado. El desvío
                // lateral pesa el doble que la distancia: entre dos candidatos
                // igual de lejos gana el que esté más enfrente.
                (along > 1.0).then_some((*id, along + across * 2.0))
            })
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(id, _)| id)
    }
}

impl Pane {
    fn new(target: Rect) -> Self {
        Self {
            rect: Rect::from_center_size(target.center(), target.size() * BORN_SCALE),
            born: 0.0,
        }
    }

    /// Acerca el panel a su hueco. El suavizado se calcula con el `dt` real, no
    /// con un paso fijo por frame, para que la animación dure lo mismo a 60 Hz
    /// que a 144.
    fn advance(&mut self, target: Rect, dt: f32) {
        let k = 1.0 - (-dt / TAU).exp();
        self.rect = Rect::from_min_max(
            self.rect.min + (target.min - self.rect.min) * k,
            self.rect.max + (target.max - self.rect.max) * k,
        );
        self.born = (self.born + dt / BORN).min(1.0);
    }

    /// ¿Ya llegó? Medio punto de tolerancia: por debajo de eso no se distingue
    /// en pantalla y seguir animando solo gastaría frames.
    fn settled(&self, target: Rect) -> bool {
        self.born >= 1.0
            && (self.rect.min - target.min).length() < 0.5
            && (self.rect.max - target.max).length() < 0.5
    }
}

/// La rejilla cuando no hay ninguna sesión abierta.
pub fn empty(ui: &mut Ui, tiling: &mut Tiling) {
    tiling.panes.clear();
    tiling.rects.clear();
    egui::CentralPanel::default()
        .frame(egui::Frame::new().fill(egui::Color32::TRANSPARENT))
        .show(ui, |ui| {
            widgets::empty_state(ui, "NO SESSIONS", "Ctrl-N para abrir la primera");
        });
}

pub fn show(
    ui: &mut Ui,
    agents: &mut [Agent],
    focused: Option<u64>,
    tiling: &mut Tiling,
    time: f64,
) -> Option<Action> {
    let mut action = None;

    egui::CentralPanel::default()
        .frame(
            egui::Frame::new()
                .fill(egui::Color32::TRANSPARENT)
                .inner_margin(egui::Margin::same(theme::GAP as i8)),
        )
        .show(ui, |ui| {
            if agents.is_empty() {
                return;
            }

            let mut targets = Vec::with_capacity(agents.len());
            split(ui.max_rect(), agents.len(), &mut targets);

            let dt = ui.input(|i| i.stable_dt).clamp(0.0, 0.1);
            let ppp = ui.ctx().pixels_per_point();
            // Un clic en cualquier punto de un panel le da el foco, como al
            // pinchar una ventana. Se mira el puntero crudo en vez de un
            // `Sense::click` sobre el panel porque dentro hay texto
            // seleccionable, y ese se quedaría con el clic antes de llegar aquí.
            //
            // Leer el puntero en crudo se salta el arbitraje de capas de egui,
            // así que hay que rehacerlo a mano: si encima del puntero hay algo
            // en una capa superior —el formulario y su velo—, el clic no es para
            // la rejilla.
            let layer = ui.layer_id();
            let press = ui
                .input(|i| {
                    i.pointer
                        .any_pressed()
                        .then(|| i.pointer.interact_pos())
                        .flatten()
                })
                .filter(|p| ui.ctx().layer_id_at(*p) == Some(layer));

            let mut animating = false;
            tiling.rects.clear();

            for (i, (agent, target)) in agents.iter_mut().zip(targets).enumerate() {
                let pane = {
                    let pane = tiling
                        .panes
                        .entry(agent.id)
                        .or_insert_with(|| Pane::new(target));
                    pane.advance(target, dt);
                    *pane
                };
                let settled = pane.settled(target);
                animating |= !settled;

                let rect = pane.rect.round_to_pixels(ppp);
                tiling.rects.push((agent.id, rect));

                if press.is_some_and(|p| rect.contains(p)) {
                    action = Some(Action::Focus(agent.id));
                }

                let active = focused == Some(agent.id);
                if let Some(a) = panel(ui, agent, i, rect, active, pane.born, settled, time) {
                    action = Some(a);
                }
            }

            // Los paneles de agentes que ya no están se olvidan; si no, el mapa
            // crecería con cada cierre durante toda la sesión.
            tiling
                .panes
                .retain(|id, _| agents.iter().any(|a| a.id == *id));

            if animating {
                ui.ctx().request_repaint();
            }
        });

    action
}

/// Un panel: marco, cabecera y terminal.
#[allow(clippy::too_many_arguments)]
fn panel(
    ui: &mut Ui,
    agent: &mut Agent,
    index: usize,
    rect: Rect,
    focused: bool,
    alpha: f32,
    settled: bool,
    time: f64,
) -> Option<Action> {
    if rect.width() < 40.0 || rect.height() < HEADER_H + 8.0 {
        return None;
    }
    let mut action = None;

    let radius = CornerRadius::same(theme::RADIUS);
    let hovered = ui.rect_contains_pointer(rect);
    let painter = ui.painter().clone();

    // Un panel recién abierto entra entero: marco, cabecera y salida a la vez.
    // Antes solo se atenuaban el marco y su divisoria, así que el contenido
    // aparecía de golpe dentro de un marco todavía traslúcido y la animación se
    // leía como un fallo de dibujado en lugar de como algo naciendo.
    let fade = |c: Color32| c.gamma_multiply(alpha);

    // El contenido de un panel sin foco retrocede un poco. Aquí antes se decía
    // que atenuar la salida de siete procesos para destacar uno salía más caro
    // de leer de lo que valía, y el argumento sigue en pie: por eso el atenuado
    // es de un 10% y no del 50% de un WM de escritorio, y por eso los niveles de
    // texto se subieron para compensarlo —un panel apagado se lee hoy igual que
    // se leía toda la interfaz antes de que esto existiera—.
    //
    // Y solo se apaga la tinta. Marco y divisoria van con `fade` a secas: son el
    // esqueleto del mosaico, y una rejilla con siete bordes medio borrados se
    // deshace. La estructura se queda nítida en los ocho; lo que retrocede es lo
    // que hay dentro.
    let ink =
        |c: Color32| c.gamma_multiply(alpha * if focused { 1.0 } else { theme::DIM_INACTIVE });

    // El halo va lo primero, debajo de todo: despega el panel del fondo. El del
    // panel con foco lleva el verde de la marca —es el mismo halo que la sombra
    // coloreada de la ventana activa en Hyprland—, y el de los demás, un blanco
    // apenas insinuado. No es una señal más: es el borde, que se derrama.
    widgets::panel_halo(
        &painter,
        rect,
        theme::RADIUS,
        fade(if focused {
            theme::pal().halo_focus
        } else {
            theme::pal().halo
        }),
    );

    // Y encima, el negro liso del panel. Es lo que deja el grano del fondo fuera
    // de la ventana: dentro no se lee nunca sobre ruido.
    painter.rect_filled(rect, radius, fade(theme::pal().panel));

    // El marco es lo que dice quién tiene el foco: el degradado de la marca el
    // activo, gris los demás. Con el atenuado de arriba son dos señales, y ahí
    // se acaban — el panel con foco no lleva además fondo propio, ni un nombre
    // de otro color, ni tipografía distinta. Decir lo mismo cinco veces no lo
    // dice más fuerte, solo gasta el idioma para el día que haya algo de verdad
    // que destacar.
    if focused {
        widgets::gradient_border(
            &painter,
            rect,
            theme::RADIUS as f32,
            1.0,
            fade(theme::pal().accent),
            fade(theme::pal().accent_text),
        );
    } else {
        painter.rect_stroke(
            rect,
            radius,
            Stroke::new(1.0, fade(theme::pal().line)),
            StrokeKind::Inside,
        );
    }

    let inner = rect.shrink(1.0);
    let header = Rect::from_min_size(inner.min, vec2(inner.width(), HEADER_H));
    painter.rect_filled(
        Rect::from_min_size(
            pos2(inner.left(), header.bottom().round()),
            vec2(inner.width(), 1.0),
        ),
        CornerRadius::ZERO,
        fade(theme::pal().line),
    );

    let cy = header.center().y;
    let mono = theme::mono(theme::MONO_XS);
    let sans = theme::sans(theme::SANS_SM);

    // De derecha a izquierda: cerrar, tiempo en vida, estado y su marca.
    let mut right = inner.right() - 6.0;
    if hovered {
        let close = Rect::from_center_size(pos2(right - 5.0, cy), vec2(13.0, 13.0));
        let resp = ui.interact(close, Id::new(("pane-close", agent.id)), Sense::click());
        let color = if resp.hovered() {
            theme::pal().red
        } else {
            theme::pal().text_faint
        };
        widgets::draw_cross(&painter, close.center(), 3.0, Stroke::new(1.0, ink(color)));
        resp.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Button, true, "Cerrar el agente")
        });
        if resp.clicked() {
            action = Some(Action::Close(agent.id));
        }
        right -= 17.0;
    }

    // Estás mirando el pasado.
    //
    // Si subes por el scrollback, el proceso sigue escribiendo y hasta ahora
    // nada te lo decía: la vista se quedaba quieta y no había forma de
    // distinguir «este panel está callado» de «este panel lleva cien líneas
    // escribiendo por debajo de donde miras». La flecha lo dice y además lo
    // arregla —es el botón que vuelve al final—, que es lo que evita tener que
    // arrastrar la barra hasta abajo a mano.
    //
    // Se dibuja **siempre** que haga falta y no solo con el ratón encima, al
    // revés que el aspa de cerrar: el aspa es una acción que vas a buscar, y
    // esto es información que tiene que encontrarte a ti.
    //
    // Va sola, sin número de líneas nuevas. El motivo está en `agent::follow`:
    // no hay ninguna cuenta que no mienta, y la flecha dice lo único que se
    // puede saber de verdad.
    if !agent.follow {
        const DICHO: &str = "Volver al final de la salida";
        let zona = Rect::from_center_size(pos2(right - 6.0, cy), vec2(15.0, HEADER_H));
        let resp = ui.interact(zona, Id::new(("pane-follow", agent.id)), Sense::click());
        let color = if resp.hovered() {
            theme::pal().accent_text
        } else {
            theme::pal().text_dim
        };
        widgets::draw_arrow_down(&painter, zona.center(), 4.0, Stroke::new(1.0, ink(color)));
        right -= 19.0;

        // Sin esto no existe para un lector de pantalla, que es justo a quien
        // más falta le hace: no puede ver dónde está la barra de scroll.
        let pulsado = resp.clicked();
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, DICHO));
        resp.on_hover_text(DICHO);
        if pulsado {
            action = Some(Action::FollowEnd(agent.id));
        }
    }

    let uptime = agent.uptime();
    right = right_text(
        &painter,
        right,
        cy,
        &uptime,
        mono.clone(),
        ink(theme::pal().text_faint),
        8.0,
    );
    right = right_text(
        &painter,
        right,
        cy,
        agent.state.label(),
        sans.clone(),
        ink(agent.state.color()),
        6.0,
    );
    let mark = Rect::from_center_size(pos2(right - 3.0, cy), vec2(6.0, 6.0));
    let mark_alpha = alpha * if focused { 1.0 } else { theme::DIM_INACTIVE };
    widgets::paint_mark(&painter, ui.ctx(), mark, &agent.state, time, mark_alpha);
    right -= 12.0;

    // El índice es el mismo que abre el atajo Alt-N: sin él, "salta al tercero"
    // obliga a contar paneles. Va en el gris flojo en todos los paneles, con o
    // sin foco: quién manda ya lo dice el marco, y el acento se reserva para
    // cuando habla flow.
    let left = inner.left() + 6.0;
    painter.text(
        pos2(left, cy),
        egui::Align2::LEFT_CENTER,
        format!("{}", index + 1),
        mono,
        ink(theme::pal().text_faint),
    );
    // La marca del agente, si el panel es uno conocido. Es lo que hace que en
    // una rejilla de ocho se vea de un golpe cuál es el claude y cuál el shell,
    // sin ir leyendo cabeceras.
    let mut name_x = left + 12.0;
    if let Some(glyph) = crate::presets::mark_of(&agent.name) {
        widgets::paint_agent(
            &painter,
            Rect::from_center_size(
                pos2(name_x + widgets::MARK_SIZE * 0.5, cy),
                vec2(widgets::MARK_SIZE, widgets::MARK_SIZE),
            ),
            glyph,
            ink(if focused {
                theme::pal().accent_text
            } else {
                theme::pal().text_dim
            }),
        );
        name_x += widgets::MARK_SIZE + 6.0;
    }
    let name = widgets::fit(
        ui,
        &agent.name.to_uppercase(),
        sans,
        ink(theme::pal().text_hi),
        (right - name_x).max(0.0),
    );
    painter.galley(
        pos2(name_x, cy - name.size().y * 0.5),
        name,
        ink(theme::pal().text_hi),
    );

    // El panel está pintado a mano de arriba abajo, así que para AccessKit no
    // existiría. Se declara entero, con el estado dentro del nombre: un lector
    // de pantalla tiene que poder oír "3, cargo, BLOCKED, 12s".
    let spoken = format!(
        "{}, {}, {}, {}",
        index + 1,
        agent.name,
        agent.state.label(),
        uptime
    );
    // Qué se está ejecutando no cabe en la cabecera de un panel de un cuarto de
    // pantalla, pero tampoco puede desaparecer: va al tooltip. Si el proceso
    // anunció un título por OSC gana al comando, que suele decir mejor en qué
    // anda ahora mismo.
    let detail = agent
        .term()
        .title
        .clone()
        .unwrap_or_else(|| agent.cmdline.clone());
    let head_resp = ui.interact(header, Id::new(("pane", agent.id)), Sense::hover());
    head_resp.widget_info(|| {
        egui::WidgetInfo::selected(
            egui::WidgetType::SelectableLabel,
            true,
            focused,
            spoken.clone(),
        )
    });
    head_resp.on_hover_text(detail);

    let content = Rect::from_min_max(
        pos2(inner.left() + 6.0, header.bottom() + 3.0),
        pos2(inner.right() - 2.0, inner.bottom() - 3.0),
    );
    if content.width() > 20.0 && content.height() > 8.0 {
        // Un fallo al lanzar deja el emulador vacío: sin esto el panel diría
        // FAILED y no habría dónde leer por qué.
        let failure = match &agent.state {
            State::Failed(msg) => Some(msg.clone()),
            _ => None,
        };
        ui.scope_builder(UiBuilder::new().max_rect(content), |ui| {
            ui.shrink_clip_rect(content);
            // La salida no se pinta con el `Painter`, así que no le valen `fade`
            // ni `ink`: se atenúa la capa entera, y con el mismo factor —lo que
            // lleve recorrido de su entrada, por lo que le toque de apagado si
            // no tiene el foco—.
            ui.multiply_opacity(alpha * if focused { 1.0 } else { theme::DIM_INACTIVE });
            match failure {
                Some(msg) => {
                    ui.label(
                        egui::RichText::new(msg)
                            .font(theme::mono(theme::MONO_SM))
                            .color(theme::pal().red),
                    );
                }
                None => output::surface(ui, agent, focused, settled, time),
            }
        });
    }

    action
}

/// Pinta `text` acabando en `x` y devuelve dónde empieza el hueco siguiente.
fn right_text(
    painter: &Painter,
    x: f32,
    y: f32,
    text: &str,
    font: FontId,
    color: Color32,
    gap: f32,
) -> f32 {
    let galley = painter.layout_no_wrap(text.to_owned(), font, color);
    let size = galley.size();
    painter.galley(pos2(x - size.x, y - size.y / 2.0), galley, color);
    x - size.x - gap
}

/// Reparte `rect` entre `n` paneles.
///
/// Cada llamada corta por el lado largo en dos mitades iguales y se reparte los
/// paneles entre ellas, la menor primero. Cortar siempre por el lado largo es
/// lo que hace que el resultado dependa de la forma de la ventana y no de una
/// rejilla escrita a mano: la misma función da 4×2 en horizontal y 2×4 en
/// vertical sin saber nada de pantallas.
///
/// Que la mitad con menos paneles vaya primero deja al agente más antiguo en el
/// hueco grande, que es donde uno espera encontrarlo.
fn split(rect: Rect, n: usize, out: &mut Vec<Rect>) {
    debug_assert!(n > 0);
    if n <= 1 {
        out.push(rect);
        return;
    }
    let head = n / 2;
    if rect.width() >= rect.height() {
        let w = ((rect.width() - theme::GAP) * 0.5).max(1.0);
        split(
            Rect::from_min_size(rect.min, vec2(w, rect.height())),
            head,
            out,
        );
        split(
            Rect::from_min_size(pos2(rect.max.x - w, rect.min.y), vec2(w, rect.height())),
            n - head,
            out,
        );
    } else {
        let h = ((rect.height() - theme::GAP) * 0.5).max(1.0);
        split(
            Rect::from_min_size(rect.min, vec2(rect.width(), h)),
            head,
            out,
        );
        split(
            Rect::from_min_size(pos2(rect.min.x, rect.max.y - h), vec2(rect.width(), h)),
            n - head,
            out,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layout(w: f32, h: f32, n: usize) -> Vec<Rect> {
        let mut out = Vec::new();
        split(
            Rect::from_min_size(egui::Pos2::ZERO, vec2(w, h)),
            n,
            &mut out,
        );
        out
    }

    #[test]
    fn reparte_a_todos_sin_solaparse() {
        for n in 1..=crate::session::MAX_PANES {
            let rects = layout(1200.0, 700.0, n);
            assert_eq!(rects.len(), n, "con {n} paneles salieron {}", rects.len());

            let area = Rect::from_min_size(egui::Pos2::ZERO, vec2(1200.0, 700.0));
            for r in &rects {
                assert!(area.contains_rect(*r), "{r:?} se sale del área");
                assert!(r.width() > 0.0 && r.height() > 0.0);
            }
            for (i, a) in rects.iter().enumerate() {
                for b in &rects[i + 1..] {
                    let overlap = a.intersect(*b);
                    assert!(
                        overlap.width() <= 0.0 || overlap.height() <= 0.0,
                        "{a:?} y {b:?} se solapan"
                    );
                }
            }
        }
    }

    #[test]
    fn se_adapta_a_la_forma_de_la_ventana() {
        // Ocho paneles en horizontal salen en cuatro columnas; los mismos ocho
        // en vertical, en cuatro filas. Es toda la adaptación que hace falta.
        let cols = |rects: &[Rect]| {
            let mut xs: Vec<i32> = rects.iter().map(|r| r.left() as i32).collect();
            xs.sort_unstable();
            xs.dedup();
            xs.len()
        };
        assert_eq!(cols(&layout(1600.0, 900.0, 8)), 4);
        assert_eq!(cols(&layout(700.0, 1400.0, 8)), 2);
    }

    #[test]
    fn el_primero_se_queda_el_hueco_grande() {
        // Con tres paneles, el reparto es 1 + 2: el primero entero a un lado.
        let rects = layout(1200.0, 700.0, 3);
        assert!(rects[0].area() > rects[1].area());
        assert!((rects[1].area() - rects[2].area()).abs() < 1.0);
    }

    #[test]
    fn el_hueco_entre_paneles_es_el_del_tema() {
        let rects = layout(1200.0, 700.0, 2);
        let hueco = rects[1].left() - rects[0].right();
        assert!((hueco - theme::GAP).abs() < 0.01, "hueco de {hueco}");
    }
}

#[cfg(test)]
mod tests_en_pantalla {
    use super::*;
    use crate::testkit::{self, Ventana};

    fn agentes(n: usize) -> Vec<Agent> {
        (0..n)
            .map(|i| testkit::agente(i as u64 + 1, &format!("p{i}"), testkit::quieto()))
            .collect()
    }

    /// Dibuja hasta que la rejilla deja de moverse. El reparto se anima, así que
    /// el rectángulo del primer frame no es el sitio donde acaba un panel.
    fn asienta(v: &mut Ventana, agentes: &mut [Agent], tiling: &mut Tiling, foco: Option<u64>) {
        for _ in 0..60 {
            let t = 0.0;
            v.frame(|ui| show(ui, agentes, foco, tiling, t));
        }
    }

    /// Sin sesiones, la rejilla dice qué hacer y se deja las listas vacías: un
    /// panel que ya no está no puede seguir contando para el foco por dirección.
    #[test]
    fn sin_sesiones_la_rejilla_se_vacia() {
        let mut v = Ventana::nueva();
        let mut t = Tiling::default();
        let mut a = agentes(2);
        asienta(&mut v, &mut a, &mut t, None);
        assert!(!t.rects.is_empty());

        v.frame(|ui| empty(ui, &mut t));
        assert!(t.rects.is_empty(), "la rejilla vacía dejó geometría vieja");
        assert!(t.panes.is_empty());
    }

    /// El foco por dirección necesita geometría: sin haber dibujado, "el de la
    /// derecha" no significa nada.
    #[test]
    fn sin_geometria_no_hay_vecinos() {
        let t = Tiling::default();
        assert_eq!(t.neighbour(1, Dir::Right), None);
    }

    /// Con cuatro paneles en una ventana ancha, cada dirección lleva a un panel
    /// distinto y ninguna se sale por un borde.
    #[test]
    fn las_flechas_llevan_al_panel_de_al_lado() {
        let mut v = Ventana::nueva();
        let mut t = Tiling::default();
        let mut a = agentes(4);
        asienta(&mut v, &mut a, &mut t, Some(1));

        let centro = |id: u64| t.rects.iter().find(|(p, _)| *p == id).unwrap().1.center();
        let (izq, der) = (Dir::Left, Dir::Right);

        // Se busca el panel que esté más a la izquierda y se comprueba que por
        // su izquierda no hay nadie y por su derecha sí. Vale para cualquier
        // reparto, que es lo que hace falta: el reparto cambia con la forma de
        // la ventana.
        let mas_a_la_izquierda = t
            .rects
            .iter()
            .min_by(|x, y| x.1.center().x.total_cmp(&y.1.center().x))
            .unwrap()
            .0;
        assert_eq!(
            t.neighbour(mas_a_la_izquierda, izq),
            None,
            "el panel del borde izquierdo tiene vecino por la izquierda"
        );
        let vecino = t
            .neighbour(mas_a_la_izquierda, der)
            .expect("el del borde izquierdo no tiene a nadie a su derecha");
        assert!(
            centro(vecino).x > centro(mas_a_la_izquierda).x,
            "el vecino de la derecha está a la izquierda"
        );

        // Y lo mismo por arriba y por abajo, que en una rejilla de cuatro
        // existen los dos.
        let arriba = t
            .rects
            .iter()
            .min_by(|x, y| x.1.center().y.total_cmp(&y.1.center().y))
            .unwrap()
            .0;
        assert_eq!(t.neighbour(arriba, Dir::Up), None);
        let abajo = t
            .neighbour(arriba, Dir::Down)
            .expect("el de arriba no tiene a nadie debajo");
        assert!(centro(abajo).y > centro(arriba).y);
    }

    /// El primero se queda el hueco grande: es donde uno espera encontrar al
    /// agente que abrió la sesión.
    #[test]
    fn el_primero_se_queda_el_hueco_grande_de_verdad() {
        let mut v = Ventana::nueva();
        let mut t = Tiling::default();
        let mut a = agentes(3);
        asienta(&mut v, &mut a, &mut t, Some(1));

        let area = |id: u64| {
            let r = t.rects.iter().find(|(p, _)| *p == id).unwrap().1;
            r.width() * r.height()
        };
        assert!(
            area(1) >= area(2) && area(1) >= area(3),
            "el primer panel no se quedó el hueco grande"
        );
    }

    /// De uno a ocho paneles, en tres formas de ventana. Es el reparto entero:
    /// `split` es recursivo y cada número de paneles recorre un camino distinto.
    #[test]
    fn se_reparte_de_uno_a_ocho_en_cualquier_ventana() {
        for (ancho, alto) in [(1480.0, 900.0), (760.0, 460.0), (300.0, 900.0)] {
            for n in 1..=crate::session::MAX_PANES {
                let mut v = Ventana::de(ancho, alto);
                let mut t = Tiling::default();
                let mut a = agentes(n);
                asienta(&mut v, &mut a, &mut t, Some(1));
                assert_eq!(t.rects.len(), n, "se dibujaron {} de {n}", t.rects.len());
            }
        }
    }

    /// Un panel nace más pequeño y crece hasta su hueco: nace en vez de
    /// aparecer. Lo que se comprueba es que llega, porque un suavizado mal
    /// puesto se queda a medio camino para siempre.
    #[test]
    fn un_panel_recien_abierto_crece_hasta_su_hueco() {
        let objetivo = Rect::from_min_size(pos2(0.0, 0.0), vec2(400.0, 300.0));
        let mut p = Pane::new(objetivo);
        assert!(p.rect.width() < objetivo.width(), "no nació más pequeño");
        assert!(!p.settled(objetivo));

        for _ in 0..120 {
            p.advance(objetivo, 1.0 / 60.0);
        }
        assert!(p.settled(objetivo), "el panel no llegó a su hueco");
    }

    /// El aspa de un panel lo cierra, y el clic en el cuerpo le da el foco. Un
    /// clic en cualquier punto de un panel lo enfoca, como al pinchar una
    /// ventana.
    #[test]
    fn el_aspa_cierra_y_el_cuerpo_enfoca() {
        let mut v = Ventana::nueva();
        let mut t = Tiling::default();
        let mut a = agentes(2);
        let segundo = a[1].id;
        asienta(&mut v, &mut a, &mut t, Some(1));

        // El cuerpo del segundo panel: se pincha en su centro, que está lejos de
        // la cabecera y de su aspa.
        let cuerpo = t.rects.iter().find(|(p, _)| *p == segundo).unwrap().1;
        v.clic(cuerpo.center());
        let accion = v.frame(|ui| show(ui, &mut a, Some(1), &mut t, 0.0));
        assert!(
            matches!(accion, Some(Action::Focus(id)) if id == segundo),
            "un clic en el cuerpo del panel no le dio el foco: {accion:?}"
        );

        // El aspa vive en la cabecera y tiene identidad propia, así que se le
        // puede preguntar a egui dónde acabó en vez de adivinarlo.
        let aspa = v
            .ctx()
            .read_response(egui::Id::new(("pane-close", segundo)))
            .expect("el aspa del panel no se dibujó")
            .rect;
        v.clic(aspa.center());
        let accion = v.frame(|ui| show(ui, &mut a, Some(segundo), &mut t, 0.0));
        assert!(
            matches!(accion, Some(Action::Close(id)) if id == segundo),
            "el aspa no cerró el panel: {accion:?}"
        );
    }

    /// Los paneles se dibujan enteros en cada estado, con el foco puesto y sin
    /// él: la cabecera, el estado y el marco cambian con las dos cosas.
    #[test]
    fn un_panel_se_dibuja_en_cada_estado() {
        use crate::agent::State;
        let mut v = Ventana::nueva();
        let mut t = Tiling::default();
        for estado in [
            State::Working,
            State::Blocked,
            State::Idle,
            State::Exited(0),
            State::Exited(3),
            State::Failed("no se pudo lanzar: el programa no existe".to_owned()),
        ] {
            let mut a = vec![testkit::agente_terminado(1, "panel", estado)];
            for foco in [Some(1), None] {
                for k in 0..3 {
                    v.frame(|ui| show(ui, &mut a, foco, &mut t, k as f64 * 0.4));
                }
            }
        }
    }

    /// Un nombre largo se recorta en la cabecera en vez de empujar al estado
    /// fuera del panel.
    #[test]
    fn un_nombre_larguisimo_no_empuja_al_estado() {
        let mut v = Ventana::de(760.0, 460.0);
        let mut t = Tiling::default();
        let mut a = vec![testkit::agente(
            1,
            "un-nombre-absurdamente-largo-que-no-cabe-en-ninguna-cabecera",
            testkit::quieto(),
        )];
        asienta(&mut v, &mut a, &mut t, Some(1));
    }
}

#[cfg(test)]
mod tests_del_aviso_de_scroll {
    use super::*;
    use crate::testkit::{self, Ventana};

    const MARCA: fn(u64) -> egui::Id = |id| egui::Id::new(("pane-follow", id));

    /// Un panel con más salida de la que cabe, ya asentado y pegado al final.
    fn panel_con_mucha_salida(v: &mut Ventana, t: &mut Tiling) -> Vec<Agent> {
        let mut a = vec![testkit::agente(1, "panel", testkit::quieto())];
        for i in 0..400 {
            a[0].feed_para_test(format!("linea {i}\r\n").as_bytes());
        }
        for _ in 0..40 {
            v.frame(|ui| show(ui, &mut a, Some(1), t, 0.0));
        }
        a
    }

    /// La marca solo está cuando hace falta: pegado al final no hay nada que
    /// avisar, y una flecha permanente sería ruido en las ocho cabeceras.
    #[test]
    fn la_marca_no_esta_mientras_sigas_el_final() {
        let mut v = Ventana::nueva();
        let mut t = Tiling::default();
        let mut a = panel_con_mucha_salida(&mut v, &mut t);

        assert!(a[0].follow, "la vista no arrancó pegada al final");
        assert!(
            v.ctx().read_response(MARCA(1)).is_none(),
            "se dibujó el aviso con la vista pegada al final"
        );
        a[0].kill();
    }

    /// Subir por el scrollback despega la vista, y entonces la marca aparece
    /// **sin tener el ratón encima de la cabecera**: el aspa de cerrar es una
    /// acción que vas a buscar, y esto es información que tiene que encontrarte
    /// a ti.
    #[test]
    fn subir_por_el_scrollback_saca_el_aviso() {
        let mut v = Ventana::nueva();
        let mut t = Tiling::default();
        let mut a = panel_con_mucha_salida(&mut v, &mut t);

        // La rueda, en el cuerpo del panel y hacia arriba.
        let cuerpo = t.rects[0].1.center();
        for _ in 0..6 {
            v.rueda(cuerpo, 120.0);
            v.frame(|ui| show(ui, &mut a, Some(1), &mut t, 0.0));
        }
        assert!(!a[0].follow, "subir con la rueda no despegó la vista");

        // Y ahora el puntero se va lejos de la cabecera: la marca se queda.
        for _ in 0..3 {
            v.puntero(egui::pos2(2.0, 2.0));
            v.frame(|ui| show(ui, &mut a, Some(1), &mut t, 0.0));
        }
        assert!(
            v.ctx().read_response(MARCA(1)).is_some(),
            "no se avisó de que la vista está mirando el pasado"
        );
        a[0].kill();
    }

    /// Y la marca es el botón: pulsarla pide volver al final, y volver al final
    /// la hace desaparecer. Es lo que evita tener que arrastrar la barra de
    /// scroll hasta abajo a mano.
    #[test]
    fn pulsar_la_marca_devuelve_la_vista_al_final() {
        let mut v = Ventana::nueva();
        let mut t = Tiling::default();
        let mut a = panel_con_mucha_salida(&mut v, &mut t);

        let cuerpo = t.rects[0].1.center();
        for _ in 0..6 {
            v.rueda(cuerpo, 120.0);
            v.frame(|ui| show(ui, &mut a, Some(1), &mut t, 0.0));
        }
        assert!(!a[0].follow);

        let marca = v
            .ctx()
            .read_response(MARCA(1))
            .expect("no se dibujó el aviso")
            .rect;
        v.clic(marca.center());
        let accion = v.frame(|ui| show(ui, &mut a, Some(1), &mut t, 0.0));
        assert!(
            matches!(accion, Some(Action::FollowEnd(1))),
            "pulsar el aviso no pidió volver al final: {accion:?}"
        );

        // Eso es lo que hace `Flow::apply`; aquí se hace a mano para poder
        // comprobar lo que viene después, que es lo que de verdad importa: que
        // la vista vuelve abajo y el aviso se va solo.
        a[0].follow = true;
        a[0].snap_to_end = true;
        for _ in 0..5 {
            v.frame(|ui| show(ui, &mut a, Some(1), &mut t, 0.0));
        }
        assert!(a[0].follow, "la vista no se quedó pegada al final");
        assert!(
            v.ctx().read_response(MARCA(1)).is_none(),
            "el aviso siguió puesto después de volver al final"
        );
        a[0].kill();
    }

    /// La marca se enciende al pasar por encima, que es la rama de color que no
    /// se recorre sola.
    #[test]
    fn la_marca_se_enciende_al_pasar_por_encima() {
        let mut v = Ventana::nueva();
        let mut t = Tiling::default();
        let mut a = panel_con_mucha_salida(&mut v, &mut t);

        let cuerpo = t.rects[0].1.center();
        for _ in 0..6 {
            v.rueda(cuerpo, 120.0);
            v.frame(|ui| show(ui, &mut a, Some(1), &mut t, 0.0));
        }
        let marca = v
            .ctx()
            .read_response(MARCA(1))
            .expect("no se dibujó el aviso")
            .rect;

        for _ in 0..3 {
            v.puntero(marca.center());
            v.frame(|ui| show(ui, &mut a, Some(1), &mut t, 0.0));
        }
        a[0].kill();
    }
}
