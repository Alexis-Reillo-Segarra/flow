//! Primitivas de dibujo.
//!
//! Todo lo que aquí se pinta a mano —las marcas de estado, la X de cerrar, las
//! divisorias— se dibuja con rectángulos y segmentos en vez de con glifos
//! (`● ◐ ○ ✕`). Un rectángulo se puede clavar en píxel exacto a cualquier
//! escala, y a 6×6 puntos eso es la diferencia entre una marca nítida y una
//! mancha gris. De paso, no depende de que la fuente traiga el símbolo.

use std::sync::Arc;

use egui::{
    epaint::StrokeKind, pos2, text::LayoutJob, vec2, Align2, Color32, CornerRadius, FontId, Galley,
    Mesh, Painter, Pos2, Rect, Response, Sense, Shape, Stroke, Ui, Vec2,
};

use crate::agent::State;
use crate::presets::Mark;
use crate::theme;

/// Marco de esquinas redondeadas con un degradado a lo largo de la diagonal.
///
/// Es lo que marca el panel con foco. Un trazo liso ya decía cuál es el activo;
/// el degradado es lo que lo hace parecer una ventana de Hyprland en vez de un
/// rectángulo seleccionado, y de paso le da al borde una dirección —de la
/// esquina de arriba a la izquierda a la de abajo a la derecha, 45°, como el
/// `col.active_border` por defecto de allí.
///
/// Va a mano y no con `rect_stroke` porque un `Stroke` es de un solo color. El
/// contorno se recorre guardando en cada punto su **normal hacia fuera**, y de
/// ahí sale una tira de triángulos de `width` de grosor con el color
/// interpolado por vértice: el degradado lo hace la GPU al rellenar, así que
/// cuesta lo mismo que el trazo liso que sustituye.
///
/// La tira lleva a cada lado un reborde de un píxel que se desvanece. Sin él
/// las curvas de las esquinas salen en escalera: los trazos de egui se suavizan
/// solos, pero una malla puesta a mano se dibuja con el borde duro, y en el
/// único elemento que dice quién tiene el foco eso se nota.
///
/// `from` y `to` tienen que ser las dos caras del mismo acento. Un degradado
/// entre dos colores distintos serían dos significados en un borde que solo
/// tiene uno que dar: este es el panel que te escucha.
pub fn gradient_border(
    painter: &Painter,
    rect: Rect,
    radius: f32,
    width: f32,
    from: Color32,
    to: Color32,
) {
    let radius = radius
        .min(rect.width() * 0.5)
        .min(rect.height() * 0.5)
        .max(0.0);
    // Un segmento por punto de radio deja la curva lisa a los tamaños que usa
    // flow y no llena la malla de vértices que nadie va a distinguir.
    let steps = (radius.round() as usize).clamp(3, 12);

    // Las cuatro esquinas en sentido horario, cada una con el centro de su arco
    // y el ángulo en el que empieza. Los tramos rectos no hacen falta: el final
    // de un arco y el principio del siguiente son sus dos extremos, y entre
    // ellos la tira de triángulos ya traza la recta.
    let (l, t, r, b) = (rect.left(), rect.top(), rect.right(), rect.bottom());
    let corners = [
        (pos2(r - radius, t + radius), -std::f32::consts::FRAC_PI_2),
        (pos2(r - radius, b - radius), 0.0),
        (pos2(l + radius, b - radius), std::f32::consts::FRAC_PI_2),
        (pos2(l + radius, t + radius), std::f32::consts::PI),
    ];

    let span = rect.width() + rect.height();
    let mix_at = |p: Pos2| {
        let k = if span > 0.0 {
            ((p.x - l) + (p.y - t)) / span
        } else {
            0.0
        };
        mix(from, to, k.clamp(0.0, 1.0))
    };

    // El reborde mide un píxel de pantalla, no un punto: es lo que suaviza, y
    // tiene que medir lo mismo tras el escalado que aplique el sistema.
    let feather = 1.0 / painter.ctx().pixels_per_point();

    // Los cuatro anillos, medidos hacia dentro desde el borde del rectángulo.
    // El desvanecido se reparte medio píxel a cada lado del trazo en vez de
    // sumarle uno entero por banda: así el marco ocupa lo mismo que el trazo de
    // 1 px de un panel sin foco. Repartirlo mal engorda el borde del panel con
    // foco, y entonces el foco se estaría diciendo dos veces —color y grosor—.
    let half = feather * 0.5;
    let ring = [-half, half, (width - half).max(half), width + half];

    let mut mesh = Mesh::default();
    for (center, start) in corners {
        for s in 0..=steps {
            let a = start + std::f32::consts::FRAC_PI_2 * (s as f32 / steps as f32);
            let normal = Vec2::new(a.cos(), a.sin());
            let outer = center + normal * radius;
            let color = mix_at(outer);
            // Los colores van premultiplicados, así que desvanecer es
            // literalmente irse a cero en los cuatro canales.
            for (k, d) in ring.iter().enumerate() {
                let opaque = k == 1 || k == 2;
                mesh.colored_vertex(
                    outer - normal * *d,
                    if opaque { color } else { Color32::TRANSPARENT },
                );
            }
        }
    }

    // Cuatro vértices por punto: el anillo `i` cose con el `i+1` los tres
    // tramos. El último cierra contra el primero, que es lo que evita la muesca
    // en la esquina donde empezamos a recorrer.
    let rings = mesh.vertices.len() / 4;
    for i in 0..rings {
        let a = (i * 4) as u32;
        let b = (((i + 1) % rings) * 4) as u32;
        for k in 0..3 {
            mesh.add_triangle(a + k, a + k + 1, b + k);
            mesh.add_triangle(a + k + 1, b + k + 1, b + k);
        }
    }
    painter.add(Shape::mesh(mesh));
}

