//! Paleta, tipografía y estilo global.
//!
//! # Color
//!
//! Tres colores llevan el peso: **negro puro** de fondo, **gris** en las líneas
//! que separan las zonas y **verde** para la marca, el foco y el cursor. No hay
//! rellenos de color ni tarjetas grises: la estructura de la interfaz se lee
//! por las divisorias de 1 px, no por bloques de tono.
//!
//! Aparte de esos tres, hay cuatro colores que solo hablan de estado —verde,
//! ámbar, rojo, gris— y nunca se usan de decoración. Si ves color en flow,
//! significa algo.
//!
//! # Tipografía
//!
//! Dos familias, cada una con un trabajo:
//!
//! - **Inter** — nombres, estados, botones, rótulos y etiquetas. Un grotesco
//!   neutro, dibujado para leerse en pantalla a cuerpos pequeños. No aporta
//!   carácter, y eso es exactamente lo que se le pide: el carácter lo pone la
//!   rejilla, no la letra.
//! - **JetBrains Mono** — todo lo que sale de un proceso, más rutas, comandos
//!   y cifras. Monoespaciada porque el terminal lo exige.
//!
//! Las dos son de contorno, así que no hay retícula de píxeles que respetar: la
//! interfaz se dibuja al tamaño que le pida el sistema y la escala no tiene que
//! ser un número entero. Aquí hubo antes dos fuentes de pixel-art que sí lo
//! exigían, y de ahí venía la mitad de las reglas raras que ya no están.
//!
//! # Espacio
//!
//! La rejilla de paneles se lee por el **hueco**, no por marcos: `GAP` separa
//! los paneles entre sí y del borde de la ventana, y es lo que hace que la
//! pantalla parezca un tiling WM en vez de una tabla.

use std::sync::Arc;

use egui::{
    Color32, Context, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Stroke, Style,
    TextStyle,
};

// ─── Superficies ──────────────────────────────────────────────────────────────

/// Negro OLED. Es el fondo de todo: paneles, barras y terminal.
///
/// Negro exacto, no un gris muy oscuro: en un panel OLED, `#000000` es el píxel
/// **apagado**, y eso es un negro que ninguna otra pantalla sabe dar. Subirlo
/// aunque fuera un punto —para que se vieran sombras, por ejemplo— lo
/// encendería entero y se perdería justo lo que lo hace bonito. La profundidad
/// se consigue por otro lado: ver `HALO`.
pub const BG: Color32 = Color32::BLACK;
/// Igual que `BG`. Existe como concepto aparte para que quede claro que las
/// barras no se distinguen por relleno, sino por su línea divisoria. Los paneles
/// de la rejilla sí se rellenan con él, y no por color: es lo que deja el grano
/// del fondo fuera de la ventana.
pub const PANEL: Color32 = Color32::BLACK;
/// Campos de texto y cajas: apenas un susurro por encima del negro.
pub const RAISED: Color32 = Color32::from_rgb(0x0a, 0x0a, 0x0c);
/// Fila seleccionada.
pub const SEL: Color32 = Color32::from_rgb(0x14, 0x16, 0x1a);
/// Hover.
pub const HOVER: Color32 = Color32::from_rgb(0x0d, 0x0e, 0x11);

/// Las divisorias de 1 px. Son el esqueleto visible de la interfaz.
///
/// 2,07:1 contra el negro. Se queda por debajo del 3:1 de la WCAG a propósito:
/// una divisoria es decoración, no transmite estado ni información, y subirla
/// más convertiría la rejilla en el elemento más ruidoso de la pantalla. Todo
/// lo que sí informa —texto, estados, foco— sí cumple AA.
pub const LINE: Color32 = Color32::from_rgb(0x3c, 0x42, 0x4b);
/// Divisoria con más presencia: borde exterior y campos con foco. 3,40:1.
pub const LINE_HI: Color32 = Color32::from_rgb(0x5a, 0x62, 0x6d);

