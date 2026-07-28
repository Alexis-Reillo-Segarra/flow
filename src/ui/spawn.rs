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
///
/// Cuál es se decide en un solo sitio, [`presets::shell`]: aquí había una
/// segunda copia de la respuesta, y las dos copias podían dejar de decir lo
/// mismo sin que nada se quejara.
pub fn shell() -> &'static str {
    presets::shell().1
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testkit::Ventana;

    fn form_de(kind: Kind) -> Form {
        let mut f = Form::new(".".to_owned());
        f.show(kind, None);
        f
    }

    fn pintar(v: &mut Ventana, form: &mut Form, full: bool) -> Option<Action> {
        let instalados = presets::Installed::detect();
        let proyectos = crate::projects::Projects::load();
        let repos: Vec<crate::repos::Repo> = Vec::new();
        v.frame_ctx(|ctx| show(ctx, form, &instalados, &proyectos, &repos, full))
    }

    /// Una sesión pregunta tres cosas y un panel dos. La que falta no está
    /// contestada de antemano: es que no existe, porque un panel hereda el
    /// directorio de su sesión y no puede desviarse.
    #[test]
    fn un_panel_no_pregunta_por_el_directorio() {
        let sesion = form_de(Kind::Session);
        assert_eq!(sesion.steps(), &SESSION_STEPS);
        assert_eq!(sesion.step, Step::Where);

        let panel = form_de(Kind::Pane);
        assert_eq!(panel.steps(), &PANE_STEPS);
        assert_eq!(panel.step, Step::What);
        assert!(
            !panel.steps().contains(&Step::Where),
            "el formulario de un panel pregunta por el directorio"
        );
    }

    /// De un paso no se sale sin contestarlo, y el aviso es del paso en el que
    /// estás. Es la razón de haber partido el cuadro: antes se llegaba a pulsar
    /// LANZAR y el error salía abajo hablando de un campo cuatro rótulos más
    /// arriba.
    #[test]
    fn no_se_pasa_de_un_paso_sin_contestarlo() {
        let mut f = form_de(Kind::Session);
        f.cwd.clear();
        assert_eq!(f.blocked(), Some("hace falta un directorio"));
        f.advance();
        assert_eq!(f.step, Step::Where, "avanzó sin directorio");

        f.cwd = ".".to_owned();
        f.advance();
        assert_eq!(f.step, Step::What);
        assert_eq!(f.blocked(), Some("hace falta un comando"));
        f.advance();
        assert_eq!(f.step, Step::What, "avanzó sin comando");

        f.cmd = "claude".to_owned();
        f.advance();
        assert_eq!(f.step, Step::Launch);
        assert!(f.is_last());
        assert_eq!(f.blocked(), None, "el último paso no bloquea");
    }

    /// Volver nunca está bloqueado, y lo escrito sigue donde estaba: retroceder
    /// es justo lo que haces cuando te has equivocado.
    #[test]
    fn volver_atras_no_borra_lo_escrito() {
        let mut f = form_de(Kind::Session);
        f.cwd = "C:/algo".to_owned();
        f.advance();
        f.cmd = "cargo test".to_owned();
        f.advance();
        assert_eq!(f.step, Step::Launch);

        f.back();
        assert_eq!(f.step, Step::What);
        f.back();
        assert_eq!(f.step, Step::Where);
        f.back();
        assert_eq!(f.step, Step::Where, "se salió del formulario por detrás");
        assert_eq!(f.cwd, "C:/algo");
        assert_eq!(f.cmd, "cargo test");
    }

    /// Un panel nace con el shell puesto y una sesión en blanco: es lo que uno
    /// quiere nueve de cada diez veces, y deja el panel a dos Enter.
    #[test]
    fn un_panel_nace_con_el_shell_puesto() {
        assert_eq!(form_de(Kind::Pane).cmd, shell());
        assert!(form_de(Kind::Session).cmd.is_empty());
    }

    /// Reabrir vuelve al primer paso. Reaparecer donde lo dejaste sonaría a
    /// favor y es lo contrario: el comando se ha borrado, así que el resumen
    /// del último paso hablaría de algo que ya no existe.
    #[test]
    fn reabrir_vuelve_al_principio() {
        let mut f = form_de(Kind::Session);
        f.cwd = ".".to_owned();
        f.advance();
        f.cmd = "claude".to_owned();
        f.advance();
        assert_eq!(f.step, Step::Launch);

        f.show(Kind::Session, None);
        assert_eq!(f.step, Step::Where);
        assert!(f.cmd.is_empty(), "el comando sobrevivió a reabrir");
    }

    /// El directorio se conserva entre aperturas —abrir dos sesiones seguidas en
    /// el mismo sitio es lo normal— salvo que se imponga uno, que es lo que pasa
    /// al añadir un panel.
    #[test]
    fn el_directorio_se_conserva_salvo_que_se_imponga_otro() {
        let mut f = form_de(Kind::Session);
        f.cwd = "C:/proyecto".to_owned();
        f.show(Kind::Session, None);
        assert_eq!(f.cwd, "C:/proyecto");

        f.show(Kind::Pane, Some("C:/otro".to_owned()));
        assert_eq!(f.cwd, "C:/otro");
    }

    /// Cerrar deja el formulario sin nada que reabrir a medias.
    #[test]
    fn cerrar_lo_deja_limpio() {
        let mut f = form_de(Kind::Pane);
        f.name = "algo".to_owned();
        f.error = Some("lo que sea".to_owned());
        f.close();
        assert!(!f.open);
        assert!(f.name.is_empty());
        assert!(f.cmd.is_empty());
        assert!(f.error.is_none());
    }

    /// Sin nombre, el panel se llama como su comando: el primer token, sin ruta
    /// ni comillas, que es lo que uno diría de viva voz.
    #[test]
    fn el_nombre_sale_del_comando_cuando_no_lo_pones() {
        let mut f = form_de(Kind::Pane);
        f.cmd = "cargo test".to_owned();
        assert_eq!(f.effective_name(), "cargo");

        f.name = "  suite  ".to_owned();
        assert_eq!(f.effective_name(), "suite", "no se recortan los espacios");
    }

    #[test]
    fn del_comando_al_nombre_se_cae_la_ruta_y_las_comillas() {
        assert_eq!(name_of("claude"), "claude");
        assert_eq!(name_of("C:/bin/claude.exe --algo"), "claude.exe");
        assert_eq!(name_of("/usr/bin/bash -i"), "bash");
        assert_eq!(name_of("\"C:/bin/x.exe\" --algo"), "x.exe");
        // Una ruta entrecomillada **con espacios** se parte por el espacio antes
        // de que nadie mire las comillas, así que de `"C:/Program Files/x/y.exe"`
        // sale `Program`. Queda escrito porque es lo que hace y no lo que
        // parece: el precio es un nombre de panel feo —el comando se lanza
        // entero e igual de bien—, así que se documenta en vez de arreglarse a
        // ciegas. Si algún día se toca, este test dirá que ha cambiado.
        assert_eq!(name_of("\"C:/Program Files/x/y.exe\""), "Program");
        assert_eq!(
            name_of("   "),
            "agent",
            "un comando vacío tiene que llamarse algo"
        );
        assert_eq!(name_of(""), "agent");
    }

    /// Una ruta que no existe se avisa, pero solo al abrir sesión: la de un
    /// panel la hereda de la suya, que por definición ya existe. Y una ruta
    /// vacía no es una carpeta nueva, es no haber escrito nada.
    #[test]
    fn se_avisa_de_un_directorio_que_no_existe() {
        let mut f = form_de(Kind::Session);
        f.cwd = "C:/esto/no/existe/en/ningun/sitio".to_owned();
        assert!(missing_dir(&f));

        f.cwd = ".".to_owned();
        assert!(!missing_dir(&f));

        f.cwd = "   ".to_owned();
        assert!(
            !missing_dir(&f),
            "una ruta vacía no es una carpeta por crear"
        );

        f.kind = Kind::Pane;
        f.cwd = "C:/esto/no/existe".to_owned();
        assert!(!missing_dir(&f), "un panel no puede desviarse de su sesión");
    }

    /// Cerrado no dibuja ni contesta: el atajo puede llegar en cualquier frame.
    #[test]
    fn cerrado_no_dibuja_nada() {
        let mut v = Ventana::nueva();
        let mut f = Form::new(".".to_owned());
        assert!(pintar(&mut v, &mut f, false).is_none());
    }

    /// Esc cierra el cuadro, y lo hace desde cualquier paso.
    #[test]
    fn esc_cierra_el_formulario() {
        let mut v = Ventana::nueva();
        let mut f = form_de(Kind::Session);
        pintar(&mut v, &mut f, false);

        v.tecla(egui::Key::Escape, egui::Modifiers::NONE);
        assert!(matches!(
            pintar(&mut v, &mut f, false),
            Some(Action::CancelSpawn)
        ));
    }

    /// Enter avanza mientras queden pasos, y en el último lanza. Es lo que hace
    /// que el formulario se pueda recorrer entero sin tocar el ratón.
    #[test]
    fn enter_avanza_y_en_el_ultimo_paso_lanza() {
        let mut v = Ventana::nueva();
        let mut f = form_de(Kind::Pane);
        f.cmd = "cargo test".to_owned();
        pintar(&mut v, &mut f, false);
        assert_eq!(f.step, Step::What);

        v.tecla(egui::Key::Enter, egui::Modifiers::NONE);
        let accion = pintar(&mut v, &mut f, false);
        assert!(accion.is_none(), "avanzar de paso no lanza nada");
        assert_eq!(f.step, Step::Launch);

        v.tecla(egui::Key::Enter, egui::Modifiers::NONE);
        assert!(matches!(
            pintar(&mut v, &mut f, false),
            Some(Action::ConfirmSpawn)
        ));
    }

    /// Con la sesión llena el cuadro se abre igual —para que el atajo nunca
    /// parezca roto— pero no lanza: dice por qué no va a hacerlo.
    #[test]
    fn con_la_sesion_llena_se_abre_pero_no_lanza() {
        let mut v = Ventana::nueva();
        let mut f = form_de(Kind::Pane);
        f.cmd = "cargo test".to_owned();
        f.step = Step::Launch;
        pintar(&mut v, &mut f, true);

        v.tecla(egui::Key::Enter, egui::Modifiers::NONE);
        assert!(
            pintar(&mut v, &mut f, true).is_none(),
            "lanzó un panel en una sesión que ya estaba llena"
        );
    }

    /// Una sesión llena no bloquea el formulario de **otra** sesión: `full`
    /// habla de la que estás mirando, y abrir una nueva no le añade paneles.
    #[test]
    fn la_sesion_llena_no_frena_abrir_otra_sesion() {
        let mut v = Ventana::nueva();
        let mut f = form_de(Kind::Session);
        f.cwd = ".".to_owned();
        f.cmd = "claude".to_owned();
        f.step = Step::Launch;
        pintar(&mut v, &mut f, true);

        v.tecla(egui::Key::Enter, egui::Modifiers::NONE);
        assert!(matches!(
            pintar(&mut v, &mut f, true),
            Some(Action::ConfirmSpawn)
        ));
    }

    /// Los tres pasos se dibujan enteros, con repos y proyectos en la lista, y
    /// en una ventana pequeña: el cuadro se ajusta a lo que hay en vez de
    /// imponer su tamaño, y en una ventana estrecha antes se salía por abajo
    /// llevándose LANZAR consigo.
    #[test]
    fn los_tres_pasos_se_dibujan_en_cualquier_ventana() {
        let instalados = presets::Installed::detect();
        let proyectos = crate::projects::Projects::load();
        let repos = vec![
            crate::repos::Repo {
                dir: "C:/repos/flow".to_owned(),
                name: "flow".to_owned(),
                branch: Some("main".to_owned()),
                touched: std::time::SystemTime::now(),
            },
            crate::repos::Repo {
                dir: "C:/repos/otro".to_owned(),
                name: "otro".to_owned(),
                branch: None,
                touched: std::time::SystemTime::now(),
            },
        ];

        for (ancho, alto) in [(1480.0, 900.0), (760.0, 460.0), (400.0, 300.0)] {
            let mut v = Ventana::de(ancho, alto);
            for kind in [Kind::Session, Kind::Pane] {
                let mut f = Form::new(".".to_owned());
                f.show(kind, None);
                f.cmd = "cargo test".to_owned();
                for _ in 0..3 {
                    v.frame_ctx(|ctx| show(ctx, &mut f, &instalados, &proyectos, &repos, false));
                    f.advance();
                }
            }
        }
    }

    /// Un error puesto se dibuja, y el aviso de carpeta por crear también.
    #[test]
    fn el_error_y_el_aviso_de_carpeta_nueva_se_dibujan() {
        let mut v = Ventana::nueva();
        let mut f = form_de(Kind::Session);
        f.error = Some("no se pudo lanzar".to_owned());
        f.cwd = "C:/una/carpeta/que/no/existe".to_owned();
        pintar(&mut v, &mut f, false);
        f.advance();
        pintar(&mut v, &mut f, false);
    }
}

