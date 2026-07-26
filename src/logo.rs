//! La marca de flow: un cubo isométrico con una `f` de pixel-art dentro.
//!
//! Se rasteriza en código en vez de cargar un PNG por dos razones:
//!
//! - Un bitmap fijo se ve borroso a cualquier zoom que no sea el suyo, y la
//!   escala de la app la pone el sistema: puede ser 1, 1,25, 1,5 o cualquier
//!   otra. Rasterizando al número exacto de píxeles que va a ocupar, cada píxel
//!   del logo cae en un píxel de pantalla sea cual sea el factor.
//! - El icono de ventana necesita un búfer RGBA de todas formas, así que la
//!   misma función sirve para la barra de título y para el icono.

use egui::{Color32, ColorImage};

/// La `f`, fila a fila. `#` es tinta.
///
/// Dibujada sobre la misma retícula que la tipografía: trazo de 2 px, remate
/// superior hacia la derecha y travesaño que sobresale por la izquierda.
const GLYPH: [&str; 9] = [
    "..####", "..##.#", "..##..", "######", "..##..", "..##..", "..##..", "..##..", "..##..",
];

/// ¿Cae `(x, y)` dentro de un hexágono de "radio" `r` centrado en el origen?
///
/// Es el hexágono de punta arriba y lados verticales, o sea la silueta de un
/// cubo en proyección isométrica.
fn in_hex(x: f32, y: f32, r: f32) -> bool {
    x.abs() <= r && y.abs() <= r - 0.5 * x.abs()
}

/// Rasteriza la marca a `size`×`size` píxeles RGBA premultiplicado.
///
/// `ink` pinta el trazo y la `f`; el interior queda transparente para que la
/// marca funcione sobre cualquier fondo.
pub fn rasterize(size: usize, ink: Color32) -> Vec<u8> {
    let mut px = vec![Color32::TRANSPARENT; size * size];

    let n = size as f32;
    // Grosor del trazo: 1 px a tamaños pequeños, proporcional a partir de ahí.
    let stroke = (n / 24.0).max(1.0) / n;
    let outer = 0.46;
    let inner = outer * 0.70;

    // La `f` se centra dentro del hexágono interior, con un margen que la deja
    // holgada respecto al trazo.
    let gw = GLYPH[0].len();
    let gh = GLYPH.len();
    let cell = (inner * 2.0 * 0.62) / gh as f32;

    for iy in 0..size {
        for ix in 0..size {
            // Coordenadas normalizadas al centro del píxel, en [-0.5, 0.5].
            let x = (ix as f32 + 0.5) / n - 0.5;
            let y = (iy as f32 + 0.5) / n - 0.5;

            let on_outer = in_hex(x, y, outer) && !in_hex(x, y, outer - stroke);
            let on_inner = in_hex(x, y, inner) && !in_hex(x, y, inner - stroke);

            // Coordenada dentro de la retícula del glifo.
            let gx = ((x / cell) + gw as f32 / 2.0).floor();
            let gy = ((y / cell) + gh as f32 / 2.0).floor();
            let in_glyph = gx >= 0.0
                && gy >= 0.0
                && (gx as usize) < gw
                && (gy as usize) < gh
                && GLYPH[gy as usize].as_bytes()[gx as usize] == b'#';

            if on_outer || on_inner || in_glyph {
                px[iy * size + ix] = ink;
            }
        }
    }

    // `Color32` ya es premultiplicado, así que sirve tal cual.
    px.iter().flat_map(|c| c.to_array()).collect()
}

/// La marca como imagen lista para subir a la GPU.
pub fn color_image(size: usize, ink: Color32) -> ColorImage {
    ColorImage::from_rgba_premultiplied([size, size], &rasterize(size, ink))
}

/// Icono de la ventana, en el formato que espera eframe.
pub fn icon() -> egui::IconData {
    const SIZE: usize = 64;
    egui::IconData {
        rgba: rasterize(SIZE, Color32::WHITE),
        width: SIZE as u32,
        height: SIZE as u32,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn el_hexagono_tiene_la_forma_correcta() {
        // Centro dentro, esquinas fuera, y la punta justo en el borde.
        assert!(in_hex(0.0, 0.0, 0.5));
        assert!(!in_hex(0.5, 0.5, 0.5));
        assert!(in_hex(0.0, 0.49, 0.5));
        // A media anchura, el alto permitido se reduce a la mitad.
        assert!(in_hex(0.5, 0.24, 0.5));
        assert!(!in_hex(0.5, 0.26, 0.5));
    }

    #[test]
    fn la_marca_tiene_tinta() {
        let rgba = rasterize(64, Color32::WHITE);
        assert_eq!(rgba.len(), 64 * 64 * 4);
        let painted = rgba.chunks_exact(4).filter(|p| p[3] > 0).count();
        // Ni vacía ni un cuadrado sólido: es un contorno con un glifo dentro.
        assert!(painted > 100, "demasiado vacía: {painted}");
        assert!(painted < 64 * 64 / 2, "demasiado llena: {painted}");
    }
}
