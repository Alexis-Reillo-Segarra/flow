//! Formulario de lanzamiento.
//!
//! Modal propio en vez de `egui::Window`: hace falta control total sobre el
//! borde (1 px duro, sin sombra ni esquinas redondeadas) y sobre el orden de
//! capas del velo de fondo.

use egui::{
    epaint::StrokeKind, vec2, Align, Context, CornerRadius, Id, Key, Layout, Rect, Sense, Stroke,
    Ui,
};

use crate::presets;
use crate::theme;
use crate::ui::{widgets, Action};

/// Ancho al que aspira el cuadro. Si no cabe, se encoge.
const BOX_W: f32 = 440.0;
/// Lo que ocupan el título y el pie, que van fuera del scroll. Es lo que hay
/// que descontarle al alto disponible para saber cuánto le queda al centro.
const RESERVED_H: f32 = 120.0;
/// Alto al que se estira el centro, tenga el paso lo que tenga dentro.
///
/// Es un **suelo**, no una medida: el primer paso crece con los repositorios que
/// tengas y no hay número que valga para todos, así que lo que hace esta
/// constante es levantar a los otros dos hasta él. Medido con dos filas de
/// botones de repositorio, que es lo que sale con una carpeta de trabajo normal.
/// Si lo subes, el último paso gana hueco vacío; si lo bajas, el pie da un salto
/// al pasar del primero al segundo.
const STEP_H: f32 = 186.0;
const AGENTS_TITLE: &str = "AGENTES";

/// Qué se está lanzando.
///
/// Es el mismo formulario porque es la misma pregunta —qué comando, dónde— pero
/// no lanza lo mismo: una sesión nueva empieza de cero en el directorio que le
/// digas, y un panel nace dentro de la sesión que estás mirando y hereda su
/// directorio.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Session,
    Pane,
}

/// En qué punto del formulario estás.
///
/// El cuadro pregunta tres cosas y antes las ponía las tres a la vez: siete
/// rótulos apilados donde lo obligatorio y lo opcional pesaban igual, y donde no
/// había forma de saber por dónde empezar más que leyéndolo entero. Ahora va de
/// una en una, y el orden es el que sigue la cabeza al pensar una sesión:
/// primero **sobre qué** se trabaja, luego **con qué**, y al final cómo se llama
/// lo que va a nacer.
///
/// Ese orden es el contrario del que había —el comando estaba primero por ser lo
/// único obligatorio—, y el cambio es a propósito: en un cuadro único importa
/// qué campo está más arriba porque compiten por tu atención a la vez, pero en
/// una secuencia lo que importa es que cada respuesta acote la siguiente. El
/// directorio es lo que hace que una sesión sea una sesión, y saberlo antes de
/// elegir agente es lo que convierte la lista de agentes en «cuál lanzo aquí».
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    /// El directorio: los proyectos, los repositorios y la ruta a mano.
    Where,
    /// El comando: los agentes, las herramientas y lo que se teclee.
    What,
    /// El nombre, el resumen de lo que va a pasar y el botón que lo hace.
    Launch,
}

/// Los tres pasos de una sesión, en orden.
const SESSION_STEPS: [Step; 3] = [Step::Where, Step::What, Step::Launch];
/// Los de un panel. **No tiene el primero**: un panel hereda el directorio de su
/// sesión y no puede desviarse —si pudiera, la sesión dejaría de significar
/// «todo esto trabaja sobre lo mismo»—, así que esa pregunta no es que esté
/// respondida de antemano, es que no existe. Un paso que solo se puede contestar
/// de una forma no es un paso, es un trámite.
const PANE_STEPS: [Step; 2] = [Step::What, Step::Launch];

pub struct Form {
    pub open: bool,
    pub kind: Kind,
    pub name: String,
    pub cmd: String,
    pub cwd: String,
    pub error: Option<String>,
    /// Por dónde va.
    step: Step,
    /// Pone el foco en el campo del paso que se acaba de abrir.
    focus_pending: bool,
}

