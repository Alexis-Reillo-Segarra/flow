//! La marca de flow: cuatro barras inclinadas que dibujan la silueta de una
//! `F`, alineadas arriba, de más a menos altura y de más a menos contraste. La
//! caída de contraste es la que da la sensación de *flow*.
//!
//! La fuente de la marca son los SVG de `assets/brand/`, y este módulo es la
//! misma geometría escrita en Rust. **Se rasteriza en código en vez de cargar un
//! PNG** por tres razones:
//!
//! - Un bitmap fijo se ve borroso a cualquier zoom que no sea el suyo, y la
//!   escala de la app la pone el sistema: puede ser 1, 1,25, 1,5 o cualquier
//!   otra. Rasterizando al número exacto de píxeles que va a ocupar, la marca
//!   cae siempre en la retícula de la pantalla.
//! - En la barra de título la marca se pinta **con el acento del tema**, así que
//!   no hay un color que se pueda hornear en un fichero: cambia con `Ctrl-Shift-T`.
//! - El icono de ventana necesita un búfer RGBA de todas formas, así que la
//!   misma geometría sirve para las dos cosas.
//!
//! Lo que sí es un fichero es el icono del **ejecutable** (`assets/brand/windows/`),
//! porque ese lo lee el Explorador de Windows y no pasa por aquí.

use egui::{Color32, ColorImage};

/// `tan(16°)`: la inclinación de las barras.
///
/// Cada barra se inclina sobre su propio centro vertical, así que sube hacia la
/// derecha arriba y baja hacia la izquierda abajo, `SKEW · altura / 2` a cada
/// lado.
const SKEW: f32 = 0.286_745;

/// Muestras por eje dentro de cada píxel.
///
/// Las cuatro barras son diagonales y sin suavizado se verían escalonadas.
/// 3×3 = 9 muestras da 10 niveles de cobertura, que a estos tamaños es más de
/// lo que el ojo separa, y cuesta lo mismo que nada: la marca se rasteriza una
/// vez por tamaño y se cachea.
const AA: usize = 3;

/// La geometría de la marca en las unidades del lienzo que la define.
///
/// Hay dos juegos y la diferencia es a propósito: a 16 px las proporciones del
/// icono grande se empastan —las barras finas se comen el hueco entre ellas— así
/// que la versión pequeña engorda la barra y abre la separación. Es la misma
/// corrección que hace cualquier tipografía entre un titular y un cuerpo de 8.
struct Geom {
    /// Ancho de cada barra, antes de inclinarla.
    bar: f32,
    /// Del arranque de una barra al de la siguiente: ancho más separación.
    pitch: f32,
    /// Altura de las cuatro, de la más alta a la más baja. Todas arrancan
    /// arriba, en `y = 0`.
    alto: [f32; 4],
    /// Cuánta tinta lleva cada barra, de la más viva a la más apagada.
    ///
    /// Son los grises de los SVG resueltos a opacidad sobre el fondo del icono:
    /// `#eeeeee`, `#ababab`, `#717171` y `#424242` sobre `#090909`. Escrito así
    /// y no como colores, la marca se puede pintar con el acento de cualquier
    /// tema conservando la caída.
    tinta: [f32; 4],
}

/// La del icono grande: barras de 26, separación de 8, alturas 120/88/56/30.
const GRANDE: Geom = Geom {
    bar: 26.0,
    pitch: 34.0,
    alto: [120.0, 88.0, 56.0, 30.0],
    tinta: [1.0, 0.707, 0.454, 0.249],
};

/// La del favicon: barras de 3,5, separación de 1,5, alturas 15/11/7/4.
const PEQUENA: Geom = Geom {
    bar: 3.5,
    pitch: 5.0,
    alto: [15.0, 11.0, 7.0, 4.0],
    tinta: [1.0, 0.734, 0.493, 0.297],
};

/// A partir de esta altura en píxeles la marca usa las proporciones grandes.
const CORTE: usize = 24;

impl Geom {
    /// Lo que sobresale por la izquierda la barra más alta al inclinarse.
    fn sangria(&self) -> f32 {
        SKEW * self.alto[0] / 2.0
    }

    /// Ancho de la caja que ocupa la marca ya inclinada.
    ///
    /// Por la izquierda sobresale el pie de la barra más alta; por la derecha,
    /// la cabeza de la más baja.
    fn ancho(&self) -> f32 {
        self.pitch * 3.0 + self.bar + self.sangria() + SKEW * self.alto[3] / 2.0
    }