/// El halo que despega un panel del fondo.
///
/// Es la sombra de un gestor de ventanas, **invertida**, y hay una razón
/// física para ello: el fondo de flow es negro OLED puro. Una sombra oscurece
/// lo que hay debajo, y sobre `#000000` no queda nada que oscurecer —una sombra
/// negra sobre negro no existe—. Así que la profundidad se da al revés: un halo
/// de luz muy tenue que se desvanece hacia fuera del panel.
///
/// El efecto que busca es el mismo que el de la sombra de Hyprland —que la
/// ventana se lea despegada del escritorio y no recortada sobre él— y para el
/// ojo funciona igual: lo que separa dos superficies es que entre ellas haya un
/// gradiente, dé igual hacia qué lado.
///
/// `color` va con muy poca alfa. Ocho paneles con halo son ocho gradientes en
/// pantalla, y en cuanto se nota que están ahí, la rejilla pasa de verse limpia
/// a verse sucia.
pub fn panel_halo(painter: &Painter, rect: Rect, radius: u8, color: Color32) {
    let halo = egui::epaint::Shadow {
        // Sin desplazamiento: no hay una fuente de luz que justifique una
        // dirección, y un halo descentrado se lee como un error de dibujado.
        offset: [0, 0],
        blur: 18,
        spread: 1,
        color,
    };
    painter.add(halo.as_shape(rect, CornerRadius::same(radius)));
}

/// Interpola dos colores. Se hace a mano sobre los bytes para que el degradado
/// recorra exactamente los dos tonos que le pasan, sin pasar por un espacio
/// intermedio que le metería un tercer color por el camino.
fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let lerp = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgba_premultiplied(
        lerp(a.r(), b.r()),
        lerp(a.g(), b.g()),
        lerp(a.b(), b.b()),
        lerp(a.a(), b.a()),
    )
}

/// Separador horizontal de exactamente 1 px, de borde a borde.
pub fn hline(ui: &mut Ui, color: Color32) {
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(vec2(width, 1.0), Sense::hover());
    ui.painter().rect_filled(rect, CornerRadius::ZERO, color);
}