impl Form {
    pub fn new(cwd: String) -> Self {
        Self {
            open: false,
            kind: Kind::Session,
            name: String::new(),
            cmd: String::new(),
            cwd,
            error: None,
            step: Step::Where,
            focus_pending: false,
        }
    }

    /// Abre el formulario.
    ///
    /// `cwd` impone un directorio —el de la sesión, al añadirle un panel, para
    /// que nazca ya mirando donde trabaja el agente—. A `None` se conserva el
    /// último que se escribió: abrir dos sesiones seguidas en el mismo sitio es
    /// lo normal, y volver a teclear la ruta, un peaje.
    ///
    /// El comando llega precargado con el shell solo al añadir un panel: es lo
    /// que uno quiere nueve de cada diez veces —abrir algo para mirar— y deja el
    /// panel a dos Enter, porque llega ya contestado el único paso que podría
    /// frenarte. Para un subagente basta con pulsar el suyo en la lista, que lo
    /// sobrescribe.
    pub fn show(&mut self, kind: Kind, cwd: Option<String>) {
        self.open = true;
        self.kind = kind;
        self.error = None;
        self.focus_pending = true;
        if let Some(cwd) = cwd {
            self.cwd = cwd;
        }
        self.name.clear();
        self.cmd = match kind {
            Kind::Session => String::new(),
            Kind::Pane => shell().to_owned(),
        };
        // Siempre por el principio. Abrir donde lo dejaste la vez anterior
        // sonaría a favor y es lo contrario: el formulario se abre con el
        // comando en blanco, así que reaparecer en el último paso te pondría
        // delante un resumen de algo que ya no existe.
        self.step = self.steps()[0];
    }

    pub fn close(&mut self) {
        self.open = false;
        self.name.clear();
        self.cmd.clear();
        self.error = None;
    }

    /// Los pasos que tiene este formulario, que dependen de qué se esté
    /// lanzando.
    fn steps(&self) -> &'static [Step] {
        match self.kind {
            Kind::Session => &SESSION_STEPS,
            Kind::Pane => &PANE_STEPS,
        }
    }

    /// Por cuál va, contando desde 0.
    fn index(&self) -> usize {
        self.steps()
            .iter()
            .position(|s| *s == self.step)
            .unwrap_or(0)
    }

    fn is_last(&self) -> bool {
        self.index() + 1 == self.steps().len()
    }

    /// Qué falta para poder salir de este paso, si es que falta algo.
    ///
    /// Se comprueba **al salir de cada paso y no al final**, que es la razón de
    /// haber partido esto en pasos: antes se llegaba a pulsar LANZAR y el error
    /// aparecía abajo del todo hablando de un campo que estaba cuatro rótulos
    /// más arriba. Aquí no se puede pasar de largo dejándose algo.
    fn blocked(&self) -> Option<&'static str> {
        match self.step {
            Step::Where if self.cwd.trim().is_empty() => Some("hace falta un directorio"),
            Step::What if self.cmd.trim().is_empty() => Some("hace falta un comando"),
            _ => None,
        }
    }

    /// Al paso siguiente, si lo hay y si este ya está contestado.
    fn advance(&mut self) {
        if self.blocked().is_some() || self.is_last() {
            return;
        }
        self.step = self.steps()[self.index() + 1];
        self.error = None;
        self.focus_pending = true;
    }

    /// Al anterior. Volver nunca está bloqueado: lo que ya escribiste sigue
    /// donde estaba y retroceder es justo lo que haces cuando te has equivocado.
    fn back(&mut self) {
        let i = self.index();
        if i == 0 {
            return;
        }
        self.step = self.steps()[i - 1];
        self.error = None;
        self.focus_pending = true;
    }

    /// Nombre efectivo del panel: el que se escribió, o el que se deduce del
    /// comando.
    pub fn effective_name(&self) -> String {
        let name = self.name.trim();
        if name.is_empty() {
            name_of(&self.cmd)
        } else {
            name.to_owned()
        }
    }
}

