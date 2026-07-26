//! Una sesión: un agente y las terminales que lo acompañan.
//!
//! Lanzar un agente abre una sesión, y la sesión es lo que agrupa todo lo que
//! tiene que ver con ese trabajo: el agente que la abrió y hasta siete paneles
//! más, **todos en el mismo directorio que él**.
//!
//! Ese detalle es el motivo de que la sesión exista, y sirve para las dos cosas
//! que uno acaba queriendo:
//!
//! - **Mirar.** Mientras `claude` escribe código en su panel, en el de al lado
//!   tienes un shell en su mismo directorio para ver por línea de comandos lo
//!   que está haciendo —`git diff`, `ls`, seguir un log— sin salir de la app y
//!   sin tener que acordarte de en qué carpeta trabajaba.
//! - **Repartir.** Si el trabajo se abre en cinco subagentes, los cinco caben
//!   aquí dentro, sobre el mismo directorio y a la vista a la vez, en vez de
//!   ser cinco sesiones sueltas que no se sabe de quién eran.

use std::path::{Path, PathBuf};

use crate::agent::{Agent, State};

/// Cuántos paneles caben en una sesión, contando al agente.
///
/// El tope no es técnico sino de legibilidad: repartida una pantalla normal
/// entre más de ocho, cada panel deja de tener columnas suficientes para que la
/// salida de un CLI se lea sin partirse.
pub const MAX_PANES: usize = 8;

pub struct Session {
    pub id: u64,
    pub name: String,
    /// El directorio que comparten todos sus paneles. Lo hereda del agente que
    /// abrió la sesión, y es lo que hace que una terminal nueva nazca ya
    /// mirando donde hay que mirar.
    pub cwd: String,
    /// Por dónde pide un agente que le abran un panel. Ver `env`.
    pub inbox: PathBuf,
    /// El primero es el agente que abrió la sesión; los demás son paneles que
    /// se han ido añadiendo. El primero se lleva el hueco grande de la rejilla,
    /// que es donde uno espera encontrar al agente.
    pub panes: Vec<Agent>,
    pub focused: Option<u64>,
}

impl Session {
    pub fn new(id: u64, name: String, cwd: String, inbox: PathBuf, agent: Agent) -> Self {
        let focused = Some(agent.id);
        Self {
            id,
            name,
            cwd,
            inbox,
            panes: vec![agent],
            focused,
        }
    }

    /// Las variables que ve todo proceso lanzado en esta sesión.
    pub fn env(&self) -> Vec<(String, String)> {
        env(&self.name, self.id, &self.cwd, &self.inbox)
    }

    pub fn is_full(&self) -> bool {
        self.panes.len() >= MAX_PANES
    }

    /// Añade un panel. `focus` distingue quién lo pidió: si lo abriste tú, el
    /// teclado se va con él, que es lo que esperas; si lo abrió el agente desde
    /// su buzón, no —estabas escribiéndole a él, y que un panel que no has
    /// pedido te robe el foco es la mejor forma de mandarle una frase a medias
    /// a otro proceso.
    pub fn push(&mut self, pane: Agent, focus: bool) {
        if focus || self.focused.is_none() {
            self.focused = Some(pane.id);
        }
        self.panes.push(pane);
    }

    pub fn index_of(&self, pane: u64) -> Option<usize> {
        self.panes.iter().position(|p| p.id == pane)
    }

    pub fn focused_mut(&mut self) -> Option<&mut Agent> {
        let i = self.focused.and_then(|id| self.index_of(id))?;
        self.panes.get_mut(i)
    }

    /// Quita un panel. Devuelve `true` si la sesión se ha quedado vacía y toca
    /// cerrarla: una sesión sin paneles no es nada que se pueda mirar.
    pub fn close_pane(&mut self, pane: u64) -> bool {
        // `Drop` de `Agent` mata el proceso, así que sacarlo de la lista basta
        // para no dejar huérfanos.
        self.panes.retain(|p| p.id != pane);
        if self.focused == Some(pane) {
            self.focused = self.panes.first().map(|p| p.id);
        }
        self.panes.is_empty()
    }

    /// El estado que resume la sesión en su pastilla de la barra.
    ///
    /// Gana el que más te reclama, no el del agente: si un `cargo test` de la
    /// sesión ha reventado o una terminal está esperando una contraseña, eso es
    /// lo que tienes que ver desde fuera aunque el agente siga tan tranquilo.
    pub fn state(&self) -> State {
        self.panes
            .iter()
            .map(|p| p.state.clone())
            .max_by_key(urgency)
            .unwrap_or(State::Idle)
    }

    /// Cuántos de sus paneles siguen vivos.
    pub fn running(&self) -> usize {
        self.panes.iter().filter(|p| p.state.is_running()).count()
    }
}

