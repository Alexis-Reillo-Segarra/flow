//! La terminal de un panel.
//!
//! Se pinta fila a fila desde la rejilla del emulador. Cada fila se monta como
//! un `LayoutJob` fusionando tramos de celdas con el mismo `Pen`, así que una
//! línea sin color es un solo tramo y una llena de ANSI son unos pocos. Con
//! `show_rows` solo se construyen las filas visibles: da igual que el
//! scrollback tenga 5000 líneas, y da igual que haya ocho terminales a la vez.

use std::time::Duration;

use egui::{
    text::{ByteIndex, LayoutJob, LayoutSection, TextFormat},
    Color32, FontId, Stroke, Ui,
};

use crate::agent::Agent;
use crate::term::Cell;
use crate::theme;

/// Dibuja el contenido del terminal dentro del hueco que le ha dado el panel.
///
/// `settled` dice si el panel ya llegó a su sitio. Mientras se está moviendo no
/// se le toca el tamaño al PTY: sería un `ioctl` y una rejilla nueva en cada
/// frame de la animación, y el proceso vería veinte tamaños distintos para
/// acabar exactamente donde iba a acabar de todas formas.
pub fn surface(ui: &mut Ui, agent: &mut Agent, focused: bool, settled: bool, time: f64) {
    let font = theme::mono(theme::MONO_SM);
    // JetBrains Mono es monoespaciada, así que cualquier glifo sirve para medir
    // el avance de celda.
    let (char_w, row_h) = ui.fonts_mut(|f| (f.glyph_width(&font, 'M'), f.row_height(&font)));
    if char_w <= 0.0 || row_h <= 0.0 {
        return;
    }

    let avail = ui.available_size();
    // Se descuenta la barra de scroll para que la última columna no quede
    // debajo de ella.
    let cols = ((avail.x - 8.0) / char_w).floor().max(20.0) as u16;
    let rows = (avail.y / row_h).floor().max(4.0) as u16;
    if settled {
        agent.resize(cols, rows);
    }

    let (cursor_line, cursor_col) = agent.term().cursor();
    // El bloque del cursor lleva el carácter en vídeo inverso —negro encima—,
    // así que se comporta como fondo de un texto y le toca la variante clara
    // del acento, no el verde de marca.
    let cursor_color = if !agent.state.is_running() {
        None
    } else if focused {
        ui.ctx().request_repaint_after(Duration::from_millis(120));
        ((time * 1.6).fract() < 0.55).then_some(theme::pal().accent_text)
    } else {
        // En un panel sin foco el cursor no parpadea: ocho cursores parpadeando
        // a destiempo es justo el ruido que esta interfaz intenta no tener. Se
        // queda como una marca apagada que dice por dónde va.
        Some(theme::pal().accent_text.gamma_multiply(0.3))
    };

    ui.spacing_mut().item_spacing.y = 0.0;
    let total = agent.term().total_lines();

    let out = egui::ScrollArea::vertical()
        // Cada panel tiene su propio scroll y su propia posición dentro del
        // scrollback; sin distinguirlos, ocho terminales compartirían una.
        .id_salt(agent.id)
        .auto_shrink([false, false])
        // Altura recortada a un número entero de filas. Las filas miden todas
        // lo mismo y el scroll va pegado abajo, así que el sobrante no se
        // reparte: se lo come entero la fila de arriba, que aparecería cortada
        // por la mitad.
        .max_height(rows as f32 * row_h)
        .stick_to_bottom(agent.follow)
        .show_rows(ui, row_h, total, |ui, range| {
            ui.spacing_mut().item_spacing.y = 0.0;
            for i in range {
                let cursor = cursor_color
                    .filter(|_| i == cursor_line)
                    .map(|c| (cursor_col, c));
                let job = match agent.term().line(i) {
                    Some(cells) => line_job(cells, &font, cursor),
                    None => LayoutJob::default(),
                };
                ui.add(
                    egui::Label::new(job)
                        .selectable(true)
                        .wrap_mode(egui::TextWrapMode::Extend),
                );
            }
        });

    // Si el usuario sube por el scrollback, dejamos de seguir el final; en
    // cuanto vuelve abajo, se reengancha. Es lo que uno espera de una terminal,
    // y así no hace falta ningún botón de "seguir".
    agent.follow = out.state.offset.y + out.inner_rect.height() >= out.content_size.y - row_h * 0.5;
}