// ─── Espacio ──────────────────────────────────────────────────────────────────

/// Hueco entre paneles y contra el borde de la ventana.
///
/// Es la única unidad de aire de la interfaz: el mismo valor separa un panel
/// del de al lado y del marco. Un hueco uniforme es lo que hace que la rejilla
/// se lea como ventanas en mosaico y no como celdas de una tabla.
///
/// Va hacia los 20 de Hyprland, pero no llega ni de lejos: aquí caben ocho
/// paneles en pantalla y el hueco que en un WM de escritorio es holgura, aquí
/// se come columnas de terminal. Con cuatro columnas, cada punto de `GAP` son
/// cinco puntos de ancho que el terminal no ve.
pub const GAP: f32 = 8.0;

/// Redondeo de las esquinas de un panel.
///
/// Seis píxeles: suficiente para que el panel se lea como una ventana suelta
/// sobre el fondo y no como la celda de una tabla, que es toda la diferencia
/// entre esto y un dashboard. Todo lo que va **dentro** sigue con esquina viva
/// —botones, campos, marcas de estado— para que el redondeo signifique una cosa
/// concreta: esto es una ventana. Si lo llevara todo, no distinguiría nada.
pub const RADIUS: u8 = 6;

/// El halo que despega un panel del fondo, y el del panel con foco.
///
/// Son las dos únicas superficies de la interfaz que no significan nada: no
/// dicen estado, no son texto y no hay que poder leerlas. Por eso van con una
/// alfa tan baja —se notan sin verse— y por eso son lo único de la paleta que no
/// pasa por los tests de contraste.
///
/// Existen porque sobre `BG` negro OLED una sombra no puede existir: oscurecer
/// el `#000000` no da nada. La profundidad se da al revés, con luz que se
/// desvanece hacia fuera. Ver `ui::widgets::panel_halo`.
pub const HALO: Color32 = Color32::from_rgba_premultiplied(0x0e, 0x0f, 0x12, 0x1c);
/// El del panel con foco lleva el verde de la marca, como la sombra coloreada
/// de la ventana activa en Hyprland. No es una señal nueva: es el borde, que se
/// derrama un poco más allá de su trazo.
pub const HALO_FOCUS: Color32 = Color32::from_rgba_premultiplied(0x0a, 0x2c, 0x21, 0x30);

/// Cuánto se apaga un panel que no tiene el foco.
///
/// Es el `dim_inactive` de Hyprland, pero muy contenido. La versión agresiva de
/// un WM de escritorio aquí rompería el producto: los ocho paneles están en
/// pantalla porque la promesa de flow es que los ves trabajar a todos a la vez,
/// y siete apagados de verdad son siete que ya no puedes leer.
///
/// Solo se apaga el **contenido** —los rótulos de la cabecera y la salida del
/// proceso—, nunca las líneas. El marco y la divisoria son el esqueleto del
/// mosaico y se quedan enteros: la estructura se lee igual de nítida en los
/// ocho, lo que retrocede es lo que hay dentro.
///
/// El 0,90 no está elegido a ojo. Es el punto donde un panel apagado se queda
/// exactamente en el contraste que tenía toda la interfaz antes de que esto
/// existiera: los niveles de texto se subieron para pagarlo, así que el
/// atenuado no oscurece los siete, **aclara el que tiene el foco**.
pub const DIM_INACTIVE: f32 = 0.90;

// ─── Texto ────────────────────────────────────────────────────────────────────
//
// Los cuatro niveles cumplen WCAG AA (4,5:1) contra el negro **y también
// atenuados**, que es el caso real peor de la interfaz: el nivel más tenue de
// un panel sin foco. El nivel más flojo existe para dar jerarquía, no para
// esconder texto: si algo se puede leer, tiene que poder leerse.
//
// Los dos niveles de abajo se subieron al añadir `DIM_INACTIVE` porque
// atenuados ya no llegaban. Al hacerlo el escalón entre ellos quedó además más
// parejo que antes: ×1,45 · ×1,62 · ×1,63.

