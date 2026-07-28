//! Estado de la aplicación y bucle de frame.

use std::path::PathBuf;
use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};

use egui::{Context, Vec2};

use crate::agent::Agent;
use crate::keys;
use crate::presets;
use crate::projects::Projects;
use crate::repos;
use crate::session::{self, Session};
use crate::ui::{bar, chrome, grain, prompt, spawn, themes, tiles, Action, Dir};

/// Tamaño con el que nace el PTY. El primer frame lo corrige al tamaño real del
/// panel; esto solo evita que el proceso arranque creyendo que la terminal es de
/// 80×24 y parta las primeras líneas donde no toca.
const INITIAL_COLS: u16 = 100;
const INITIAL_ROWS: u16 = 30;

/// A partir de este ancho de **monitor** en píxeles, una pantalla es "grande":
/// un 4K con el escritorio al 100% deja el texto en un tamaño que no hay quien
/// lea de lejos.
const BIG_SCREEN_PX: f32 = 3200.0;
/// Cuánto se agranda la interfaz en esas pantallas.
const BIG_SCREEN_SCALE: f32 = 1.5;
/// Por encima de este escalado del sistema ya no hace falta ayudar a nadie.
const DENSE_DPI: f32 = 1.25;

/// Cada cuánto se miran los buzones de las sesiones.
///
/// Un tercio de segundo es imperceptible para quien pidió el panel y deja el
/// coste en un `read_dir` sobre un directorio vacío, que no se nota. Y solo se
/// mira si hay algo vivo: si no corre nada, nadie puede haber escrito.
const INBOX_POLL: Duration = Duration::from_millis(300);

pub struct Flow {
    sessions: Vec<Session>,
    /// Qué sesión se está mirando. Las demás siguen corriendo, pero no se
    /// dibujan.
    current: Option<u64>,
    /// Un contador para todo: sesiones y paneles comparten espacio de ids, así
    /// que un id identifica una cosa y solo una en toda la app.
    next_id: u64,
    form: spawn::Form,
    /// El selector de temas, cerrado casi siempre.
    picker: themes::Picker,
    /// Con qué tema se armó el estilo de egui que hay puesto. Es lo que hace que
    /// cambiar de tema se note en los widgets de egui y no solo en lo que
    /// pintamos a mano: si no coincide con el activo, se vuelve a instalar.
    styled: usize,
    /// Qué agentes hay instalados. Se detecta al arrancar, no cada frame.
    installed: presets::Installed,
    /// Los directorios en los que ya has trabajado, para no volver a teclear la
    /// ruta. Se leen al arrancar y se reescriben al abrir una sesión.
    projects: Projects,
    /// Los repositorios que flow ha encontrado por su cuenta. Vacío hasta que el
    /// barrido conteste, que es lo normal durante los primeros frames.
    repos: Vec<repos::Repo>,
    /// Por donde llega el resultado del barrido. `None` antes de empezarlo y
    /// después de recogerlo: se hace una vez por ejecución.
    repos_rx: Option<Receiver<Vec<repos::Repo>>>,
    repos_done: bool,
    tiling: tiles::Tiling,
    /// Escala aplicada ahora mismo; 0 mientras no se ha decidido ninguna.
    scale: f32,
    /// Dónde viven los buzones de esta ejecución. Va en el temporal del sistema
    /// y lleva el PID: dos flow a la vez no se pisan, y nada de esto acaba en el
    /// repositorio del usuario.
    inbox_root: PathBuf,
    last_poll: Instant,
}

impl Flow {
    pub fn new() -> Self {
        // El directorio desde el que se lanzó flow es el que propone la primera
        // sesión; a partir de ahí, el formulario recuerda el último.
        let home = std::env::current_dir()
            .map(|p| p.display().to_string())
            .unwrap_or_default();
        Self {
            sessions: Vec::new(),
            current: None,
            next_id: 1,
            form: spawn::Form::new(home),
            picker: themes::Picker::default(),
            styled: crate::theme::active(),
            installed: presets::Installed::detect(),
            projects: Projects::load(),
            repos: Vec::new(),
            repos_rx: None,
            repos_done: false,
            tiling: tiles::Tiling::default(),
            scale: 0.0,
            inbox_root: std::env::temp_dir()
                .join("flow")
                .join(std::process::id().to_string()),
            last_poll: Instant::now(),
        }
    }

    /// Arranque de desarrollo: deja la pantalla llena sin lanzar nada a mano.
    ///
    /// No es código temporal aunque lo pareciera: es la única forma de mirar el
    /// reparto de ocho paneles, el formulario o el selector de temas sin
    /// montarlos a mano cada vez, y está documentada en `AGENTS.md`. Sin
    /// variables de entorno puestas no hace absolutamente nada, así que no le
    /// cuesta nada a quien solo abre la aplicación.
    ///
    /// | Variable | Efecto |
    /// | --- | --- |
    /// | `FLOW_DEMO=8` | Una sesión con 8 paneles de shell |
    /// | `FLOW_FORM=session`\|`pane` | Arranca con el formulario abierto |
    /// | `FLOW_PICKER=1` | Arranca con el selector de temas abierto |
    pub fn demo(mut self) -> Self {
        let cwd = std::env::current_dir()
            .map(|p| p.display().to_string())
            .unwrap_or_default();
        if let Ok(n) = std::env::var("FLOW_DEMO") {
            self.open_session("claude".to_owned(), spawn::shell().to_owned(), cwd);
            for i in 1..n.parse().unwrap_or(1usize) {
                self.add_pane(0, format!("panel-{i}"), spawn::shell().to_owned(), true);
            }
        }
        if std::env::var("FLOW_PICKER").is_ok() {
            self.apply(Action::OpenThemes);
        }
        if let Ok(kind) = std::env::var("FLOW_FORM") {
            let kind = if kind == "pane" {
                spawn::Kind::Pane
            } else {
                spawn::Kind::Session
            };
            self.apply(Action::OpenSpawn(kind));
        }
        self
    }

    fn id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    fn current_index(&self) -> Option<usize> {
        self.current
            .and_then(|id| self.sessions.iter().position(|s| s.id == id))
    }

    fn current(&self) -> Option<&Session> {
        self.current_index().map(|i| &self.sessions[i])
    }

    fn current_mut(&mut self) -> Option<&mut Session> {
        self.current_index().map(|i| &mut self.sessions[i])
    }

    /// Cambia de sesión, o de panel dentro de ella.
    ///
    /// No hay que hacer nada más con el teclado: lo que se escriba va al panel
    /// con el foco, y acabamos de cambiar cuál es. Aquí antes se le devolvía el
    /// foco a un campo de texto de abajo, que es justo lo que ya no existe.
    fn switch(&mut self, session: u64) {
        self.current = Some(session);
    }

    fn focus(&mut self, pane: u64) {
        if let Some(s) = self.current_mut() {
            if s.index_of(pane).is_some() {
                s.focused = Some(pane);
            }
        }
    }

