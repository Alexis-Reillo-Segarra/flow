//! `flow run`: pedir un panel desde dentro de una sesión.
//!
//! El buzón —un fichero con un comando dentro de `FLOW_INBOX`— ya se podía usar
//! con un `echo` a pelo, y esa sigue siendo la definición del protocolo (ver
//! `crate::session::env`). Esto es solo la forma cómoda de escribirlo:
//!
//! ```text
//! flow run npm run dev
//! ```
//!
//! Existe porque la fricción estaba matando el mecanismo. Lo que un agente
//! tenía que recordar para usarlo era la ruta del buzón, inventarse un nombre de
//! fichero que no chocara con otro, y acertar con la redirección y las comillas
//! —que no se escriben igual en `cmd`, en PowerShell y en un shell de Unix—.
//! Son cuatro cosas que salen mal a la primera y una que sale mal en silencio:
//! si el `echo` se equivoca de comillas, el fichero se escribe con el comando a
//! medias y lo que se abre al lado es un panel con un error raro. Un subcomando
//! con nombre no tiene ninguno de esos cuatro problemas.
//!
//! # Por qué no ejecuta nada
//!
//! `flow run` **no lanza el proceso**: lo pide y se va. El que lo lanza es flow,
//! en su propio PTY, para que sea un panel de verdad y no la salida de un
//! proceso colgando de otro. La consecuencia práctica, que conviene decirle al
//! agente en su fichero de contexto: **la salida no vuelve aquí**, se ve en el
//! panel. Por eso esto es para lo que dura o interesa mirar —un servidor, la
//! suite larga, seguir un log, un subagente— y no para un `git status` cuya
//! respuesta el agente necesita leer para seguir trabajando.

use std::path::{Path, PathBuf};

/// Lo que el proceso tiene que hacer al terminar de mirar la línea de comandos.
pub enum Cli {
    /// No era un subcomando: abrir la ventana, como siempre.
    Gui,
    /// Ya está todo dicho; salir con este código.
    Done(i32),
}

/// Mira los argumentos antes de que arranque la GUI.
///
/// flow es una aplicación de ventana y esto es lo único que hace por la línea de
/// comandos, así que no hay un analizador de argumentos ni falta: un subcomando,
/// y lo que no sea eso abre la ventana. `--help` y `--version` entran porque en
/// cuanto un binario acepta un subcomando, que además abra una ventana cuando le
/// preguntas cómo se usa es de mala educación.
pub fn cli() -> Cli {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("run") => {
            attach_console();
            Cli::Done(run(&args[1..]))
        }
        Some("--help" | "-h" | "help") => {
            attach_console();
            println!("{HELP}");
            Cli::Done(0)
        }
        Some("--version" | "-V") => {
            attach_console();
            println!("flow {}", env!("CARGO_PKG_VERSION"));
            Cli::Done(0)
        }
        _ => Cli::Gui,
    }
}

const HELP: &str = "\
flow — orquestador de agentes CLI

  flow                 abre la ventana
  flow run <comando>   abre <comando> en un panel de tu sesión

`flow run` solo funciona desde dentro de flow, porque es lo que le da la sesión
en la que abrir el panel. No ejecuta el comando: se lo pide a flow, que lo abre
en su propio PTY. La salida se ve en el panel, no aquí.

  flow run cargo test
  flow run npm run dev
";

