//! El selector de temas.
//!
//! Modal como el formulario de lanzamiento, y por lo mismo: control total sobre
//! el borde y sobre el velo. Aquí, además, hay una razón propia para que sea un
//! modal y no un control fijo en la barra: elegir tema es algo que se hace una
//! vez y no se vuelve a tocar, así que su sitio es un atajo —`Ctrl-Shift-T`— y
//! no un icono ocupando chrome para siempre.
//!
//! **Lo que se elige se ve mientras se elige.** Moverse por la lista aplica el
//! tema de verdad, a la app entera, no a una miniatura: los ocho paneles, el
//! grano, la salida de los procesos. Un tema se juzga con la terminal llena de
//! texto, y una muestra de 60×20 píxeles no dice nada de lo que vas a mirar seis
//! horas. `Esc` deja las cosas como estaban, que es lo que hace que probar no
//! cueste nada.

use egui::{vec2, Align, Context, CornerRadius, Id, Key, Layout, Rect, Sense, Stroke, Ui};

use crate::theme;
use crate::ui::{widgets, Action};

/// Ancho al que aspira el cuadro. Si no cabe, se encoge.
const BOX_W: f32 = 380.0;
/// Alto de una fila de la lista.
const ROW_H: f32 = 30.0;
/// Lado de cada muestra de color.
const SWATCH: f32 = 9.0;

/// Qué tema se está mirando y cuál había antes de abrir esto.
#[derive(Default)]
pub struct Picker {
    pub open: bool,
    /// El que está resaltado, que es también el que está aplicado: aquí no hay
    /// diferencia entre lo señalado y lo puesto, y esa es la idea.
    selected: usize,
    /// A dónde volver si se cancela.
    previous: usize,
}

impl Picker {
    /// Abre el selector recordando desde dónde, para poder deshacer.
    pub fn show(&mut self) {
        self.open = true;
        self.selected = theme::active();
        self.previous = self.selected;
    }

    pub fn close(&mut self) {
        self.open = false;
    }

    /// Resalta —y aplica— el tema `i`.
    pub fn pick(&mut self, i: usize) {
        self.selected = i.min(theme::themes().len() - 1);
        theme::set_active(self.selected);
    }

    /// El tema al que hay que volver al cancelar.
    pub fn previous(&self) -> usize {
        self.previous
    }

    /// El que se queda al aceptar.
    pub fn selected(&self) -> usize {
        self.selected
    }

    /// Qué tema queda `delta` filas más allá, dando la vuelta por los extremos:
    /// con cinco temas, bajar desde el último para llegar al primero es más
    /// rápido que subir cuatro veces.
    ///
    /// Devuelve el índice en vez de aplicarlo porque quien dibuja no muta: la
    /// vista propone `Action::PickTheme` y `Flow::apply` lo resuelve al final
    /// del frame, como todo lo demás.
    fn step(&self, delta: isize) -> usize {
        let n = theme::themes().len() as isize;
        (self.selected as isize + delta).rem_euclid(n) as usize
    }
}