    /// ¿Cae `(x, y)` —en unidades del lienzo, con el origen en la esquina
    /// superior izquierda de la caja— dentro de la barra `i`?
    fn dentro(&self, i: usize, x: f32, y: f32) -> bool {
        let alto = self.alto[i];
        if y < 0.0 || y > alto {
            return false;
        }
        // Deshacer la inclinación: la barra se inclinó sobre su propio centro,
        // así que se devuelve el punto a la vertical antes de comparar.
        let recto = x - self.sangria() + SKEW * (y - alto / 2.0);
        let base = self.pitch * i as f32;
        recto >= base && recto <= base + self.bar
    }
}

/// La geometría que toca para una marca de `alto` píxeles.
fn geom(alto: usize) -> &'static Geom {
    if alto >= CORTE {
        &GRANDE
    } else {
        &PEQUENA
    }
}

/// Qué caja en píxeles ocupa una marca de `alto` píxeles.
///
/// La marca **no es cuadrada** y quien la coloque tiene que preguntarlo aquí, no
/// suponerlo: **las dos proporciones no son la misma** —1,25 la grande y 1,41 la
/// pequeña, porque engordar la barra y abrir la separación ensancha la marca— y
/// dar por buena la de la grande a 16 px le corta la última barra.
pub fn caja(alto: usize) -> (usize, usize) {
    let g = geom(alto);
    let ancho = (alto as f32 * g.ancho() / g.alto[0]).round() as usize;
    (ancho, alto)
}

/// `src` sobre `dst`, los dos en premultiplicado.
fn sobre(src: Color32, dst: Color32) -> Color32 {
    if src.a() == 255 {
        return src;
    }
    let inv = 1.0 - src.a() as f32 / 255.0;
    let mezcla = |s: u8, d: u8| (s as f32 + d as f32 * inv).round().clamp(0.0, 255.0) as u8;
    Color32::from_rgba_premultiplied(
        mezcla(src.r(), dst.r()),
        mezcla(src.g(), dst.g()),
        mezcla(src.b(), dst.b()),
        mezcla(src.a(), dst.a()),
    )
}

/// Pinta las cuatro barras dentro del rectángulo `(x0, y0, w, h)` de `px`.
///
/// El rectángulo se da en píxeles y la marca lo llena entero: quien llama es
/// quien decide el hueco y el margen.
fn barras(px: &mut [Color32], stride: usize, x0: f32, y0: f32, w: f32, h: f32, ink: Color32) {
    // Un lienzo sin píxeles no es un error, es no tener nada que pintar: puede
    // salir de una ventana de cero de alto mientras se minimiza.
    if stride == 0 || px.is_empty() {
        return;
    }
    let g = geom(h.round() as usize);
    let escala = h / g.alto[0];
    let ancho = g.ancho() * escala;
    // Si la caja no tiene la proporción de la marca, se centra en vez de
    // estirarse: una marca deformada es peor que una marca con aire al lado.
    let x0 = x0 + (w - ancho) / 2.0;

    // Solo se recorren las filas y columnas que la marca puede tocar.
    let ini_x = x0.floor().max(0.0) as usize;
    let fin_x = ((x0 + ancho).ceil() as usize).min(stride);
    let ini_y = y0.floor().max(0.0) as usize;
    let fin_y = ((y0 + h).ceil() as usize).min(px.len() / stride);
    let paso = 1.0 / AA as f32;

    for iy in ini_y..fin_y {
        for ix in ini_x..fin_x {
            // Cobertura de cada barra dentro de este píxel, por muestreo.
            let mut cobertura = [0u32; 4];
            for sy in 0..AA {
                for sx in 0..AA {
                    let x = (ix as f32 + (sx as f32 + 0.5) * paso - x0) / escala;
                    let y = (iy as f32 + (sy as f32 + 0.5) * paso - y0) / escala;
                    for (i, c) in cobertura.iter_mut().enumerate() {
                        if g.dentro(i, x, y) {
                            *c += 1;
                            // Las barras no se solapan: encontrada la suya, fuera.
                            break;
                        }
                    }
                }
            }

            for (i, c) in cobertura.iter().enumerate() {
                if *c == 0 {
                    continue;
                }
                let a = *c as f32 / (AA * AA) as f32;
                let capa = ink.gamma_multiply(g.tinta[i] * a);
                let destino = &mut px[iy * stride + ix];
                *destino = sobre(capa, *destino);
            }
        }
    }
}

/// La marca suelta, sin fondo, a `w`×`h` píxeles.
///
/// Es lo que va en la barra de título: fondo transparente y la tinta del tema,
/// para que funcione sobre cualquier fondo y cambie al cambiar de tema.
pub fn rasterize(w: usize, h: usize, ink: Color32) -> Vec<u8> {
    let mut px = vec![Color32::TRANSPARENT; w * h];
    barras(&mut px, w, 0.0, 0.0, w as f32, h as f32, ink);
    px.iter().flat_map(|c| c.to_array()).collect()
}

