//! Catálogo de agentes conocidos.
//!
//! flow no integra ningún agente: todos son procesos de terminal corrientes y
//! cualquier comando vale. Esto es solo comodidad — evitar teclear lo mismo
//! cada vez— y, sobre todo, **descubrimiento**: al abrir el formulario ves qué
//! agentes tienes instalados de verdad en esta máquina, en vez de una lista de
//! nombres que a lo mejor no existen.
//!
//! Añadir uno nuevo es una línea en `CATALOG`.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// La forma con la que se dibuja un agente.
///
/// **Son marcas dibujadas, no los logotipos oficiales.** flow no empaqueta
/// ningún recurso de marca ajeno: no los tiene, y meter el logotipo de otra
/// empresa dentro del binario es un asunto de derechos que un lanzador no
/// necesita tener. Lo que hay aquí son formas geométricas propias, elegidas para
/// **evocar** cada agente —el destello de Anthropic, el anillo de OpenAI, el
/// brillo de cuatro puntas de Gemini— y, sobre todo, para distinguirse entre
/// ellas de un vistazo a diez píxeles, que es el trabajo que tienen que hacer.
///
/// Van dibujadas con segmentos y no con emoji ni con una fuente de iconos, por
/// la misma razón que el resto de la interfaz: un trazo se clava en el píxel a
/// cualquier escala y no depende de que el sistema traiga el símbolo.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mark {
    /// Destello radial.
    Burst,
    /// Anillo.
    Ring,
    /// Brillo de cuatro puntas.
    Sparkle,
    /// Corchetes de terminal.
    Brackets,
    /// Punta de flecha.
    Chevron,
    Square,
    Triangle,
    Diamond,
    /// Rayo.
    Bolt,
    /// Punto macizo: lo que no tiene forma propia.
    Dot,
}

pub struct Preset {
    /// Cómo se llama para un humano.
    pub label: &'static str,
    /// Con qué forma se dibuja. Ver `Mark`.
    pub mark: Mark,
    /// Ejecutable que se busca en el PATH para saber si está instalado.
    pub program: &'static str,
    /// Línea de comando que se escribe en el formulario.
    pub command: &'static str,
    /// Qué es, para el tooltip.
    pub about: &'static str,
}

/// Agentes de programación que se manejan por terminal.
///
/// El orden es el que se ve en pantalla. Los que no estén instalados no
/// aparecen, así que la lista se adapta sola a cada máquina.
pub const AGENTS: &[Preset] = &[
    Preset {
        label: "claude",
        mark: Mark::Burst,
        program: "claude",
        command: "claude",
        about: "Claude Code — el agente de Anthropic",
    },
    Preset {
        label: "codex",
        mark: Mark::Ring,
        program: "codex",
        command: "codex",
        about: "Codex CLI — el agente de OpenAI",
    },
    Preset {
        label: "gemini",
        mark: Mark::Sparkle,
        program: "gemini",
        command: "gemini",
        about: "Gemini CLI — el agente de Google",
    },
    Preset {
        label: "opencode",
        mark: Mark::Brackets,
        program: "opencode",
        command: "opencode",
        about: "OpenCode — agente de terminal open source",
    },
    Preset {
        label: "aider",
        mark: Mark::Chevron,
        program: "aider",
        command: "aider",
        about: "Aider — programación en pareja sobre git",
    },
    Preset {
        label: "cursor",
        mark: Mark::Square,
        program: "cursor-agent",
        command: "cursor-agent",
        about: "Cursor Agent — el agente de Cursor en terminal",
    },
    Preset {
        label: "amp",
        mark: Mark::Triangle,
        program: "amp",
        command: "amp",
        about: "Amp — el agente de Sourcegraph",
    },
    Preset {
        label: "goose",
        mark: Mark::Diamond,
        program: "goose",
        command: "goose session",
        about: "Goose — el agente de Block",
    },
    Preset {
        label: "crush",
        mark: Mark::Bolt,
        program: "crush",
        command: "crush",
        about: "Crush — el agente de Charm",
    },
];

/// Comandos de uso diario que no son agentes pero se lanzan igual de a menudo.
///
/// Es una función y no una constante por culpa del primero: cuál es «el shell»
/// no se sabe hasta que se mira el entorno. Ver [`shell`].
pub fn tools() -> &'static [Preset] {
    static TOOLS: OnceLock<[Preset; 3]> = OnceLock::new();
    TOOLS.get_or_init(|| {
        let (program, command) = shell();
        [
            Preset {
                label: "shell",
                mark: Mark::Chevron,
                program,
                command,
                about: "Una terminal normal y corriente",
            },
            Preset {
                label: "tests",
                mark: Mark::Square,
                program: "cargo",
                command: "cargo test --color always",
                about: "Los tests de este proyecto",
            },
            Preset {
                label: "git",
                mark: Mark::Dot,
                program: "git",
                command: "git status",
                about: "Estado del repositorio",
            },
        ]
    })
}