/// 12,87:1 — atenuado, 10,40:1.
pub const TEXT: Color32 = Color32::from_rgb(0xc6, 0xcb, 0xd2);
/// 21:1 — atenuado, 16,83:1.
pub const TEXT_HI: Color32 = Color32::WHITE;
/// 7,93:1 — atenuado, 6,50:1.
pub const TEXT_DIM: Color32 = Color32::from_rgb(0x97, 0xa0, 0xab);
/// 5,48:1 — el mínimo de la interfaz, y el que fija el techo de `DIM_INACTIVE`:
/// atenuado se queda en 4,58:1, a un pelo del 4,5:1 de la WCAG.
pub const TEXT_FAINT: Color32 = Color32::from_rgb(0x7b, 0x83, 0x8e);

// ─── Marca ────────────────────────────────────────────────────────────────────

/// Verde flow. El color de la marca: logo, foco y selección.
///
/// No se usa nunca para estados. Si algo lleva el verde de la marca es porque
/// es flow hablando, no un agente.
///
/// 4,41:1 contra el negro. Cumple el 3:1 que la WCAG pide a un componente de
/// interfaz —un marco, un borde, un relleno— pero se queda por debajo del 4,5:1
/// de un texto, así que **solo se usa como superficie o trazo**. Cuando el
/// acento tiene que ser una letra, va `ACCENT_TEXT`.
pub const ACCENT: Color32 = Color32::from_rgb(0x1e, 0x82, 0x5f);

/// La misma marca, aclarada hasta 10,50:1, para cuando el acento es texto.
///
/// Mismo tono (159°) y misma saturación que `ACCENT`: se lee como el mismo
/// color, no como otro. Es el que llevan los rótulos, los glifos finos y el
/// bloque del cursor, que tiene texto negro encima y por tanto también responde
/// al mínimo de un texto.
///
/// También es el extremo claro del degradado del panel con foco. El degradado
/// va de una cara del acento a la otra en vez de estrenar un tercer verde: son
/// los dos tonos que ya existen, comparten tono, y los dos pasan de sobra el
/// 3:1 que la WCAG le pide a un trazo, así que el borde cumple **en todo su
/// recorrido** y no solo en un extremo.
pub const ACCENT_TEXT: Color32 = Color32::from_rgb(0x30, 0xcf, 0x97);

// ─── Estados ──────────────────────────────────────────────────────────────────

/// Trabajando (latiendo) y terminado con éxito (sólido).
pub const GREEN: Color32 = Color32::from_rgb(0x6e, 0xe7, 0x87);
/// Bloqueado esperando respuesta. El único estado que reclama atención.
pub const AMBER: Color32 = Color32::from_rgb(0xf0, 0xb4, 0x5c);
/// Terminado con error o imposible de lanzar.
pub const RED: Color32 = Color32::from_rgb(0xf2, 0x69, 0x6e);
/// Vivo pero sin actividad. 5,70:1 — es un color de estado, así que también
/// tiene que cumplir AA aunque su papel sea el de "no me mires".
pub const SLATE: Color32 = Color32::from_rgb(0x7c, 0x86, 0x95);

// ─── Paleta ANSI ──────────────────────────────────────────────────────────────