/// La marca como imagen lista para subir a la GPU.
pub fn color_image(w: usize, h: usize, ink: Color32) -> ColorImage {
    ColorImage::from_rgba_premultiplied([w, h], &rasterize(w, h, ink))
}

/// Cobertura de un rectángulo redondeado dentro del píxel `(ix, iy)`.
///
/// El icono es un cuadrado con las esquinas redondeadas, y sin suavizar esas
/// cuatro curvas se verían dentadas justo en lo primero que se mira.
fn cobertura_redondeada(ix: usize, iy: usize, lado: f32, r: f32) -> f32 {
    let paso = 1.0 / AA as f32;
    let mut dentro = 0;
    for sy in 0..AA {
        for sx in 0..AA {
            let x = ix as f32 + (sx as f32 + 0.5) * paso;
            let y = iy as f32 + (sy as f32 + 0.5) * paso;
            // Distancia a la esquina más cercana, medida desde el centro del
            // arco: dentro del radio en las dos direcciones o no es esquina.
            let dx = (r - x).max(x - (lado - r)).max(0.0);
            let dy = (r - y).max(y - (lado - r)).max(0.0);
            if dx * dx + dy * dy <= r * r {
                dentro += 1;
            }
        }
    }
    dentro as f32 / (AA * AA) as f32
}