/// Escribe la petición en el buzón. Devuelve el código de salida del proceso.
fn run(args: &[String]) -> i32 {
    if args.is_empty() {
        eprintln!("flow run: falta el comando. Ejemplo: flow run cargo test");
        return 2;
    }
    let Some(inbox) = std::env::var_os("FLOW_INBOX").map(PathBuf::from) else {
        // El caso normal de este error es haberlo escrito en una terminal
        // cualquiera, así que el mensaje dice qué es lo que falta y no solo que
        // falta algo.
        eprintln!(
            "flow run: esto solo funciona dentro de flow (no hay FLOW_INBOX).\n\
             Abre el comando en una sesión de flow, o ejecútalo aquí a secas."
        );
        return 1;
    };

    let cmdline = join(args);
    match deposit(&inbox, &cmdline) {
        Ok(()) => {
            // Se dice que se ha pedido y no que se ha lanzado, porque son cosas
            // distintas: lo abre flow, y si la sesión está llena no lo abre.
            println!("flow: pedido un panel para «{cmdline}»");
            0
        }
        Err(err) => {
            eprintln!("flow run: no se pudo escribir en el buzón: {err}");
            1
        }
    }
}

/// Deja el fichero en el buzón, ya entero.
///
/// Se escribe fuera y se mueve dentro, en vez de escribir directamente en el
/// buzón, porque flow lo lee cada 300 ms sin saber si alguien está escribiendo:
/// pillar el fichero a medias abriría un panel con medio comando. El temporal va
/// en el directorio padre —el mismo del que cuelgan los buzones de todas las
/// sesiones— para que mover sea un `rename` en el mismo volumen, que es atómico,
/// y no una copia.
///
/// El nombre lleva delante los nanosegundos con ceros a la izquierda porque
/// `app::collect_requests` ordena los ficheros por nombre: si un agente suelta
/// cinco de golpe, los paneles salen en el orden en que los pidió.
fn deposit(inbox: &Path, cmdline: &str) -> std::io::Result<()> {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let name = format!("{stamp:039}-{}", std::process::id());

    let staging = inbox.parent().unwrap_or(inbox);
    let tmp = staging.join(format!("{name}.tmp"));
    let final_path = inbox.join(format!("{name}.cmd"));

    std::fs::write(&tmp, cmdline)?;
    match std::fs::rename(&tmp, &final_path) {
        Ok(()) => Ok(()),
        Err(err) => {
            // Si no se pudo mover, el temporal no puede quedarse ahí para
            // siempre: nadie más lo va a limpiar.
            let _ = std::fs::remove_file(&tmp);
            Err(err)
        }
    }
}

/// Rehace la línea de comandos a partir de los argumentos.
///
/// El comando acaba pasando por un shell (`cmd /C` en Windows, `$SHELL -c`
/// fuera), así que lo que se escribe en el buzón es una línea, no una lista. Al
/// volver a juntarla hay que devolverle las comillas al argumento que traía
/// espacios, porque el shell del usuario ya se las quitó al llamarnos: sin esto,
/// `flow run cargo test -- --nombre "dos palabras"` llegaría al panel como tres
/// argumentos donde había uno.
///
/// No pretende ser un citador completo —para eso está el shell— y con un
/// comando que necesite comillas de las raras lo honrado es pasarlo entero como
/// un solo argumento: `flow run "algo | otra cosa"`.
fn join(args: &[String]) -> String {
    args.iter()
        .map(|a| quote(a))
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_owned()
}

fn quote(arg: &str) -> String {
    if arg.is_empty() {
        return "\"\"".to_owned();
    }
    if !arg.contains(char::is_whitespace) {
        return arg.to_owned();
    }
    // Comillas dobles en Windows y simples fuera, que es lo que entiende cada
    // shell. Si el argumento ya trae las suyas, se deja tal cual: rodearlo otra
    // vez sería peor que no tocarlo.
    if cfg!(windows) {
        if arg.contains('"') {
            arg.to_owned()
        } else {
            format!("\"{arg}\"")
        }
    } else if arg.contains('\'') {
        arg.to_owned()
    } else {
        format!("'{arg}'")
    }
}