#[cfg(test)]
mod tests_a_raton {
    use super::*;
    use crate::testkit::Ventana;

    /// Recorre el formulario a ratón, pinchando por toda su superficie, y
    /// comprueba que se llega al final: los botones de avanzar, volver y
    /// cancelar están ahí y hacen lo suyo.
    ///
    /// Se pincha a barrido y no en coordenadas escritas a mano porque el cuadro
    /// se ajusta a la ventana —se encoge, se centra y le crecen filas con los
    /// agentes detectados—, así que un punto fijo sería un test que se rompe al
    /// instalar `codex`.
    #[test]
    fn el_formulario_se_recorre_entero_a_raton() {
        let instalados = presets::Installed::detect();
        let proyectos = crate::projects::Projects::load();
        let repos: Vec<crate::repos::Repo> = Vec::new();

        let mut v = Ventana::nueva();
        let mut f = Form::new(".".to_owned());
        f.show(Kind::Session, None);
        f.cmd = "cargo test".to_owned();

        let caja = v.rect();
        let mut cancelado = false;
        for fila in 0..40 {
            for col in 0..20 {
                let p = egui::pos2(
                    caja.width() * (col as f32 + 0.5) / 20.0,
                    caja.height() * (fila as f32 + 0.5) / 40.0,
                );
                v.clic(p);
                let accion =
                    v.frame_ctx(|ctx| show(ctx, &mut f, &instalados, &proyectos, &repos, false));
                match accion {
                    Some(Action::CancelSpawn) => cancelado = true,
                    Some(Action::ConfirmSpawn) => {}
                    _ => {}
                }
            }
        }
        assert!(cancelado, "no se encontró por dónde cerrar el formulario");
        assert!(
            f.step == Step::Launch || f.step == Step::What || f.step == Step::Where,
            "el formulario acabó en un paso que no existe"
        );
    }

    /// Lo mismo con la sesión llena y con un panel: son los dos avisos que el
    /// cuadro tiene que saber dar sin dejar de dibujarse.
    #[test]
    fn el_formulario_de_un_panel_lleno_tambien_se_recorre() {
        let instalados = presets::Installed::detect();
        let proyectos = crate::projects::Projects::load();
        let repos: Vec<crate::repos::Repo> = Vec::new();

        let mut v = Ventana::de(760.0, 460.0);
        let mut f = Form::new(".".to_owned());
        f.show(Kind::Pane, None);

        let caja = v.rect();
        for fila in 0..24 {
            for col in 0..12 {
                let p = egui::pos2(
                    caja.width() * (col as f32 + 0.5) / 12.0,
                    caja.height() * (fila as f32 + 0.5) / 24.0,
                );
                v.clic(p);
                v.frame_ctx(|ctx| show(ctx, &mut f, &instalados, &proyectos, &repos, true));
            }
        }
    }
}
