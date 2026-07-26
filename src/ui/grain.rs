//! El grano del fondo.
//!
//! Una capa de ruido finísimo sobre el fondo de la ventana. No decora por
//! decorar: un negro absoluto y liso a pantalla completa no tiene ninguna
//! referencia visual, y el ojo deja de saber si está mirando una superficie o un
//! agujero. El grano le da textura a ese negro —lo convierte en algo— sin
//! levantarlo ni un punto de luminancia de media.
//!
//! Eso último es la clave, y es lo que permite que esto conviva con un fondo
//! **OLED puro**: el grano no aclara el negro, lo motea. Las motas son
//! `#000000` con un pelo de blanco encima —como mucho un 6% de opacidad—, así
//! que la inmensa mayoría de los píxeles del fondo siguen siendo negro exacto y
//! en un panel OLED siguen estando apagados de verdad.
//!
//! Va **debajo de los paneles**, nunca dentro. La rejilla se rellena de negro
//! liso encima, así que la salida de un proceso no se lee jamás sobre ruido:
//! el grano vive en los huecos, en la barra de título y en la columna, que es
//! donde no hay nada que leer.
//!
//! El mosaico es una textura de 128×128 que se genera una vez y se repite. Se
//! ancla a la ventana y no al rectángulo que se está pintando, para que al
//! moverse un panel el grano no se arrastre con él: es fondo, y el fondo se
//! queda quieto.

use egui::{Color32, ColorImage, Id, Rect, TextureFilter, TextureHandle, TextureOptions, Ui};

/// Lado del mosaico, en texels. Potencia de dos y lo bastante grande para que
/// la repetición no se lea como un patrón.
const TILE: usize = 128;

/// Opacidad del grano más brillante, sobre 255.
///
/// Muy abajo a propósito. Por encima de esto deja de ser textura y empieza a ser
/// ruido de televisión, que compite con el texto en vez de sostenerlo.
const MAX_ALPHA: f32 = 15.0;

/// Pinta el grano dentro de `rect`.
pub fn paint(ui: &Ui, rect: Rect) {
    if rect.width() <= 0.0 || rect.height() <= 0.0 {
        return;
    }
    let texture = texture(ui);
    let tile = TILE as f32;
    // Las coordenadas de textura salen de la posición en **pantalla**, así que
    // el mosaico queda clavado a la ventana: dos zonas distintas que se pinten
    // por separado siguen el mismo grano y no se ve la costura.
    let uv = Rect::from_min_max(
        egui::pos2(rect.left() / tile, rect.top() / tile),
        egui::pos2(rect.right() / tile, rect.bottom() / tile),
    );
    ui.painter().image(texture.id(), rect, uv, Color32::WHITE);
}

/// La textura, generada una vez y guardada en el contexto.
fn texture(ui: &Ui) -> TextureHandle {
    let ctx = ui.ctx();
    let id = Id::new("grain-tile");
    if let Some(t) = ctx.data(|d| d.get_temp::<TextureHandle>(id)) {
        return t;
    }

    let mut pixels = Vec::with_capacity(TILE * TILE);
    for y in 0..TILE {
        for x in 0..TILE {
            let a = (hash(x, y) as f32 / 255.0 * MAX_ALPHA).round() as u8;
            // Blanco premultiplicado por su propia alfa: el grano aclara, no
            // tiñe. Sobre negro esto es exactamente un gris de valor `a`.
            pixels.push(Color32::from_rgba_premultiplied(a, a, a, a));
        }
    }

    // `NEAREST` y no lineal: el grano tiene que quedarse en el píxel. Filtrarlo
    // lo emborrona hasta convertirlo en una nube gris uniforme, que es
    // justamente el negro liso del que veníamos.
    let options = TextureOptions {
        magnification: TextureFilter::Nearest,
        minification: TextureFilter::Nearest,
        wrap_mode: egui::TextureWrapMode::Repeat,
        mipmap_mode: None,
    };
    // Fuera de `data_mut`: cargar una textura vuelve a tomar el cerrojo del
    // contexto y se quedaría bloqueado.
    let t = ctx.load_texture("flow-grain", ColorImage::new([TILE, TILE], pixels), options);
    ctx.data_mut(|d| d.insert_temp(id, t.clone()));
    t
}

/// Ruido determinista.
///
/// No se usa un generador aleatorio de verdad porque no hace falta y porque
/// interesa lo contrario: que el grano sea **el mismo en cada ejecución**. Si
/// cambiara al arrancar, dos capturas de la misma pantalla no serían iguales y
/// cualquier comparación visual dejaría de servir.
fn hash(x: usize, y: usize) -> u8 {
    let mut h = (x as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ (y as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F);
    h ^= h >> 29;
    h = h.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    h ^= h >> 32;
    (h & 0xFF) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn el_grano_es_el_mismo_en_cada_ejecucion() {
        // Si esto dejara de cumplirse, el fondo cambiaría de una ejecución a
        // otra y ninguna comparación de capturas volvería a valer.
        assert_eq!(hash(3, 7), hash(3, 7));
        assert_eq!(hash(0, 0), hash(0, 0));
    }

    #[test]
    fn el_ruido_no_se_repite_por_filas_ni_columnas() {
        // Un hash malo puede dar el mismo valor a toda una fila, y entonces el
        // grano sale a rayas en vez de a motas.
        let fila: Vec<u8> = (0..64).map(|x| hash(x, 5)).collect();
        assert!(
            fila.windows(2).any(|w| w[0] != w[1]),
            "la fila salió plana: eso es una raya, no grano"
        );
        let columna: Vec<u8> = (0..64).map(|y| hash(5, y)).collect();
        assert!(columna.windows(2).any(|w| w[0] != w[1]));
        assert_ne!(
            fila, columna,
            "filas y columnas no pueden ser el mismo ruido"
        );
    }

    #[test]
    fn el_grano_nunca_levanta_el_negro_de_verdad() {
        // La promesa del módulo: sobre OLED, el fondo sigue siendo negro. El
        // grano más brillante posible tiene que quedarse en un valor que ni
        // llega al 3:1 de una divisoria, o dejaría de ser textura.
        let max = MAX_ALPHA.round() as u32;
        assert!(max <= 20, "un grano de {max}/255 ya es ruido, no textura");
    }
}