/// En Windows el binario es de subsistema gráfico —para que abrir flow no deje
/// una consola negra detrás de la ventana—, y eso significa que no trae consola
/// propia. Si lo llamó una terminal, hay que engancharse a la suya para que se
/// vea lo que imprime; si lo llamó un agente, sus tuberías ya están heredadas y
/// esto no hace falta ni molesta.
#[cfg(windows)]
fn attach_console() {
    // -1 es ATTACH_PARENT_PROCESS. Falla si ya hay consola, que es justo el caso
    // en el que no había nada que hacer.
    unsafe {
        windows_sys::Win32::System::Console::AttachConsole(u32::MAX);
    }
}

#[cfg(not(windows))]
fn attach_console() {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn junta_los_argumentos_devolviendo_las_comillas_donde_hacian_falta() {
        let arg = |s: &str| s.to_owned();
        assert_eq!(join(&[arg("cargo"), arg("test")]), "cargo test");
        // Lo que traía espacios los sigue trayendo, y se nota que es uno solo.
        let con_espacios = join(&[arg("cargo"), arg("test"), arg("dos palabras")]);
        if cfg!(windows) {
            assert_eq!(con_espacios, "cargo test \"dos palabras\"");
        } else {
            assert_eq!(con_espacios, "cargo test 'dos palabras'");
        }
        // Un comando entero entre comillas llega entero.
        assert_eq!(join(&[arg("npm run dev")]), quote("npm run dev"));
    }

    #[test]
    fn un_argumento_que_ya_trae_comillas_no_se_toca() {
        // Rodear de comillas algo que ya las lleva rompe más de lo que arregla.
        let ya = if cfg!(windows) {
            "dice \"hola\" fuerte"
        } else {
            "dice 'hola' fuerte"
        };
        assert_eq!(quote(ya), ya);
    }

    #[test]
    fn el_fichero_aparece_entero_o_no_aparece() {
        // El temporal se escribe fuera del buzón y se mueve dentro. Lo que se
        // comprueba aquí es lo que de eso se puede ver desde fuera: en el buzón
        // acaba un `.cmd` con el comando entero, y ningún resto al lado.
        let raiz = std::env::temp_dir().join(format!("flow-test-{}", std::process::id()));
        let inbox = raiz.join("s1");
        std::fs::create_dir_all(&inbox).unwrap();

        deposit(&inbox, "cargo test").unwrap();

        let dentro: Vec<_> = std::fs::read_dir(&inbox).unwrap().flatten().collect();
        assert_eq!(dentro.len(), 1);
        let path = dentro[0].path();
        assert_eq!(path.extension().unwrap(), "cmd");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "cargo test");
        // El temporal no se queda por el camino.
        let fuera: Vec<_> = std::fs::read_dir(&raiz)
            .unwrap()
            .flatten()
            .filter(|e| e.path().is_file())
            .collect();
        assert!(fuera.is_empty(), "quedó un temporal sin mover");

        std::fs::remove_dir_all(&raiz).unwrap();
    }

    #[test]
    fn dos_peticiones_seguidas_no_se_pisan_y_salen_en_orden() {
        // `app::collect_requests` ordena por nombre, así que el nombre tiene que
        // ordenar por tiempo: si un agente pide cinco paneles de golpe, salen en
        // el orden en que los pidió.
        let raiz = std::env::temp_dir().join(format!("flow-test-orden-{}", std::process::id()));
        let inbox = raiz.join("s1");
        std::fs::create_dir_all(&inbox).unwrap();

        deposit(&inbox, "primero").unwrap();
        deposit(&inbox, "segundo").unwrap();

        let mut nombres: Vec<PathBuf> = std::fs::read_dir(&inbox)
            .unwrap()
            .flatten()
            .map(|e| e.path())
            .collect();
        assert_eq!(nombres.len(), 2, "una petición pisó a la otra");
        nombres.sort();
        let leer = |p: &PathBuf| std::fs::read_to_string(p).unwrap();
        assert_eq!(leer(&nombres[0]), "primero");
        assert_eq!(leer(&nombres[1]), "segundo");

        std::fs::remove_dir_all(&raiz).unwrap();
    }
}