/// El nombre que se le pone a un panel a partir de su comando: el primer token,
/// sin ruta ni comillas, que casi siempre es lo que uno diría de viva voz ("el
/// claude", "el cargo"). También lo usa lo que llega por el buzón, donde nadie
/// escribe un nombre.
pub fn name_of(cmd: &str) -> String {
    cmd.split_whitespace()
        .next()
        .unwrap_or("agent")
        .trim_matches(|c| c == '"' || c == '\'')
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or("agent")
        .to_owned()
}

/// ¿La ruta escrita apunta a un sitio que todavía no existe?
///
/// Solo tiene sentido al abrir sesión: un panel hereda el directorio de la suya,
/// que por definición ya existe. Una ruta vacía tampoco cuenta —eso es no haber
/// escrito nada, no haber pedido una carpeta nueva—.
pub fn missing_dir(form: &Form) -> bool {
    let cwd = form.cwd.trim();
    form.kind == Kind::Session && !cwd.is_empty() && !std::path::Path::new(cwd).is_dir()
}

/// El shell del sistema, que es con lo que llega precargado un panel nuevo.
pub fn shell() -> &'static str {
    if cfg!(windows) {
        "cmd"
    } else {
        "bash -i"
    }
}

/// `full` es "ya no caben más paneles en esta sesión": el formulario se abre
/// igual —para que el atajo nunca parezca roto— pero dice por qué no va a
/// lanzar nada.
pub fn show(
    ctx: &Context,
    form: &mut Form,
    installed: &presets::Installed,
    projects: &crate::projects::Projects,
    repos: &[crate::repos::Repo],
    full: bool,
) -> Option<Action> {
    if !form.open {
        return None;
    }
    let mut action = None;

    // Velo: apaga el fondo sin ocultarlo del todo, para no perder el contexto
    // de qué agentes ya hay corriendo, y deja el ratón fuera de él.
    widgets::veil(ctx, "spawn-dim", 190);

    if ctx.input(|i| i.key_pressed(Key::Escape)) {
        return Some(Action::CancelSpawn);
    }

    // El cuadro se ajusta a la ventana en vez de imponerle un tamaño: se
    // encoge de ancho si no cabe, se centra, y el alto lo tiene limitado a lo
    // que haya. Antes era un rectángulo fijo colocado a mano al 22% de la
    // altura, y en una ventana pequeña —o con muchos agentes detectados, que
    // añaden filas de botones— se salía por abajo llevándose consigo LAUNCH y
    // CANCEL: el formulario se abría y no había forma de lanzar nada.
    let screen = ctx.content_rect();
    let margin = theme::GAP * 2.0;
    let box_w = (screen.width() - margin * 2.0).clamp(200.0, BOX_W);
    let box_h = (screen.height() - margin * 2.0).max(120.0);

    egui::Area::new(Id::new("spawn-form"))
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
                        egui::RichText::new(match form.kind {
                            Kind::Session => "NEW SESSION",
                            Kind::Pane => "ADD TO SESSION",
                        })
                        .font(theme::sans(theme::SANS_MD))
                        .color(theme::pal().accent_text),
                    );
                    // La divisoria de siempre, partida en tramos: dice por qué
                    // paso vas sin añadirle al cuadro nada que antes no hubiera.
                    widgets::step_line(ui, form.steps().len(), form.index());
                    ui.add_space(2.0);

                    // Al añadir a una sesión, lo primero es decir a cuál: el
                    // directorio es lo que comparten sus paneles y lo que hace
                    // que la terminal nueva sirva para mirar lo que hace el
                    // agente en vez de para perderse. Va fuera de los pasos
                    // porque no es una pregunta: es dónde estás.
                    if form.kind == Kind::Pane {
                        widgets::mono_label(ui, &form.cwd, theme::pal().text_dim);
                        ui.add_space(4.0);
                    }

                    // Lo de en medio va en un scroll y el pie se queda fuera:
                    // pase lo que pase con el alto de la ventana o con cuántos
                    // agentes haya detectados, los botones siguen ahí.
                    egui::ScrollArea::vertical()
                        .max_height((box_h - RESERVED_H).max(80.0))
                        .auto_shrink([false, true])
                        .show(ui, |ui| {
                            ui.spacing_mut().item_spacing.y = 6.0;
                            // El centro mide lo mismo en los tres pasos aunque
                            // uno tenga tres rótulos y otro uno. Sin esto el
                            // cuadro crece y se encoge al avanzar, y como está
                            // centrado se mueve por los cuatro lados a la vez:
                            // los botones del pie se van de debajo del ratón
                            // entre un paso y el siguiente. Un asistente es una
                            // ventana cuyo contenido cambia, no tres ventanas.
                            ui.set_min_height(STEP_H.min((box_h - RESERVED_H).max(80.0)));

                            match form.step {
                                Step::Where => step_where(ui, form, projects, repos),
                                Step::What => step_what(ui, form, installed),
                                Step::Launch => step_launch(ui, form),
                            }

                            if let Some(err) = &form.error {
                                ui.add_space(4.0);
                                widgets::mono_label(ui, err, theme::pal().red);
                            }
                        });

                    ui.add_space(10.0);
                    widgets::hline(ui, theme::pal().line);
                    ui.add_space(8.0);

                    // Un solo sitio decide si se puede seguir, y de él salen a
                    // la vez el rótulo del pie y si el botón llega a dibujarse:
                    // así no puede pasar que el pie diga que falta algo y el
                    // botón deje pasar igualmente.
                    // `full` es de la sesión que estás mirando, así que solo
                    // manda cuando lo que se lanza va dentro de ella. Abrir una
                    // sesión nueva no le cabe a nadie: antes, con la de delante
                    // llena, el formulario de NEW SESSION se dejaba recorrer
                    // entero para negarse a lanzar al final por un panel que no
                    // era suyo.
                    let stuck = if full && form.kind == Kind::Pane && form.is_last() {
                        Some("esta sesión está llena: cierra un panel")
                    } else {
                        form.blocked()
                    };

                    // Los botones se colocan **antes** que el rótulo, aunque se
                    // lean después: son los que no pueden faltar. Con la ventana
                    // estrecha y tres botones en el pie, el rótulo ya no cabe, y
                    // puesto primero se los comía por debajo —«Enter sig|
                    // CANCELAR»—. Así se lleva el ancho que sobre y se recorta
                    // con puntos suspensivos, que es lo que hay que hacer con un
                    // texto de ayuda: encogerse antes que tapar lo que se pulsa.
                    ui.horizontal(|ui| {
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.spacing_mut().item_spacing.x = 6.0;
                            // El de avanzar va el primero porque en este layout
                            // el primero es el de más a la derecha, que es donde
                            // se busca lo que lleva adelante.
                            if stuck.is_none() {
                                let next = match (form.is_last(), missing_dir(form)) {
                                    (false, _) => "SIGUIENTE",
                                    (true, true) => "CREAR Y LANZAR",
                                    (true, false) => "LANZAR",
                                };
                                if widgets::button(ui, next, theme::pal().accent_text).clicked() {
                                    if form.is_last() {
                                        action = Some(Action::ConfirmSpawn);
                                    } else {
                                        form.advance();
                                    }
                                }
                            }
                            // ATRÁS solo cuando hay atrás. Un botón que no lleva
                            // a ningún sitio es peor que su ausencia: hay que
                            // pulsarlo para descubrir que no hacía nada.
                            if form.index() > 0
                                && widgets::button(ui, "ATRÁS", theme::pal().text_dim).clicked()
                            {
                                form.back();
                            }
                            if widgets::button(ui, "CANCELAR", theme::pal().text_dim).clicked() {
                                action = Some(Action::CancelSpawn);
                            }

                            // Y el rótulo con lo que haya quedado, otra vez de
                            // izquierda a derecha para que no se pegue a los
                            // botones ni se lea al revés que el resto del cuadro.
                            let hint = match (stuck, form.is_last()) {
                                (Some(por_que), _) => por_que,
                                (None, true) => "Esc cancela · Enter lanza",
                                (None, false) => "Esc cancela · Enter sigue",
                            };
                            ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                                let galley = widgets::fit(
                                    ui,
                                    hint,
                                    theme::mono(theme::MONO_XS),
                                    if stuck.is_some() {
                                        theme::pal().amber
                                    } else {
                                        theme::pal().text_faint
                                    },
                                    ui.available_width(),
                                );
                                ui.add(egui::Label::new(galley));
                            });
                        });
                    });

                    // Enter hace lo que haga el botón de la derecha, que es lo
                    // que lo mantiene rápido para quien ya sabe lo que quiere:
                    // tres Enter y la sesión está abierta, sin tocar el ratón.
                    if ui.input(|i| i.key_pressed(Key::Enter)) && stuck.is_none() {
                        if form.is_last() {
                            action = Some(Action::ConfirmSpawn);
                        } else {
                            form.advance();
                        }
                    }
                });
        });

    action
}