/// Los 16 colores ANSI, armonizados con la paleta para que el output de los
/// procesos no desentone con el chrome de la app.
///
/// Los que tienen nombre propio en la paleta lo reutilizan, pero solo cuando el
/// nombre coincide: el cian de aquí es un cian de verdad y no el color de la
/// marca. Un proceso que pide cian espera cian, y atarlo al acento significaría
/// que cambiar la marca repinta la salida ajena.
pub const ANSI: [Color32; 16] = [
    Color32::from_rgb(0x14, 0x16, 0x1a), // 0 negro
    RED,                                 // 1 rojo
    GREEN,                               // 2 verde
    AMBER,                               // 3 amarillo
    Color32::from_rgb(0x6f, 0xa8, 0xf5), // 4 azul
    Color32::from_rgb(0xc7, 0x92, 0xea), // 5 magenta
    Color32::from_rgb(0x35, 0xd6, 0xf5), // 6 cian
    TEXT,                                // 7 blanco
    Color32::from_rgb(0x45, 0x4a, 0x52), // 8 negro brillante
    Color32::from_rgb(0xff, 0x8b, 0x90), // 9 rojo brillante
    Color32::from_rgb(0x95, 0xff, 0xaa), // 10 verde brillante
    Color32::from_rgb(0xff, 0xd0, 0x82), // 11 amarillo brillante
    Color32::from_rgb(0x92, 0xc2, 0xff), // 12 azul brillante
    Color32::from_rgb(0xdd, 0xb0, 0xff), // 13 magenta brillante
    Color32::from_rgb(0x8d, 0xe9, 0xff), // 14 cian brillante
    Color32::WHITE,                      // 15 blanco brillante
];

/// Resuelve un índice de la paleta 256 a color.
pub fn ansi256(i: u8) -> Color32 {
    match i {
        0..=15 => ANSI[i as usize],
        16..=231 => {
            // Cubo 6×6×6.
            let i = i - 16;
            let level = |v: u8| if v == 0 { 0u8 } else { 55 + v * 40 };
            Color32::from_rgb(level(i / 36), level((i / 6) % 6), level(i % 6))
        }
        232..=255 => {
            // Rampa de 24 grises.
            let v = 8 + (i - 232) * 10;
            Color32::from_rgb(v, v, v)
        }
    }
}

// ─── Tipografía ───────────────────────────────────────────────────────────────

const SANS_FAMILY: &str = "sans";

/// Inter. El cuerpo de toda la interfaz.
pub const SANS_SM: f32 = 12.5;
/// Inter para rótulos: títulos del formulario y estados vacíos.
pub const SANS_MD: f32 = 13.5;

/// JetBrains Mono. La salida de los procesos.
///
/// 13 puntos es el tamaño al que se lee un editor. Al no haber ya retícula de
/// píxeles, no hay ningún múltiplo que respetar: manda que se lea.
pub const MONO_SM: f32 = 13.0;
/// La misma, más pequeña, para las cifras del chrome: tiempos en vida,
/// contadores, rutas. Son datos de apoyo y compiten con el nombre que llevan al
/// lado si van al mismo cuerpo.
pub const MONO_XS: f32 = 11.5;

/// Sans: nombres, estados, botones y etiquetas.
pub fn sans(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(SANS_FAMILY.into()))
}

/// Mono: salida de procesos, rutas, comandos y cifras.
pub fn mono(size: f32) -> FontId {
    FontId::new(size, FontFamily::Monospace)
}

/// Carga las fuentes embebidas y arma las dos familias.
///
/// El orden de cada familia es una cadena de respaldo: lo que a una le falte se
/// busca en la siguiente. Las dos traen Latin-1 completo, así que en la práctica
/// solo entra en juego con símbolos raros y emoji, que caen en los respaldos de
/// egui.
fn font_definitions() -> FontDefinitions {
    let mut fonts = FontDefinitions::default();

    let mut embed = |name: &str, bytes: &'static [u8]| {
        fonts
            .font_data
            .insert(name.to_owned(), Arc::new(FontData::from_static(bytes)));
    };
    embed("inter", include_bytes!("../assets/fonts/Inter-Regular.ttf"));
    embed(
        "jetbrains",
        include_bytes!("../assets/fonts/JetBrainsMono-Regular.ttf"),
    );

    // Respaldos que ya trae egui, para emoji y demás rarezas.
    let fallbacks: Vec<String> = ["Ubuntu-Light", "NotoEmoji-Regular", "emoji-icon-font"]
        .iter()
        .filter(|n| fonts.font_data.contains_key(**n))
        .map(|n| (*n).to_owned())
        .collect();

    let chain = |head: &[&str]| -> Vec<String> {
        head.iter()
            .map(|s| (*s).to_owned())
            .chain(fallbacks.iter().cloned())
            .collect()
    };

    fonts.families.insert(
        FontFamily::Name(SANS_FAMILY.into()),
        chain(&["inter", "jetbrains"]),
    );
    fonts
        .families
        .insert(FontFamily::Monospace, chain(&["jetbrains", "inter"]));
    fonts
        .families
        .insert(FontFamily::Proportional, chain(&["inter", "jetbrains"]));

    fonts
}