/// Marca de estado: un cuadrado de 6×6 con un lenguaje visual por estado.
///
/// - `WORKING` late despacio, para que se note movimiento sin distraer.
/// - `BLOCKED` parpadea duro: es el único estado que reclama atención.
/// - `IDLE` va hueco — vivo pero sin actividad.
/// - Los estados terminales son sólidos y quietos.
///
/// Recibe el `Rect` ya calculado porque quien la usa —la cabecera de un panel—
/// coloca su contenido a mano y no pasa por el layout de egui. `alpha` es lo
/// que lleva recorrido de su animación de entrada el panel que la contiene: la
/// marca nace con él, no encima de él.
pub fn paint_mark(
    painter: &egui::Painter,
    ctx: &egui::Context,
    rect: Rect,
    state: &State,
    time: f64,
    alpha: f32,
) {
    let color = state.color().gamma_multiply(alpha);
    let hollow = |c: Color32| {
        painter.rect_stroke(
            rect,
            CornerRadius::ZERO,
            Stroke::new(1.0, c),
            StrokeKind::Inside,
        );
    };

    match state {
        State::Working => {
            // Onda entre 0.45 y 1.0 de opacidad, ciclo de ~1.6 s.
            let phase = ((time * 3.9).sin() as f32 + 1.0) * 0.5;
            painter.rect_filled(
                rect,
                CornerRadius::ZERO,
                color.gamma_multiply(0.45 + 0.55 * phase),
            );
            ctx.request_repaint_after(std::time::Duration::from_millis(50));
        }
        State::Blocked => {
            // Cuadrada, no senoidal: es un aviso, no una respiración.
            //
            // 1,25 Hz. La WCAG 2.3.1 pone el límite en 3 destellos por segundo;
            // se queda muy por debajo a propósito, porque este indicador puede
            // estar parpadeando en pantalla durante minutos. Y nunca es la
            // única señal: al lado va siempre la palabra BLOCKED.
            if (time * 1.25).fract() < 0.55 {
                painter.rect_filled(rect, CornerRadius::ZERO, color);
            } else {
                hollow(color);
            }
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
        State::Idle => hollow(color),
        State::Exited(_) | State::Failed(_) => {
            painter.rect_filled(rect, CornerRadius::ZERO, color);
        }
    }
}

/// Lado de la marca de un agente. Lo bastante grande para que ocho formas
/// distintas se distingan, lo bastante pequeño para caber en la cabecera de un
/// panel de un cuarto de pantalla.
pub const MARK_SIZE: f32 = 10.0;

/// Dibuja la marca de un agente centrada en `rect`.
///
/// Todas se dibujan dentro del mismo círculo imaginario y con el mismo grosor de
/// trazo, para que puestas en columna se lean como una familia y no como un
/// muestrario. Son formas propias, no logotipos ajenos: ver `presets::Mark`.
pub fn paint_agent(painter: &Painter, rect: Rect, mark: Mark, color: Color32) {
    // Centro redondeado al píxel: sin esto, un trazo de 1 px cae a caballo entre
    // dos y se dibuja gris en vez de nítido.
    let c = pos2(rect.center().x.round(), rect.center().y.round());
    let r = (rect.width().min(rect.height()) * 0.5).floor().max(3.0);
    let stroke = Stroke::new(1.0, color);
    let ray = |a: f32, from: f32, to: f32| {
        let d = Vec2::new(a.cos(), a.sin());
        [c + d * (r * from), c + d * (r * to)]
    };

    match mark {
        Mark::Burst => {
            // Ocho radios desde un hueco central: el destello de Anthropic.
            for k in 0..8 {
                let a = std::f32::consts::TAU * k as f32 / 8.0;
                painter.line_segment(ray(a, 0.35, 1.0), stroke);
            }
        }
        Mark::Ring => {
            painter.circle_stroke(c, r * 0.82, stroke);
        }
        Mark::Sparkle => {
            // Cuatro puntas con la cintura estrecha, como el brillo de Gemini.
            // Va de relleno y no de trazo: a este tamaño, el contorno de una
            // punta fina se cierra sobre sí mismo y sale un borrón.
            let mut mesh = Mesh::default();
            for k in 0..4 {
                let a = std::f32::consts::TAU * k as f32 / 4.0;
                let tip = ray(a, 0.0, 1.0)[1];
                let l = ray(a + std::f32::consts::FRAC_PI_4, 0.0, 0.30)[1];
                let n = ray(a - std::f32::consts::FRAC_PI_4, 0.0, 0.30)[1];
                let base = mesh.vertices.len() as u32;
                for p in [c, l, tip, n] {
                    mesh.colored_vertex(p, color);
                }
                mesh.add_triangle(base, base + 1, base + 2);
                mesh.add_triangle(base, base + 2, base + 3);
            }
            painter.add(Shape::mesh(mesh));
        }
        Mark::Brackets => {
            // `[ ]`, la cara de una terminal.
            for side in [-1.0f32, 1.0] {
                let x = c.x + side * r * 0.75;
                let arm = side * -r * 0.4;
                painter.line_segment([pos2(x, c.y - r), pos2(x, c.y + r)], stroke);
                painter.line_segment([pos2(x, c.y - r), pos2(x + arm, c.y - r)], stroke);
                painter.line_segment([pos2(x, c.y + r), pos2(x + arm, c.y + r)], stroke);
            }
        }
        Mark::Chevron => {
            painter.line_segment(
                [pos2(c.x - r * 0.5, c.y - r), pos2(c.x + r * 0.5, c.y)],
                stroke,
            );
            painter.line_segment(
                [pos2(c.x + r * 0.5, c.y), pos2(c.x - r * 0.5, c.y + r)],
                stroke,
            );
        }
        Mark::Square => {
            painter.rect_stroke(
                Rect::from_center_size(c, vec2(r * 1.6, r * 1.6)),
                CornerRadius::ZERO,
                stroke,
                StrokeKind::Inside,
            );
        }
        Mark::Triangle => {
            let p = [
                pos2(c.x, c.y - r),
                pos2(c.x + r, c.y + r * 0.75),
                pos2(c.x - r, c.y + r * 0.75),
            ];
            painter.line_segment([p[0], p[1]], stroke);
            painter.line_segment([p[1], p[2]], stroke);
            painter.line_segment([p[2], p[0]], stroke);
        }
        Mark::Diamond => {
            let p = [
                pos2(c.x, c.y - r),
                pos2(c.x + r, c.y),
                pos2(c.x, c.y + r),
                pos2(c.x - r, c.y),
            ];
            for k in 0..4 {
                painter.line_segment([p[k], p[(k + 1) % 4]], stroke);
            }
        }
        Mark::Bolt => {
            painter.line_segment(
                [pos2(c.x + r * 0.5, c.y - r), pos2(c.x - r * 0.4, c.y)],
                stroke,
            );
            painter.line_segment([pos2(c.x - r * 0.4, c.y), pos2(c.x + r * 0.4, c.y)], stroke);
            painter.line_segment(
                [pos2(c.x + r * 0.4, c.y), pos2(c.x - r * 0.5, c.y + r)],
                stroke,
            );
        }
        Mark::Dot => {
            painter.rect_filled(
                Rect::from_center_size(c, vec2(r * 0.9, r * 0.9)),
                CornerRadius::ZERO,
                color,
            );
        }
    }
}

/// Botón de borde duro. Sin relleno salvo al pasar por encima: el estado de
/// reposo es solo un contorno de 1 px.
///
/// `accent` es el color con el que el botón se enciende, y acaba siendo texto
/// al pasar por encima: quien lo llame tiene que pasar un color que cumpla el
/// contraste de un texto (`ACCENT_TEXT`, no `ACCENT`).
pub fn button(ui: &mut Ui, label: &str, accent: Color32) -> Response {
    labelled_button(ui, label, None, accent)
}

/// El mismo botón, con la marca del agente delante del nombre.
///
/// La marca hace el trabajo que el nombre no puede hacer solo: en una fila de
/// nueve botones que ponen `claude codex gemini opencode…`, todos miden y pesan
/// lo mismo y hay que leerlos uno a uno. Con una forma delante, el que buscas se
/// encuentra sin leer.
pub fn agent_button(ui: &mut Ui, label: &str, mark: Mark, accent: Color32) -> Response {
    labelled_button(ui, label, Some(mark), accent)
}

fn labelled_button(ui: &mut Ui, label: &str, mark: Option<Mark>, accent: Color32) -> Response {
    let font = theme::sans(theme::SANS_SM);
    let galley = ui.painter().layout_no_wrap(label.to_owned(), font, accent);
    let padding = vec2(8.0, 3.0);
    // La marca y el hueco que la separa del nombre. Si no hay marca no ocupa
    // nada, así que un botón sin ella mide exactamente lo que medía antes.
    let glyph = mark.map_or(0.0, |_| MARK_SIZE + 5.0);
    let size = galley.size() + padding * 2.0 + vec2(glyph, 0.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());

    let (bg, border, fg) = if response.is_pointer_button_down_on() {
        (accent.gamma_multiply(0.22), accent, theme::TEXT_HI)
    } else if response.hovered() {
        (theme::HOVER, accent.gamma_multiply(0.75), accent)
    } else {
        (Color32::TRANSPARENT, theme::LINE, theme::TEXT_DIM)
    };

    let painter = ui.painter();
    painter.rect_filled(rect, CornerRadius::ZERO, bg);
    painter.rect_stroke(
        rect,
        CornerRadius::ZERO,
        Stroke::new(1.0, border),
        StrokeKind::Inside,
    );
    if let Some(mark) = mark {
        paint_agent(
            painter,
            Rect::from_center_size(
                pos2(rect.left() + padding.x + MARK_SIZE * 0.5, rect.center().y),
                vec2(MARK_SIZE, MARK_SIZE),
            ),
            mark,
            fg,
        );
    }
    painter.galley(rect.min + padding + vec2(glyph, 0.0), galley, fg);

    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }

    // Sin esto el botón no existe para un lector de pantalla: está pintado a
    // mano, así que hay que declararle a AccessKit qué es y cómo se llama.
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), label)
    });
    response
}