pub fn show(ctx: &Context, picker: &Picker) -> Option<Action> {
    if !picker.open {
        return None;
    }
    let mut action = None;

    // Velo, pero apenas: aquí el fondo no es contexto que se pueda apagar, es
    // **lo que se está eligiendo**. El del formulario de lanzamiento va a 190
    // porque allí lo de detrás solo tiene que reconocerse; este empezó copiando
    // aquello a 120 y con eso un tema se veía a la mitad de su brillo, o sea que
    // se elegía a ciegas. Lo justo para que el cuadro se despegue y ni un punto
    // más.
    widgets::veil(ctx, "themes-dim", 55);

    if ctx.input(|i| i.key_pressed(Key::Escape)) {
        return Some(Action::CancelThemes);
    }
    if ctx.input(|i| i.key_pressed(Key::Enter)) {
        return Some(Action::ConfirmThemes);
    }
    if ctx.input(|i| i.key_pressed(Key::ArrowDown)) {
        action = Some(Action::PickTheme(picker.step(1)));
    }
    if ctx.input(|i| i.key_pressed(Key::ArrowUp)) {
        action = Some(Action::PickTheme(picker.step(-1)));
    }

    let screen = ctx.content_rect();
    let margin = theme::GAP * 2.0;
    let box_w = (screen.width() - margin * 2.0).clamp(200.0, BOX_W);
    let max_list = (screen.height() - margin * 2.0 - 110.0).max(ROW_H);

    egui::Area::new(Id::new("themes"))
        .order(egui::Order::Foreground)
        .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
        .show(ctx, |ui| {
            egui::Frame::new()
                .fill(theme::pal().raised)
                .stroke(Stroke::new(1.0, theme::pal().line_hi))
                .corner_radius(CornerRadius::same(theme::RADIUS))
                .inner_margin(egui::Margin::same(14))
                .show(ui, |ui| {
                    ui.set_width(box_w - 28.0);
                    ui.spacing_mut().item_spacing.y = 6.0;

                    ui.label(
                        egui::RichText::new("TEMA")
                            .font(theme::sans(theme::SANS_MD))
                            .color(theme::pal().accent_text),
                    );
                    widgets::hline(ui, theme::pal().line);
                    ui.add_space(2.0);

                    egui::ScrollArea::vertical()
                        .max_height(max_list)
                        .auto_shrink([false, true])
                        .show(ui, |ui| {
                            ui.spacing_mut().item_spacing.y = 0.0;
                            for i in 0..theme::themes().len() {
                                if row(ui, i, i == picker.selected) {
                                    action = Some(Action::PickTheme(i));
                                }
                            }
                        });

                    ui.add_space(8.0);
                    widgets::hline(ui, theme::pal().line);
                    ui.add_space(8.0);

                    // Los botones se quedan su fila entera y el rótulo va debajo,
                    // en vez de compartir línea como en el formulario de
                    // lanzamiento: aquí el texto de ayuda es más largo que allí y
                    // los dos botones se le montaban encima en cuanto la ventana
                    // se estrechaba un poco.
                    // El `horizontal` de fuera no es decorativo: sin él, el ui de
                    // derecha a izquierda se queda con **toda** la altura que
                    // quede y el cuadro crece hasta salirse de la ventana por
                    // arriba y por abajo, dejando a la vista solo la franja de
                    // los botones.
                    ui.horizontal(|ui| {
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.spacing_mut().item_spacing.x = 6.0;
                            if widgets::button(ui, "GUARDAR", theme::pal().accent_text).clicked() {
                                action = Some(Action::ConfirmThemes);
                            }
                            if widgets::button(ui, "CANCELAR", theme::pal().text_dim).clicked() {
                                action = Some(Action::CancelThemes);
                            }
                        });
                    });
                    ui.add_space(6.0);
                    hint(ui, "↑↓ prueba · Enter guarda · Esc deshace");

                    // Dónde va a quedar escrito, para quien quiera escribirse el
                    // suyo. Es la única pista que hay del fichero, y va aquí
                    // porque este es el momento en que a alguien le importa.
                    if let Some(path) = crate::config::file() {
                        ui.add_space(2.0);
                        hint(ui, &format!("tus temas: {}", path.display()));
                    }
                });
        });

    action
}

/// Una línea de ayuda, recortada al ancho del cuadro. Recortar importa porque
/// una de las dos es una ruta, y una ruta larga no se puede acortar sola.
fn hint(ui: &mut Ui, text: &str) {
    let galley = widgets::fit(
        ui,
        text,
        theme::mono(theme::MONO_XS),
        theme::pal().text_faint,
        ui.available_width(),
    );
    ui.add(egui::Label::new(galley));
}

