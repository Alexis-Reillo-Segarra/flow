//! Paleta, tipografía y estilo global.
//!
//! # Color
//!
//! Tres colores llevan el peso: el **fondo**, el **gris** de las líneas que
//! separan las zonas y el **acento** de la marca, el foco y el cursor. No hay
//! rellenos de color ni tarjetas grises: la estructura de la interfaz se lee
//! por las divisorias de 1 px, no por bloques de tono.
//!
//! Aparte de esos tres, hay cuatro colores que solo hablan de estado —verde,
//! ámbar, rojo, gris— y nunca se usan de decoración. Si ves color en flow,
//! significa algo.
//!
//! # Temas
//!
//! La paleta ya no es fija: es un dato, `Palette`, y hay varios. El de casa
//! —`flow`, negro OLED y el blanco de marca— sigue siendo el que sale de
//! fábrica, y al lado van cuatro temas conocidos para quien ya tenga el
//! escritorio de un color. Se cambia en caliente con `Ctrl-Shift-T` y se puede
//! escribir uno propio en el fichero de configuración (ver `crate::config`).
//!
//! El de casa es **monocromo entero**: fondo, divisorias, texto, acento y los
//! cuatro estados viven en la línea del negro al blanco. Lo único con tono que
//! puede salir en pantalla es lo que escribe un proceso, así que la regla de la
//! casa —si ves color en flow, significa algo— llega a un sitio donde ya no
//! hace falta enunciarla: si ves color, no lo ha puesto flow.
//!
//! **Un tema no es una lista de gustos: es un contrato**, y los tests de este
//! módulo lo recorren entero **para todos los temas incluidos**, no solo para el
//! activo. Un tema entra si:
//!
//! 1. Sus cuatro niveles de texto cumplen 4,5:1 contra **su** fondo, y también
//!    atenuados —con `DIM_INACTIVE`, el nivel más tenue de un panel sin foco es
//!    el peor caso real de la interfaz—.
//! 2. Su acento tiene dos caras, una de trazo (≥3:1) y una de texto (≥4,5:1),
//!    **con el mismo tono**, para que se lean como el mismo color y no como dos.
//! 3. Sus cuatro colores de estado cumplen 4,5:1 y se distinguen entre sí, por
//!    tono (25° mínimo) **o** por claridad (1,5:1 mínimo). Lo primero es lo que
//!    hacen los temas de color, y se pide tono y no claridad porque quien no
//!    separe rojo de verde los tiene casi a la misma luminancia. Lo segundo es
//!    lo único que le queda a un tema monocromo, donde no hay tono que separar.
//! 4. Sus slots ANSI conservan su nombre —el cian del tema es cian— y ninguno
//!    de los dieciséis es exactamente el acento: si lo fuera, lo que dice flow
//!    y lo que escribe un proceso saldrían del mismo color.
//!
//! De ahí salen las únicas licencias que se han tomado con las paletas
//! originales: donde un tono del tema de origen no llegaba a 4,5:1 sobre su
//! propio fondo —el rojo de Gruvbox y el de Nord— se ha aclarado lo justo. El
//! contrato manda sobre la fidelidad, porque un rojo que no se lee no avisa de
//! nada. Lo mismo con algún "cian" que en su tema tira a verde: se le empuja al
//! azul lo justo para que el slot 6 siga significando cian.
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
//! La tipografía **no** es parte del tema, y no es un descuido: las dos familias
//! van embebidas en el binario, un tema es un fichero de texto y no puede traer
//! una fuente, y el tamaño ya lo decide el sistema (ver `app::auto_scale`). Un
//! tema cambia el color.
//!
//! # Espacio
//!
//! La rejilla de paneles se lee por el **hueco**, no por marcos: `GAP` separa
//! los paneles entre sí y del borde de la ventana, y es lo que hace que la
//! pantalla parezca un tiling WM en vez de una tabla. Tampoco es parte del tema:
//! es la forma de la interfaz, y esa no cambia porque cambien los colores.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, OnceLock};

use egui::{
    Color32, Context, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Stroke, Style,
    TextStyle,
};

// ─── Espacio ──────────────────────────────────────────────────────────────────

/// Hueco entre paneles y contra el borde de la ventana.
///
/// Es la única unidad de aire de la interfaz: el mismo valor separa un panel del
/// de al lado y del marco. Un hueco uniforme es lo que hace que la rejilla se
/// lea como ventanas en mosaico y no como celdas de una tabla.
///
/// Va hacia los 20 de Hyprland, pero no llega ni de lejos: aquí caben ocho
/// paneles en pantalla y el hueco que en un WM de escritorio es holgura, aquí se
/// come columnas de terminal. Con cuatro columnas, cada punto de `GAP` son cinco
/// puntos de ancho que el terminal no ve.
pub const GAP: f32 = 8.0;

/// Redondeo de las esquinas de un panel.
///
/// Seis píxeles: suficiente para que el panel se lea como una ventana suelta
/// sobre el fondo y no como la celda de una tabla, que es toda la diferencia
/// entre esto y un dashboard. Todo lo que va **dentro** sigue con esquina viva
/// —botones, campos, marcas de estado— para que el redondeo signifique una cosa
/// concreta: esto es una ventana. Si lo llevara todo, no distinguiría nada.
pub const RADIUS: u8 = 6;

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
/// El 0,90 no está elegido a ojo: es lo que aguanta el nivel de texto más flojo
/// sin caerse del 4,5:1 de la WCAG, y ahora lo comprueba un test **en todos los
/// temas**. Si lo bajas, hay temas que dejan de cumplir.
pub const DIM_INACTIVE: f32 = 0.90;