/// Estilo global: esquinas a 0, bordes de 1 px, cero relleno decorativo.
fn style() -> Style {
    let mut style = Style {
        text_styles: [
            (TextStyle::Heading, sans(SANS_MD)),
            (TextStyle::Body, sans(SANS_SM)),
            (TextStyle::Button, sans(SANS_SM)),
            (TextStyle::Small, mono(MONO_SM)),
            (TextStyle::Monospace, mono(MONO_SM)),
        ]
        .into(),
        ..Style::default()
    };

    let v = &mut style.visuals;
    v.dark_mode = true;
    v.panel_fill = PANEL;
    v.window_fill = BG;
    v.extreme_bg_color = BG;
    v.faint_bg_color = RAISED;
    v.code_bg_color = RAISED;
    v.override_text_color = Some(TEXT);
    v.window_stroke = Stroke::new(1.0, LINE);
    v.window_corner_radius = CornerRadius::ZERO;
    v.menu_corner_radius = CornerRadius::ZERO;
    v.selection.bg_fill = SEL;
    v.selection.stroke = Stroke::new(1.0, ACCENT);
    // Un enlace es texto, así que le toca la variante clara.
    v.hyperlink_color = ACCENT_TEXT;

    // Nada de sombras: aquí la profundidad la da la divisoria, no el desenfoque.
    // Una sombra bajo cada panel convertiría la rejilla en una pila de tarjetas,
    // que es justo lo que no es.
    v.window_shadow = egui::epaint::Shadow::NONE;
    v.popup_shadow = egui::epaint::Shadow::NONE;

    for w in [
        &mut v.widgets.noninteractive,
        &mut v.widgets.inactive,
        &mut v.widgets.hovered,
        &mut v.widgets.active,
        &mut v.widgets.open,
    ] {
        w.corner_radius = CornerRadius::ZERO;
        w.expansion = 0.0;
        w.bg_stroke = Stroke::new(1.0, LINE);
        w.fg_stroke = Stroke::new(1.0, TEXT);
    }
    v.widgets.noninteractive.bg_fill = PANEL;
    v.widgets.noninteractive.weak_bg_fill = PANEL;
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, TEXT_DIM);
    v.widgets.inactive.bg_fill = RAISED;
    v.widgets.inactive.weak_bg_fill = RAISED;
    v.widgets.hovered.bg_fill = HOVER;
    v.widgets.hovered.weak_bg_fill = HOVER;
    v.widgets.hovered.bg_stroke = Stroke::new(1.0, LINE_HI);
    v.widgets.hovered.fg_stroke = Stroke::new(1.0, TEXT_HI);
    v.widgets.active.bg_fill = SEL;
    v.widgets.active.weak_bg_fill = SEL;
    v.widgets.active.bg_stroke = Stroke::new(1.0, ACCENT);
    v.widgets.active.fg_stroke = Stroke::new(1.0, TEXT_HI);

    let s = &mut style.spacing;
    s.item_spacing = egui::vec2(6.0, 4.0);
    s.button_padding = egui::vec2(8.0, 4.0);
    s.window_margin = egui::Margin::ZERO;
    // La barra de scroll, en reposo, es una línea de 2 px al 25%: con ocho
    // terminales en pantalla, ocho barras sólidas serían el elemento más
    // llamativo de la interfaz. Se engorda y se enciende sola al acercarte.
    // `floating_allocated_width` le reserva su carril para que aun así nunca
    // llegue a taparle una columna al terminal.
    s.scroll.floating = true;
    s.scroll.floating_width = 2.0;
    s.scroll.floating_allocated_width = 4.0;
    s.scroll.bar_width = 6.0;
    s.scroll.bar_inner_margin = 2.0;
    s.scroll.bar_outer_margin = 0.0;
    s.scroll.dormant_handle_opacity = 0.25;
    s.scroll.active_handle_opacity = 0.5;
    s.scroll.interact_handle_opacity = 0.9;
    s.scroll.dormant_background_opacity = 0.0;
    s.scroll.active_background_opacity = 0.0;
    s.scroll.interact_background_opacity = 0.5;
    s.interact_size.y = 18.0;

    style
}