/// Paso 1: sobre qué se trabaja.
fn step_where(
    ui: &mut Ui,
    form: &mut Form,
    projects: &crate::projects::Projects,
    repos: &[crate::repos::Repo],
) {
    // Los proyectos que ya has usado, para no volver a teclear la ruta. Pulsas
    // el nombre y rellena el campo, que sigue ahí debajo por si el sitio es otro.
    dir_row(
        ui,
        "PROYECTO",
        &mut form.cwd,
        projects.dirs().iter().map(|dir| DirChip {
            label: crate::projects::name_of(dir).into(),
            dir,
            detail: dir.into(),
        }),
    );

    // Y los repositorios que flow ha encontrado por su cuenta, en un grupo
    // **aparte** y debajo. Lo que has abierto tú manda y no se mezcla con lo que
    // te propone la máquina: son dos cosas con distinta autoridad, y juntarlas
    // haría que la lista bailara entre arranques sin que hayas hecho nada. Los
    // que ya están arriba no se repiten aquí; de eso se encarga `repos::scan`.
    dir_row(
        ui,
        "REPOS",
        &mut form.cwd,
        repos.iter().map(|repo| DirChip {
            label: repo.name.as_str().into(),
            dir: &repo.dir,
            // La rama va aquí y no en el botón: en una fila de diez, el nombre
            // es lo que buscas, y añadirle «· main» a cada uno duplica el ancho
            // para decir lo mismo diez veces.
            detail: match &repo.branch {
                Some(rama) => format!("{}  ·  {rama}", repo.dir).into(),
                None => repo.dir.as_str().into(),
            },
        }),
    );

    let resp = field(ui, "DIR", &mut form.cwd, "");
    take_focus(form, &resp);
    // Que la ruta no exista no es un error todavía: escribir una carpeta nueva
    // es una forma legítima de empezar un proyecto. Se avisa aquí mientras se
    // teclea, y en el último paso el botón se llama CREAR Y LANZAR, para que
    // crear un directorio en tu disco no ocurra nunca sin haberlo leído.
    if missing_dir(form) {
        ui.add_space(3.0);
        widgets::mono_label(ui, "no existe todavía", theme::pal().amber);
    }
}