// ─── La paleta ────────────────────────────────────────────────────────────────

/// Los colores de un tema.
///
/// Todo lo que un tema puede cambiar está aquí y solo aquí. Si un color se
/// escribe a pelo en un módulo de `ui`, ese color no cambia al cambiar de tema y
/// la promesa se rompe por un sitio: la regla es que de `ui` no sale ningún
/// `Color32` literal que no venga de `pal()`.
#[derive(Clone, Debug)]
pub struct Palette {
    /// Cómo se llama, tal y como se escribe en el fichero de configuración.
    pub name: String,
    /// Una línea sobre de dónde viene. Se lee en el selector.
    pub about: String,

    // ── Superficies ──
    /// El fondo de todo: paneles, barras y terminal.
    pub bg: Color32,
    /// Igual que `bg` en todos los temas incluidos. Existe como concepto aparte
    /// porque los paneles de la rejilla sí se rellenan con él, y no por color:
    /// es lo que deja el grano del fondo fuera de la ventana.
    pub panel: Color32,
    /// Campos de texto y cajas: apenas un susurro por encima del fondo.
    pub raised: Color32,
    /// Fila seleccionada.
    pub sel: Color32,
    /// Hover.
    pub hover: Color32,
    /// Las divisorias de 1 px: el esqueleto visible de la interfaz.
    ///
    /// Se queda por debajo del 3:1 de la WCAG a propósito, en todos los temas:
    /// una divisoria es decoración, no transmite estado ni información, y
    /// subirla más convertiría la rejilla en el elemento más ruidoso de la
    /// pantalla. Todo lo que sí informa —texto, estados, foco— sí cumple AA.
    pub line: Color32,
    /// Divisoria con más presencia: borde exterior y campos con foco.
    pub line_hi: Color32,

    // ── Profundidad ──
    /// El halo que despega un panel del fondo.
    ///
    /// Él y `halo_focus` son las dos únicas superficies de la interfaz que no
    /// significan nada: no dicen estado, no son texto y no hay que poder
    /// leerlas. Por eso van con una alfa tan baja —se notan sin verse— y por eso
    /// son lo único de la paleta que no pasa por los tests de contraste.
    ///
    /// Existen porque sobre un fondo casi negro una sombra no puede existir:
    /// oscurecer el `#000000` del tema de casa no da nada. La profundidad se da
    /// al revés, con luz que se desvanece hacia fuera. Ver
    /// `ui::widgets::panel_halo`.
    pub halo: Color32,
    /// El del panel con foco lleva el acento, como la sombra coloreada de la
    /// ventana activa en Hyprland. No es una señal nueva: es el borde, que se
    /// derrama un poco más allá de su trazo.
    pub halo_focus: Color32,

    // ── Texto ──
    // Los cuatro niveles cumplen AA contra `bg` y también atenuados.
    pub text: Color32,
    pub text_hi: Color32,
    pub text_dim: Color32,
    /// El nivel más flojo, y el que fija el techo de `DIM_INACTIVE`. Existe para
    /// dar jerarquía, no para esconder texto: si algo se puede leer, tiene que
    /// poder leerse.
    pub text_faint: Color32,

    // ── Marca ──
    /// El acento **solo como superficie o trazo**: bordes, rellenos, marcos.
    ///
    /// Cumple el 3:1 que la WCAG pide a un componente de interfaz, pero puede
    /// quedarse por debajo del 4,5:1 de un texto. Cuando el acento tiene que ser
    /// una letra, va `accent_text`.
    pub accent: Color32,
    /// La misma marca aclarada, para cuando el acento **es** una letra o un
    /// glifo fino. Mismo tono que `accent`: se lee como el mismo color, no como
    /// otro.
    ///
    /// También es el extremo claro del degradado del panel con foco. El
    /// degradado va de una cara del acento a la otra en vez de estrenar un
    /// tercer color: son los dos tonos que ya existen, comparten tono, y los dos
    /// pasan el 3:1 que la WCAG le pide a un trazo, así que el borde cumple **en
    /// todo su recorrido** y no solo en un extremo.
    pub accent_text: Color32,

    // ── Estados ──
    //
    // Nunca decoran: si ves uno, significa eso. Los nombres son el **papel** y
    // no el tono: en los cuatro temas de color son verde, ámbar y rojo de
    // verdad, y en el de casa, que es monocromo, son cuatro grises ordenados
    // por cuánto te reclama cada estado. Se llaman así porque además de nombrar
    // el papel son el formato del fichero de configuración, y renombrarlos
    // rompería los temas que la gente ya tenga escritos.
    /// Trabajando (latiendo) y terminado con éxito (sólido).
    pub green: Color32,
    /// Bloqueado esperando respuesta. El único estado que reclama atención —lo
    /// hace parpadeando, no siendo el más claro—. También los avisos del
    /// formulario y el botón `^C`.
    pub amber: Color32,
    /// Terminado con error o imposible de lanzar. También todo lo que hace
    /// daño: el botón `KILL`, la X de cerrar y los mensajes de error. Por eso
    /// es el más claro de los cuatro en el tema monocromo.
    pub red: Color32,
    /// Vivo pero sin actividad. Cumple AA como los demás aunque su papel sea el
    /// de "no me mires", y es el más apagado de los cuatro.
    pub slate: Color32,

    /// Los 16 colores ANSI, armonizados con el tema para que el output de los
    /// procesos no desentone con el chrome de la app.
    ///
    /// Los que tienen nombre propio en la paleta lo reutilizan, pero solo cuando
    /// el nombre coincide: el cian de aquí es un cian de verdad y no el color de
    /// la marca. Un proceso que pide cian espera cian, y atarlo al acento
    /// significaría que cambiar la marca repinta la salida ajena.
    pub ansi: [Color32; 16],
}