/// Instala fuentes y estilo en el contexto. Se llama una sola vez al arrancar.
pub fn install(ctx: &Context) {
    ctx.set_fonts(font_definitions());
    // El mismo estilo en ambos temas: flow es oscuro y punto. Así da igual que
    // el sistema esté en claro o que el usuario lo cambie en caliente.
    let style = Arc::new(style());
    ctx.set_style_of(egui::Theme::Dark, style.clone());
    ctx.set_style_of(egui::Theme::Light, style);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Contraste WCAG 2.1 contra el negro puro, que es el fondo de todo.
    ///
    /// Con `BG` negro la fórmula se queda en `(L + 0,05) / 0,05`, así que basta
    /// con la luminancia relativa del color de delante.
    fn vs_black(c: Color32) -> f64 {
        let channel = |v: u8| {
            let v = v as f64 / 255.0;
            if v <= 0.04045 {
                v / 12.92
            } else {
                ((v + 0.055) / 1.055).powf(2.4)
            }
        };
        let l = 0.2126 * channel(c.r()) + 0.7152 * channel(c.g()) + 0.0722 * channel(c.b());
        (l + 0.05) / 0.05
    }

    /// Todo lo que es texto en la interfaz. La lista es la que recorren los dos
    /// tests de contraste: un color de texto que no esté aquí es un color que
    /// dentro de dos meses no cumple, porque nadie lo comprueba.
    const TEXTOS: [(&str, Color32); 9] = [
        ("TEXT", TEXT),
        ("TEXT_HI", TEXT_HI),
        ("TEXT_DIM", TEXT_DIM),
        ("TEXT_FAINT", TEXT_FAINT),
        ("ACCENT_TEXT", ACCENT_TEXT),
        ("GREEN", GREEN),
        ("AMBER", AMBER),
        ("RED", RED),
        ("SLATE", SLATE),
    ];

    #[test]
    fn todo_lo_que_es_texto_cumple_aa() {
        // 4,5:1 es el mínimo de la WCAG para texto normal. La promesa del README
        // es que **todo** lo legible lo cumple, incluido el nivel más tenue: el
        // gris flojo existe para dar jerarquía, no para esconder texto.
        for (nombre, color) in TEXTOS {
            let ratio = vs_black(color);
            assert!(
                ratio >= 4.5,
                "{nombre} se queda en {ratio:.2}:1, y es texto"
            );
        }
    }

    #[test]
    fn el_texto_de_un_panel_apagado_tambien_cumple_aa() {
        // Este es el caso real peor de la interfaz, y el que hay que pagar por
        // tener `DIM_INACTIVE`: con siete paneles apagados, su nivel más tenue
        // es el texto más flojo que llega a estar en pantalla. Si este test se
        // salta, atenuar deja de ser una decisión de diseño y pasa a ser texto
        // que no se puede leer.
        for (nombre, color) in TEXTOS {
            let ratio = vs_black(color.gamma_multiply(DIM_INACTIVE));
            assert!(
                ratio >= 4.5,
                "{nombre} apagado se queda en {ratio:.2}:1 (entero va a {:.2}:1)",
                vs_black(color)
            );
        }
    }

    // Clippy avisa de que la condición es constante, y tiene razón: eso es
    // justo lo que se busca. El test no comprueba un cálculo, guarda un rango
    // acordado para que cambiar la constante haga ruido al pasar los tests y no
    // dos semanas después, cuando alguien note que ya no se leen los paneles.
    #[test]
    #[allow(clippy::assertions_on_constants)]
    fn el_atenuado_se_queda_del_lado_de_lo_leve() {
        // El atenuado explica el foco; no esconde los otros siete. Si alguien lo
        // baja buscando el efecto de un WM de escritorio, esto avisa antes de
        // que el producto —ver a los ocho trabajar— deje de cumplirse.
        assert!(
            DIM_INACTIVE >= 0.85,
            "un atenuado de {DIM_INACTIVE} ya no deja leer los paneles de al lado"
        );
        assert!(
            DIM_INACTIVE < 1.0,
            "con 1.0 el atenuado no hace nada; bórralo antes que dejarlo mintiendo"
        );
    }

    #[test]
    fn el_degradado_del_foco_es_trazo_valido_de_punta_a_punta() {
        // El borde del panel con foco va de `ACCENT` a `ACCENT_TEXT`. Un
        // degradado solo cumple si **los dos** extremos cumplen: pasar por 3:1
        // en un extremo y quedarse corto en el otro es un borde que se apaga por
        // una esquina. El tono ya lo comprueba el test de las dos caras.
        for (nombre, color) in [("ACCENT", ACCENT), ("ACCENT_TEXT", ACCENT_TEXT)] {
            let ratio = vs_black(color);
            assert!(
                ratio >= 3.0,
                "{nombre} se queda en {ratio:.2}:1 y es un extremo del degradado"
            );
        }
    }

    #[test]
    fn el_acento_cumple_lo_de_un_componente_de_interfaz() {
        // `ACCENT` nunca es una letra: es marco, relleno y trazo. A eso la WCAG
        // 1.4.11 le pide 3:1, no 4,5:1. Este test es el que impide que alguien
        // lo use de color de texto sin darse cuenta de que no llega.
        let ratio = vs_black(ACCENT);
        assert!(ratio >= 3.0, "el acento se queda en {ratio:.2}:1");
        assert!(
            ratio < 4.5,
            "el acento ya llega a {ratio:.2}:1: si sube, `ACCENT_TEXT` sobra"
        );
    }

    #[test]
    fn el_cian_ansi_no_es_el_de_la_marca() {
        // Un proceso que escribe en cian espera cian. Si el slot 6 se atara al
        // acento, cambiar la marca repintaría la salida ajena.
        assert_ne!(ANSI[6], ACCENT);
        assert_ne!(ANSI[6], ACCENT_TEXT);
        // Cian de verdad: más azul que rojo y con el verde arriba.
        assert!(ANSI[6].b() > ANSI[6].r() && ANSI[6].g() > ANSI[6].r());
    }

    #[test]
    fn las_dos_caras_del_acento_son_el_mismo_color() {
        // Mismo tono; solo cambia la claridad. Si alguien retoca uno de los dos
        // y se van de tono, dejan de leerse como la misma marca.
        let hue = |c: Color32| {
            let (r, g, b) = (
                c.r() as f64 / 255.0,
                c.g() as f64 / 255.0,
                c.b() as f64 / 255.0,
            );
            let max = r.max(g).max(b);
            let d = max - r.min(g).min(b);
            let h = if d == 0.0 {
                0.0
            } else if max == r {
                60.0 * (((g - b) / d) % 6.0)
            } else if max == g {
                60.0 * ((b - r) / d + 2.0)
            } else {
                60.0 * ((r - g) / d + 4.0)
            };
            (h + 360.0) % 360.0
        };
        let (a, b) = (hue(ACCENT), hue(ACCENT_TEXT));
        assert!(
            (a - b).abs() < 3.0,
            "el acento va a {a:.0}° y su texto a {b:.0}°"
        );
    }
}