/// Monta una fila de celdas en un `LayoutJob`, fusionando tramos con el mismo
/// estilo. `cursor` marca la columna que se pinta en vídeo inverso y con qué
/// color de bloque.
fn line_job(cells: &[Cell], font: &FontId, cursor: Option<(usize, Color32)>) -> LayoutJob {
    let mut job = LayoutJob::default();
    job.wrap.max_width = f32::INFINITY;
    job.break_on_newline = false;

    let cursor_col = cursor.map(|(c, _)| c);

    // Las celdas vacías del final no se dibujan; solo se conservan si el cursor
    // vive más allá del último carácter (el caso normal al escribir).
    let mut end = cells.len();
    while end > 0 && cells[end - 1] == Cell::default() {
        end -= 1;
    }
    if let Some(c) = cursor_col {
        end = end.max(c + 1);
    }

    let at = |k: usize| cells.get(k).copied().unwrap_or_default();

    let mut i = 0;
    while i < end {
        let is_cursor = cursor_col == Some(i);
        let pen = at(i).pen;

        // Los caracteres van directos al texto del job. Antes se juntaba el
        // tramo en un `String` aparte que `append` copiaba dentro y se tiraba:
        // una reserva por tramo, por fila y por frame, y con ocho terminales en
        // pantalla eso son unos cuantos miles por segundo que no hacían nada.
        let start = ByteIndex(job.text.len());
        let mut j = i;
        while j < end && at(j).pen == pen && (cursor_col == Some(j)) == is_cursor {
            job.text.push(at(j).ch);
            j += 1;
        }

        let (fg, bg) = match cursor {
            Some((_, block)) if is_cursor => (theme::pal().bg, block),
            _ => (
                pen.fg_color(),
                pen.bg_color().unwrap_or(Color32::TRANSPARENT),
            ),
        };

        job.sections.push(LayoutSection {
            leading_space: 0.0,
            byte_range: start..ByteIndex(job.text.len()),
            format: TextFormat {
                font_id: font.clone(),
                color: fg,
                background: bg,
                underline: if pen.underline {
                    Stroke::new(1.0, fg)
                } else {
                    Stroke::NONE
                },
                ..Default::default()
            },
        });
        i = j;
    }

    // Un job vacío mide cero y rompería la alineación de `show_rows`, que asume
    // filas de altura constante.
    if job.is_empty() {
        job.append(
            " ",
            0.0,
            TextFormat {
                font_id: font.clone(),
                color: Color32::TRANSPARENT,
                ..Default::default()
            },
        );
    }
    job
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testkit::{self, Ventana};

    /// El panel le dice al proceso de qué tamaño es su terminal, y solo cuando
    /// la rejilla ha dejado de moverse: durante la animación el hueco cambia
    /// cada frame, y un `SIGWINCH` por frame es lo que hace que una TUI se
    /// repinte entera sesenta veces por segundo.
    #[test]
    fn el_tamano_se_le_dice_al_proceso_cuando_la_rejilla_para() {
        let mut v = Ventana::nueva();
        let mut a = testkit::agente(1, "quieto", testkit::quieto());
        let antes = a.term().size();

        v.frame(|ui| surface(ui, &mut a, true, false, 0.0));
        assert_eq!(
            a.term().size(),
            antes,
            "se redimensionó el terminal con la rejilla todavía moviéndose"
        );

        v.frame(|ui| surface(ui, &mut a, true, true, 0.0));
        assert_ne!(
            a.term().size(),
            antes,
            "la rejilla se asentó y no se le dijo el tamaño al proceso"
        );
        a.kill();
    }

    /// Un hueco ridículo no le pide al terminal una rejilla de cero columnas:
    /// por debajo hay un mínimo, porque una rejilla vacía no tiene dónde
    /// escribir.
    #[test]
    fn un_hueco_diminuto_no_deja_el_terminal_sin_columnas() {
        let mut v = Ventana::de(120.0, 90.0);
        let mut a = testkit::agente(1, "quieto", testkit::quieto());
        v.frame(|ui| surface(ui, &mut a, true, true, 0.0));
        let (cols, rows) = a.term().size();
        assert!(cols >= 20, "el terminal se quedó en {cols} columnas");
        assert!(rows >= 4, "el terminal se quedó en {rows} filas");
        a.kill();
    }

    /// Mientras la vista esté pegada al final, sigue pegada. Es lo que hace que
    /// la salida de un proceso se lea sola sin tocar nada.
    #[test]
    fn la_vista_arranca_pegada_al_final() {
        let mut v = Ventana::nueva();
        let mut a = testkit::agente(1, "eco", testkit::saluda());
        testkit::deja_hablar(&mut a);

        for _ in 0..3 {
            v.frame(|ui| surface(ui, &mut a, true, true, 0.0));
        }
        assert!(a.follow, "la vista se despegó del final sin que nadie subiera");
    }

    /// El cursor parpadea en el panel con foco y no en los demás —ocho cursores
    /// a destiempo son justo el ruido que esta interfaz no quiere—, y en un
    /// proceso muerto no hay cursor que enseñar.
    #[test]
    fn el_cursor_se_dibuja_segun_el_foco_y_la_vida() {
        let mut v = Ventana::nueva();
        let mut a = testkit::agente(1, "quieto", testkit::quieto());
        testkit::deja_hablar(&mut a);

        // Con foco, las dos mitades del parpadeo; sin foco, apagado.
        for t in [0.0, 0.45] {
            v.frame(|ui| surface(ui, &mut a, true, true, t));
        }
        v.frame(|ui| surface(ui, &mut a, false, true, 0.0));
        a.kill();

        let mut muerto = testkit::agente_terminado(2, "eco", crate::agent::State::Exited(0));
        v.frame(|ui| surface(ui, &mut muerto, true, true, 0.0));
    }

    /// Una fila con colores, negrita, subrayado y vídeo inverso se monta
    /// fusionando los tramos que comparten estilo: una línea sin color tiene que
    /// ser **un** tramo de texto y no doscientos.
    #[test]
    fn los_tramos_con_el_mismo_estilo_se_juntan() {
        let mut v = Ventana::nueva();
        let mut a = testkit::agente(1, "quieto", testkit::quieto());
        // Se le mete la salida al emulador directamente: lo que se prueba es el
        // montaje de la fila, no que un shell escriba lo que se le pide.
        a.feed_para_test(b"\x1b[1;31mrojo\x1b[0m normal \x1b[7minverso\x1b[0m");

        let job = v.frame(|_| {
            let font = theme::mono(theme::MONO_SM);
            let cells = a.term().line(0).expect("no hay ninguna fila").to_vec();
            line_job(&cells, &font, None)
        });
        assert!(
            job.sections.len() >= 3,
            "los tres estilos de la fila salieron en {} tramos",
            job.sections.len()
        );
        assert!(
            job.sections.len() < 20,
            "una fila de tres estilos se partió en {} tramos",
            job.sections.len()
        );
        a.kill();
    }

    /// Una fila vacía mide cero y rompería la alineación de las que vienen
    /// detrás, así que se monta con algo que ocupe una fila.
    #[test]
    fn una_fila_vacia_sigue_ocupando_una_fila() {
        let mut v = Ventana::nueva();
        let (vacia, llena) = v.frame(|ui| {
            let font = theme::mono(theme::MONO_SM);
            let vacia = line_job(&[], &font, None);
            let llena = line_job(&[], &font, Some((0, theme::pal().accent_text)));
            (
                ui.fonts_mut(|f| f.layout_job(vacia)).rect.height(),
                ui.fonts_mut(|f| f.layout_job(llena)).rect.height(),
            )
        });
        assert!(vacia > 0.0, "una fila vacía midió cero");
        assert!((vacia - llena).abs() < 1.0);
    }
}