/// Cuánto se oscurece la cara de texto del acento para sacar la de trazo cuando
/// un tema solo declara una.
///
/// Escalar los tres canales por el mismo factor **no mueve el tono** —es una
/// propiedad de HSV, no una casualidad—, así que las dos caras del acento salen
/// del mismo color por construcción y no por haber acertado a ojo con dos
/// valores. Cada tema incluido usa el suyo: cuanto más claro es su fondo, menos
/// margen tiene la cara de trazo para bajar sin caerse del 3:1.
const STROKE_FACE: f32 = 0.70;

/// Cuánto del acento se derrama en el halo del panel con foco.
const HALO_FACE: f32 = 0.345;

/// El halo neutro. Es el mismo en todos los temas: una luz tan tenue que teñirla
/// por tema no cambiaría nada que se pueda ver.
const HALO: Color32 = Color32::from_rgba_premultiplied(0x0e, 0x0f, 0x12, 0x1c);

/// Escala los tres canales por igual, conservando el tono y la alfa.
fn scaled(c: Color32, k: f32) -> Color32 {
    let ch = |v: u8| (v as f32 * k).round().clamp(0.0, 255.0) as u8;
    Color32::from_rgba_premultiplied(ch(c.r()), ch(c.g()), ch(c.b()), c.a())
}

/// El halo del panel con foco a partir del acento: el borde derramándose.
fn halo_face(accent: Color32) -> Color32 {
    let c = scaled(accent, HALO_FACE);
    Color32::from_rgba_premultiplied(c.r(), c.g(), c.b(), 0x30)
}