/// Lo que ve un proceso lanzado en una sesión.
///
/// Es el contrato entre flow y lo que corre dentro, y existe por una razón: un
/// agente lanzado aquí **no sabe que está aquí**. Para él es una terminal
/// cualquiera. Estas variables se lo dicen y, sobre todo, le dan una forma de
/// pedir algo que desde una terminal normal no existe: *ábreme esto al lado*.
///
/// El buzón es un directorio y el protocolo es dejar un fichero dentro con el
/// comando escrito. Se eligió eso y no un puerto, un socket ni un binario
/// auxiliar porque cualquier proceso —un agente, un script, un `echo` a pelo—
/// sabe escribir un fichero, en cualquier lenguaje y en los dos sistemas, sin
/// que flow tenga que abrir nada al exterior.
///
/// `FLOW_HOWTO` lleva las instrucciones en texto llano a propósito: un agente
/// es un modelo de lenguaje, así que la forma de explicarle cómo se usa esto es
/// explicárselo.
pub fn env(name: &str, id: u64, cwd: &str, inbox: &Path) -> Vec<(String, String)> {
    let inbox = inbox.display().to_string();
    let howto = format!(
        "Estás corriendo dentro de flow, un orquestador de agentes. Tu sesión se \
         llama «{name}» y todos sus paneles trabajan sobre {cwd}. Puedes abrir \
         paneles nuevos en tu propia sesión —una terminal para que se vea lo que \
         ejecutas, o más agentes para repartirte el trabajo— escribiendo el \
         comando en un fichero nuevo dentro del directorio FLOW_INBOX ({inbox}). \
         Ejemplo: `echo cargo test > \"{inbox}/1.cmd\"`. flow lee el fichero, lo \
         borra y abre el comando como un panel más al lado del tuyo, en el mismo \
         directorio. Un fichero, un panel. Caben {MAX_PANES} por sesión. Lanza \
         así todo proceso que dure o que interese mirar, en vez de dejarlo \
         corriendo donde nadie lo ve."
    );
    vec![
        ("FLOW".to_owned(), "1".to_owned()),
        ("FLOW_SESSION".to_owned(), name.to_owned()),
        ("FLOW_SESSION_ID".to_owned(), id.to_string()),
        ("FLOW_DIR".to_owned(), cwd.to_owned()),
        ("FLOW_INBOX".to_owned(), inbox),
        ("FLOW_PANES".to_owned(), MAX_PANES.to_string()),
        ("FLOW_HOWTO".to_owned(), howto),
    ]
}

/// Cuánto reclama cada estado. Solo ordena; los números no significan nada.
fn urgency(state: &State) -> u8 {
    match state {
        State::Blocked => 4,
        State::Failed(_) => 3,
        State::Exited(code) if *code != 0 => 3,
        State::Working => 2,
        State::Idle => 1,
        State::Exited(_) => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Una sesión de mentira: no hace falta lanzar procesos para comprobar cómo
    /// se resumen los estados.
    fn con_estados(estados: &[State]) -> State {
        estados
            .iter()
            .cloned()
            .max_by_key(urgency)
            .unwrap_or(State::Idle)
    }

    #[test]
    fn el_contrato_le_dice_al_agente_donde_esta_y_como_pedir() {
        let vars = env("claude", 7, "/repo", Path::new("/tmp/flow/7"));
        let get = |k: &str| {
            vars.iter()
                .find(|(n, _)| n == k)
                .map(|(_, v)| v.clone())
                .unwrap_or_default()
        };
        assert_eq!(get("FLOW"), "1");
        assert_eq!(get("FLOW_SESSION"), "claude");
        assert_eq!(get("FLOW_DIR"), "/repo");
        // El buzón tiene que salir tal cual en las instrucciones: es lo único
        // que el agente necesita copiar para poder usarlo.
        let howto = get("FLOW_HOWTO");
        assert!(howto.contains(&get("FLOW_INBOX")));
        assert!(howto.contains("claude") && howto.contains("/repo"));
    }

    #[test]
    fn el_resumen_ensena_lo_que_te_reclama() {
        // Un agente trabajando tan feliz no puede tapar una terminal que está
        // esperando que le contestes.
        assert_eq!(
            con_estados(&[State::Working, State::Blocked]),
            State::Blocked
        );
        // Ni un fallo.
        assert_eq!(
            con_estados(&[State::Working, State::Exited(1)]),
            State::Exited(1)
        );
        // Pero terminar bien no tiene que ensombrecer al que sigue trabajando.
        assert_eq!(
            con_estados(&[State::Exited(0), State::Working]),
            State::Working
        );
        // Y lo que pide algo gana a lo que solo ha roto.
        assert_eq!(
            con_estados(&[State::Failed("x".into()), State::Blocked]),
            State::Blocked
        );
    }
}