    fn apply(&mut self, action: Action) {
        match action {
            Action::Switch(id) => self.switch(id),
            Action::CloseSession(id) => {
                // `Drop` de cada `Agent` mata su proceso, así que soltar la
                // sesión se lleva por delante todo lo que colgaba de ella.
                if let Some(s) = self.sessions.iter().find(|s| s.id == id) {
                    let _ = std::fs::remove_dir_all(&s.inbox);
                }
                self.sessions.retain(|s| s.id != id);
                if self.current == Some(id) {
                    self.current = self.sessions.first().map(|s| s.id);
                }
            }
            Action::Focus(id) => self.focus(id),
            Action::FocusIndex(n) => {
                if let Some(id) = self.current().and_then(|s| s.panes.get(n)).map(|p| p.id) {
                    self.focus(id);
                }
            }
            Action::FocusDir(dir) => {
                let from = self.current().and_then(|s| s.focused);
                if let Some(next) = from.and_then(|id| self.tiling.neighbour(id, dir)) {
                    self.focus(next);
                }
            }
            Action::Kill(id) => {
                if let Some(s) = self.current_mut() {
                    if let Some(i) = s.index_of(id) {
                        s.panes[i].kill();
                    }
                }
            }
            Action::FollowEnd(id) => {
                if let Some(s) = self.current_mut() {
                    if let Some(i) = s.index_of(id) {
                        s.panes[i].follow = true;
                        s.panes[i].snap_to_end = true;
                    }
                }
            }
            Action::Close(id) => {
                let empty = self.current_mut().map(|s| s.close_pane(id));
                if empty == Some(true) {
                    // Cerrar el último panel cierra la sesión: una sesión sin
                    // paneles no es nada que se pueda mirar.
                    if let Some(session) = self.current {
                        self.apply(Action::CloseSession(session));
                    }
                }
            }
            Action::Restart(id) => {
                if let Some(s) = self.current_mut() {
                    if let Some(i) = s.index_of(id) {
                        let old = &s.panes[i];
                        let (name, cmd, cwd) =
                            (old.name.clone(), old.cmdline.clone(), old.cwd.clone());
                        let (cols, rows) = (old.term().cols as u16, old.term().rows as u16);
                        let env = s.env();
                        // Se reutiliza el id para no perder el foco ni el sitio
                        // en la rejilla. La asignación tira el panel viejo, y su
                        // `Drop` se encarga del proceso anterior.
                        s.panes[i] = Agent::spawn(id, name, cmd, cwd, cols, rows, &env);
                    }
                }
            }
            Action::SendRaw(bytes) => {
                if let Some(p) = self.current_mut().and_then(|s| s.focused_mut()) {
                    p.send(&bytes);
                }
            }
            Action::OpenSpawn(kind) => {
                // Añadir a una sesión que no existe no significa nada: se
                // reinterpreta como abrir la primera.
                let kind = match self.current {
                    None => spawn::Kind::Session,
                    Some(_) => kind,
                };
                // Un panel nace en el directorio de su sesión; una sesión, en el
                // último que se escribió.
                let cwd = match kind {
                    spawn::Kind::Pane => self.current().map(|s| s.cwd.clone()),
                    spawn::Kind::Session => None,
                };
                self.form.show(kind, cwd);
            }
            Action::CancelSpawn => self.form.close(),
            Action::ConfirmSpawn => self.launch(),
            Action::OpenThemes => self.picker.show(),
            Action::PickTheme(i) => self.picker.pick(i),
            Action::ConfirmThemes => {
                // Se escribe al aceptar y no al ir probando: el fichero es del
                // usuario y guarda además sus temas propios, así que no se toca
                // una vez por tecla mientras recorre la lista.
                crate::config::save_theme(&crate::theme::themes()[self.picker.selected()].name);
                self.picker.close();
            }
            Action::CancelThemes => {
                self.picker.pick(self.picker.previous());
                self.picker.close();
            }
        }
    }

    fn launch(&mut self) {
        let cmd = self.form.cmd.trim().to_owned();
        if cmd.is_empty() {
            self.form.error = Some("hace falta un comando".to_owned());
            return;
        }
        let name = self.form.effective_name();

        match self.form.kind {
            spawn::Kind::Session => {
                let cwd = self.form.cwd.trim().to_owned();
                if cwd.is_empty() {
                    self.form.error = Some("hace falta un directorio".to_owned());
                    return;
                }
                // Una ruta que no existe se crea. Empezar un proyecto en una
                // carpeta que todavía no está es un caso normal, y obligar a
                // salirse a un explorador de archivos para crearla lo era menos.
                // No ocurre a escondidas: el formulario lo avisa debajo del
                // campo y el botón se llama CREAR Y LANZAR mientras sea el caso.
                if !std::path::Path::new(&cwd).is_dir() {
                    if let Err(err) = std::fs::create_dir_all(&cwd) {
                        self.form.error = Some(format!("no se pudo crear {cwd}: {err}"));
                        return;
                    }
                }
                self.open_session(name, cmd, cwd);
            }
            spawn::Kind::Pane => {
                let Some(i) = self.current_index() else {
                    self.form.error = Some("no hay ninguna sesión abierta".to_owned());
                    return;
                };
                if self.sessions[i].is_full() {
                    self.form.error = Some("esta sesión ya está llena".to_owned());
                    return;
                }
                self.add_pane(i, name, cmd, true);
            }
        }
        self.form.close();
    }

    fn open_session(&mut self, name: String, cmd: String, cwd: String) {
        // Abrir una sesión es el momento en que un directorio demuestra ser un
        // proyecto: es lo que lo mete en la lista de recientes.
        self.projects.touch(&cwd);
        let session = self.id();
        let inbox = self.inbox_root.join(format!("s{session}"));
        // Si el buzón no se puede crear, la sesión se abre igual: se queda sin
        // canal de control, que es peor que tenerlo pero mucho mejor que no
        // poder lanzar el agente.
        let _ = std::fs::create_dir_all(&inbox);

        let env = session::env(&name, session, &cwd, &inbox);
        let pane = self.id();
        let agent = Agent::spawn(
            pane,
            name.clone(),
            cmd,
            cwd.clone(),
            INITIAL_COLS,
            INITIAL_ROWS,
            &env,
        );
        self.sessions
            .push(Session::new(session, name, cwd, inbox, agent));
        self.switch(session);
    }

    /// Añade un panel a la sesión `i`. Hereda su directorio y su entorno: eso
    /// es lo que hace que el panel nuevo sirva para mirar lo que hace el agente
    /// y no sea una terminal suelta que da la casualidad de estar al lado.
    fn add_pane(&mut self, i: usize, name: String, cmd: String, focus: bool) {
        let cwd = self.sessions[i].cwd.clone();
        let env = self.sessions[i].env();
        let id = self.id();
        let pane = Agent::spawn(id, name, cmd, cwd, INITIAL_COLS, INITIAL_ROWS, &env);
        self.sessions[i].push(pane, focus);
    }