const fn rgb(hex: u32) -> Color32 {
    Color32::from_rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

/// Los 16 slots ANSI de un tema, escritos como se leen.
fn ansi(slots: [u32; 16]) -> [Color32; 16] {
    slots.map(rgb)
}

impl Palette {
    /// Arma un tema a partir de lo único que de verdad lo distingue.
    ///
    /// El resto —la cara de trazo del acento y los dos halos— se deriva, porque
    /// son cosas que tienen que cumplir una relación con lo anterior y no
    /// decisiones sueltas: derivarlas es lo que impide que un tema nuevo llegue
    /// con dos acentos que no comparten tono.
    #[allow(clippy::too_many_arguments)]
    fn new(
        name: &str,
        about: &str,
        surfaces: [u32; 6],
        texts: [u32; 4],
        accent_text: u32,
        stroke_face: f32,
        status: [u32; 4],
        ansi_slots: [u32; 16],
    ) -> Self {
        let [bg, raised, sel, hover, line, line_hi] = surfaces;
        let [text, text_hi, text_dim, text_faint] = texts;
        let [green, amber, red, slate] = status;
        let accent_text = rgb(accent_text);
        let accent = scaled(accent_text, stroke_face);
        Self {
            name: name.to_owned(),
            about: about.to_owned(),
            bg: rgb(bg),
            panel: rgb(bg),
            raised: rgb(raised),
            sel: rgb(sel),
            hover: rgb(hover),
            line: rgb(line),
            line_hi: rgb(line_hi),
            halo: HALO,
            halo_focus: halo_face(accent),
            text: rgb(text),
            text_hi: rgb(text_hi),
            text_dim: rgb(text_dim),
            text_faint: rgb(text_faint),
            accent,
            accent_text,
            green: rgb(green),
            amber: rgb(amber),
            red: rgb(red),
            slate: rgb(slate),
            ansi: ansi(ansi_slots),
        }
    }

    /// Cambia un color por su nombre en el fichero de configuración.
    ///
    /// Es la única puerta por la que entra un tema escrito a mano, así que la
    /// lista de nombres de aquí **es** el formato del fichero: lo que no esté
    /// aquí no se puede tocar desde fuera, y `crate::config` avisa de la clave
    /// que no reconoce en vez de tragársela en silencio.
    pub fn set(&mut self, key: &str, c: Color32) -> bool {
        match key {
            // `bg` mueve también el relleno del panel: son el mismo color en
            // todos los temas incluidos, y pedirle a alguien que escriba dos
            // veces el mismo valor solo sirve para que un día no coincidan.
            "bg" => {
                self.bg = c;
                self.panel = c;
            }
            "panel" => self.panel = c,
            "raised" => self.raised = c,
            "sel" => self.sel = c,
            "hover" => self.hover = c,
            "line" => self.line = c,
            "line_hi" => self.line_hi = c,
            "text" => self.text = c,
            "text_hi" => self.text_hi = c,
            "text_dim" => self.text_dim = c,
            "text_faint" => self.text_faint = c,
            // El acento se declara por su cara de texto y la de trazo sale sola:
            // es la única forma de que compartan tono sin pedirle cuentas a mano
            // a quien escribe el tema. Quien quiera la otra exacta la pone
            // después con `accent_stroke`.
            "accent" => {
                self.accent_text = c;
                self.accent = scaled(c, STROKE_FACE);
                self.halo_focus = halo_face(self.accent);
            }
            "accent_stroke" => {
                self.accent = c;
                self.halo_focus = halo_face(c);
            }
            "green" => self.green = c,
            "amber" => self.amber = c,
            "red" => self.red = c,
            "slate" => self.slate = c,
            _ => match key
                .strip_prefix("ansi")
                .and_then(|n| n.parse::<usize>().ok())
            {
                Some(i) if i < 16 => self.ansi[i] = c,
                _ => return false,
            },
        }
        true
    }
}

/// Los temas que vienen dentro del binario.
///
/// El primero es el que sale de fábrica. Los otros cuatro son los que ya tiene
/// puesto medio mundo en su editor; se han portado tono a tono, con las únicas
/// correcciones que pedía el contraste (ver el comentario del módulo).
pub fn builtin() -> Vec<Palette> {
    vec![
        // El tema de casa: negro OLED, gris y blanco. Monocromo.
        //
        // El fondo es `#000000` exacto, y es una decisión, no un valor por
        // defecto que nadie revisó: en un panel OLED ese valor es el píxel
        // **apagado**, y eso es un negro que ninguna otra pantalla sabe dar.
        // Subirlo aunque fuera un punto —para que se vieran sombras, por
        // ejemplo— lo encendería entero. Los demás temas traen su propio fondo
        // porque un tema es justo eso; el de casa no lo toca.
        //
        // El chrome no tiene tono: ni el negro, ni los grises de las
        // divisorias, ni los cuatro niveles de texto. Antes tiraban a azul un
        // par de puntos —`#c6cbd2`, `#3c424b`— y ese matiz es justo lo que un
        // tema monocromo no puede permitirse: con el acento en blanco no hay
        // ningún tono al que arrimarse, así que un gris azulado no se lee como
        // "neutro con carácter", se lee como una desviación.
        //
        // Aquí el acento **es** el blanco puro, y de ahí sale casi todo lo
        // demás:
        //
        // - El resto de la interfaz se queda por debajo. `text_hi` baja de
        //   `#ffffff` a `#e6e6e6` porque el blanco pasa a ser de la marca: si
        //   lo llevara también el texto destacado, el foco dejaría de tener
        //   nada que lo separe de un rótulo cualquiera. Con hue no se puede
        //   distinguir; con claridad sí.
        // - El slot ANSI 15 se mueve a `#f2f2f2`. Ningún slot puede ser
        //   exactamente el acento —hay un test— y aquí choca el "blanco
        //   brillante" del terminal. Se mueve el slot y no el acento, que es lo
        //   que se hace siempre: dos puntos de blanco no se ven en la salida de
        //   un proceso, y el acento sí tiene que ser el blanco de verdad.
        //
        // Los cuatro colores de estado también son grises, así que en este tema
        // **no hay un solo color de interfaz**: lo único con tono que puede
        // aparecer en pantalla es lo que escribe un proceso. Los nombres del
        // campo —`green`, `amber`, `red`— se quedan porque nombran el papel y
        // no el tono, y porque son el formato del fichero de configuración: los
        // otros cuatro temas siguen poniendo ahí verde, ámbar y rojo de verdad.
        //
        // Lo que sostiene esto es que el estado nunca se dijo solo con color.
        // Van cuatro canales a la vez y el color siempre fue el prescindible:
        //
        // - **La palabra**, siempre al lado: `WORKING`, `BLOCKED`, `IDLE`,
        //   `DONE`, `EXIT`, `FAILED`.
        // - **La forma**: `IDLE` va hueco y todo lo demás sólido, que es lo que
        //   separa `DONE` de `IDLE` ahora que los dos son un gris parecido.
        // - **El ritmo**: `WORKING` late, `BLOCKED` parpadea, los terminales
        //   están quietos.
        // - **La claridad**, que es lo que sustituye al tono aquí. El orden es
        //   el del daño y no el de la urgencia: `red` arriba del todo, `amber`
        //   debajo, luego lo que va bien y `IDLE` el más apagado.
        //
        // Ese orden **no** es el que parecía obvio, y conviene saber por qué
        // antes de "arreglarlo". Mirando solo las marcas de estado sale al
        // revés: quien te reclama es `BLOCKED`, no un proceso que ya murió, así
        // que `amber` querría ir arriba. Pero estos dos campos no pintan solo
        // marcas — `red` es además el botón `KILL`, la X de cerrar y los
        // mensajes de error, y `amber` es el `^C` y los avisos del formulario—,
        // y en esos seis sitios «error» tiene que pesar más que «aviso». Con
        // tono se podía tener las dos cosas a la vez porque el rojo y el ámbar
        // se separaban sin ordenarse; en una sola dimensión hay que elegir, y
        // pierde la marca de estado, que es la que tiene con qué compensar:
        // `BLOCKED` parpadea, y el parpadeo gana a cualquier claridad. Si algún
        // día subes `amber` por encima de `red`, `KILL` pasa a verse más flojo
        // que `^C`.
        //
        // El par que de verdad depende de la claridad es `DONE` contra
        // `EXIT`/`FAILED`: los dos son sólidos y quietos, así que solo los
        // separan la claridad y la palabra. De ahí que sean los dos que más se
        // han apartado —7,10:1 y 18,43:1 sobre el fondo, 2,60:1 entre ellos—, y
        // de ahí que el test de estados acepte ahora separación por claridad
        // además de por tono: ver `los_estados_se_distinguen_sin_mirar_el_color`.
        //
        // El factor de trazo, 0,43, deja la cara oscura del acento en
        // `#6e6e6e`: 4,12:1, entre el 3:1 que la WCAG pide a un componente y el
        // 4,5:1 que pide a un texto, que es donde tiene que quedarse para que
        // `accent_text` siga teniendo motivo de existir.
        Palette::new(
            "flow",
            "negro OLED, gris y blanco: monocromo",
            [0x000000, 0x0a0a0a, 0x161616, 0x0e0e0e, 0x424242, 0x626262],
            [0xcbcbcb, 0xe6e6e6, 0xa0a0a0, 0x848484],
            0xffffff,
            0.43,
            [0x969696, 0xc0c0c0, 0xf0f0f0, 0x868686],
            [
                0x161616, 0xf2696e, 0x6ee787, 0xf0b45c, 0x6fa8f5, 0xc792ea, 0x35d6f5, 0xcbcbcb,
                0x4a4a4a, 0xff8b90, 0x95ffaa, 0xffd082, 0x92c2ff, 0xddb0ff, 0x8de9ff, 0xf2f2f2,
            ],
        ),
        // Catppuccin Mocha. El acento es el mauve de la casa.
        Palette::new(
            "catppuccin",
            "Catppuccin Mocha: malva sobre base azulada",
            [0x1e1e2e, 0x232338, 0x313244, 0x292940, 0x45475a, 0x6c7086],
            [0xcdd6f4, 0xf5f5ff, 0xbac2de, 0xa6adc8],
            0xcba6f7,
            0.70,
            [0xa6e3a1, 0xf9e2af, 0xf38ba8, 0x9399b2],
            [
                0x45475a, 0xf38ba8, 0xa6e3a1, 0xf9e2af, 0x89b4fa, 0xf5c2e7, 0x94e2d5, 0xbac2de,
                0x585b70, 0xf7a2bb, 0xb9ebb4, 0xfdecc8, 0xa6c8ff, 0xf9cff0, 0xaeeae0, 0xf5f5ff,
            ],
        ),
        // Gruvbox Dark. El acento **no** es el naranja de la casa, y es la única
        // desviación de fondo de esta lista: el naranja de Gruvbox y su amarillo
        // de aviso se quedan a 13° de tono, así que el acento habría dejado de
        // distinguirse del estado BLOCKED. Se usa el aqua, que está a 100° del
        // estado más cercano. El rojo se ha aclarado desde `#fb4934`, que sobre
        // `#282828` se quedaba en 4,29:1.
        Palette::new(
            "gruvbox",
            "Gruvbox Dark: cálido, contrastado, retro",
            [0x282828, 0x32302f, 0x3c3836, 0x35322f, 0x504945, 0x7c6f64],
            [0xebdbb2, 0xfbf1c7, 0xd5c4a1, 0xbdae93],
            0x8ec7b4,
            0.72,
            [0x8ec07c, 0xfabd2f, 0xff7b6b, 0xa89984],
            [
                0x3c3836, 0xfb4934, 0xb8bb26, 0xfabd2f, 0x83a598, 0xd3869b, 0x6ba896, 0xa89984,
                0x665c54, 0xff7b6b, 0xcdd44a, 0xffd75f, 0xa3c5bb, 0xe8a7b8, 0x8ecfb8, 0xebdbb2,
            ],
        ),
        // Tokyo Night.
        Palette::new(
            "tokyonight",
            "Tokyo Night: azul noche",
            [0x1a1b26, 0x1f2233, 0x292e42, 0x24283b, 0x3b4261, 0x565f89],
            [0xc0caf5, 0xe7eaf7, 0xa9b1d6, 0x8d97c4],
            0x7aa2f7,
            0.72,
            [0x9ece6a, 0xe0af68, 0xf7768e, 0x9099b8],
            [
                0x24283b, 0xf7768e, 0x9ece6a, 0xe0af68, 0x82aaff, 0xbb9af7, 0x7dcfff, 0xa9b1d6,
                0x414868, 0xff899d, 0xc3e88d, 0xffc777, 0x8db0ff, 0xc099ff, 0xb4f9f8, 0xc0caf5,
            ],
        ),
        // Nord. Su fondo es el más claro de la lista, así que es el tema con
        // menos margen: el rojo se ha aclarado desde `#bf616a`, que sobre
        // `#2e3440` no llegaba ni a 4:1.
        Palette::new(
            "nord",
            "Nord: frío, gris azulado",
            [0x2e3440, 0x353c4a, 0x434c5e, 0x3b4252, 0x4c566a, 0x7b88a1],
            [0xe5e9f0, 0xeceff4, 0xd8dee9, 0xb8c0cf],
            0x88c0d0,
            0.72,
            [0xa3be8c, 0xebcb8b, 0xeba0a8, 0xa7b0c0],
            [
                0x3b4252, 0xbf616a, 0xa3be8c, 0xebcb8b, 0x81a1c1, 0xb48ead, 0x8fbcbb, 0xe5e9f0,
                0x4c566a, 0xd68f97, 0xb5d19c, 0xf0d9a0, 0x9dbbd8, 0xc9a3c2, 0x9fd6d5, 0xeceff4,
            ],
        ),
    ]
}

// ─── El tema activo ───────────────────────────────────────────────────────────
//
// La lista se arma una vez al arrancar —los incluidos más los del fichero de
// configuración— y se filtra a `'static`. Es memoria que no se libera, y a
// propósito: son cinco o seis paletas de 200 bytes que viven lo que vive el
// proceso, y a cambio `pal()` es una indexación sin cerrojo ni contador, que es
// lo que hace falta cuando se llama una vez por celda de terminal y por frame.

static THEMES: OnceLock<&'static [Palette]> = OnceLock::new();
static ACTIVE: AtomicUsize = AtomicUsize::new(0);

/// Instala la lista de temas. Se llama una vez, al arrancar, antes de dibujar
/// nada. Si ya había una, se queda la primera: dos listas distintas en marcha
/// significarían que un índice guardado apunta a otro tema.
pub fn install_themes(list: Vec<Palette>) {
    let _ = THEMES.set(Box::leak(list.into_boxed_slice()));
}

/// Todos los temas disponibles, en el orden en que se ofrecen.
///
/// Si nadie instaló nada —los tests, por ejemplo— salen los incluidos: que la
/// paleta dependa de haber llamado antes a otra función sería una bomba de
/// relojería en cuanto alguien pintase algo desde un test.
pub fn themes() -> &'static [Palette] {
    THEMES.get_or_init(|| Box::leak(builtin().into_boxed_slice()))
}

