//! La línea de comandos, ejecutando el binario de verdad.
//!
//! Es el único test que no puede vivir dentro del módulo: `run::cli()` lee los
//! argumentos del proceso, y los de un `cargo test` son los de `cargo test`. La
//! única forma honesta de probar `flow --version` es ejecutar `flow --version`.
//!
//! Lo que se prueba aquí importa más de lo que parece para un binario de
//! ventana: **estos tres caminos no deben abrir ninguna ventana**. Si alguno se
//! escapa al arranque de `eframe`, el test se queda colgado con una ventana
//! abierta en la máquina de integración continua, que es exactamente el fallo
//! que hay que coger.
//!
//! `flow` a secas no se prueba aquí por lo mismo: abre la ventana, y eso ya es
//! su trabajo.

use std::process::Command;

fn flow() -> Command {
    Command::new(env!("CARGO_BIN_EXE_flow"))
}

/// La versión sale por la salida estándar y el proceso termina bien.
#[test]
fn la_version_se_imprime_y_no_abre_ventana() {
    let out = flow().arg("--version").output().expect("no se pudo ejecutar flow");
    assert!(out.status.success(), "flow --version salió con error");
    let texto = String::from_utf8_lossy(&out.stdout);
    assert!(
        texto.contains(env!("CARGO_PKG_VERSION")),
        "no dijo la versión: {texto}"
    );

    // Y por su forma corta, que es la que se teclea.
    let corta = flow().arg("-V").output().unwrap();
    assert!(corta.status.success());
}

/// La ayuda explica el subcomando y para qué sirve. En cuanto un binario acepta
/// un subcomando, que además abra una ventana cuando le preguntas cómo se usa es
/// de mala educación.
#[test]
fn la_ayuda_se_imprime_por_sus_tres_nombres() {
    for arg in ["--help", "-h", "help"] {
        let out = flow().arg(arg).output().expect("no se pudo ejecutar flow");
        assert!(out.status.success(), "flow {arg} salió con error");
        let texto = String::from_utf8_lossy(&out.stdout);
        assert!(texto.contains("flow run"), "la ayuda de {arg} no habla de run");
    }
}

/// `flow run` sin comando dice qué falta con un ejemplo, porque el error más
/// probable es no acordarse de cómo se escribe.
#[test]
fn run_sin_comando_avisa() {
    let out = flow().arg("run").output().expect("no se pudo ejecutar flow");
    assert_eq!(out.status.code(), Some(2));
    let texto = String::from_utf8_lossy(&out.stderr);
    assert!(texto.contains("falta el comando"), "no dijo qué falta: {texto}");
}

/// Fuera de flow no hay sesión en la que abrir el panel. El mensaje dice qué es
/// lo que falta y no solo que falta algo: el caso normal de este error es
/// haberlo escrito en una terminal cualquiera.
#[test]
fn run_fuera_de_flow_explica_que_esto_es_de_dentro() {
    let out = flow()
        .arg("run")
        .arg("cargo test")
        .env_remove("FLOW_INBOX")
        .output()
        .expect("no se pudo ejecutar flow");
    assert_eq!(out.status.code(), Some(1));
    let texto = String::from_utf8_lossy(&out.stderr);
    assert!(
        texto.contains("dentro de flow"),
        "el mensaje no explica de qué va: {texto}"
    );
}

/// Dentro de una sesión, la petición acaba en el buzón: un fichero, un panel.
/// Y el comando llega entero, con sus argumentos juntos como se escribieron.
#[test]
fn run_dentro_de_flow_deja_la_peticion_en_el_buzon() {
    let inbox = std::env::temp_dir().join("flow-tests").join("cli-buzon");
    let _ = std::fs::remove_dir_all(&inbox);
    std::fs::create_dir_all(&inbox).unwrap();

    let out = flow()
        .args(["run", "cargo", "test", "--", "--nocapture"])
        .env("FLOW_INBOX", &inbox)
        .output()
        .expect("no se pudo ejecutar flow");
    assert!(out.status.success(), "flow run falló dentro de una sesión");

    let peticiones: Vec<_> = std::fs::read_dir(&inbox).unwrap().flatten().collect();
    assert_eq!(peticiones.len(), 1, "no salió exactamente una petición");
    assert_eq!(
        std::fs::read_to_string(peticiones[0].path()).unwrap(),
        "cargo test -- --nocapture"
    );

    // Se dice «pedido» y no «abierto», porque son cosas distintas: lo abre flow,
    // y si la sesión está llena no lo abre.
    let dicho = String::from_utf8_lossy(&out.stdout);
    assert!(dicho.contains("pedido"), "dijo otra cosa: {dicho}");

    let _ = std::fs::remove_dir_all(&inbox);
}