    /// Busca repositorios en el disco, en un hilo.
    ///
    /// Va aparte del hilo de dibujo porque lo que tarda esto no lo decide el
    /// número de directorios sino el antivirus, y eso no se puede acotar desde
    /// aquí: un barrido que normalmente son milisegundos puede ser un segundo en
    /// una máquina con la carpeta de trabajo vigilada. La ventana no puede
    /// quedarse quieta por eso.
    ///
    /// Se hace **una vez por ejecución**. Volver a mirar cada vez que se abre el
    /// formulario sería tocar el disco por abrir un menú, y lo que se gana es un
    /// repositorio que hayas clonado en los últimos diez minutos.
    fn start_repo_scan(&mut self, ctx: &Context) {
        let recent: Vec<String> = self.projects.dirs().to_vec();
        let (tx, rx) = std::sync::mpsc::channel();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let found = repos::scan(&repos::roots(&recent), &recent);
            // Si nadie escucha, es que flow se ha cerrado mientras mirábamos: no
            // es un error, es que ya no importa.
            if tx.send(found).is_ok() {
                // Sin esto el resultado se queda en el canal hasta que algo más
                // pida un frame, y con la app parada eso puede no pasar nunca.
                ctx.request_repaint();
            }
        });
        self.repos_rx = Some(rx);
    }

    /// Atiende lo que los agentes hayan dejado en sus buzones.
    ///
    /// El fichero se borra antes de lanzar nada, y pase lo que pase: si el
    /// comando falla, el panel lo enseñará —para eso está—, pero un fichero que
    /// sobreviviera a su lectura reabriría lo mismo cada 300 ms para siempre.
    fn collect_requests(&mut self) {
        for i in 0..self.sessions.len() {
            let Ok(dir) = std::fs::read_dir(&self.sessions[i].inbox) else {
                continue;
            };
            // Por nombre: si un agente suelta cinco de golpe, que los paneles
            // salgan en el orden en que los escribió y no en el del sistema de
            // ficheros.
            let mut files: Vec<PathBuf> = dir
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.is_file())
                .collect();
            files.sort();

            for path in files {
                let content = std::fs::read_to_string(&path).unwrap_or_default();
                let _ = std::fs::remove_file(&path);
                let cmd = content.trim().to_owned();
                // Una sesión llena descarta lo que le pidan: no hay dónde
                // ponerlo. Se dice en el README, porque desde dentro del agente
                // no hay forma de verlo.
                if cmd.is_empty() || self.sessions[i].is_full() {
                    continue;
                }
                let name = spawn::name_of(&cmd);
                // Sin robar el foco: lo pidió el agente, no quien está
                // escribiendo.
                self.add_pane(i, name, cmd, false);
            }
        }
    }

    /// Lo que se escribe va al panel con el foco, y va tal cual: flow no tiene
    /// una línea de entrada propia donde componer antes de mandar.
    ///
    /// Es lo que hace que un panel sea una terminal de verdad y no una caja de
    /// salida: `Ctrl-C` interrumpe, las flechas recorren el historial, `Tab`
    /// completa y una TUI a pantalla completa responde al teclado. La traducción
    /// entera —y qué teclas se queda flow— está en [`crate::keys`].
    ///
    /// No se llama con un modal abierto: ahí el teclado es del formulario, que
    /// tiene sus propios campos de texto.
    fn type_into_pane(&mut self, ctx: &Context) {
        if self.form.open || self.picker.open {
            return;
        }
        let Some(pane) = self.current_mut().and_then(|s| s.focused_mut()) else {
            return;
        };
        // A un proceso muerto no se le escribe: el PTY ya no tiene quien lea al
        // otro lado. La barra de abajo ofrece RESTART para eso.
        if !pane.state.is_running() {
            return;
        }
        let modes = pane.term().modes();
        let bytes = ctx.input(|i| keys::encode(&i.events, modes));
        if !bytes.is_empty() {
            pane.send(&bytes);
        }
    }

    /// Atajos globales. No se procesan si el modal está abierto: ahí manda el
    /// formulario, y Ctrl-N sobre un campo de texto sería confuso.
    ///
    /// El reparto es el de un gestor de ventanas: **Ctrl** se mueve entre
    /// sesiones y **Alt**, dentro de la que estás mirando.
    ///
    /// Lo que se decida aquí hay que declararlo en [`keys::reservada`], que es
    /// quien impide que la misma tecla llegue **además** al proceso. Son las
    /// dos mitades de una sola decisión y viven separadas porque una necesita
    /// `&mut Flow` y la otra tiene que poder probarse sin ventana.
    fn shortcuts(&mut self, ctx: &Context) -> Option<Action> {
        if self.form.open || self.picker.open {
            return None;
        }
        ctx.input(|i| {
            if i.modifiers.ctrl && i.key_pressed(egui::Key::N) {
                return Some(Action::OpenSpawn(spawn::Kind::Session));
            }
            if i.modifiers.ctrl && i.key_pressed(egui::Key::T) {
                // Con Shift, el tema. Va con Ctrl-T porque es lo mismo que se
                // teclea para "abrir algo dentro de flow", y el tema es de la
                // app entera: la tecla lo dice.
                if i.modifiers.shift {
                    return Some(Action::OpenThemes);
                }
                return Some(Action::OpenSpawn(spawn::Kind::Pane));
            }
            if i.modifiers.ctrl && i.key_pressed(egui::Key::W) {
                // Con Shift se va la sesión entera. Es lo único que mata varios
                // procesos de golpe, así que se pide con las dos manos: antes
                // esto estaba en una X del tamaño de un sello en la esquina de
                // la pastilla, pegada al clic de cambiar de sesión.
                if i.modifiers.shift {
                    return self.current.map(Action::CloseSession);
                }
                return self.current().and_then(|s| s.focused).map(Action::Close);
            }
            const DIGITS: [egui::Key; 9] = [
                egui::Key::Num1,
                egui::Key::Num2,
                egui::Key::Num3,
                egui::Key::Num4,
                egui::Key::Num5,
                egui::Key::Num6,
                egui::Key::Num7,
                egui::Key::Num8,
                egui::Key::Num9,
            ];
            for (n, key) in DIGITS.iter().enumerate() {
                if !i.key_pressed(*key) {
                    continue;
                }
                // Ctrl-1..9 salta a la sesión n-ésima; Alt-1..8, al panel
                // n-ésimo de esta. Los números son los que llevan escritos la
                // pastilla y la cabecera del panel.
                if i.modifiers.ctrl {
                    return self.sessions.get(n).map(|s| Action::Switch(s.id));
                }
                if i.modifiers.alt {
                    return Some(Action::FocusIndex(n));
                }
            }
            // Alt-flechas mueve el foco por la rejilla, como en un tiling WM.
            // Alt y no Ctrl porque Ctrl-flecha ya significa "una palabra" dentro
            // del campo de texto, que es justo donde estás cuando quieres
            // cambiar de panel.
            if i.modifiers.alt {
                for (key, dir) in [
                    (egui::Key::ArrowLeft, Dir::Left),
                    (egui::Key::ArrowRight, Dir::Right),
                    (egui::Key::ArrowUp, Dir::Up),
                    (egui::Key::ArrowDown, Dir::Down),
                ] {
                    if i.key_pressed(key) {
                        return Some(Action::FocusDir(dir));
                    }
                }
            }
            None
        })
    }
}

/// Escala de la interfaz.
///
/// Sustituye al antiguo selector `1× 2× 3×` de la barra de título, y hoy no
/// tiene casi nada que decidir: las dos fuentes son de contorno, así que no hay
/// retícula que respetar ni factores prohibidos, y lo correcto es hacer lo que
/// hace cualquier aplicación —seguir al sistema—. Si tienes el escritorio al
/// 150%, flow va al 150%.
///
/// La única corrección es para pantallas grandes con el escalado del sistema en
/// 100%, donde 13 puntos son 13 píxeles de nada: ahí se agranda un 50%.
///
/// Se mira el tamaño del **monitor** y no el de la ventana a propósito. Es un
/// dato que no cambia mientras arrastras el borde, así que la interfaz no puede
/// pegar un salto de tamaño a mitad de un redimensionado. Redimensionar debe
/// enseñar más terminal, no letra más grande.
fn auto_scale(monitor: Option<Vec2>, dpi: f32) -> f32 {
    let big = monitor.is_some_and(|m| m.x >= BIG_SCREEN_PX);
    if big && dpi <= DENSE_DPI {
        dpi * BIG_SCREEN_SCALE
    } else {
        dpi
    }
}