/// El índice del tema activo.
pub fn active() -> usize {
    ACTIVE.load(Ordering::Relaxed).min(themes().len() - 1)
}

/// Cambia de tema. El índice se recorta a la lista: un tema que ya no está no
/// puede dejar la interfaz sin colores.
pub fn set_active(i: usize) {
    ACTIVE.store(i.min(themes().len() - 1), Ordering::Relaxed);
}

/// Busca un tema por su nombre, como se escribe en el fichero.
pub fn index_of(name: &str) -> Option<usize> {
    let name = name.trim();
    themes().iter().position(|p| p.name == name)
}

/// La paleta activa. Todo el color de la interfaz sale de aquí.
pub fn pal() -> &'static Palette {
    &themes()[active()]
}

/// Resuelve un índice de la paleta 256 a color.
pub fn ansi256(i: u8) -> Color32 {
    match i {
        0..=15 => pal().ansi[i as usize],
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
    let p = pal();
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
    v.panel_fill = p.panel;
    v.window_fill = p.bg;
    v.extreme_bg_color = p.bg;
    v.faint_bg_color = p.raised;
    v.code_bg_color = p.raised;
    v.override_text_color = Some(p.text);
    v.window_stroke = Stroke::new(1.0, p.line);
    v.window_corner_radius = CornerRadius::ZERO;
    v.menu_corner_radius = CornerRadius::ZERO;
    v.selection.bg_fill = p.sel;
    v.selection.stroke = Stroke::new(1.0, p.accent);
    // Un enlace es texto, así que le toca la variante clara.
    v.hyperlink_color = p.accent_text;

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
        w.bg_stroke = Stroke::new(1.0, p.line);
        w.fg_stroke = Stroke::new(1.0, p.text);
    }
    v.widgets.noninteractive.bg_fill = p.panel;
    v.widgets.noninteractive.weak_bg_fill = p.panel;
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, p.text_dim);
    v.widgets.inactive.bg_fill = p.raised;
    v.widgets.inactive.weak_bg_fill = p.raised;
    v.widgets.hovered.bg_fill = p.hover;
    v.widgets.hovered.weak_bg_fill = p.hover;
    v.widgets.hovered.bg_stroke = Stroke::new(1.0, p.line_hi);
    v.widgets.hovered.fg_stroke = Stroke::new(1.0, p.text_hi);
    v.widgets.active.bg_fill = p.sel;
    v.widgets.active.weak_bg_fill = p.sel;
    v.widgets.active.bg_stroke = Stroke::new(1.0, p.accent);
    v.widgets.active.fg_stroke = Stroke::new(1.0, p.text_hi);

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
    apply_style(ctx);
}