/// Texto en la mono, para contenido (rutas, comandos, cifras).
pub fn mono_label(ui: &mut Ui, text: &str, color: Color32) {
    ui.label(
        egui::RichText::new(text)
            .font(theme::mono(theme::MONO_XS))
            .color(color),
    );
}

/// Una X dibujada con dos segmentos, no el glifo `✕`. La cruz tiene que quedar
/// ópticamente centrada con la raya de minimizar y el cuadrado de maximizar,
/// que también van dibujados; un glifo trae su propio interlineado y se
/// desalinea respecto a los otros dos.
pub fn draw_cross(painter: &egui::Painter, center: egui::Pos2, half: f32, stroke: Stroke) {
    painter.line_segment(
        [
            pos2(center.x - half, center.y - half),
            pos2(center.x + half, center.y + half),
        ],
        stroke,
    );
    painter.line_segment(
        [
            pos2(center.x + half, center.y - half),
            pos2(center.x - half, center.y + half),
        ],
        stroke,
    );
}

/// Compone `text` en una sola línea, recortado con `…` si no cabe en `max`.
///
/// El recorte lo hace el propio maquetador, en una sola pasada. Antes esto
/// probaba a maquetar el texto carácter a carácter hasta que dejaba de caber:
/// para el nombre de un panel eran una veintena de composiciones por panel y
/// por frame —con ocho paneles, más de un centenar— para acabar en el mismo
/// sitio. Ahora es una, y `egui` la sirve de su caché mientras el nombre y el
/// ancho no cambien, que es siempre salvo al redimensionar.
pub fn fit(ui: &Ui, text: &str, font: FontId, color: Color32, max: f32) -> Arc<Galley> {
    let mut job = LayoutJob::simple_singleline(text.to_owned(), font, color);
    job.wrap.max_width = max.max(0.0);
    job.wrap.max_rows = 1;
    // Un nombre de panel puede no tener espacios en el que cortar, así que se
    // parte por donde haga falta; si no, uno largo se dibujaría entero fuera de
    // su hueco.
    job.wrap.break_anywhere = true;
    ui.fonts_mut(|f| f.layout_job(job))
}

/// Mensaje centrado para estados vacíos.
pub fn empty_state(ui: &mut Ui, title: &str, hint: &str) {
    let rect = ui.available_rect_before_wrap();
    let painter = ui.painter();
    let center = rect.center();
    painter.text(
        pos2(center.x, center.y - 10.0),
        Align2::CENTER_CENTER,
        title,
        theme::sans(theme::SANS_MD),
        theme::TEXT_FAINT,
    );
    painter.text(
        pos2(center.x, center.y + 10.0),
        Align2::CENTER_CENTER,
        hint,
        theme::mono(theme::MONO_SM),
        theme::TEXT_FAINT,
    );
}