impl eframe::App for Flow {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        let [r, g, b, a] = crate::theme::pal().bg.to_array();
        [
            r as f32 / 255.0,
            g as f32 / 255.0,
            b as f32 / 255.0,
            a as f32 / 255.0,
        ]
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.dibujar(ui);
    }
}

impl Flow {
    /// El frame entero: escala, estilo, atajos, teclado, buzones y las cinco
    /// vistas, en ese orden.
    ///
    /// Vive aquí y no dentro de `eframe::App::ui` por una razón práctica: el
    /// `&mut eframe::Frame` que pide el rasgo no se puede construir sin una
    /// ventana de verdad —sus campos son privados de la caja—, y sin poder
    /// construirlo no hay forma de correr un frame en un test. El parámetro no
    /// se usaba, así que sacar el cuerpo aquí no cambia nada y deja el bucle
    /// entero al alcance del banco de pruebas.
    fn dibujar(&mut self, ui: &mut egui::Ui) {
        let ctx = &ui.ctx().clone();

        let (monitor, dpi) = ctx.input(|i| {
            (
                i.viewport().monitor_size,
                i.viewport().native_pixels_per_point.unwrap_or(1.0),
            )
        });
        let want = auto_scale(monitor, dpi);
        if (want - self.scale).abs() > f32::EPSILON {
            ctx.set_pixels_per_point(want);
            self.scale = want;
        }

        // El estilo de egui lleva colores dentro —el relleno de un campo, el
        // borde de un botón—, así que un cambio de tema hay que volver a
        // instalárselo. Se compara con el activo en vez de hacerlo desde donde
        // se cambia el tema porque aquí es donde hay `ctx`, y porque así también
        // queda cubierto un tema que llegue de otro sitio.
        if self.styled != crate::theme::active() {
            crate::theme::apply_style(ctx);
            self.styled = crate::theme::active();
        }

        // Los repositorios del disco, una vez por ejecución y en un hilo. Se
        // arranca aquí y no en `new` porque hace falta el contexto para pedir el
        // frame en el que se enseñará el resultado.
        if !self.repos_done && self.repos_rx.is_none() {
            self.start_repo_scan(ctx);
        }
        if let Some(rx) = &self.repos_rx {
            if let Ok(found) = rx.try_recv() {
                self.repos = found;
                self.repos_rx = None;
                self.repos_done = true;
            }
        }

        // Drenar los PTYs de **todas** las sesiones, no solo de la que se ve: un
        // agente que trabaja en otra sesión tiene que seguir avanzando y llegar
        // a BLOCKED aunque no lo estés mirando, que es justo cuando importa que
        // su pastilla se ponga a parpadear.
        let mut dirty = false;
        let mut alive = false;
        for session in &mut self.sessions {
            for pane in &mut session.panes {
                dirty |= pane.pump();
                alive |= pane.state.is_running();
            }
        }
        if dirty {
            ctx.request_repaint();
        }
        // Aunque no llegue output, el estado depende del tiempo (un panel pasa a
        // BLOCKED por llevar rato callado), así que hay que seguir mirando.
        if alive {
            ctx.request_repaint_after(Duration::from_millis(150));
            if self.last_poll.elapsed() >= INBOX_POLL {
                self.last_poll = Instant::now();
                self.collect_requests();
            }
        }

        // Si la sesión que se miraba se fue, se hereda la primera: la barra de
        // entrada nunca se queda apuntando a nadie habiendo sesiones.
        if self.current_index().is_none() {
            self.current = self.sessions.first().map(|s| s.id);
        }

        let time = ctx.input(|i| i.time);
        let mut action = self.shortcuts(ctx);
        // Y lo que no se queda flow, al panel con el foco. Va antes de dibujar
        // para que el eco del proceso llegue cuanto antes, y después de los
        // atajos por el mismo motivo por el que `keys::reservada` existe: las
        // teclas de la app no se escriben en la terminal.
        self.type_into_pane(ctx);

        // El grano, lo primero de todo y sobre la ventana entera: barra de
        // título, columna y rejilla comparten el mismo fondo, así que compartir
        // también el grano es lo que evita que se vea la costura entre ellos.
        // Los paneles se rellenan de negro liso encima, así que ni la salida de
        // un proceso ni el formulario se leen nunca sobre ruido.
        grain::paint(ui, ctx.content_rect());

        let current = self.current_index();
        if let Some(a) = chrome::titlebar(ui, current.map(|i| &self.sessions[i])) {
            action = Some(a);
        }
        // La columna va después de la barra de título y antes de la rejilla: los
        // paneles de egui se reparten lo que queda en el orden en que se
        // declaran, así que este orden es el que hace que la columna llegue
        // hasta abajo y la rejilla se quede con el resto.
        if let Some(a) = bar::sidebar(ui, &self.sessions, self.current, time) {
            action = Some(a);
        }

        let focused = current.and_then(|i| {
            let s = &self.sessions[i];
            s.focused.and_then(|id| s.index_of(id)).map(|p| (i, p))
        });
        if let Some(a) = prompt::show(ui, focused.map(|(i, p)| &self.sessions[i].panes[p])) {
            action = Some(a);
        }

        if let Some(i) = current {
            let session = &mut self.sessions[i];
            if let Some(a) = tiles::show(
                ui,
                &mut session.panes,
                session.focused,
                &mut self.tiling,
                time,
            ) {
                action = Some(a);
            }
        } else {
            tiles::empty(ui, &mut self.tiling);
        }

        let full = self.current().is_some_and(|s| s.is_full());
        if let Some(a) = spawn::show(
            ctx,
            &mut self.form,
            &self.installed,
            &self.projects,
            &self.repos,
            full,
        ) {
            action = Some(a);
        }
        if let Some(a) = themes::show(ctx, &self.picker) {
            action = Some(a);
        }

        chrome::resize_handles(ctx);
        chrome::window_border(ctx);

        if let Some(action) = action {
            self.apply(action);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn monitor(w: f32, h: f32) -> Option<Vec2> {
        Some(egui::vec2(w, h))
    }

    #[test]
    fn por_defecto_manda_el_sistema() {
        // 1080p al 100% y un portátil al 150%: en los dos casos, lo que diga el
        // escritorio. Es lo que hace cualquier aplicación.
        assert_eq!(auto_scale(monitor(1920.0, 1080.0), 1.0), 1.0);
        assert_eq!(auto_scale(monitor(1920.0, 1080.0), 1.5), 1.5);
        assert_eq!(auto_scale(monitor(2560.0, 1440.0), 1.0), 1.0);
    }

    #[test]
    fn una_pantalla_grande_sin_escalar_se_agranda() {
        // 4K al 100%: el sistema no ayuda y 13 puntos serían 13 píxeles.
        assert_eq!(auto_scale(monitor(3840.0, 2160.0), 1.0), 1.5);
        // El mismo 4K con el escritorio ya al 150%: no hay nada que corregir.
        assert_eq!(auto_scale(monitor(3840.0, 2160.0), 1.5), 1.5);
    }

    #[test]
    fn sin_saber_el_monitor_no_se_inventa_nada() {
        assert_eq!(auto_scale(None, 1.0), 1.0);
        assert_eq!(auto_scale(None, 2.0), 2.0);
    }
}

#[cfg(test)]
mod tests_del_estado {
    use super::*;
    use crate::testkit::{self, Ventana};
    use crate::ui::Dir;

    /// Un `Flow` que no toca nada del usuario: ni su lista de proyectos, ni su
    /// configuración, ni el buzón de otro test.
    fn flow() -> Flow {
        static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        let mut f = Flow::new();
        f.projects = Projects::en_memoria();
        f.inbox_root = std::env::temp_dir()
            .join("flow-tests")
            .join(format!("buzon-{n}"));
        let _ = std::fs::remove_dir_all(&f.inbox_root);
        f
    }

    /// Abre `n` sesiones de un proceso que no hace nada.
    fn con_sesiones(n: usize) -> Flow {
        let mut f = flow();
        for i in 0..n {
            f.open_session(
                format!("s{i}"),
                testkit::quieto().to_owned(),
                ".".to_owned(),
            );
        }
        f
    }

    fn ids_de_paneles(f: &Flow) -> Vec<u64> {
        f.current()
            .map(|s| s.panes.iter().map(|p| p.id).collect())
            .unwrap_or_default()
    }

    /// Cada cosa que nace se lleva un número, y nunca el de otra: el id es lo
    /// que ata un panel a su sitio en la rejilla y a su fila en la columna.
    #[test]
    fn los_identificadores_no_se_repiten() {
        let mut f = flow();
        let vistos: Vec<u64> = (0..5).map(|_| f.id()).collect();
        let mut ordenados = vistos.clone();
        ordenados.sort_unstable();
        ordenados.dedup();
        assert_eq!(vistos.len(), ordenados.len(), "se repitió un identificador");
    }

    /// Abrir una sesión la deja puesta: lo normal después de abrir algo es
    /// querer mirarlo.
    #[test]
    fn abrir_una_sesion_la_deja_puesta() {
        let f = con_sesiones(2);
        assert_eq!(f.sessions.len(), 2);
        assert_eq!(
            f.current,
            Some(f.sessions[1].id),
            "no se quedó en la última"
        );
        assert_eq!(f.current().unwrap().panes.len(), 1);
    }

    /// Cerrar la sesión que estás mirando salta a otra, y cerrar otra no te
    /// mueve de donde estabas.
    #[test]
    fn cerrar_una_sesion_deja_puesta_otra() {
        let mut f = con_sesiones(3);
        let (a, b, c) = (f.sessions[0].id, f.sessions[1].id, f.sessions[2].id);

        f.apply(Action::Switch(b));
        f.apply(Action::CloseSession(c));
        assert_eq!(f.current, Some(b), "cerrar otra sesión me movió de sitio");

        f.apply(Action::CloseSession(b));
        assert_eq!(
            f.current,
            Some(a),
            "cerrar la puesta no saltó a la que queda"
        );

        f.apply(Action::CloseSession(a));
        assert!(f.sessions.is_empty());
        assert_eq!(f.current, None, "sin sesiones seguía habiendo una puesta");
    }

    /// Cerrar el último panel cierra la sesión: una sesión sin paneles no es
    /// nada que se pueda mirar.
    #[test]
    fn cerrar_el_ultimo_panel_cierra_la_sesion() {
        let mut f = con_sesiones(1);
        f.add_pane(0, "otro".to_owned(), testkit::quieto().to_owned(), true);
        let paneles = ids_de_paneles(&f);
        assert_eq!(paneles.len(), 2);

        f.apply(Action::Close(paneles[1]));
        assert_eq!(
            f.sessions.len(),
            1,
            "cerrar un panel de dos cerró la sesión"
        );

        f.apply(Action::Close(paneles[0]));
        assert!(
            f.sessions.is_empty(),
            "el último panel no se llevó la sesión"
        );
    }

    /// El foco se le da al panel que se pide, y solo si es de esta sesión: un
    /// id de otra no puede robarle el foco a la que estás mirando.
    #[test]
    fn el_foco_solo_se_mueve_dentro_de_la_sesion_puesta() {
        let mut f = con_sesiones(2);
        let de_otra = f.sessions[0].panes[0].id;
        let mio = f.sessions[1].panes[0].id;

        f.apply(Action::Focus(de_otra));
        assert_eq!(f.current().unwrap().focused, Some(mio));

        f.add_pane(1, "otro".to_owned(), testkit::quieto().to_owned(), false);
        let segundo = f.sessions[1].panes[1].id;
        f.apply(Action::Focus(segundo));
        assert_eq!(f.current().unwrap().focused, Some(segundo));
    }

    /// `Alt-1..8` salta al panel n-ésimo, y un número que no existe no hace
    /// nada: con dos paneles, Alt-5 no puede dejar el foco en la nada.
    #[test]
    fn saltar_a_un_panel_que_no_existe_no_hace_nada() {
        let mut f = con_sesiones(1);
        f.add_pane(0, "otro".to_owned(), testkit::quieto().to_owned(), false);
        let paneles = ids_de_paneles(&f);

        f.apply(Action::FocusIndex(1));
        assert_eq!(f.current().unwrap().focused, Some(paneles[1]));

        f.apply(Action::FocusIndex(7));
        assert_eq!(
            f.current().unwrap().focused,
            Some(paneles[1]),
            "saltar a un panel que no existe movió el foco"
        );
    }

    /// El foco por dirección necesita que la rejilla se haya dibujado: sin
    /// geometría, «el de la derecha» no significa nada y no se mueve nadie.
    #[test]
    fn mover_el_foco_por_direccion_necesita_haber_dibujado() {
        let mut f = con_sesiones(1);
        f.add_pane(0, "otro".to_owned(), testkit::quieto().to_owned(), false);
        let paneles = ids_de_paneles(&f);
        f.apply(Action::Focus(paneles[0]));

        f.apply(Action::FocusDir(Dir::Right));
        assert_eq!(
            f.current().unwrap().focused,
            Some(paneles[0]),
            "se movió el foco sin saber dónde está cada panel"
        );

        // Y con la rejilla ya repartida, sí se mueve.
        let mut v = Ventana::nueva();
        for _ in 0..40 {
            let foco = f.current().unwrap().focused;
            let paneles = &mut f.sessions[0].panes;
            v.frame(|ui| crate::ui::tiles::show(ui, paneles, foco, &mut f.tiling, 0.0));
        }
        f.apply(Action::FocusDir(Dir::Right));
        assert_eq!(f.current().unwrap().focused, Some(paneles[1]));
    }

    /// Reiniciar un panel se queda con su id: si cambiara, el panel saltaría de
    /// sitio en la rejilla y perdería el foco justo cuando lo estás mirando.
    #[test]
    fn reiniciar_un_panel_le_deja_su_sitio() {
        let mut f = con_sesiones(1);
        let id = f.sessions[0].panes[0].id;
        let nombre = f.sessions[0].panes[0].name.clone();

        f.apply(Action::Restart(id));
        assert_eq!(
            f.sessions[0].panes[0].id, id,
            "el panel reiniciado cambió de id"
        );
        assert_eq!(f.sessions[0].panes[0].name, nombre);
        assert_eq!(f.sessions[0].panes.len(), 1);
    }

    /// Matar un panel lo deja muerto pero no lo cierra: sigue en la rejilla con
    /// lo que escribió, que es justo para lo que sirve.
    #[test]
    fn matar_un_panel_no_lo_cierra() {
        let mut f = con_sesiones(1);
        let id = f.sessions[0].panes[0].id;
        f.apply(Action::Kill(id));
        assert_eq!(f.sessions[0].panes.len(), 1, "matar un panel lo cerró");
    }

    /// Los bytes de los botones de abajo van al panel con el foco, y a un
    /// proceso muerto no se le escribe.
    #[test]
    fn los_bytes_van_al_panel_con_el_foco() {
        let mut f = con_sesiones(1);
        f.apply(Action::SendRaw(vec![0x03]));

        let id = f.sessions[0].panes[0].id;
        f.apply(Action::Kill(id));
        f.apply(Action::SendRaw(vec![0x03]));

        // Sin sesiones tampoco hay a quién escribirle.
        f.apply(Action::CloseSession(f.current.unwrap()));
        f.apply(Action::SendRaw(vec![0x03]));
    }

    /// Pedir un panel sin tener sesión se reinterpreta como abrir la primera:
    /// añadir a lo que no existe no significa nada, y el atajo no puede quedarse
    /// sin hacer nada.
    #[test]
    fn pedir_un_panel_sin_sesiones_abre_una_sesion() {
        let mut f = flow();
        f.apply(Action::OpenSpawn(spawn::Kind::Pane));
        assert!(f.form.open);
        assert_eq!(f.form.kind, spawn::Kind::Session);
    }

    /// Un panel nace en el directorio de su sesión, y el formulario lo dice
    /// desde el principio en vez de preguntarlo.
    #[test]
    fn el_formulario_de_un_panel_hereda_el_directorio_de_la_sesion() {
        let mut f = flow();
        let dir = std::env::temp_dir().join("flow-tests").join("dir-heredado");
        std::fs::create_dir_all(&dir).unwrap();
        let dir = dir.display().to_string();
        f.open_session("s".to_owned(), testkit::quieto().to_owned(), dir.clone());

        f.apply(Action::OpenSpawn(spawn::Kind::Pane));
        assert_eq!(f.form.cwd, dir);

        // Y una sesión nueva no hereda nada: se queda el último que se escribió.
        f.apply(Action::OpenSpawn(spawn::Kind::Session));
        assert_eq!(f.form.cwd, dir, "el último directorio escrito se perdió");
    }

    /// Lanzar sin comando o sin directorio se queda en el formulario diciendo
    /// qué falta, en vez de abrir un panel que no puede funcionar.
    #[test]
    fn lanzar_sin_lo_imprescindible_avisa_en_vez_de_lanzar() {
        let mut f = flow();
        f.apply(Action::OpenSpawn(spawn::Kind::Session));
        f.form.cmd = "   ".to_owned();
        f.apply(Action::ConfirmSpawn);
        assert_eq!(f.form.error.as_deref(), Some("hace falta un comando"));
        assert!(f.sessions.is_empty());

        f.form.cmd = testkit::quieto().to_owned();
        f.form.cwd = "  ".to_owned();
        f.apply(Action::ConfirmSpawn);
        assert_eq!(f.form.error.as_deref(), Some("hace falta un directorio"));
        assert!(f.sessions.is_empty());
    }

    /// Una carpeta que todavía no existe se crea. Empezar un proyecto en una
    /// carpeta que no está es un caso normal, y obligar a salirse a un
    /// explorador de archivos para crearla lo era menos.
    #[test]
    fn lanzar_en_una_carpeta_que_no_existe_la_crea() {
        let mut f = flow();
        let dir = std::env::temp_dir()
            .join("flow-tests")
            .join("carpeta-por-crear")
            .join(format!("{:?}", std::thread::current().id()).replace(['(', ')'], "-"));
        let _ = std::fs::remove_dir_all(&dir);
        assert!(!dir.is_dir());

        f.apply(Action::OpenSpawn(spawn::Kind::Session));
        f.form.cmd = testkit::quieto().to_owned();
        f.form.cwd = dir.display().to_string();
        f.apply(Action::ConfirmSpawn);

        assert!(dir.is_dir(), "no se creó la carpeta");
        assert_eq!(f.sessions.len(), 1);
        assert!(!f.form.open, "el formulario se quedó abierto tras lanzar");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// En una sesión llena no cabe otro panel, y el formulario lo dice en vez
    /// de tragárselo.
    #[test]
    fn una_sesion_llena_no_admite_otro_panel() {
        let mut f = con_sesiones(1);
        for i in 1..crate::session::MAX_PANES {
            f.add_pane(0, format!("p{i}"), testkit::quieto().to_owned(), false);
        }
        assert!(f.sessions[0].is_full());

        f.apply(Action::OpenSpawn(spawn::Kind::Pane));
        f.form.cmd = testkit::quieto().to_owned();
        f.apply(Action::ConfirmSpawn);
        assert_eq!(f.form.error.as_deref(), Some("esta sesión ya está llena"));
        assert_eq!(f.sessions[0].panes.len(), crate::session::MAX_PANES);
    }

    /// Lanzar un panel sin ninguna sesión abierta se queda en el formulario: es
    /// el caso que solo se alcanza si la sesión se cierra con el cuadro puesto.
    #[test]
    fn lanzar_un_panel_sin_sesion_avisa() {
        let mut f = flow();
        f.form.show(spawn::Kind::Pane, None);
        f.form.cmd = testkit::quieto().to_owned();
        f.apply(Action::ConfirmSpawn);
        assert_eq!(
            f.form.error.as_deref(),
            Some("no hay ninguna sesión abierta")
        );
    }

    /// El buzón: un fichero, un panel. Salen en el orden en que se escribieron,
    /// el fichero se borra siempre, y el panel que pide un agente no roba el
    /// foco a quien está escribiendo.
    #[test]
    fn el_buzon_abre_un_panel_por_fichero_y_en_orden() {
        let mut f = con_sesiones(1);
        let foco = f.sessions[0].focused;
        let inbox = f.sessions[0].inbox.clone();
        std::fs::create_dir_all(&inbox).unwrap();
        std::fs::write(inbox.join("1.cmd"), "cmd /C echo primero").unwrap();
        std::fs::write(inbox.join("2.cmd"), "cmd /C echo segundo").unwrap();
        std::fs::write(inbox.join("3.cmd"), "   ").unwrap();

        f.collect_requests();

        assert_eq!(
            f.sessions[0].panes.len(),
            3,
            "no salió un panel por fichero"
        );
        assert_eq!(f.sessions[0].panes[1].cmdline, "cmd /C echo primero");
        assert_eq!(f.sessions[0].panes[2].cmdline, "cmd /C echo segundo");
        assert_eq!(f.sessions[0].focused, foco, "el panel pedido robó el foco");
        assert_eq!(
            std::fs::read_dir(&inbox).unwrap().count(),
            0,
            "quedaron peticiones sin borrar: se reabrirían cada 300 ms"
        );
    }

    /// Lo que se le pide al buzón con la sesión llena se descarta en silencio, y
    /// es a propósito: no hay dónde ponerlo y flow no tiene forma de contestar.
    #[test]
    fn el_buzon_descarta_lo_que_no_cabe() {
        let mut f = con_sesiones(1);
        for i in 1..crate::session::MAX_PANES {
            f.add_pane(0, format!("p{i}"), testkit::quieto().to_owned(), false);
        }
        let inbox = f.sessions[0].inbox.clone();
        std::fs::create_dir_all(&inbox).unwrap();
        std::fs::write(inbox.join("1.cmd"), "cmd /C echo nadie").unwrap();

        f.collect_requests();
        assert_eq!(f.sessions[0].panes.len(), crate::session::MAX_PANES);
        assert_eq!(std::fs::read_dir(&inbox).unwrap().count(), 0);
    }

    /// Un buzón que no se puede leer no para la aplicación: la sesión se abre
    /// igual, sin canal de control, que es peor que tenerlo pero mucho mejor que
    /// no poder lanzar el agente.
    #[test]
    fn un_buzon_que_no_esta_no_para_nada() {
        let mut f = con_sesiones(1);
        let _ = std::fs::remove_dir_all(&f.sessions[0].inbox);
        f.collect_requests();
        assert_eq!(f.sessions[0].panes.len(), 1);
    }

    /// Lo que se teclea va al panel con el foco. Con un modal delante no va a
    /// ninguna parte: ahí el teclado es del formulario.
    #[test]
    fn lo_que_se_teclea_va_al_panel_y_no_al_modal() {
        let mut v = Ventana::nueva();
        let mut f = con_sesiones(1);

        v.escribe("hola");
        v.frame_ctx(|ctx| f.type_into_pane(ctx));

        f.form.show(spawn::Kind::Pane, None);
        v.escribe("esto es del formulario");
        v.frame_ctx(|ctx| f.type_into_pane(ctx));
        f.form.close();

        f.picker.show();
        v.escribe("esto es del selector");
        v.frame_ctx(|ctx| f.type_into_pane(ctx));
        f.picker.close();

        // A un proceso muerto tampoco: el PTY ya no tiene quien lea al otro lado.
        let id = f.sessions[0].panes[0].id;
        f.apply(Action::Kill(id));
        crate::testkit::espera_a_que_termine(&mut f.sessions[0].panes[0]);
        v.escribe("a un muerto");
        v.frame_ctx(|ctx| f.type_into_pane(ctx));

        // Y sin sesiones no hay a quién.
        f.apply(Action::CloseSession(f.current.unwrap()));
        v.escribe("a nadie");
        v.frame_ctx(|ctx| f.type_into_pane(ctx));
    }

    /// Los atajos, uno a uno. Es la mitad ejecutora de la decisión que
    /// `keys::reservada` defiende por el otro lado.
    #[test]
    fn los_atajos_hacen_lo_que_dicen() {
        use egui::{Key, Modifiers};
        let ctrl = Modifiers::CTRL;
        let ctrl_shift = Modifiers::CTRL | Modifiers::SHIFT;
        let alt = Modifiers::ALT;

        let mut v = Ventana::nueva();
        let mut f = con_sesiones(2);
        f.add_pane(1, "otro".to_owned(), testkit::quieto().to_owned(), false);

        let pulsa = |v: &mut Ventana, f: &mut Flow, key, mods| {
            v.tecla(key, mods);
            v.frame_ctx(|ctx| f.shortcuts(ctx))
        };

        assert!(matches!(
            pulsa(&mut v, &mut f, Key::N, ctrl),
            Some(Action::OpenSpawn(spawn::Kind::Session))
        ));
        assert!(matches!(
            pulsa(&mut v, &mut f, Key::T, ctrl),
            Some(Action::OpenSpawn(spawn::Kind::Pane))
        ));
        assert!(matches!(
            pulsa(&mut v, &mut f, Key::T, ctrl_shift),
            Some(Action::OpenThemes)
        ));
        assert!(matches!(
            pulsa(&mut v, &mut f, Key::W, ctrl),
            Some(Action::Close(_))
        ));
        assert!(matches!(
            pulsa(&mut v, &mut f, Key::W, ctrl_shift),
            Some(Action::CloseSession(_))
        ));

        let primera = f.sessions[0].id;
        assert!(matches!(
            pulsa(&mut v, &mut f, Key::Num1, ctrl),
            Some(Action::Switch(id)) if id == primera
        ));
        assert!(
            pulsa(&mut v, &mut f, Key::Num9, ctrl).is_none(),
            "saltó a una sesión novena que no existe"
        );
        assert!(matches!(
            pulsa(&mut v, &mut f, Key::Num2, alt),
            Some(Action::FocusIndex(1))
        ));
        for (key, dir) in [
            (Key::ArrowLeft, Dir::Left),
            (Key::ArrowRight, Dir::Right),
            (Key::ArrowUp, Dir::Up),
            (Key::ArrowDown, Dir::Down),
        ] {
            assert!(matches!(
                pulsa(&mut v, &mut f, key, alt),
                Some(Action::FocusDir(d)) if d == dir
            ));
        }

        // Una tecla suelta no es un atajo: el teclado es del proceso.
        assert!(pulsa(&mut v, &mut f, Key::A, Modifiers::NONE).is_none());
    }

    /// Con un modal abierto no hay atajos: ahí manda el formulario, y `Ctrl-N`
    /// sobre un campo de texto sería confuso.
    #[test]
    fn con_un_modal_abierto_no_hay_atajos() {
        let mut v = Ventana::nueva();
        let mut f = con_sesiones(1);

        f.form.show(spawn::Kind::Session, None);
        v.tecla(egui::Key::N, egui::Modifiers::CTRL);
        assert!(v.frame_ctx(|ctx| f.shortcuts(ctx)).is_none());
        f.form.close();

        f.picker.show();
        v.tecla(egui::Key::N, egui::Modifiers::CTRL);
        assert!(v.frame_ctx(|ctx| f.shortcuts(ctx)).is_none());
    }

    /// Cerrar el panel con foco cuando no hay ninguno no puede inventarse uno.
    #[test]
    fn sin_sesiones_los_atajos_no_se_inventan_nada() {
        let mut v = Ventana::nueva();
        let mut f = flow();
        v.tecla(egui::Key::W, egui::Modifiers::CTRL);
        assert!(v.frame_ctx(|ctx| f.shortcuts(ctx)).is_none());
        v.tecla(egui::Key::W, egui::Modifiers::CTRL | egui::Modifiers::SHIFT);
        assert!(v.frame_ctx(|ctx| f.shortcuts(ctx)).is_none());
        v.tecla(egui::Key::Num1, egui::Modifiers::CTRL);
        assert!(v.frame_ctx(|ctx| f.shortcuts(ctx)).is_none());
    }

    /// El selector de temas aplica mientras eliges, deshace al cancelar y solo
    /// escribe en el fichero al aceptar: el fichero es del usuario y guarda
    /// además sus temas propios, así que no se toca una vez por tecla.
    #[test]
    fn el_selector_de_temas_solo_escribe_al_aceptar() {
        let dir = std::env::temp_dir().join("flow-tests").join("config-temas");
        std::fs::create_dir_all(&dir).unwrap();
        let fichero = dir.join("config");
        let _ = std::fs::remove_file(&fichero);
        crate::config::redirigir_para_test(fichero.clone());

        let inicial = crate::theme::active();
        let mut f = flow();

        f.apply(Action::OpenThemes);
        f.apply(Action::PickTheme(2));
        assert_eq!(crate::theme::active(), 2, "elegir no aplicó el tema");
        assert!(!fichero.exists(), "se escribió el fichero antes de aceptar");

        f.apply(Action::CancelThemes);
        assert_eq!(crate::theme::active(), inicial, "cancelar no deshizo");
        assert!(!fichero.exists());

        f.apply(Action::OpenThemes);
        f.apply(Action::PickTheme(3));
        f.apply(Action::ConfirmThemes);
        assert!(!f.picker.open);
        let escrito = std::fs::read_to_string(&fichero).expect("no se escribió el tema elegido");
        assert!(
            escrito.contains(&crate::theme::themes()[3].name),
            "el fichero no dice el tema aceptado: {escrito}"
        );

        crate::theme::set_active(inicial);
        let _ = std::fs::remove_file(&fichero);
    }

    /// El barrido de repositorios va en un hilo y se recoge una sola vez: es lo
    /// que evita que abrir el formulario toque el disco.
    #[test]
    fn los_repositorios_se_buscan_una_sola_vez() {
        let mut v = Ventana::nueva();
        let mut f = flow();
        v.frame_ctx(|ctx| f.start_repo_scan(ctx));
        assert!(f.repos_rx.is_some());

        // Se espera al hilo, que en un disco vigilado por un antivirus puede
        // tardar: por eso mismo va aparte del dibujo.
        for _ in 0..300 {
            if let Some(rx) = &f.repos_rx {
                if let Ok(found) = rx.try_recv() {
                    f.repos = found;
                    f.repos_rx = None;
                    f.repos_done = true;
                    break;
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        assert!(
            f.repos_done,
            "el barrido de repositorios no llegó a terminar"
        );
    }
}

#[cfg(test)]
mod tests_del_frame {
    use super::*;
    use crate::testkit::{self, Ventana};

    fn flow() -> Flow {
        static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1000);
        let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let mut f = Flow::new();
        f.projects = Projects::en_memoria();
        f.inbox_root = std::env::temp_dir()
            .join("flow-tests")
            .join(format!("frame-{n}"));
        f
    }

    /// La aplicación entera, frame a frame, con la pantalla vacía y con
    /// sesiones. Es el único test que recorre el bucle de dibujo completo —
    /// escala, estilo, atajos, teclado, buzones y las cinco vistas— y por eso
    /// es el que se entera de que dos de ellas se pelean por el mismo hueco.
    #[test]
    fn la_aplicacion_entera_dibuja_un_frame_tras_otro() {
        let mut v = Ventana::nueva();
        let mut f = flow();

        // Sin nada abierto: la rejilla dice cómo empezar.
        for _ in 0..3 {
            v.frame(|ui| f.dibujar(ui));
        }
        assert!(f.sessions.is_empty());

        // Con sesiones y varios paneles, hasta que la rejilla se asienta.
        f.open_session(
            "uno".to_owned(),
            testkit::quieto().to_owned(),
            ".".to_owned(),
        );
        f.add_pane(0, "dos".to_owned(), testkit::quieto().to_owned(), false);
        f.open_session(
            "tres".to_owned(),
            testkit::quieto().to_owned(),
            ".".to_owned(),
        );
        for _ in 0..30 {
            v.frame(|ui| f.dibujar(ui));
        }
        assert_eq!(f.sessions.len(), 2);
    }

    /// Un atajo pulsado durante un frame de verdad hace lo suyo de punta a
    /// punta: lo recoge `shortcuts`, lo resuelve `apply` y se ve en el estado.
    #[test]
    fn un_atajo_pulsado_en_un_frame_llega_hasta_el_final() {
        let mut v = Ventana::nueva();
        let mut f = flow();
        v.frame(|ui| f.dibujar(ui));

        v.tecla(egui::Key::N, egui::Modifiers::CTRL);
        v.frame(|ui| f.dibujar(ui));
        assert!(f.form.open, "Ctrl-N no abrió el formulario");

        v.tecla(egui::Key::Escape, egui::Modifiers::NONE);
        v.frame(|ui| f.dibujar(ui));
        assert!(!f.form.open, "Esc no cerró el formulario");
    }

    /// Los dos modales se dibujan encima de todo lo demás, cada uno con su velo,
    /// sin que la rejilla de debajo deje de existir.
    #[test]
    fn los_modales_se_dibujan_encima_de_la_rejilla() {
        let mut v = Ventana::nueva();
        let mut f = flow();
        f.open_session(
            "uno".to_owned(),
            testkit::quieto().to_owned(),
            ".".to_owned(),
        );

        f.form.show(spawn::Kind::Pane, None);
        for _ in 0..3 {
            v.frame(|ui| f.dibujar(ui));
        }
        f.form.close();

        let antes = crate::theme::active();
        f.picker.show();
        for _ in 0..3 {
            v.frame(|ui| f.dibujar(ui));
        }
        f.picker.close();
        crate::theme::set_active(antes);
    }

    /// Lo que se teclea durante un frame acaba en el panel con el foco. Es el
    /// camino entero: del evento de `egui` a los bytes del PTY.
    #[test]
    fn lo_tecleado_en_un_frame_llega_al_panel() {
        let mut v = Ventana::nueva();
        let mut f = flow();
        f.open_session(
            "uno".to_owned(),
            testkit::quieto().to_owned(),
            ".".to_owned(),
        );
        v.frame(|ui| f.dibujar(ui));

        v.escribe("echo hola");
        v.tecla(egui::Key::Enter, egui::Modifiers::NONE);
        v.frame(|ui| f.dibujar(ui));
    }

    /// La ventana se aprieta hasta el mínimo que declara `main.rs` y por debajo:
    /// la interfaz se encoge pero sigue entera, que es lo que dice el comentario
    /// del `ViewportBuilder`.
    #[test]
    fn la_interfaz_aguanta_una_ventana_diminuta() {
        for (ancho, alto) in [(760.0, 460.0), (400.0, 300.0), (200.0, 150.0)] {
            let mut v = Ventana::de(ancho, alto);
            let mut f = flow();
            f.open_session(
                "uno".to_owned(),
                testkit::quieto().to_owned(),
                ".".to_owned(),
            );
            f.add_pane(0, "dos".to_owned(), testkit::quieto().to_owned(), false);
            for _ in 0..5 {
                v.frame(|ui| f.dibujar(ui));
            }
        }
    }

    /// El arranque de desarrollo abre lo que le pidan las variables de entorno,
    /// y sin ellas no hace absolutamente nada: es lo que hace que no le cueste
    /// nada a quien solo abre la aplicación.
    #[test]
    fn el_arranque_de_desarrollo_solo_hace_algo_si_se_lo_piden() {
        static TURNO: std::sync::Mutex<()> = std::sync::Mutex::new(());
        let _guardia = TURNO.lock().unwrap_or_else(|e| e.into_inner());

        let sin_nada = Flow::new().demo();
        assert!(sin_nada.sessions.is_empty());
        assert!(!sin_nada.form.open);
        assert!(!sin_nada.picker.open);

        let antes = crate::theme::active();
        std::env::set_var("FLOW_DEMO", "3");
        std::env::set_var("FLOW_FORM", "pane");
        std::env::set_var("FLOW_PICKER", "1");
        let mut con_todo = Flow::new();
        con_todo.projects = Projects::en_memoria();
        let con_todo = con_todo.demo();
        std::env::remove_var("FLOW_DEMO");
        std::env::remove_var("FLOW_FORM");
        std::env::remove_var("FLOW_PICKER");

        assert_eq!(con_todo.sessions.len(), 1);
        assert_eq!(con_todo.sessions[0].panes.len(), 3);
        assert!(con_todo.form.open);
        assert_eq!(con_todo.form.kind, spawn::Kind::Pane);
        assert!(con_todo.picker.open);
        crate::theme::set_active(antes);
    }
}