/// Icono de la ventana, en el formato que espera eframe.
///
/// Aquí sí va el icono entero —la placa oscura con las esquinas redondeadas y la
/// marca en sus grises— y no la marca suelta: esto acaba en la barra de tareas y
/// en el conmutador de ventanas, sobre un fondo que pone el sistema y que puede
/// ser claro u oscuro. Una marca transparente en blanco desaparecería en la
/// mitad de los escritorios; la placa se ve igual en todos.
///
/// Los colores son fijos y no los del tema: la identidad de la app en el sistema
/// no cambia porque hayas elegido Gruvbox.
pub fn icon() -> egui::IconData {
    // 256 es el tamaño que pide Windows para el icono grande. Lo que haga falta
    // más pequeño lo reduce el sistema desde aquí, que es mejor que ampliar.
    const LADO: usize = 256;
    const CANVAS: f32 = 280.0;

    let mut px = vec![Color32::TRANSPARENT; LADO * LADO];

    // La placa: `#090909`, radio 44 sobre un lienzo de 280.
    let fondo = Color32::from_rgb(0x09, 0x09, 0x09);
    let radio = 44.0 / CANVAS * LADO as f32;
    for iy in 0..LADO {
        for ix in 0..LADO {
            let c = cobertura_redondeada(ix, iy, LADO as f32, radio);
            if c > 0.0 {
                px[iy * LADO + ix] = fondo.gamma_multiply(c);
            }
        }
    }

    // La marca, en el sitio exacto que ocupa en el SVG: de `x = 58,795` a
    // `x = 208,301` y de `y = 80` a `y = 200` del lienzo de 280.
    let u = LADO as f32 / CANVAS;
    barras(
        &mut px,
        LADO,
        58.795 * u,
        80.0 * u,
        149.506 * u,
        120.0 * u,
        Color32::from_rgb(0xee, 0xee, 0xee),
    );

    egui::IconData {
        rgba: px.iter().flat_map(|c| c.to_array()).collect(),
        width: LADO as u32,
        height: LADO as u32,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Opacidad media de la columna de píxeles `x` de una marca de `w`×`h`.
    fn tinta_en(rgba: &[u8], w: usize, h: usize, x: usize) -> f32 {
        let suma: u32 = (0..h).map(|y| rgba[(y * w + x) * 4 + 3] as u32).sum();
        suma as f32 / h as f32 / 255.0
    }

    #[test]
    fn la_marca_tiene_la_proporcion_de_los_svg() {
        // Las cajas de `flow-mark-dark.svg` y de `flow-favicon.svg`, medidas de
        // la primera barra a la última. Si esto cambia, la marca dibujada y los
        // ficheros de `assets/brand/` dejan de ser la misma marca.
        for (g, ancho, alto, quien) in [
            (&GRANDE, 208.301 - 58.795, 120.0, "flow-mark-dark.svg"),
            (&PEQUENA, 25.823 - 4.599, 15.0, "flow-favicon.svg"),
        ] {
            assert!(
                (g.ancho() - ancho).abs() < 1e-2 && (g.alto[0] - alto).abs() < 1e-3,
                "la caja no es la de {quien}: {}×{} en vez de {ancho}×{alto}",
                g.ancho(),
                g.alto[0]
            );
        }
    }

    #[test]
    fn la_caja_le_da_a_la_marca_justo_lo_que_ocupa() {
        // Las dos proporciones no son la misma —la pequeña es más ancha porque
        // engorda la barra— así que una caja calculada con la proporción que no
        // toca corta la última barra por la derecha. Se comprueba comparando la
        // tinta que cabe en la caja con la que sale en un lienzo del doble de
        // ancho: si la caja recorta, falta tinta.
        for alto in [12usize, 16, 20, 24, 64, 120] {
            let (ancho, _) = caja(alto);
            let justa: u64 = rasterize(ancho, alto, Color32::WHITE)
                .chunks_exact(4)
                .map(|p| p[3] as u64)
                .sum();
            let holgada: u64 = rasterize(ancho * 2, alto, Color32::WHITE)
                .chunks_exact(4)
                .map(|p| p[3] as u64)
                .sum();
            // El 1% es el margen del redondeo de la caja a píxeles enteros.
            assert!(
                justa * 100 >= holgada * 99,
                "a {alto} px de alto la caja recorta la marca: {justa} contra {holgada}"
            );
        }
    }

    #[test]
    fn la_marca_tiene_tinta() {
        let (w, h) = (150, 120);
        let rgba = rasterize(w, h, Color32::WHITE);
        assert_eq!(rgba.len(), w * h * 4);
        let pintados = rgba.chunks_exact(4).filter(|p| p[3] > 0).count();
        // Ni vacía ni un bloque sólido: son cuatro barras con hueco entre ellas
        // y mucho aire debajo de las tres cortas.
        assert!(pintados > w * h / 8, "demasiado vacía: {pintados}");
        assert!(pintados < w * h / 2, "demasiado llena: {pintados}");
    }

    #[test]
    fn las_barras_pierden_tinta_de_izquierda_a_derecha() {
        // Es *la* idea de la marca: la caída de contraste es lo que la hace
        // fluir. Se mide en el centro de cada barra, a media altura de la más
        // baja para que las cuatro estén presentes.
        let (w, h) = (300, 240);
        let rgba = rasterize(w, h, Color32::WHITE);
        let g = &GRANDE;
        let escala = h as f32 / g.alto[0];

        let mut previa = f32::INFINITY;
        for i in 0..4 {
            // Centro de la barra, ya inclinado, a la altura del pie de la más baja.
            let y = g.alto[3] / 2.0;
            let x = g.sangria() + g.pitch * i as f32 + g.bar / 2.0 - SKEW * (y - g.alto[i] / 2.0);
            let col = (x * escala).round() as usize;
            let tinta = tinta_en(&rgba, w, h, col);
            assert!(tinta > 0.0, "la barra {i} no tiene tinta en x={col}");
            assert!(
                tinta < previa,
                "la barra {i} no es más apagada que la anterior: {tinta} contra {previa}"
            );
            previa = tinta;
        }
    }

    #[test]
    fn las_barras_se_inclinan_a_la_derecha() {
        // La primera columna con tinta de la fila de arriba tiene que caer a la
        // derecha de la de la fila de abajo: si sale al revés, la marca está
        // reflejada y deja de leerse como una `F`.
        let (w, h) = (150, 120);
        let rgba = rasterize(w, h, Color32::WHITE);
        let primera = |y: usize| (0..w).find(|&x| rgba[(y * w + x) * 4 + 3] > 32);
        let arriba = primera(1).expect("la fila de arriba está vacía");
        let abajo = primera(h - 2).expect("la fila de abajo está vacía");
        assert!(
            arriba > abajo,
            "la inclinación va al revés: arriba en {arriba}, abajo en {abajo}"
        );
    }

    #[test]
    fn el_icono_es_una_placa_con_las_esquinas_redondeadas() {
        let icono = icon();
        let lado = icono.width as usize;
        assert_eq!(icono.rgba.len(), lado * lado * 4);
        let alfa = |x: usize, y: usize| icono.rgba[(y * lado + x) * 4 + 3];
        // La esquina de verdad está fuera del radio; el centro es opaco.
        assert_eq!(alfa(0, 0), 0, "la esquina no está redondeada");
        assert_eq!(alfa(lado / 2, lado / 2), 255, "la placa tiene agujeros");
        // Y la marca se ve sobre la placa: el gris claro contra `#090909`.
        let claro = icono
            .rgba
            .chunks_exact(4)
            .filter(|p| p[0] > 0x80 && p[3] == 255)
            .count();
        assert!(claro > lado * lado / 50, "la marca no se ve: {claro}");
    }
}
