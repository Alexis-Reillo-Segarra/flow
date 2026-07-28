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
