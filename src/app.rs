//! Estado de la aplicación y bucle de frame.

use std::path::PathBuf;
use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};

use egui::{Context, Vec2};

use crate::agent::Agent;
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
    input: String,
    /// Pide que el campo de entrada tome el foco en el próximo frame.
    focus_input: bool,
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
            input: String::new(),
            focus_input: false,
            tiling: tiles::Tiling::default(),
            scale: 0.0,
            inbox_root: std::env::temp_dir()
                .join("flow")
                .join(std::process::id().to_string()),
            last_poll: Instant::now(),
        }
    }

    // TEMPORAL: quitar antes de terminar.
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

    /// Cambia de sesión, o de panel dentro de ella. En ambos casos el foco del
    /// teclado se va al campo de entrada: lo normal tras saltar a algo es
    /// querer contestarle, sobre todo si está BLOCKED esperando eso mismo.
    fn switch(&mut self, session: u64) {
        if self.current != Some(session) {
            self.current = Some(session);
            self.input.clear();
            self.focus_input = true;
        }
    }

    fn focus(&mut self, pane: u64) {
        let clear = match self.current_mut() {
            Some(s) if s.focused != Some(pane) && s.index_of(pane).is_some() => {
                s.focused = Some(pane);
                true
            }
            _ => false,
        };
        if clear {
            self.input.clear();
            self.focus_input = true;
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
                    self.input.clear();
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
            Action::Close(id) => {
                let empty = self.current_mut().map(|s| s.close_pane(id));
                if empty == Some(true) {
                    // Cerrar el último panel cierra la sesión: una sesión sin
                    // paneles no es nada que se pueda mirar.
                    if let Some(session) = self.current {
                        self.apply(Action::CloseSession(session));
                    }
                } else {
                    self.input.clear();
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
            Action::Send(text) => {
                let mut bytes = text.into_bytes();
                bytes.push(b'\r');
                if let Some(p) = self.current_mut().and_then(|s| s.focused_mut()) {
                    p.send(&bytes);
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
                self.focus_input = true;
                self.input.clear();
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

    /// Atajos globales. No se procesan si el modal está abierto: ahí manda el
    /// formulario, y Ctrl-N sobre un campo de texto sería confuso.
    ///
    /// El reparto es el de un gestor de ventanas: **Ctrl** se mueve entre
    /// sesiones y **Alt**, dentro de la que estás mirando.
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
        if let Some(a) = prompt::show(
            ui,
            focused.map(|(i, p)| &self.sessions[i].panes[p]),
            &mut self.input,
            &mut self.focus_input,
        ) {
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
