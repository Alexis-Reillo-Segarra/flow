//! Un agente = un proceso corriendo en un PTY real.
//!
//! No emulamos la terminal desde fuera ni capturamos stdout por tubería: se
//! abre un pseudo-terminal de verdad, así que los procesos hijos se creen
//! interactivos, mantienen el color y pueden pedir input. Es la única forma de
//! que `claude`, `codex` o cualquier CLI se comporte igual que en tu terminal.
//!
//! Cada agente arranca dos hilos: uno lee del PTY sin bloquear al resto y otro
//! espera el código de salida. Ambos hablan con la UI por un canal, y la UI los
//! drena una vez por frame en `pump()`.

use std::io::{Read, Write};
use std::sync::mpsc::{channel, Receiver, TryRecvError};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{Context as _, Result};
use portable_pty::{native_pty_system, ChildKiller, CommandBuilder, MasterPty, PtySize};
use vte::Parser;

use crate::term::Term;

/// Sin output durante este tiempo, deja de considerarse "trabajando".
const SETTLE: Duration = Duration::from_millis(400);
/// A partir de aquí, si la última línea parece una pregunta, está bloqueado.
const BLOCKED_AFTER: Duration = Duration::from_millis(1200);
/// Y a partir de aquí, sin más señales, está simplemente inactivo.
const IDLE_AFTER: Duration = Duration::from_secs(10);

const SCROLLBACK: usize = 5000;

#[derive(Clone, PartialEq, Debug)]
pub enum State {
    /// Escupiendo output ahora mismo.
    Working,
    /// Callado y con pinta de estar esperando que le contestes.
    Blocked,
    /// Vivo pero sin hacer nada visible.
    Idle,
    /// Terminó por su cuenta.
    Exited(u32),
    /// No se pudo lanzar, o el PTY se rompió.
    Failed(String),
}

impl State {
    pub fn label(&self) -> &'static str {
        match self {
            State::Working => "WORKING",
            State::Blocked => "BLOCKED",
            State::Idle => "IDLE",
            State::Exited(0) => "DONE",
            State::Exited(_) => "EXIT",
            State::Failed(_) => "FAILED",
        }
    }

    /// `WORKING` y `DONE` comparten color a propósito: los distingue la
    /// animación —uno late, el otro está quieto— y la etiqueta, no el tono.
    ///
    /// Los nombres de los campos son el papel, no el tono. En los cuatro temas
    /// de color son verde, ámbar y rojo de verdad; en el de casa, que es
    /// monocromo, son cuatro grises ordenados por cuánto te reclama cada
    /// estado. Lo que no cambia es que el color nunca va solo: van con él la
    /// palabra de `label`, la forma de `widgets::paint_mark` —`IDLE` hueco,
    /// el resto sólido— y su ritmo.
    pub fn color(&self) -> egui::Color32 {
        match self {
            State::Working | State::Exited(0) => crate::theme::pal().green,
            State::Blocked => crate::theme::pal().amber,
            State::Idle => crate::theme::pal().slate,
            State::Exited(_) | State::Failed(_) => crate::theme::pal().red,
        }
    }

    pub fn is_running(&self) -> bool {
        matches!(self, State::Working | State::Blocked | State::Idle)
    }
}

enum Event {
    Data(Vec<u8>),
    Exit(u32),
    Error(String),
}

pub struct Agent {
    pub id: u64,
    pub name: String,
    pub cmdline: String,
    pub cwd: String,
    pub state: State,
    pub started: Instant,

    /// Si el usuario no ha hecho scroll, la vista sigue pegada al final. Quién
    /// lo pone y quién lo lee están los dos en `ui::output`: la posición dentro
    /// del scrollback la lleva el `ScrollArea` de egui, y esto es solo si hay
    /// que reengancharla al final o dejarla donde el usuario la dejó.
    pub follow: bool,

    term: Term,
    parser: Parser,
    last_output: Instant,
    /// ¿La última línea parece un prompt? Se invalida con cada byte que llega.
    prompt_cache: Option<bool>,

    rx: Receiver<Event>,
    writer: Option<Box<dyn Write + Send>>,
    master: Option<Box<dyn MasterPty + Send>>,
    killer: Option<Box<dyn ChildKiller + Send + Sync>>,
}

