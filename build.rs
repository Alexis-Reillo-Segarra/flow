//! Le pega al ejecutable de Windows sus recursos: el icono que ve el Explorador
//! y la ficha de "Propiedades" del fichero.
//!
//! **Por qué esto existe.** El icono de la ventana lo dibuja `src/logo.rs` en
//! tiempo de ejecución, y con eso basta mientras flow está abierto. Pero
//! `flow.exe` se reparte suelto —se descarga, se copia al escritorio, se ancla a
//! la barra de tareas— y en todos esos sitios Windows lee el icono *del fichero*
//! sin abrirlo. Sin recursos incrustados sale el icono genérico de aplicación,
//! que en un programa que se distribuye como un único `.exe` es justo la parte
//! que hace dudar de si es de fiar.
//!
//! **Por qué se enlaza un `.res` ya compilado.** Pasar de `.rc` a `.res` es
//! trabajo de `rc.exe`, que viene con el SDK de Windows y no está en el `PATH`:
//! encontrarlo es lo que hacen las cajas tipo `embed-resource`. Con el `.res`
//! versionado no hace falta ninguna: compilar flow sigue siendo `cargo build` y
//! nada más, que es la promesa del README. El coste es regenerarlo a mano cuando
//! se toque el icono o la versión, y de eso avisa la comprobación de abajo.
//!
//! Ver `assets/brand/windows/README.md` para el comando exacto.

use std::path::Path;

fn main() {
    let rc = "assets/brand/windows/flow.rc";
    let res = "assets/brand/windows/flow.res";
    println!("cargo:rerun-if-changed={rc}");
    println!("cargo:rerun-if-changed={res}");
    println!("cargo:rerun-if-changed=build.rs");

    // Fuera de Windows no hay recursos que pegar, y con el enlazador de MinGW el
    // formato `.res` tampoco vale: allí flow se compila igual, solo que el icono
    // del fichero lo pone el escritorio.
    let windows = std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows");
    let msvc = std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc");
    if !windows || !msvc {
        return;
    }

    comprobar_version(rc);

    // Absoluta: el enlazador no corre en la raíz del paquete.
    let dir = std::env::var("CARGO_MANIFEST_DIR").expect("cargo siempre la pone");
    let res = Path::new(&dir).join(res);
    assert!(
        res.exists(),
        "falta {}: regenéralo con rc.exe (ver assets/brand/windows/README.md)",
        res.display()
    );
    // `-bins` y no `rustc-link-arg` a secas: solo lo quieren los ejecutables. A
    // los binarios de test les daría igual, pero no tienen por qué cargarlo.
    println!("cargo:rustc-link-arg-bins={}", res.display());
}

/// La versión del `.rc` tiene que ser la del `Cargo.toml`.
///
/// El `.res` es un binario que nadie va a leer al subir la versión, así que sin
/// esto la ficha de Propiedades del `.exe` se quedaría diciendo `0.1.0` para
/// siempre y nadie se enteraría hasta que alguien mirase. Falla el build, que es
/// la única forma de que se entere quien está publicando.
fn comprobar_version(rc: &str) {
    let ver = std::env::var("CARGO_PKG_VERSION").expect("cargo siempre la pone");
    let mut n = ver.split('.');
    let (may, men, par) = (
        n.next().unwrap_or("0"),
        n.next().unwrap_or("0"),
        n.next().unwrap_or("0"),
    );

    let texto = std::fs::read_to_string(rc).unwrap_or_else(|e| panic!("no se pudo leer {rc}: {e}"));
    // El `.rc` está alineado en columnas y eso son varios espacios seguidos;
    // comparar sobre el texto aplanado evita atarse a la alineación.
    let plano = texto.split_whitespace().collect::<Vec<_>>().join(" ");

    for esperado in [
        format!("FILEVERSION {may}, {men}, {par}, 0"),
        format!("PRODUCTVERSION {may}, {men}, {par}, 0"),
        format!("VALUE \"FileVersion\", \"{ver}\""),
        format!("VALUE \"ProductVersion\", \"{ver}\""),
    ] {
        assert!(
            plano.contains(&esperado),
            "{rc} no dice la versión {ver}: falta «{esperado}».\n\
             Actualízalo y vuelve a compilar el .res \
             (ver assets/brand/windows/README.md)."
        );
    }
}