/// El shell del usuario: el programa que se busca en el `PATH` y el comando con
/// el que llega precargado un panel de «shell».
///
/// **Fuera de Windows no se da por hecho `bash`.** Era lo que había, y estaba
/// mal en los dos sistemas donde flow no se desarrolla: en macOS el shell por
/// defecto es `zsh` desde Catalina, así que abrir un panel de shell te sacaba
/// del tuyo —sin tu `.zshrc`, sin tus alias, sin tu prompt—; y en una imagen
/// mínima de Linux, o en un sistema de quien usa `fish`, puede no haber `bash`
/// instalado, y entonces el botón «shell» del formulario **no aparecía**,
/// porque la detección del `PATH` no encontraba nada que lanzar.
///
/// Lo que dice cuál es el shell de alguien es `$SHELL`, que lo pone el sistema
/// al iniciar sesión. Se le quita la ruta y se queda el nombre —`/bin/zsh` →
/// `zsh`— por dos motivos: el `PATH` es donde se busca, y el nombre es lo que
/// se lee en el formulario, donde `/usr/local/bin/fish -i` no cabe.
///
/// Si `$SHELL` no está —un servicio, un contenedor, un `env -i`— se prueban por
/// orden los tres que podrían estar, y el último es `sh`, que POSIX obliga a
/// tener. Así el botón sale siempre.
///
/// Se calcula una vez: el shell de alguien no cambia a mitad de ejecución.
pub fn shell() -> (&'static str, &'static str) {
    static SHELL: OnceLock<(String, String)> = OnceLock::new();
    let (program, command) = SHELL.get_or_init(|| {
        if cfg!(windows) {
            return ("cmd".to_owned(), "cmd".to_owned());
        }
        let program = std::env::var_os("SHELL")
            .map(PathBuf::from)
            .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| {
                ["bash", "zsh", "sh"]
                    .into_iter()
                    .find(|s| in_path(s))
                    .unwrap_or("sh")
                    .to_owned()
            });
        // `-i` para que sea un shell de verdad —con tu configuración cargada y
        // tu prompt— y no un intérprete mudo. Lo entienden los tres.
        let command = format!("{program} -i");
        (program, command)
    });
    (program.as_str(), command.as_str())
}

/// La forma que le toca a un panel por cómo se llama.
///
/// El nombre de un panel sale del primer token de su comando (ver
/// `ui::spawn::name_of`), así que un panel lanzado con `claude` se llama
/// «claude» y aquí se le reconoce. Lo que no está en el catálogo no lleva marca:
/// un `cargo test` no es un agente y ponerle una forma sería inventarle una
/// identidad que no tiene.
pub fn mark_of(name: &str) -> Option<Mark> {
    let name = name.trim().to_lowercase();
    AGENTS
        .iter()
        .chain(tools())
        .find(|p| p.label == name || p.program == name)
        .map(|p| p.mark)
}

/// Los `program` que se han encontrado en el PATH.
///
/// Se calcula una vez al arrancar: recorrer el PATH en cada frame sería
/// absurdo, y que aparezca un agente instalado a mitad de sesión no es un caso
/// que merezca la pena vigilar.
pub struct Installed(HashSet<&'static str>);

impl Installed {
    pub fn detect() -> Self {
        let names: Vec<&'static str> = AGENTS.iter().chain(tools()).map(|p| p.program).collect();
        Installed(names.into_iter().filter(|n| in_path(n)).collect())
    }

    pub fn has(&self, program: &str) -> bool {
        self.0.contains(program)
    }

    /// Los agentes disponibles, en el orden del catálogo.
    pub fn agents(&self) -> impl Iterator<Item = &'static Preset> + '_ {
        AGENTS.iter().filter(|p| self.has(p.program))
    }

    /// Las herramientas disponibles.
    pub fn tools(&self) -> impl Iterator<Item = &'static Preset> + '_ {
        tools().iter().filter(|p| self.has(p.program))
    }

    pub fn agent_count(&self) -> usize {
        self.agents().count()
    }
}

/// ¿Existe `name` como ejecutable en el PATH?
///
/// Cada sistema tiene su forma de decir «esto se puede ejecutar», y las dos
/// están aquí porque las dos hacen falta:
///
/// - En **Windows** hay que probar además las extensiones de `PATHEXT`, porque
///   casi ningún CLI de Node es un `.exe`: `claude` suele ser `claude.cmd`.
/// - En **Unix** lo que manda es el bit de ejecución, no la extensión. Sin
///   mirarlo, un fichero cualquiera que se llame como un agente —un `README`
///   sin extensión, un script a medio escribir— lo daría por instalado y el
///   formulario ofrecería lanzar algo que no arranca.
fn in_path(name: &str) -> bool {
    let Some(path) = std::env::var_os("PATH") else {
        return false;
    };

    let exts: Vec<String> = if cfg!(windows) {
        std::env::var("PATHEXT")
            .unwrap_or_else(|_| ".EXE;.CMD;.BAT;.COM".to_owned())
            .split(';')
            .filter(|e| !e.is_empty())
            .map(|e| e.to_lowercase())
            .collect()
    } else {
        Vec::new()
    };

    std::env::split_paths(&path).any(|dir| {
        let base = dir.join(name);
        if ejecutable(&base) {
            return true;
        }
        exts.iter().any(|ext| {
            let mut with_ext = PathBuf::from(base.as_os_str());
            with_ext.as_mut_os_string().push(ext);
            ejecutable(&with_ext)
        })
    })
}

#[cfg(unix)]
fn ejecutable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    // Cualquiera de los tres bits: quién lo puede ejecutar es cosa del sistema
    // al lanzarlo, y comprobarlo aquí de verdad pediría mirar el usuario y los
    // grupos del proceso para nada.
    std::fs::metadata(path).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
}