/// Paso 2: con qué se trabaja.
fn step_what(ui: &mut Ui, form: &mut Form, installed: &presets::Installed) {
    // Solo se ofrecen los que están instalados de verdad: una lista de agentes
    // que no tienes es peor que ninguna lista.
    let mut row = |ui: &mut Ui, title: &str, items: Vec<&presets::Preset>| {
        if items.is_empty() {
            return;
        }
        ui.label(
            egui::RichText::new(title)
                .font(theme::sans(theme::SANS_SM))
                .color(theme::pal().text_faint),
        );
        ui.add_space(3.0);
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(4.0, 4.0);
            for p in items {
                // Los agentes descansan en el acento y las herramientas en gris:
                // son la razón de que flow exista y lo otro es el apoyo.
                let ink = if std::ptr::eq(title, AGENTS_TITLE) {
                    theme::pal().accent_text
                } else {
                    theme::pal().text_dim
                };
                if widgets::agent_button(ui, p.label, p.mark, ink)
                    .on_hover_text(p.about)
                    .clicked()
                {
                    form.cmd = p.command.to_owned();
                }
            }
        });
        ui.add_space(6.0);
    };

    row(ui, AGENTS_TITLE, installed.agents().collect());
    row(ui, "HERRAMIENTAS", installed.tools().collect());

    if installed.agent_count() == 0 {
        widgets::mono_label(
            ui,
            "no se detectó ningún agente en el PATH",
            theme::pal().text_faint,
        );
        ui.add_space(6.0);
    }

    // El campo va **debajo** de los botones y no encima como estaba: aquí ya no
    // compite con seis rótulos más, y lo que se hace nueve de cada diez veces es
    // pulsar un agente. El campo es para la décima, y para ver qué se pulsó.
    let resp = field(ui, "COMMAND", &mut form.cmd, "cargo test");
    take_focus(form, &resp);
}