/// Vuelve a instalar el estilo con la paleta activa.
///
/// Se llama al cambiar de tema, y solo el estilo: las fuentes no dependen del
/// tema y volver a ponerlas tiraría el atlas de glifos para reconstruirlo igual.
pub fn apply_style(ctx: &Context) {
    // El mismo estilo en ambos temas de egui: flow es oscuro y punto. Así da
    // igual que el sistema esté en claro o que el usuario lo cambie en caliente.
    let style = Arc::new(style());
    ctx.set_style_of(egui::Theme::Dark, style.clone());
    ctx.set_style_of(egui::Theme::Light, style);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Luminancia relativa de la WCAG 2.1.
    fn luminance(c: Color32) -> f64 {
        let channel = |v: u8| {
            let v = v as f64 / 255.0;
            if v <= 0.04045 {
                v / 12.92
            } else {
                ((v + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * channel(c.r()) + 0.7152 * channel(c.g()) + 0.0722 * channel(c.b())
    }

    /// Contraste de `fg` sobre `bg`. Antes bastaba con la luminancia del color
    /// de delante, porque el fondo era negro puro y la fórmula se quedaba en
    /// `(L + 0,05) / 0,05`. Con temas de fondo propio hay que hacer la división
    /// entera.
    fn contrast(fg: Color32, bg: Color32) -> f64 {
        let (a, b) = (luminance(fg), luminance(bg));
        let (hi, lo) = if a > b { (a, b) } else { (b, a) };
        (hi + 0.05) / (lo + 0.05)
    }

    /// El mismo color después de atenuarlo con `DIM_INACTIVE`.
    ///
    /// `gamma_multiply` baja también la alfa, así que lo que acaba en pantalla
    /// no es el color oscurecido sino el color **mezclado con el fondo del
    /// panel** al 90%. Sobre el negro de flow daba lo mismo —mezclar con negro
    /// es oscurecer— pero sobre el fondo de un tema con color no, y el test
    /// tiene que mirar lo que se ve.
    fn dimmed_over(c: Color32, bg: Color32) -> Color32 {
        let mix = |f: u8, b: u8| {
            (f as f32 * DIM_INACTIVE + b as f32 * (1.0 - DIM_INACTIVE)).round() as u8
        };
        Color32::from_rgb(mix(c.r(), bg.r()), mix(c.g(), bg.g()), mix(c.b(), bg.b()))
    }

    fn hue(c: Color32) -> f64 {
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
    }

    /// Separación de tono, por el lado corto de la rueda.
    fn hue_gap(a: Color32, b: Color32) -> f64 {
        let d = (hue(a) - hue(b)).abs();
        d.min(360.0 - d)
    }

    /// Todo lo que es texto en la interfaz. La lista es la que recorren los dos
    /// tests de contraste: un color de texto que no esté aquí es un color que
    /// dentro de dos meses no cumple, porque nadie lo comprueba.
    fn textos(p: &Palette) -> [(&'static str, Color32); 9] {
        [
            ("text", p.text),
            ("text_hi", p.text_hi),
            ("text_dim", p.text_dim),
            ("text_faint", p.text_faint),
            ("accent_text", p.accent_text),
            ("green", p.green),
            ("amber", p.amber),
            ("red", p.red),
            ("slate", p.slate),
        ]
    }

    #[test]
    fn todo_lo_que_es_texto_cumple_aa() {
        // 4,5:1 es el mínimo de la WCAG para texto normal. La promesa del README
        // es que **todo** lo legible lo cumple, incluido el nivel más tenue: el
        // gris flojo existe para dar jerarquía, no para esconder texto. Y se le
        // pide a todos los temas: uno que no cumpla no es una preferencia del
        // usuario, es una interfaz que no se lee.
        for p in themes() {
            for (nombre, color) in textos(p) {
                let ratio = contrast(color, p.bg);
                assert!(
                    ratio >= 4.5,
                    "[{}] {nombre} se queda en {ratio:.2}:1, y es texto",
                    p.name
                );
            }
        }
    }

    #[test]
    fn el_texto_de_un_panel_apagado_tambien_cumple_aa() {
        // Este es el caso real peor de la interfaz, y el que hay que pagar por
        // tener `DIM_INACTIVE`: con siete paneles apagados, su nivel más tenue
        // es el texto más flojo que llega a estar en pantalla. Si este test se
        // salta, atenuar deja de ser una decisión de diseño y pasa a ser texto
        // que no se puede leer.
        for p in themes() {
            for (nombre, color) in textos(p) {
                let ratio = contrast(dimmed_over(color, p.bg), p.bg);
                assert!(
                    ratio >= 4.5,
                    "[{}] {nombre} apagado se queda en {ratio:.2}:1 (entero va a {:.2}:1)",
                    p.name,
                    contrast(color, p.bg)
                );
            }
        }
    }

    // Clippy avisa de que la condición es constante, y tiene razón: eso es justo
    // lo que se busca. El test no comprueba un cálculo, guarda un rango acordado
    // para que cambiar la constante haga ruido al pasar los tests y no dos
    // semanas después, cuando alguien note que ya no se leen los paneles.
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
        // El borde del panel con foco va de `accent` a `accent_text`. Un
        // degradado solo cumple si **los dos** extremos cumplen: pasar por 3:1
        // en un extremo y quedarse corto en el otro es un borde que se apaga por
        // una esquina.
        for p in themes() {
            for (nombre, color) in [("accent", p.accent), ("accent_text", p.accent_text)] {
                let ratio = contrast(color, p.bg);
                assert!(
                    ratio >= 3.0,
                    "[{}] {nombre} se queda en {ratio:.2}:1 y es un extremo del degradado",
                    p.name
                );
            }
        }
    }

    #[test]
    fn las_dos_caras_del_acento_son_el_mismo_color() {
        // Mismo tono; solo cambia la claridad. Salen la una de la otra por
        // construcción —escalar los tres canales por igual no mueve el tono—,
        // así que este test protege sobre todo de que alguien ponga las dos a
        // mano y se le vayan.
        for p in themes() {
            let gap = hue_gap(p.accent, p.accent_text);
            assert!(
                gap < 3.0,
                "[{}] el acento va a {:.0}° y su texto a {:.0}°",
                p.name,
                hue(p.accent),
                hue(p.accent_text)
            );
        }
    }

    #[test]
    fn el_acento_de_flow_cumple_lo_de_un_componente_de_interfaz() {
        // `accent` nunca es una letra: es marco, relleno y trazo. A eso la WCAG
        // 1.4.11 le pide 3:1, no 4,5:1, y el gris de casa se queda a propósito
        // entre los dos: si subiera, `accent_text` sobraría. Es una regla del
        // tema de flow y no del contrato —un tema con el fondo más claro puede
        // necesitar un trazo más contrastado—, así que se comprueba solo aquí.
        let p = &themes()[0];
        assert_eq!(p.name, "flow", "el tema de casa es el primero de la lista");
        let ratio = contrast(p.accent, p.bg);
        assert!(ratio >= 3.0, "el acento se queda en {ratio:.2}:1");
        assert!(
            ratio < 4.5,
            "el acento ya llega a {ratio:.2}:1: si sube, `accent_text` sobra"
        );
    }

    #[test]
    fn los_estados_se_distinguen_sin_mirar_el_color() {
        // El color nunca va solo —cada estado lleva su palabra, su forma y su
        // ritmo—, pero eso no es excusa para que dos estados se vean igual: el
        // caso peor es `DONE` contra `EXIT`, los dos sólidos y los dos quietos.
        // Tienen que separarse de un vistazo en una marca de 6 px.
        //
        // Hay dos maneras de conseguirlo y valen las dos, porque son la misma
        // idea aplicada a paletas distintas:
        //
        // - **Por tono**, 25° mínimo, que es lo que hacen los cuatro temas de
        //   color. Se pide tono y no claridad porque quien no distingue rojo de
        //   verde los tiene casi a la misma luminancia: en Nord el verde y el
        //   rojo de estado se quedan en 1,02:1 entre ellos y aun así no se
        //   confunden.
        // - **Por claridad**, 1,5:1 mínimo, que es lo único que le queda a un
        //   tema monocromo. En gris no hay tono que separar, así que el eje
        //   pasa a ser la luminancia — y, de paso, ahí la separación funciona
        //   igual para todo el mundo, incluido quien no distinga colores.
        //
        // Lo que **no** vale es no cumplir ninguna de las dos. Un tema que
        // ponga tres grises casi iguales deja `DONE` y `FAILED` con la palabra
        // como única diferencia, y la palabra es el respaldo, no la señal.
        const TONO_MIN: f64 = 25.0;
        const CLARIDAD_MIN: f64 = 1.5;

        for p in themes() {
            for (a, na, b, nb) in [
                (p.green, "verde", p.amber, "ámbar"),
                (p.amber, "ámbar", p.red, "rojo"),
                (p.green, "verde", p.red, "rojo"),
            ] {
                let tono = hue_gap(a, b);
                let claridad = contrast(a, b);
                assert!(
                    tono >= TONO_MIN || claridad >= CLARIDAD_MIN,
                    "[{}] {na} y {nb} se confunden: {tono:.0}° de tono \
                     (hacen falta {TONO_MIN:.0}) y {claridad:.2}:1 de claridad \
                     (hacen falta {CLARIDAD_MIN:.2})",
                    p.name
                );
            }
        }
    }

    #[test]
    fn el_tema_de_casa_no_tiene_un_solo_color_de_interfaz() {
        // El de casa es monocromo, y eso incluye los estados: lo único con tono
        // que puede salir en pantalla es lo que escribe un proceso. Es la regla
        // «si ves color en flow, significa algo» llevada al extremo en que ya no
        // hace falta enunciarla, y es fácil de romper sin querer —basta con
        // recuperar el verde de estado de antes— así que se comprueba.
        //
        // Los 16 slots ANSI no entran: no son color de interfaz, son la salida
        // ajena, y volverlos grises repintaría el `git diff` de otro programa.
        let p = &themes()[0];
        assert_eq!(p.name, "flow", "el tema de casa es el primero de la lista");
        for (nombre, c) in textos(p).iter().chain(
            [
                ("bg", p.bg),
                ("raised", p.raised),
                ("sel", p.sel),
                ("hover", p.hover),
                ("line", p.line),
                ("line_hi", p.line_hi),
                ("accent", p.accent),
            ]
            .iter(),
        ) {
            assert!(
                c.r() == c.g() && c.g() == c.b(),
                "{nombre} es {c:?} y el tema de casa no tiene tono"
            );
        }
    }

    #[test]
    fn ningun_slot_ansi_es_el_de_la_marca() {
        // Un proceso que escribe en cian espera cian, no la marca de flow. Y
        // vale para los dieciséis: si un slot fuera exactamente el acento,
        // «esto lo dice flow» y «esto lo ha escrito el proceso» se pintarían del
        // mismo color, que es la única cosa que el color de acento tiene que
        // distinguir. Muerde de verdad en los temas portados, donde el color de
        // la casa suele ser también uno de sus ANSI: ahí se ha movido el slot,
        // no el acento.
        for p in themes() {
            for (i, slot) in p.ansi.iter().enumerate() {
                assert_ne!(*slot, p.accent, "[{}] el slot {i}", p.name);
                assert_ne!(*slot, p.accent_text, "[{}] el slot {i}", p.name);
            }
            let cyan = p.ansi[6];
            // Cian de verdad: más azul que rojo y con el verde arriba.
            assert!(
                cyan.b() > cyan.r() && cyan.g() > cyan.r(),
                "[{}] el slot 6 es {cyan:?} y no se lee como cian",
                p.name
            );
        }
    }

    #[test]
    fn los_temas_tienen_nombres_distintos_y_utiles() {
        // El nombre es lo que se escribe en el fichero de configuración, así que
        // dos iguales harían inalcanzable al segundo.
        let list = themes();
        for (i, p) in list.iter().enumerate() {
            assert!(!p.name.is_empty(), "un tema sin nombre no se puede elegir");
            assert!(
                !p.name.contains(char::is_whitespace),
                "'{}' lleva espacios y el fichero se lee por palabras",
                p.name
            );
            assert!(!p.about.is_empty(), "[{}] sin descripción", p.name);
            assert!(
                list[..i].iter().all(|q| q.name != p.name),
                "'{}' está repetido",
                p.name
            );
        }
    }

    #[test]
    fn cambiar_de_tema_cambia_la_paleta() {
        let inicial = active();
        for (i, p) in themes().iter().enumerate() {
            set_active(i);
            assert_eq!(pal().name, p.name);
        }
        // Un índice imposible no deja la interfaz sin colores.
        set_active(9999);
        assert_eq!(active(), themes().len() - 1);
        set_active(inicial);
    }

    #[test]
    fn un_tema_se_puede_retocar_por_el_nombre_de_sus_colores() {
        let mut p = themes()[0].clone();
        assert!(p.set("bg", rgb(0x101010)));
        assert_eq!(p.bg, rgb(0x101010));
        // `bg` arrastra al panel: son el mismo color y separarlos sin querer
        // dejaría el grano asomando dentro de la ventana.
        assert_eq!(p.panel, rgb(0x101010));
        // El acento se declara una vez y la cara de trazo sale sola, más oscura
        // y del mismo tono.
        assert!(p.set("accent", rgb(0x66ccff)));
        assert_eq!(p.accent_text, rgb(0x66ccff));
        assert!(hue_gap(p.accent, p.accent_text) < 3.0);
        assert!(luminance(p.accent) < luminance(p.accent_text));
        assert!(p.set("ansi15", rgb(0xffffff)));
        assert_eq!(p.ansi[15], rgb(0xffffff));
        // Lo que no existe se dice, no se traga.
        assert!(!p.set("ansi16", rgb(0x000000)));
        assert!(!p.set("bakcground", rgb(0x000000)));
    }
}