impl Agent {
    /// Lanza `cmdline` dentro de un PTY nuevo.
    ///
    /// El comando se pasa siempre por el shell del sistema en vez de trocearlo
    /// aquí. Es deliberado: en Windows resuelve los shims `.cmd` (`npm`, `npx`,
    /// muchos CLIs de Node no son `.exe`) y en ambos lados permite escribir
    /// tuberías y variables como en cualquier terminal.
    ///
    /// `env` son las variables de la sesión (ver `session::env`): es lo único
    /// que le dice al proceso que está corriendo dentro de un orquestador y no
    /// en una terminal suelta.
    #[allow(clippy::too_many_arguments)]
    pub fn spawn(
        id: u64,
        name: String,
        cmdline: String,
        cwd: String,
        cols: u16,
        rows: u16,
        env: &[(String, String)],
    ) -> Self {
        let started = Instant::now();
        match Self::try_spawn(id, &name, &cmdline, &cwd, cols, rows, env) {
            Ok(mut agent) => {
                agent.started = started;
                agent
            }
            Err(err) => {
                // Un fallo al arrancar no debe tirar la app: el agente entra en
                // la lista igualmente, en estado Failed y con el motivo visible.
                let (_tx, rx) = channel();
                Agent {
                    id,
                    name,
                    cmdline,
                    cwd,
                    state: State::Failed(format!("{err:#}")),
                    started,
                    follow: true,
                    term: Term::new(cols as usize, rows as usize, SCROLLBACK),
                    parser: Parser::new(),
                    last_output: started,
                    prompt_cache: None,
                    rx,
                    writer: None,
                    master: None,
                    killer: None,
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn try_spawn(
        id: u64,
        name: &str,
        cmdline: &str,
        cwd: &str,
        cols: u16,
        rows: u16,
        env: &[(String, String)],
    ) -> Result<Self> {
        let pty = native_pty_system();
        let pair = pty
            .openpty(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .context("no se pudo abrir el PTY")?;

        let mut cmd = if cfg!(windows) {
            let mut c = CommandBuilder::new("cmd.exe");
            c.arg("/C");
            c.arg(cmdline);
            c
        } else {
            let mut c = CommandBuilder::new(
                std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_owned()),
            );
            c.arg("-c");
            c.arg(cmdline);
            c
        };
        cmd.cwd(cwd);
        // Muchos CLIs miran TERM antes de decidir si emiten color.
        cmd.env("TERM", "xterm-256color");
        cmd.env("COLORTERM", "truecolor");
        for (key, value) in env {
            cmd.env(key, value);
        }

        let mut child = pair
            .slave
            .spawn_command(cmd)
            .with_context(|| format!("no se pudo lanzar `{cmdline}`"))?;
        // Soltar el slave es obligatorio: mientras siga abierto por nuestro
        // lado, el lector del master nunca vería EOF al morir el hijo.
        drop(pair.slave);

        let killer = child.clone_killer();
        let mut reader = pair
            .master
            .try_clone_reader()
            .context("no se pudo clonar el lector del PTY")?;
        let writer = pair
            .master
            .take_writer()
            .context("no se pudo tomar el escritor del PTY")?;

        let (tx, rx) = channel();

        let tx_read = tx.clone();
        thread::Builder::new()
            .name(format!("flow-pty-{id}"))
            .spawn(move || {
                let mut buf = [0u8; 8192];
                loop {
                    match reader.read(&mut buf) {
                        Ok(0) => break, // EOF: el hijo cerró el PTY.
                        Ok(n) => {
                            if tx_read.send(Event::Data(buf[..n].to_vec())).is_err() {
                                break; // La UI soltó el agente.
                            }
                        }
                        Err(e) => {
                            let _ = tx_read.send(Event::Error(e.to_string()));
                            break;
                        }
                    }
                }
            })
            .context("no se pudo crear el hilo lector")?;

        thread::Builder::new()
            .name(format!("flow-wait-{id}"))
            .spawn(move || {
                let code = child.wait().map(|s| s.exit_code()).unwrap_or(1);
                let _ = tx.send(Event::Exit(code));
            })
            .context("no se pudo crear el hilo de espera")?;

        let now = Instant::now();
        Ok(Self {
            id,
            name: name.to_owned(),
            cmdline: cmdline.to_owned(),
            cwd: cwd.to_owned(),
            state: State::Working,
            started: now,
            follow: true,
            term: Term::new(cols as usize, rows as usize, SCROLLBACK),
            parser: Parser::new(),
            last_output: now,
            prompt_cache: None,
            rx,
            writer: Some(writer),
            master: Some(pair.master),
            killer: Some(killer),
        })
    }

    pub fn term(&self) -> &Term {
        &self.term
    }

    /// Le mete al emulador una salida escrita a mano, como si la hubiera
    /// mandado el proceso.
    ///
    /// Solo para tests, y por una razón concreta: lo que un shell escribe de
    /// verdad depende del sistema, de su versión y de si el usuario tiene un
    /// prompt de colores. Un test que necesite una fila con negrita, rojo y
    /// vídeo inverso tiene que poder pedirla, no esperar a que aparezca.
    #[cfg(test)]
    pub fn feed_para_test(&mut self, bytes: &[u8]) {
        self.parser.advance(&mut self.term, bytes);
        self.prompt_cache = None;
    }

    /// Cuánto lleva vivo, formateado corto (`12s`, `4m`, `2h`).
    pub fn uptime(&self) -> String {
        let s = self.started.elapsed().as_secs();
        match s {
            0..=59 => format!("{s}s"),
            60..=3599 => format!("{}m", s / 60),
            _ => format!("{}h", s / 3600),
        }
    }

    /// Drena el canal, alimenta el emulador y recalcula el estado.
    /// Devuelve `true` si algo cambió y toca repintar.
    pub fn pump(&mut self) -> bool {
        let mut changed = false;

        loop {
            match self.rx.try_recv() {
                Ok(Event::Data(bytes)) => {
                    self.parser.advance(&mut self.term, &bytes);
                    // Contestar las consultas del proceso (posición del cursor,
                    // device attributes). ConPTY se queda bloqueado esperando
                    // esto nada más arrancar, así que no puede esperar al
                    // siguiente frame.
                    let replies = std::mem::take(&mut self.term.replies);
                    if !replies.is_empty() {
                        self.send(&replies);
                    }
                    self.last_output = Instant::now();
                    self.prompt_cache = None; // Llegó texto nuevo: hay que releer.
                    changed = true;
                }
                Ok(Event::Exit(code)) => {
                    self.state = State::Exited(code);
                    self.writer = None;
                    self.killer = None;
                    changed = true;
                }
                Ok(Event::Error(msg)) => {
                    if self.state.is_running() {
                        self.state = State::Failed(msg);
                    }
                    changed = true;
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => break,
            }
        }

        if self.state.is_running() {
            let next = self.classify();
            if next != self.state {
                self.state = next;
                changed = true;
            }
        }

        changed
    }

    /// Heurística de estado. No hay forma general de preguntarle a un proceso
    /// arbitrario "¿estás esperándome?", así que se deduce del ritmo del output
    /// y de la forma de la última línea. Es exactamente lo que hace un humano
    /// mirando la terminal de reojo.
    ///
    /// Se llama en cada frame, así que el análisis de la última línea —lo único
    /// caro que hay aquí— se hace una sola vez por ráfaga de salida y se
    /// guarda: mientras el proceso siga callado la respuesta no puede cambiar.
    fn classify(&mut self) -> State {
        let quiet = self.last_output.elapsed();
        if quiet < SETTLE {
            return State::Working;
        }
        if quiet >= BLOCKED_AFTER {
            let prompt = *self
                .prompt_cache
                .get_or_insert_with(|| looks_like_prompt(&self.term.last_nonempty_line()));
            if prompt {
                return State::Blocked;
            }
        }
        if quiet >= IDLE_AFTER {
            return State::Idle;
        }
        State::Working
    }

    /// Manda texto al proceso tal cual (hay que incluir el `\r` si hace falta).
    pub fn send(&mut self, data: &[u8]) {
        if let Some(w) = self.writer.as_mut() {
            if w.write_all(data).and_then(|_| w.flush()).is_err() {
                self.state = State::Failed("el PTY cerró la escritura".to_owned());
            }
        }
    }

    /// Ajusta el tamaño del PTY y de la rejilla. Llamarlo cuando cambia el
    /// tamaño del panel: sin esto, los procesos siguen creyendo que la
    /// terminal mide 80×24 y parten las líneas donde no toca.
    pub fn resize(&mut self, cols: u16, rows: u16) {
        if cols as usize == self.term.cols && rows as usize == self.term.rows {
            return;
        }
        if let Some(m) = self.master.as_ref() {
            let _ = m.resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            });
        }
        self.term.resize(cols as usize, rows as usize);
    }

    pub fn kill(&mut self) {
        if let Some(k) = self.killer.as_mut() {
            let _ = k.kill();
        }
    }
}

impl Drop for Agent {
    fn drop(&mut self) {
        // Al cerrar la app o quitar un agente de la lista, no dejamos huérfano
        // el proceso hijo.
        self.kill();
    }
}

/// ¿La última línea tiene pinta de estar pidiendo algo?
///
/// Se comprueba contra la línea ya recortada por la derecha, porque casi todos
/// los prompts dejan el cursor justo detrás del signo.
fn looks_like_prompt(line: &str) -> bool {
    let line = line.trim_end();
    if line.is_empty() {
        return false;
    }

    // Confirmaciones y peticiones explícitas, en inglés y español.
    const NEEDLES: [&str; 12] = [
        "(y/n)",
        "[y/n]",
        "[Y/n]",
        "[y/N]",
        "(s/n)",
        "press enter",
        "presiona enter",
        "password",
        "contraseña",
        "passphrase",
        "overwrite?",
        "continue?",
    ];
    let lower = line.to_lowercase();
    if NEEDLES.iter().any(|n| lower.contains(n)) {
        return true;
    }

    // Un prompt suele acabar en el signo y poco más. El límite de longitud
    // evita que una frase normal acabada en "?" o ":" cuente como pregunta.
    let last = line.chars().last().unwrap_or(' ');
    matches!(last, '?' | ':' | '>' | '❯' | '›' | '$' | '#') && line.chars().count() < 120
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detecta_prompts_reales() {
        assert!(looks_like_prompt("Do you want to continue? (y/n)"));
        assert!(looks_like_prompt("¿Sobrescribo el archivo? "));
        assert!(looks_like_prompt("Enter your name:"));
        assert!(looks_like_prompt("❯"));
        assert!(looks_like_prompt("C:\\Repos\\flow>"));
        assert!(looks_like_prompt("Password:"));
    }

    #[test]
    fn ignora_texto_corriente() {
        assert!(!looks_like_prompt(""));
        assert!(!looks_like_prompt("   "));
        assert!(!looks_like_prompt("running 42 tests"));
        assert!(!looks_like_prompt("test auth::login ... ok"));
        // Suficientemente larga como para ser prosa, no un prompt.
        assert!(!looks_like_prompt(&format!("{}?", "palabra ".repeat(20))));
    }

    #[test]
    fn el_estado_se_pinta_distinto() {
        assert_ne!(State::Working.color(), State::Blocked.color());
        assert_ne!(State::Exited(0).color(), State::Exited(1).color());
        assert_eq!(State::Exited(0).label(), "DONE");
    }
}

#[cfg(test)]
mod tests_del_proceso {
    use super::*;
    use crate::testkit;

    /// Un comando que no se puede lanzar no tira la aplicación: el panel entra
    /// en la lista igualmente y acaba muerto con su motivo, en vez de llevarse
    /// por delante las otras siete terminales.
    ///
    /// Quién da el error depende del sistema, y por eso el test acepta las dos
    /// formas: en Windows el comando se lo traga `cmd.exe`, que arranca sin
    /// problema y sale con código de error, así que el panel termina en `EXIT`;
    /// donde el lanzamiento falle de verdad, el panel nace ya en `FAILED` con lo
    /// que dijo el sistema. Lo que se prueba es lo que importa en los dos casos:
    /// que el panel existe, que no se queda vivo para siempre y que dice algo.
    #[test]
    fn un_comando_que_no_arranca_no_tira_la_aplicacion() {
        let mut a = Agent::spawn(
            1,
            "imposible".to_owned(),
            "esto-no-es-un-programa-de-verdad".to_owned(),
            ".".to_owned(),
            80,
            24,
            &[],
        );
        if let State::Failed(msg) = &a.state {
            assert!(!msg.is_empty(), "falló sin decir por qué");
            return;
        }
        testkit::espera_a_que_termine(&mut a);
        match &a.state {
            State::Exited(code) => assert_ne!(*code, 0, "un comando inventado salió bien"),
            otro => panic!("el panel se quedó en {otro:?}"),
        }
    }

    /// A un panel fallido se le puede escribir y redimensionar sin que pase
    /// nada: la interfaz no sabe que está muerto hasta que lo mira.
    #[test]
    fn a_un_panel_fallido_se_le_puede_hablar_sin_consecuencias() {
        let mut a = Agent::spawn(
            1,
            "imposible".to_owned(),
            "loquesea".to_owned(),
            "C:/no/existe".to_owned(),
            80,
            24,
            &[],
        );
        a.send(b"hola\r");
        a.resize(120, 40);
        a.kill();
        assert!(!a.pump(), "un panel muerto dijo que había cambiado algo");
    }

    /// Un proceso que termina pasa a `EXIT` con su código, y ahí se queda: ni
    /// vuelve a `WORKING` ni la heurística lo revive.
    #[test]
    fn un_proceso_que_termina_se_queda_en_su_codigo() {
        let mut a = testkit::agente(1, "eco", testkit::saluda());
        testkit::espera_a_que_termine(&mut a);
        assert_eq!(a.state, State::Exited(0));

        a.pump();
        assert_eq!(a.state, State::Exited(0), "el estado final se movió solo");
    }

    /// Un comando que sale con error se distingue de uno que sale bien: es lo
    /// que separa `DONE` de `EXIT` en la cabecera.
    #[test]
    fn el_codigo_de_salida_llega_entero() {
        let cmd = if cfg!(windows) {
            "cmd /C exit 3"
        } else {
            "sh -c \"exit 3\""
        };
        let mut a = testkit::agente(1, "falla", cmd);
        testkit::espera_a_que_termine(&mut a);
        assert_eq!(a.state, State::Exited(3));
    }

    /// Matar un panel lo termina de verdad. Es el botón `KILL`, y es lo único
    /// de la interfaz que mata un proceso a mano.
    #[test]
    fn matar_un_proceso_lo_termina() {
        let mut a = testkit::agente(1, "quieto", testkit::quieto());
        a.kill();
        testkit::espera_a_que_termine(&mut a);
        assert!(!a.state.is_running());
    }

    /// Redimensionar al mismo tamaño no toca el PTY: se llama en cada frame en
    /// el que la rejilla está quieta, y un `SIGWINCH` por frame repinta una TUI
    /// entera sesenta veces por segundo.
    #[test]
    fn redimensionar_a_lo_mismo_no_molesta_al_proceso() {
        let mut a = testkit::agente(1, "quieto", testkit::quieto());
        a.resize(80, 24);
        assert_eq!(a.term().size(), (80, 24));
        a.resize(100, 30);
        assert_eq!(a.term().size(), (100, 30));
        a.kill();
    }

    /// Cuánto lleva vivo, dicho corto. La cabecera tiene sitio para tres
    /// caracteres, no para «3600 segundos».
    #[test]
    fn el_tiempo_de_vida_se_dice_corto() {
        let mut a = testkit::agente(1, "quieto", testkit::quieto());
        assert!(a.uptime().ends_with('s'));

        a.started = std::time::Instant::now() - std::time::Duration::from_secs(300);
        assert_eq!(a.uptime(), "5m");
        a.started = std::time::Instant::now() - std::time::Duration::from_secs(7200);
        assert_eq!(a.uptime(), "2h");
        a.kill();
    }

    /// El emulador deja contestada la pregunta del proceso, y quien la manda al
    /// PTY es `pump`, en la misma vuelta en la que llegó.
    ///
    /// Importa porque ConPTY se queda bloqueado nada más arrancar preguntando
    /// dónde está el cursor: si la respuesta esperase al siguiente frame, no
    /// llegaría ni un byte más de salida hasta entonces. Aquí se le mete la
    /// pregunta a mano —sin pasar por el proceso— y por eso queda a la vista lo
    /// que `pump` recoge.
    #[test]
    fn el_emulador_deja_contestada_la_pregunta_del_proceso() {
        let mut a = testkit::agente(1, "quieto", testkit::quieto());
        a.feed_para_test(b"\x1b[6n");
        assert!(
            !a.term().replies.is_empty(),
            "el emulador no contestó a la consulta de cursor"
        );
        a.kill();
    }
}