/// Paso 3: cómo se llama y qué va a pasar.
fn step_launch(ui: &mut Ui, form: &mut Form) {
    let resp = field(ui, "NAME", &mut form.name, "");
    take_focus(form, &resp);
    // El nombre es opcional y siempre sale uno: el que se deduzca del comando.
    // Enseñarlo de hueco en vez de poner "(opcional)" contesta la pregunta que
    // de verdad se hace aquí, que no es si hay que rellenarlo sino cómo se va a
    // llamar el panel si no lo rellenas.
    if form.name.trim().is_empty() {
        ui.add_space(3.0);
        widgets::mono_label(
            ui,
            &format!("se llamará {}", form.effective_name()),
            theme::pal().text_faint,
        );
    }

    ui.add_space(8.0);
    ui.label(
        egui::RichText::new("SE LANZARÁ")
            .font(theme::sans(theme::SANS_SM))
            .color(theme::pal().text_faint),
    );
    ui.add_space(3.0);
    // El comando entero, sin recortar: es lo único que no se puede deshacer
    // después y el sitio donde se ve una errata antes de que arranque un proceso.
    widgets::mono_label(ui, form.cmd.trim(), theme::pal().text_hi);
    // El directorio solo en una sesión: en un panel ya está fijo arriba del
    // cuadro y repetirlo aquí sería decir dos veces lo mismo en un paso que
    // existe para resumir.
    if form.kind == Kind::Session {
        widgets::mono_label(
            ui,
            form.cwd.trim(),
            if missing_dir(form) {
                theme::pal().amber
            } else {
                theme::pal().text_dim
            },
        );
        if missing_dir(form) {
            widgets::mono_label(ui, "se creará al lanzar", theme::pal().amber);
        }
    }
}

/// Le da el foco al campo principal del paso recién abierto.
///
/// Cada paso tiene uno solo y siempre es el último, así que llegar a un paso
/// nuevo deja el cursor donde se va a escribir: el asistente se recorre entero
/// sin tocar el ratón.
fn take_focus(form: &mut Form, resp: &egui::Response) {
    if form.focus_pending {
        resp.request_focus();
        form.focus_pending = false;
    }
}