#[cfg(not(unix))]
fn ejecutable(path: &Path) -> bool {
    path.is_file()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn el_catalogo_esta_bien_formado() {
        for p in AGENTS.iter().chain(tools()) {
            assert!(!p.label.is_empty());
            assert!(!p.about.is_empty());
            // El comando tiene que empezar por el programa que se busca, o la
            // detección estaría mintiendo sobre lo que se va a ejecutar.
            assert!(
                p.command.starts_with(p.program),
                "`{}` dice buscar `{}` pero ejecuta `{}`",
                p.label,
                p.program,
                p.command
            );
        }
    }

    #[test]
    fn no_hay_etiquetas_repetidas() {
        let mut seen = HashSet::new();
        for p in AGENTS.iter().chain(tools()) {
            assert!(seen.insert(p.label), "etiqueta repetida: {}", p.label);
        }
    }

    #[test]
    fn detecta_algo_que_seguro_existe() {
        // En cualquier máquina donde corran estos tests hay un shell.
        let shell = if cfg!(windows) { "cmd" } else { "sh" };
        assert!(in_path(shell), "no se encontró `{shell}` en el PATH");
        assert!(!in_path("no-existe-este-binario-xyzzy-42"));
    }

    /// El shell es **el del usuario**, no `bash` por decreto: en macOS es `zsh`
    /// y en un sistema mínimo puede no haber `bash` instalado, y entonces el
    /// botón «shell» del formulario no aparecía.
    #[test]
    fn el_shell_es_el_del_usuario_y_siempre_hay_uno() {
        let (program, command) = shell();
        assert!(!program.is_empty(), "el catálogo se quedó sin shell");
        assert!(command.starts_with(program));
        // Y es un nombre para buscar en el PATH, no una ruta: `$SHELL` viene
        // como `/bin/zsh` y lo que se busca —y lo que se lee en el formulario—
        // es `zsh`.
        assert!(
            !program.contains('/') && !program.contains('\\'),
            "el shell llegó como una ruta y no como un nombre: {program}"
        );
        // Sea cual sea, tiene que estar de verdad en esta máquina: si no, el
        // formulario se quedaría sin el único botón que hay siempre.
        assert!(
            Installed::detect().has(program),
            "el shell elegido (`{program}`) no está en el PATH"
        );
    }

    /// Un fichero cualquiera que se llame como un agente no cuenta: fuera de
    /// Windows lo que dice si algo se puede lanzar es el bit de ejecución.
    #[test]
    #[cfg(unix)]
    fn en_unix_un_fichero_sin_permiso_de_ejecucion_no_es_un_ejecutable() {
        use std::os::unix::fs::PermissionsExt;

        let dir = std::env::temp_dir().join(format!("flow-test-ejec-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("parece-un-agente");
        std::fs::write(&path, "no soy un programa").unwrap();

        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        assert!(!ejecutable(&path), "un fichero sin +x pasó por ejecutable");

        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert!(
            ejecutable(&path),
            "un fichero con +x no pasó por ejecutable"
        );

        // Y un directorio nunca lo es, aunque lleve los bits puestos.
        assert!(!ejecutable(&dir));

        std::fs::remove_dir_all(&dir).unwrap();
    }
}
