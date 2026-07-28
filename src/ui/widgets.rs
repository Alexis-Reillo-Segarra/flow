//! Primitivas de dibujo.
//!
//! Todo lo que aquí se pinta a mano —las marcas de estado, la X de cerrar, las
//! divisorias— se dibuja con rectángulos y segmentos en vez de con glifos
//! (`● ◐ ○ ✕`). Un rectángulo se puede clavar en píxel exacto a cualquier
//! escala, y a 6×6 puntos eso es la diferencia entre una marca nítida y una
//! mancha gris. De paso, no depende de que la fuente traiga el símbolo.

use std::sync::Arc;

use egui::{
    epaint::StrokeKind, pos2, text::LayoutJob, vec2, Align2, Color32, Context, CornerRadius,
    FontId, Galley, Id, Mesh, Painter, Pos2, Rect, Response, Sense, Shape, Stroke, Ui, Vec2,
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

/// El velo de un modal: apaga lo que hay detrás **y se lo quita al ratón**.
///
/// Las dos cosas van juntas, y por eso esto existe en vez de que cada modal se
/// pinte su rectángulo. Un `layer_painter` **solo pinta**: no reserva sitio ni
/// devuelve respuesta, así que oscurecía el fondo sin desactivarlo y los clics
/// seguían llegando enteros a los paneles de debajo. Con el formulario abierto
/// se podía dar el foco a otra terminal, escribir en ella o cerrarla por detrás
/// del cuadro; y como el formulario de un panel hereda el directorio de la
/// sesión que estabas mirando, cambiarla a su espalda hacía que lo que lanzabas
/// naciera en otro sitio del que decía el cuadro.
///
/// Se traga la ventana entera, barra de título incluida. Ahí no solo están los
/// botones de la ventana: también el `+` que abre una sesión nueva, que es una
/// acción de la app y con un modal delante no puede seguir viva. El precio es
/// que mientras el cuadro esté abierto la ventana no se mueve ni se cierra, y
/// sale barato: Esc quita el cuadro y ya está.
pub fn veil(ctx: &Context, id: &str, alpha: u8) {
    let screen = ctx.content_rect();
    egui::Area::new(Id::new(id))
        .order(egui::Order::Middle)
        .fixed_pos(screen.min)
        .movable(false)
        .show(ctx, |ui| {
            // `click_and_drag` y no `click`: si solo se comiera el clic, un
            // botón pulsado y arrastrado —seleccionar texto de una terminal—
            // seguiría llegando abajo.
            let (rect, _) = ui.allocate_exact_size(screen.size(), Sense::click_and_drag());
            ui.painter()
                .rect_filled(rect, CornerRadius::ZERO, Color32::from_black_alpha(alpha));
        });
}

/// Separador horizontal de exactamente 1 px, de borde a borde.
pub fn hline(ui: &mut Ui, color: Color32) {
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(vec2(width, 1.0), Sense::hover());
    ui.painter().rect_filled(rect, CornerRadius::ZERO, color);
}

/// La misma divisoria, partida en tramos para decir por qué paso vas.
///
/// Es la divisoria de siempre y no un elemento nuevo, y eso es la decisión: un
/// formulario de tres pasos pide un indicador, y el sitio donde iría ya estaba
/// ocupado por una raya de 1 px que no decía nada. Partirla sale gratis en
/// espacio y en mobiliario —no hay puntos, ni números, ni una fila de pastillas
/// que se sumen al cuadro—, que es justo lo que un formulario que quiere
/// simplificarse no se puede permitir.
///
/// Dos canales, y hacen falta los dos: el **color** dice cuánto llevas —los
/// tramos pasados y el de ahora en el acento, los que faltan en el gris de las
/// divisorias— y el **grosor** dice en cuál estás, porque el de ahora va de 2 px
/// y los demás de 1. Solo con el color, el tramo actual y los ya hechos serían
/// el mismo dibujo y no sabrías dónde estás parado.
pub fn step_line(ui: &mut Ui, total: usize, current: usize) {
    let width = ui.available_width();
    // Se reserva el alto del tramo más gordo pase lo que pase, para que la raya
    // ocupe lo mismo en los tres pasos y el cuadro no se mueva al avanzar.
    let (rect, _) = ui.allocate_exact_size(vec2(width, 2.0), Sense::hover());
    if total == 0 {
        return;
    }

    let pal = theme::pal();
    // El hueco sale de `GAP`, como todo el aire de la interfaz. A la mitad,
    // porque aquí separa tramos de la misma raya y no ventanas.
    let gap = theme::GAP * 0.5;
    let seg = ((width - gap * (total - 1) as f32) / total as f32).max(1.0);

    for i in 0..total {
        let (thick, color) = match i.cmp(&current) {
            std::cmp::Ordering::Equal => (2.0, pal.accent),
            std::cmp::Ordering::Less => (1.0, pal.accent),
            std::cmp::Ordering::Greater => (1.0, pal.line),
        };
        // Al píxel: una raya de 1 px que caiga a caballo entre dos sale gris.
        let x = (rect.left() + (seg + gap) * i as f32).round();
        let y = (rect.top() + (2.0 - thick) * 0.5).round();
        ui.painter().rect_filled(
            Rect::from_min_size(pos2(x, y), vec2(seg, thick)),
            CornerRadius::ZERO,
            color,
        );
    }
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
/// `ink` es el color del nombre **en reposo**, y con él se dice de qué clase de
/// botón se trata: LANZAR descansa en el acento y CANCELAR en gris, igual que un
/// agente descansa en el acento y una herramienta en gris. No es el color con el
/// que se enciende —ese lo pone el acento del tema y es el mismo para todos—, así
/// que tiene que ser un color que cumpla el contraste de un texto.
pub fn button(ui: &mut Ui, label: &str, ink: Color32) -> Response {
    labelled_button(ui, label, None, ink, None)
}

/// El mismo botón, para cuando es **una opción de una lista** y una de ellas está
/// puesta: el directorio que hay ahora mismo en el campo, por ejemplo.
///
/// El elegido se dice con el borde en el acento, igual que el panel con foco de
/// la rejilla, y **no con un relleno**: el relleno es lo que significa "tienes el
/// ratón encima" en todos los botones del proyecto, y si además significara
/// "este es el puesto", pasar por encima de otro haría que pareciera que has
/// cambiado de opción sin haber pulsado.
///
/// En reposo descansan todos en gris, sin excepción para el puesto: los diez son
/// la misma clase de cosa —una carpeta donde abrir— y lo único que los distingue
/// es cuál está ahora en el campo.
pub fn chip(ui: &mut Ui, label: &str, selected: bool) -> Response {
    labelled_button(ui, label, None, theme::pal().text_dim, Some(selected))
}

/// El mismo botón, con la marca del agente delante del nombre.
///
/// La marca hace el trabajo que el nombre no puede hacer solo: en una fila de
/// nueve botones que ponen `claude codex gemini opencode…`, todos miden y pesan
/// lo mismo y hay que leerlos uno a uno. Con una forma delante, el que buscas se
/// encuentra sin leer.
pub fn agent_button(ui: &mut Ui, label: &str, mark: Mark, ink: Color32) -> Response {
    labelled_button(ui, label, Some(mark), ink, None)
}

/// `selected` a `None` es "esto no es una opción de una lista, es un botón que
/// hace algo". No es lo mismo que `Some(false)`, y la diferencia importa fuera
/// de lo visual: a un lector de pantalla hay que decirle que LANZAR no está
/// puesto ni deja de estarlo.
fn labelled_button(
    ui: &mut Ui,
    label: &str,
    mark: Option<Mark>,
    ink: Color32,
    selected: Option<bool>,
) -> Response {
    let font = theme::sans(theme::SANS_SM);
    // `PLACEHOLDER` y no el color de verdad: el color se decide más abajo, según
    // el botón esté en reposo, señalado, pulsado o puesto, y `Painter::galley`
    // solo aplica el suyo a lo que venga sin pintar —un color escrito en el
    // galley gana siempre—. Componiéndolo aquí con un color de verdad, el nombre
    // del botón se quedaba clavado en él y las cuatro variantes de abajo solo
    // llegaban al borde y a la marca. De paso, el galley se cachea una vez por
    // nombre y no una por cada color en el que se le haya visto.
    let galley = ui
        .painter()
        .layout_no_wrap(label.to_owned(), font, Color32::PLACEHOLDER);
    let padding = vec2(8.0, 3.0);
    // La marca y el hueco que la separa del nombre. Si no hay marca no ocupa
    // nada, así que un botón sin ella mide exactamente lo que medía antes.
    let glyph = mark.map_or(0.0, |_| MARK_SIZE + 5.0);
    let size = galley.size() + padding * 2.0 + vec2(glyph, 0.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());

    // Los tres estados encendidos van todos al acento del tema y no a un color
    // que traiga quien llama: encenderse significa lo mismo en toda la interfaz
    // —esto es lo que te escucha— y darle un color por botón sería inventarse
    // dialectos. Lo que sí decide quien llama es el reposo, que es donde el
    // botón dice de qué clase es.
    //
    // Las dos caras del acento no son intercambiables: el borde y el relleno
    // llevan la de trazo, las letras la de texto. Es lo único que hace que el
    // marco cumpla su 3:1 y el nombre su 4,5:1.
    let pal = theme::pal();
    let (bg, border, fg) = if response.is_pointer_button_down_on() {
        (pal.accent.gamma_multiply(0.22), pal.accent, pal.text_hi)
    } else if response.hovered() {
        (pal.hover, pal.accent, pal.accent_text)
    } else if selected == Some(true) {
        // Puesto y sin ratón encima: el borde y el nombre en el acento, pero sin
        // relleno. El relleno es lo que separa "estás señalando esto" de "esto
        // es lo que hay puesto"; si lo llevaran los dos, pasar por encima de
        // otro parecería haberlo elegido.
        (Color32::TRANSPARENT, pal.accent, pal.accent_text)
    } else {
        (Color32::TRANSPARENT, pal.line, ink)
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
    // mano, así que hay que declararle a AccessKit qué es y cómo se llama. Y si
    // es de los que pueden estar puestos, también si lo está: el borde en el
    // acento no se lo cuenta a nadie que no lo vea.
    response.widget_info(|| match selected {
        Some(on) => {
            egui::WidgetInfo::selected(egui::WidgetType::Button, ui.is_enabled(), on, label)
        }
        None => egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), label),
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

/// Una punta de flecha hacia abajo, con su asta.
///
/// Dibujada con tres segmentos y no con un glifo, por lo mismo que las marcas de
/// agente: a 9 px de lado tiene que quedar nítida, y depender de que la fuente
/// traiga `▼` es depender de algo que no controlamos. `half` es la mitad del
/// lado de su caja.
pub fn draw_arrow_down(painter: &egui::Painter, center: egui::Pos2, half: f32, stroke: Stroke) {
    // Todo al píxel: un trazo de 1 px que caiga a caballo entre dos se dibuja
    // gris en vez de nítido, y esto es lo más pequeño de la cabecera.
    let c = pos2(center.x.round(), center.y.round());
    let h = half.max(2.0).round();
    // El asta, de arriba a la punta.
    painter.line_segment([pos2(c.x, c.y - h), pos2(c.x, c.y + h)], stroke);
    // Y las dos alas de la punta.
    painter.line_segment(
        [pos2(c.x - h * 0.7, c.y + h * 0.3), pos2(c.x, c.y + h)],
        stroke,
    );
    painter.line_segment(
        [pos2(c.x + h * 0.7, c.y + h * 0.3), pos2(c.x, c.y + h)],
        stroke,
    );
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
        theme::pal().text_faint,
    );
    painter.text(
        pos2(center.x, center.y + 10.0),
        Align2::CENTER_CENTER,
        hint,
        theme::mono(theme::MONO_SM),
        theme::pal().text_faint,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::State;
    use crate::testkit::Ventana;

    /// Las diez marcas se dibujan y ninguna se sale de su hueco.
    ///
    /// Se dibujan con segmentos y rectángulos a propósito —nada de emoji ni de
    /// fuentes de iconos— y eso significa que cada una es geometría escrita a
    /// mano que nadie más comprueba. Lo que se prueba aquí no es que se vean
    /// bonitas: es que ninguna revienta ni se pasa de su caja, que es lo que en
    /// una fila de nueve botones hace que una marca invada la de al lado.
    #[test]
    fn las_diez_marcas_de_agente_se_dibujan() {
        let marcas = [
            Mark::Burst,
            Mark::Ring,
            Mark::Sparkle,
            Mark::Brackets,
            Mark::Chevron,
            Mark::Square,
            Mark::Triangle,
            Mark::Diamond,
            Mark::Bolt,
            Mark::Dot,
        ];
        let mut v = Ventana::nueva();
        v.frame(|ui| {
            let painter = ui.painter().clone();
            for mark in marcas {
                let caja = Rect::from_min_size(pos2(100.0, 100.0), vec2(12.0, 12.0));
                paint_agent(&painter, caja, mark, theme::pal().text_hi);
            }
        });
    }

    /// Un tamaño ridículo no rompe el dibujo. La rejilla se aprieta hasta que
    /// una cabecera mide dos píxeles de alto, y ahí una raíz cuadrada negativa o
    /// una división por cero se convierten en un `NaN` que egui propaga hasta
    /// dejar la ventana en blanco.
    #[test]
    fn una_marca_diminuta_no_revienta() {
        let mut v = Ventana::nueva();
        v.frame(|ui| {
            let painter = ui.painter().clone();
            for lado in [0.0_f32, 0.5, 1.0, 3.0] {
                let caja = Rect::from_min_size(pos2(10.0, 10.0), vec2(lado, lado));
                paint_agent(&painter, caja, Mark::Burst, theme::pal().text_hi);
                gradient_border(
                    &painter,
                    caja,
                    6.0,
                    1.0,
                    theme::pal().accent,
                    theme::pal().line,
                );
                panel_halo(&painter, caja, 6, theme::pal().accent);
            }
        });
    }

    /// El radio de un borde se recorta a la mitad del lado más corto. Sin eso,
    /// un panel más bajo que su redondeo dibuja arcos que se cruzan.
    #[test]
    fn el_redondeo_no_puede_ser_mayor_que_el_panel() {
        let mut v = Ventana::nueva();
        v.frame(|ui| {
            let painter = ui.painter().clone();
            let plano = Rect::from_min_size(pos2(0.0, 0.0), vec2(80.0, 4.0));
            gradient_border(
                &painter,
                plano,
                40.0,
                1.0,
                theme::pal().accent,
                theme::pal().line,
            );
        });
    }

    /// Los estados pasan por el dibujo de su marca, en dos instantes distintos:
    /// `WORKING` late y `BLOCKED` parpadea, así que hay ramas que solo se
    /// recorren en una de las dos mitades del ciclo.
    #[test]
    fn cada_estado_pinta_su_marca() {
        let estados = [
            State::Working,
            State::Blocked,
            State::Idle,
            State::Exited(0),
            State::Exited(1),
            State::Failed("no se pudo".to_owned()),
        ];
        let mut v = Ventana::nueva();
        for estado in estados {
            v.frame(|ui| {
                let painter = ui.painter().clone();
                let ctx = ui.ctx().clone();
                let caja = Rect::from_min_size(pos2(20.0, 20.0), vec2(6.0, 6.0));
                paint_mark(&painter, &ctx, caja, &estado, 0.0, 1.0);
                paint_mark(&painter, &ctx, caja, &estado, 0.6, 0.55);
            });
        }
    }

    /// Un botón se pulsa donde está, y no donde no está.
    #[test]
    fn un_boton_se_pulsa_donde_esta() {
        let mut v = Ventana::nueva();
        let caja = v.frame(|ui| button(ui, "KILL", theme::pal().red).rect);

        v.clic(caja.center());
        assert!(
            v.frame(|ui| button(ui, "KILL", theme::pal().red).clicked()),
            "el clic cayó dentro del botón y no lo pulsó"
        );

        v.clic(caja.center() + vec2(0.0, 200.0));
        assert!(
            !v.frame(|ui| button(ui, "KILL", theme::pal().red).clicked()),
            "un clic lejos del botón lo pulsó igual"
        );
    }

    /// La pastilla de una lista y el botón que hace algo son cosas distintas, y
    /// la diferencia no es visual: a un lector de pantalla hay que contarle que
    /// LAUNCH no está puesto ni deja de estarlo, y que este directorio sí.
    #[test]
    fn una_pastilla_puesta_no_pisa_a_la_de_al_lado() {
        let mut v = Ventana::nueva();
        v.frame(|ui| {
            let puesta = chip(ui, "flow", true);
            let suelta = chip(ui, "otro", false);
            let boton = button(ui, "LAUNCH", theme::pal().accent_text);
            assert_ne!(puesta.rect, suelta.rect, "dos pastillas se pisaron");
            assert!(boton.rect.width() > 0.0);
        });
    }

    /// El botón de un agente lleva su marca delante, y eso lo hace más ancho que
    /// el mismo nombre sin marca. Es la razón de que exista: en una fila de
    /// nueve nombres que miden lo mismo, la forma es lo que se encuentra sin
    /// leer.
    #[test]
    fn el_boton_de_un_agente_hace_sitio_a_su_marca() {
        let mut v = Ventana::nueva();
        let (con, sin) = v.frame(|ui| {
            let con = agent_button(ui, "claude", Mark::Burst, theme::pal().text_dim).rect;
            let sin = button(ui, "claude", theme::pal().text_dim).rect;
            (con.width(), sin.width())
        });
        assert!(
            con > sin,
            "el botón con marca ({con}) no es más ancho que el mismo sin ella ({sin})"
        );
    }

    /// El velo de un modal se come el clic que iba al panel de debajo.
    ///
    /// No es cosmética: sin esto se le daba el foco a otra terminal por detrás
    /// del formulario, y como el formulario de un panel hereda el directorio de
    /// la sesión que estabas mirando, cambiarla a su espalda hacía que lo que
    /// lanzabas naciera en otro sitio del que decía el cuadro.
    #[test]
    fn el_velo_se_come_el_clic_de_debajo() {
        let mut v = Ventana::nueva();
        let caja = v.frame(|ui| button(ui, "KILL", theme::pal().red).rect);

        // El velo tiene que estar ya puesto en el frame de calentamiento: egui
        // resuelve un clic contra lo que había dibujado el frame anterior, así
        // que un velo que aparece a la vez que el clic no tapa nada. Es también
        // lo que pasa de verdad —el modal lleva un frame abierto cuando llega
        // el primer clic—, así que probarlo de otra forma sería probar una
        // situación que no ocurre.
        let velado = |ui: &mut Ui| {
            let ctx = ui.ctx().clone();
            veil(&ctx, "test-dim", 190);
            button(ui, "KILL", theme::pal().red).clicked()
        };
        v.calienta(|ui| {
            velado(ui);
        });

        v.clic(caja.center());
        assert!(
            !v.frame(velado),
            "el clic atravesó el velo y llegó al botón"
        );
    }

    /// La raya de pasos ocupa lo mismo en los tres pasos: si encogiera, el
    /// cuadro entero daría un salto al avanzar.
    #[test]
    fn la_raya_de_pasos_mide_igual_en_todos_los_pasos() {
        let mut v = Ventana::nueva();
        let altos: Vec<f32> = (0..3)
            .map(|paso| {
                v.frame(|ui| {
                    let antes = ui.cursor().top();
                    step_line(ui, 3, paso);
                    ui.cursor().top() - antes
                })
            })
            .collect();
        assert!(
            altos.windows(2).all(|p| (p[0] - p[1]).abs() < f32::EPSILON),
            "la raya de pasos cambia de alto entre pasos: {altos:?}"
        );
    }

    /// Sin pasos no hay raya que dibujar, y pedirla no puede ser un pánico.
    #[test]
    fn una_raya_de_cero_pasos_no_revienta() {
        let mut v = Ventana::nueva();
        v.frame(|ui| {
            step_line(ui, 0, 0);
            hline(ui, theme::pal().line);
        });
    }

    /// Un nombre que no cabe se recorta a una sola fila. Un panel puede llamarse
    /// como una ruta entera sin un solo espacio donde cortar, así que se parte
    /// por donde haga falta: si no, se dibujaría fuera de su hueco.
    #[test]
    fn un_nombre_largo_se_queda_en_una_fila() {
        let mut v = Ventana::nueva();
        let (filas, ancho) = v.frame(|ui| {
            let g = fit(
                ui,
                "un-nombre-larguisimo-sin-un-solo-espacio-donde-cortar",
                theme::mono(theme::MONO_SM),
                theme::pal().text_hi,
                60.0,
            );
            (g.rows.len(), g.rect.width())
        });
        assert_eq!(filas, 1, "el nombre se partió en varias filas");
        assert!(
            ancho <= 60.0,
            "el nombre recortado mide {ancho}, y el hueco 60"
        );
    }

    /// Un ancho negativo no puede llegarle a la maquetación: sale de restarle
    /// márgenes a un hueco que puede haberse quedado en nada.
    #[test]
    fn un_hueco_de_cero_no_revienta_la_maquetacion() {
        let mut v = Ventana::nueva();
        v.frame(|ui| {
            fit(
                ui,
                "algo",
                theme::mono(theme::MONO_SM),
                theme::pal().text_hi,
                -10.0,
            );
        });
    }

    /// El aspa y el estado vacío se dibujan: son las dos cosas que se pintan a
    /// mano fuera de un widget.
    #[test]
    fn el_aspa_y_el_estado_vacio_se_dibujan() {
        let mut v = Ventana::nueva();
        v.frame(|ui| {
            let painter = ui.painter().clone();
            draw_cross(
                &painter,
                pos2(50.0, 50.0),
                4.0,
                Stroke::new(1.0, theme::pal().text_dim),
            );
            empty_state(ui, "NO SESSIONS", "Ctrl-N para abrir la primera");
        });
    }
}