/// Una ruta que se ofrece en la fila de botones de debajo de un rótulo.
///
/// `label` y `detail` son `Cow` porque de los dos sitios que llenan estas filas
/// uno tiene los textos ya hechos —el nombre de un repositorio se calculó al
/// barrer el disco— y el otro los compone al vuelo. Esto se dibuja en cada
/// frame que el formulario está abierto, y no hay razón para volver a reservar
/// veinte cadenas sesenta veces por segundo para dejarlas igual.
struct DirChip<'a> {
    /// Cómo se llama de viva voz: lo que va escrito en el botón.
    label: std::borrow::Cow<'a, str>,
    /// La ruta que se escribe en el campo al pulsarlo.
    dir: &'a str,
    /// Lo que se enseña al parar el ratón encima. Siempre lleva la ruta entera,
    /// porque el nombre corto es ambiguo a propósito y en algún sitio hay que
    /// poder ver de qué carpeta se está hablando.
    detail: std::borrow::Cow<'a, str>,
}

/// Un rótulo y debajo las rutas que se ofrecen, en botones que se reparten en
/// las líneas que hagan falta.
///
/// Los dos grupos —lo que has usado y lo que flow ha encontrado— se dibujan con
/// esto porque **hacen exactamente lo mismo**: pulsas un nombre y la ruta cae en
/// el campo. Lo que los separa es de dónde salen y en qué orden aparecen, no
/// cómo se ven; darles dos dibujos distintos sería decir con la forma algo que
/// no es verdad.
///
/// Un grupo sin nada no deja rótulo huérfano: no se dibuja.
fn dir_row<'a>(
    ui: &mut Ui,
    title: &str,
    cwd: &mut String,
    chips: impl Iterator<Item = DirChip<'a>>,
) {
    let mut chips = chips.peekable();
    if chips.peek().is_none() {
        return;
    }
    ui.label(
        egui::RichText::new(title)
            .font(theme::sans(theme::SANS_SM))
            .color(theme::pal().text_faint),
    );
    ui.add_space(3.0);
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(4.0, 4.0);
        for chip in chips {
            // Se comparan como rutas y no como cadenas: el campo puede traer la
            // misma carpeta escrita con la otra barra o en otras mayúsculas
            // —pegada del explorador, por ejemplo— y sigue siendo esta.
            let chosen = crate::repos::same_path(cwd.trim(), chip.dir);
            if widgets::chip(ui, &chip.label, chosen)
                .on_hover_text(chip.detail.as_ref())
                .clicked()
            {
                *cwd = chip.dir.to_owned();
            }
        }
    });
    ui.add_space(6.0);
}

/// Campo etiquetado: rótulo en sans encima, caja de 1 px con el valor en mono.
/// El valor va monoespaciado porque casi siempre es una ruta o un comando, y
/// ahí importa distinguir `l` de `1` y `O` de `0`.
fn field(ui: &mut Ui, label: &str, value: &mut String, hint: &str) -> egui::Response {
    ui.label(
        egui::RichText::new(label)
            .font(theme::sans(theme::SANS_SM))
            .color(theme::pal().text_faint),
    );

    let width = ui.available_width();
    let height = 22.0;
    let (rect, _) = ui.allocate_exact_size(vec2(width, height), Sense::hover());

    ui.painter()
        .rect_filled(rect, CornerRadius::ZERO, theme::pal().bg);

    let inner = Rect::from_min_max(rect.min + vec2(6.0, 4.0), rect.max - vec2(6.0, 3.0));
    let resp = ui
        .scope_builder(egui::UiBuilder::new().max_rect(inner), |ui| {
            ui.add(
                egui::TextEdit::singleline(value)
                    .font(theme::mono(theme::MONO_SM))
                    .text_color(theme::pal().text_hi)
                    .frame(egui::Frame::NONE)
                    .desired_width(inner.width())
                    .hint_text(
                        egui::RichText::new(hint)
                            .font(theme::mono(theme::MONO_SM))
                            .color(theme::pal().text_faint),
                    ),
            )
        })
        .inner;

    let border = if resp.has_focus() {
        theme::pal().accent
    } else {
        theme::pal().line
    };
    ui.painter().rect_stroke(
        rect,
        CornerRadius::ZERO,
        Stroke::new(1.0, border),
        StrokeKind::Inside,
    );

    resp
}