/// Una fila: el nombre, de dónde viene y sus colores. Devuelve si se ha pulsado.
///
/// Las muestras no son decoración: son las que dicen que el tema que se llama
/// `nord` es el azulado, y las que hacen que la lista se pueda leer de un
/// vistazo en vez de nombre a nombre. Van a esquina viva, como todo lo que es
/// una fila de una lista.
fn row(ui: &mut Ui, i: usize, selected: bool) -> bool {
    let p = &theme::themes()[i];
    let width = ui.available_width();

    // Lo que ocupan las muestras por la derecha, más su aire. El texto se
    // compone recortado a lo que queda: un tema propio puede llamarse como
    // quiera, y sin esto el nombre largo se dibujaría por debajo de los colores.
    let swatches = 10.0 + SWATCH * 5.0 + 3.0 * 4.0;
    let text_w = (width - 10.0 - swatches - 8.0).max(20.0);

    let text_color = if selected {
        theme::pal().text_hi
    } else {
        theme::pal().text_dim
    };
    let name = widgets::fit(ui, &p.name, theme::sans(theme::SANS_SM), text_color, text_w);
    let about = widgets::fit(
        ui,
        &p.about,
        theme::mono(theme::MONO_XS),
        theme::pal().text_faint,
        text_w,
    );

    let (rect, resp) = ui.allocate_exact_size(vec2(width, ROW_H), Sense::click());
    let painter = ui.painter();

    if selected {
        painter.rect_filled(rect, CornerRadius::ZERO, theme::pal().sel);
        // La barra del que manda, en el acento: la misma señal que el marco del
        // panel con foco, sin repetir el degradado en algo de 2 px.
        painter.rect_filled(
            Rect::from_min_size(rect.min, vec2(2.0, rect.height())),
            CornerRadius::ZERO,
            theme::pal().accent,
        );
    } else if resp.hovered() {
        painter.rect_filled(rect, CornerRadius::ZERO, theme::pal().hover);
    }

    painter.galley(
        egui::pos2(rect.left() + 10.0, rect.center().y - 11.0),
        name,
        text_color,
    );
    painter.galley(
        egui::pos2(rect.left() + 10.0, rect.center().y + 1.0),
        about,
        theme::pal().text_faint,
    );

    // Las cinco que cuentan: el fondo, el acento y los tres estados. El fondo va
    // con su divisoria porque sobre el negro de flow, sin borde, no habría nada
    // que ver.
    let colores = [p.bg, p.accent_text, p.green, p.amber, p.red];
    let mut x = rect.right() - 10.0 - SWATCH;
    for (k, c) in colores.iter().enumerate().rev() {
        let sq = Rect::from_min_size(
            egui::pos2(x.round(), (rect.center().y - SWATCH * 0.5).round()),
            vec2(SWATCH, SWATCH),
        );
        painter.rect_filled(sq, CornerRadius::ZERO, *c);
        if k == 0 {
            painter.rect_stroke(
                sq,
                CornerRadius::ZERO,
                Stroke::new(1.0, p.line_hi),
                egui::epaint::StrokeKind::Inside,
            );
        }
        x -= SWATCH + 3.0;
    }

    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    // Pintada a mano: para AccessKit no existe hasta que se le dice qué es.
    resp.widget_info(|| {
        egui::WidgetInfo::selected(
            egui::WidgetType::RadioButton,
            true,
            selected,
            format!("{}, {}", p.name, p.about),
        )
    });
    resp.clicked()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testkit::Ventana;

    /// Elegir tema es mirarlo puesto: no hay diferencia entre lo señalado y lo
    /// aplicado, y esa es la idea. Un tema se juzga con la terminal llena de
    /// texto, no en una miniatura.
    #[test]
    fn moverse_por_la_lista_aplica_el_tema_de_verdad() {
        let antes = theme::active();
        let mut p = Picker::default();
        p.show();

        p.pick(2);
        assert_eq!(p.selected(), 2);
        assert_eq!(theme::active(), 2, "señalar un tema no lo aplicó");

        theme::set_active(antes);
    }

    /// Se recuerda desde dónde se abrió, para poder deshacer: probar no cuesta
    /// nada porque Esc deja las cosas como estaban.
    #[test]
    fn cancelar_devuelve_al_que_habia() {
        let antes = theme::active();
        theme::set_active(1);

        let mut p = Picker::default();
        p.show();
        assert_eq!(p.previous(), 1);
        p.pick(3);
        assert_eq!(theme::active(), 3);

        theme::set_active(p.previous());
        assert_eq!(theme::active(), 1);
        theme::set_active(antes);
    }

    /// Un índice fuera de la lista no puede dejar la interfaz sin colores.
    #[test]
    fn un_indice_imposible_se_recorta_a_la_lista() {
        let antes = theme::active();
        let mut p = Picker::default();
        p.show();
        p.pick(9999);
        assert_eq!(p.selected(), theme::themes().len() - 1);
        theme::set_active(antes);
    }

    /// Las flechas dan la vuelta por los extremos: con cinco temas, bajar desde
    /// el último para llegar al primero es más rápido que subir cuatro veces.
    #[test]
    fn la_lista_da_la_vuelta_por_los_extremos() {
        let antes = theme::active();
        let n = theme::themes().len();
        let mut p = Picker::default();
        p.show();

        p.pick(0);
        assert_eq!(p.step(-1), n - 1, "subir desde el primero no dio la vuelta");
        p.pick(n - 1);
        assert_eq!(p.step(1), 0, "bajar desde el último no dio la vuelta");
        theme::set_active(antes);
    }

    #[test]
    fn cerrado_no_dibuja_nada() {
        let mut v = Ventana::nueva();
        let p = Picker::default();
        assert!(v.frame_ctx(|ctx| show(ctx, &p)).is_none());
    }

    /// El teclado del selector: Esc deshace, Enter se queda con lo que estés
    /// probando y las flechas recorren la lista.
    #[test]
    fn el_teclado_recorre_confirma_y_cancela() {
        let antes = theme::active();
        let mut v = Ventana::nueva();
        let mut p = Picker::default();
        p.show();
        p.pick(0);
        v.frame_ctx(|ctx| show(ctx, &p));

        v.tecla(egui::Key::ArrowDown, egui::Modifiers::NONE);
        assert!(matches!(
            v.frame_ctx(|ctx| show(ctx, &p)),
            Some(Action::PickTheme(1))
        ));

        v.tecla(egui::Key::ArrowUp, egui::Modifiers::NONE);
        let arriba = theme::themes().len() - 1;
        assert!(matches!(
            v.frame_ctx(|ctx| show(ctx, &p)),
            Some(Action::PickTheme(i)) if i == arriba
        ));

        v.tecla(egui::Key::Enter, egui::Modifiers::NONE);
        assert!(matches!(
            v.frame_ctx(|ctx| show(ctx, &p)),
            Some(Action::ConfirmThemes)
        ));

        v.tecla(egui::Key::Escape, egui::Modifiers::NONE);
        assert!(matches!(
            v.frame_ctx(|ctx| show(ctx, &p)),
            Some(Action::CancelThemes)
        ));
        theme::set_active(antes);
    }

    /// Los cinco temas se dibujan con sus muestras, y en una ventana en la que
    /// la lista no cabe entera: el alto de la lista se recorta a lo que haya.
    #[test]
    fn el_selector_se_dibuja_con_todos_los_temas() {
        let antes = theme::active();
        for (ancho, alto) in [(1480.0, 900.0), (760.0, 460.0), (320.0, 200.0)] {
            let mut v = Ventana::de(ancho, alto);
            let mut p = Picker::default();
            p.show();
            for i in 0..theme::themes().len() {
                p.pick(i);
                v.frame_ctx(|ctx| show(ctx, &p));
            }
        }
        theme::set_active(antes);
    }
}
