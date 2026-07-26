//! Encontrar los repositorios que tienes vivos en el equipo.
//!
//! La lista de [proyectos](crate::projects) son las rutas que **ya has usado**,
//! y no sirve la primera vez: recién descargado flow, el formulario te pide que
//! teclees una ruta entera y no te propone nada. Esto lo llena solo.
//!
//! # Qué es un repositorio
//!
//! Un directorio con un `.git` dentro, y **da igual que sea carpeta o fichero**:
//! en un *worktree* o en un submódulo, `.git` es un fichero de una línea con un
//! `gitdir:` que apunta a otro sitio. Comprobar que es un directorio —que es lo
//! primero que sale— deja fuera justo los repos de quien usa worktrees.
//!
//! No se ejecuta `git` ni una vez. La rama se lee de `.git/HEAD`, que es un
//! fichero de una línea. Lo que **no** se mira es si el repo está sucio: eso es
//! leer el índice y recorrer el árbol de trabajo, o un `git status` por repo, y
//! es la diferencia entre abrir un formulario y esperar a que abra.
//!
//! # Dónde se busca, y por qué no en todas partes
//!
//! Barrer el disco entero está descartado: `C:\` son cientos de miles de
//! directorios, un `stat` por entrada pasando por el antivirus, y basta una
//! unidad de red montada o una carpeta de OneDrive que descargue a demanda para
//! que el recorrido se quede colgado minutos. Todo eso para encontrar veinte
//! carpetas.
//!
//! La señal buena ya la tenemos gratis: **los proyectos que has abierto**. Si
//! trabajas en `C:\Repos\projects\flow`, entonces `C:\Repos\projects` es una
//! huerta de repositorios y mirar ahí cuesta un `read_dir`. A eso se le suman los
//! sitios donde los ponen las herramientas por defecto —`~\source\repos` de
//! Visual Studio, `~\Documents\GitHub` de GitHub Desktop— y los nombres que usa
//! todo el mundo.
//!
//! Con eso, `DEPTH` niveles y podando al encontrar, esto son unos cientos de
//! `read_dir`. Aun así corre en un hilo aparte: el coste real de tocar el disco
//! no lo decide el número de directorios sino lo que tarde el antivirus, y eso
//! no se puede acotar desde aquí.
//!
//! # Qué es "vivo"
//!
//! La fecha de modificación de `.git`. Un commit, un checkout, un fetch o un
//! `git add` la tocan, así que ordena por «cuándo trabajé aquí por última vez»
//! sin ejecutar nada. Un repo que llevas ocho meses sin tocar se cae al fondo él
//! solo, y no hace falta ninguna heurística más.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Cuántos niveles se baja desde cada raíz.
///
/// Tres cubre `C:\Repos\projects\flow` desde `C:\Repos` y `~\source\repos\org\repo`,
/// que son las dos formas en que la gente ordena esto. El cuarto nivel multiplica
/// el coste para encontrar lo que ya no llamarías «mis repos».
const DEPTH: usize = 3;

/// Tope de directorios que se llegan a mirar, pase lo que pase.
///
/// No es una optimización, es un freno: si alguien tiene una raíz con diez mil
/// carpetas, el hilo tiene que terminar igualmente y con lo que haya encontrado.
const MAX_VISITS: usize = 4_000;

/// Cuántos repositorios se ofrecen. Pasado ese punto la lista deja de ser una
/// sugerencia y pasa a ser un explorador de archivos, que es justo lo que el
/// formulario no es.
pub const MAX: usize = 10;

/// Lo que nunca se abre. Son los directorios que más entradas tienen y menos
/// repositorios contienen; sin esta lista, un `node_modules` se come el
/// presupuesto entero de `MAX_VISITS`.
const SKIP: [&str; 12] = [
    ".git",
    "node_modules",
    "target",
    "vendor",
    "dist",
    "build",
    "out",
    ".venv",
    "venv",
    "__pycache__",
    ".cache",
    ".next",
];

/// Los nombres que la gente le pone a la carpeta donde guarda sus repos, más los
/// que crean solas las herramientas.
const HOME_DIRS: [&str; 8] = [
    "source/repos",     // Visual Studio
    "Documents/GitHub", // GitHub Desktop
    "repos",
    "Repos",
    "dev",
    "code",
    "projects",
    "git",
];

/// Un repositorio encontrado.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Repo {
    /// La ruta, tal y como se le pasaría a una sesión.
    pub dir: String,
    /// El último tramo: como lo llamarías de viva voz.
    pub name: String,
    /// La rama, si se pudo leer. Es un dato de apoyo, no un motivo para no
    /// enseñar el repo: un repo recién clonado sin `HEAD` legible sigue siendo
    /// un sitio donde abrir sesión.
    pub branch: Option<String>,
    /// Cuándo se tocó `.git` por última vez. Es lo que ordena la lista.
    pub touched: SystemTime,
}

/// Por dónde empezar a mirar.
///
/// Primero los vecinos de lo que ya has usado —el padre de un proyecto tuyo es
/// casi siempre la carpeta donde tienes los demás— y después los sitios de
/// siempre dentro de tu perfil.
pub fn roots(recent: &[String]) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = Vec::new();
    let mut push = |p: PathBuf| {
        // La raíz de una unidad no es una raíz de búsqueda: quien abra una
        // sesión en `C:\algo` no está pidiendo que se recorra `C:\` entero.
        if p.parent().is_none() || !p.is_dir() || out.contains(&p) {
            return;
        }
        out.push(p);
    };

    for dir in recent {
        if let Some(parent) = Path::new(dir.trim()).parent() {
            push(parent.to_path_buf());
        }
    }

    if let Some(home) = home() {
        for name in HOME_DIRS {
            push(home.join(name));
        }
        // El propio perfil, pero solo su primer nivel: ahí es donde acaba el
        // repo que alguien clonó con prisa, y mirarlo entero es lo que no se
        // puede hacer.
        push(home);
    }
    out
}

fn home() -> Option<PathBuf> {
    let var = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
    std::env::var_os(var).map(PathBuf::from)
}

/// Recorre las raíces y devuelve lo que encuentre, lo más reciente primero.
///
/// `exclude` son las rutas que ya están en la lista de recientes: ofrecerlas otra
/// vez en un segundo grupo sería enseñar dos veces lo mismo.
pub fn scan(roots: &[PathBuf], exclude: &[String]) -> Vec<Repo> {
    let mut found: Vec<Repo> = Vec::new();
    let mut visits = 0usize;

    for root in roots {
        // El propio directorio raíz puede ser un repositorio; si lo es, no se
        // baja dentro.
        let desde = found.len();
        walk(root, 0, &mut visits, &mut found);
        for repo in &mut found[desde..] {
            repo.name = label(root, Path::new(&repo.dir));
        }
        if visits >= MAX_VISITS {
            break;
        }
    }

    let ya = |dir: &str| exclude.iter().any(|e| same_path(e.trim(), dir));
    found.retain(|r| !ya(&r.dir));
    // Lo más reciente primero, que es la definición de "vivo" de este módulo.
    found.sort_by(|a, b| b.touched.cmp(&a.touched).then_with(|| a.dir.cmp(&b.dir)));
    found.dedup_by(|a, b| a.dir == b.dir);
    found.truncate(MAX);
    found
}

/// Cómo se llama un repositorio en el botón.
///
/// El último tramo de la ruta se queda corto en cuanto alguien agrupa: en
/// `C:\Repos\proyectos` con `roomatch\client`, `roomatch\api` y `roomatch\proxy`
/// dentro, la lista sale con tres botones que ponen `client`, `api` y `proxy` y
/// no dicen de qué son. Así que si el repo **no** cuelga directamente de la raíz
/// que se barrió, se le pone delante su carpeta padre.
///
/// Se queda en dos tramos y no en la ruta relativa entera porque el botón
/// compite por el ancho con otros nueve, y el tercer nivel ya no desambigua
/// nada que no desambiguara el segundo. La ruta completa está en el `hover`.
fn label(root: &Path, dir: &Path) -> String {
    let relativo = dir.strip_prefix(root).unwrap_or(dir);
    let tramos: Vec<String> = relativo
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .filter(|s| !s.is_empty())
        .collect();
    match tramos.len() {
        // La propia raíz es el repositorio.
        0 => crate::projects::name_of(&dir.display().to_string()),
        1 => tramos[0].clone(),
        n => format!("{}/{}", tramos[n - 2], tramos[n - 1]),
    }
}

/// Compara dos rutas como las escribiría una persona: en Windows sin distinguir
/// mayúsculas y dando igual la barra, porque `C:/repos` y `C:\Repos` son el
/// mismo sitio y enseñarlo dos veces queda a tonto.
///
/// El formulario la usa para lo mismo visto del revés: decidir cuál de los
/// botones de ruta es el que está puesto ahora. Comparando las cadenas a pelo,
/// una ruta pegada del explorador con la otra barra deja los diez botones
/// apagados aunque uno de ellos sea justo el sitio al que apunta el campo.
pub fn same_path(a: &str, b: &str) -> bool {
    let norm = |s: &str| {
        let s = s.trim_end_matches(['/', '\\']).replace('/', "\\");
        if cfg!(windows) {
            s.to_lowercase()
        } else {
            s
        }
    };
    norm(a) == norm(b)
}

fn walk(dir: &Path, depth: usize, visits: &mut usize, out: &mut Vec<Repo>) {
    if *visits >= MAX_VISITS || out.len() >= MAX_VISITS {
        return;
    }
    *visits += 1;

    // Podar al encontrar: dentro de un repositorio no hay más repositorios que
    // te interesen —los submódulos son suyos, no tuyos—, y seguir bajando por
    // el árbol de trabajo es lo más caro que se puede hacer aquí.
    if let Some(repo) = read_repo(dir) {
        out.push(repo);
        return;
    }
    if depth >= DEPTH {
        return;
    }

    let Ok(entries) = std::fs::read_dir(dir) else {
        return; // Sin permisos, o ya no está. No es un error: es un sitio menos.
    };
    for entry in entries.flatten() {
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        // `is_symlink` cubre también las junctions de Windows, que es por donde
        // se entra en un bucle infinito sin enterarse.
        if !kind.is_dir() || kind.is_symlink() {
            continue;
        }
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if SKIP.contains(&name.as_ref()) || name.starts_with('.') {
            continue;
        }
        walk(&entry.path(), depth + 1, visits, out);
        if *visits >= MAX_VISITS {
            return;
        }
    }
}

/// ¿Este directorio es un repositorio? Devuelve lo que se sabe de él.
fn read_repo(dir: &Path) -> Option<Repo> {
    let git = dir.join(".git");
    // Existe, y punto: en un worktree o un submódulo `.git` es un fichero con un
    // `gitdir:` dentro. Pedir que sea un directorio deja fuera esos.
    let meta = std::fs::metadata(&git).ok()?;
    Some(Repo {
        dir: dir.display().to_string(),
        name: crate::projects::name_of(&dir.display().to_string()),
        branch: branch_of(&git),
        // Si el sistema no sabe decir la fecha, el repo aparece igual pero el
        // último: no saber cuándo lo tocaste no lo hace más reciente.
        touched: meta.modified().unwrap_or(SystemTime::UNIX_EPOCH),
    })
}

/// La rama actual, leyendo `.git/HEAD`.
///
/// En un worktree `.git` es un fichero y su `HEAD` está en otro sitio; ahí se
/// devuelve `None` en vez de ponerse a seguir el `gitdir:`. La rama es un dato
/// de apoyo, no vale una segunda vuelta de lecturas.
fn branch_of(git: &Path) -> Option<String> {
    let head = std::fs::read_to_string(git.join("HEAD")).ok()?;
    let head = head.trim();
    match head.strip_prefix("ref: refs/heads/") {
        Some(name) => Some(name.to_owned()),
        // HEAD suelto: se enseña el principio del hash, que es como lo dice git.
        None if head.len() >= 7 && head.chars().all(|c| c.is_ascii_hexdigit()) => {
            Some(head[..7].to_owned())
        }
        None => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Un árbol de mentira en el temporal, para no depender de lo que haya en el
    /// disco de quien ejecute los tests.
    struct Arbol(PathBuf);

    impl Arbol {
        fn nuevo(nombre: &str) -> Self {
            let raiz =
                std::env::temp_dir().join(format!("flow-repos-{nombre}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&raiz);
            std::fs::create_dir_all(&raiz).unwrap();
            Self(raiz)
        }
        /// El árbol entero como única raíz de búsqueda, que es lo que se le pasa
        /// a `scan` en casi todos los tests.
        fn raices(&self) -> &[PathBuf] {
            std::slice::from_ref(&self.0)
        }
        fn dir(&self, rel: &str) -> PathBuf {
            let p = self.0.join(rel);
            std::fs::create_dir_all(&p).unwrap();
            p
        }
        /// Un repo con `.git` de carpeta y su `HEAD`.
        fn repo(&self, rel: &str, rama: &str) -> PathBuf {
            let p = self.dir(rel);
            let git = p.join(".git");
            std::fs::create_dir_all(&git).unwrap();
            std::fs::write(git.join("HEAD"), format!("ref: refs/heads/{rama}\n")).unwrap();
            p
        }
        /// Un worktree: `.git` es un fichero.
        fn worktree(&self, rel: &str) -> PathBuf {
            let p = self.dir(rel);
            std::fs::write(p.join(".git"), "gitdir: /otro/sitio\n").unwrap();
            p
        }
    }

    impl Drop for Arbol {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn nombres(repos: &[Repo]) -> Vec<&str> {
        repos.iter().map(|r| r.name.as_str()).collect()
    }

    #[test]
    fn encuentra_los_repos_y_les_lee_la_rama() {
        let a = Arbol::nuevo("basico");
        a.repo("uno", "main");
        a.repo("dos", "feature/temas");
        a.dir("tres"); // Una carpeta cualquiera no es un repositorio.

        let mut encontrados = scan(a.raices(), &[]);
        encontrados.sort_by(|x, y| x.name.cmp(&y.name));
        assert_eq!(nombres(&encontrados), ["dos", "uno"]);
        assert_eq!(encontrados[0].branch.as_deref(), Some("feature/temas"));
    }

    #[test]
    fn un_repo_agrupado_lleva_su_carpeta_delante() {
        // Tres repos que se llaman `client`, `api` y `proxy` no dicen nada; con
        // su carpeta delante se leen de un vistazo.
        let a = Arbol::nuevo("agrupados");
        a.repo("roomatch/client", "main");
        a.repo("roomatch/api", "main");
        a.repo("suelto", "main");

        let mut encontrados = scan(a.raices(), &[]);
        encontrados.sort_by(|x, y| x.name.cmp(&y.name));
        assert_eq!(
            nombres(&encontrados),
            ["roomatch/api", "roomatch/client", "suelto"]
        );
    }

    #[test]
    fn un_worktree_tambien_es_un_repo() {
        // `.git` es un fichero aquí. Es el caso que se pierde si se comprueba
        // que sea un directorio.
        let a = Arbol::nuevo("worktree");
        a.worktree("wt");
        let encontrados = scan(a.raices(), &[]);
        assert_eq!(nombres(&encontrados), ["wt"]);
        // Sin HEAD legible, pero se ofrece igual.
        assert_eq!(encontrados[0].branch, None);
    }

    #[test]
    fn dentro_de_un_repo_no_se_sigue_buscando() {
        // Un submódulo es del repo, no tuyo, y el árbol de trabajo es lo más
        // caro de recorrer que hay aquí.
        let a = Arbol::nuevo("poda");
        a.repo("padre", "main");
        a.repo("padre/vendor/hijo", "main");
        let encontrados = scan(a.raices(), &[]);
        assert_eq!(nombres(&encontrados), ["padre"]);
    }

    #[test]
    fn no_se_entra_en_node_modules_ni_en_lo_oculto() {
        let a = Arbol::nuevo("saltos");
        a.repo("node_modules/paquete", "main");
        a.repo(".cosas/otro", "main");
        a.repo("bueno", "main");
        assert_eq!(nombres(&scan(a.raices(), &[])), ["bueno"]);
    }

    #[test]
    fn no_baja_mas_de_lo_dicho() {
        let a = Arbol::nuevo("hondo");
        a.repo("uno/dos/tres/muy-hondo", "main");
        assert!(scan(a.raices(), &[]).is_empty());
        // Y desde un nivel más abajo, el mismo repo sí aparece, con su carpeta
        // padre delante por estar agrupado.
        assert_eq!(nombres(&scan(&[a.0.join("uno")], &[])), ["tres/muy-hondo"]);
    }

    #[test]
    fn lo_que_ya_esta_en_recientes_no_se_repite() {
        let a = Arbol::nuevo("excluir");
        let repo = a.repo("mio", "main");
        let ruta = repo.display().to_string();
        assert!(scan(a.raices(), std::slice::from_ref(&ruta)).is_empty());
        // Y da igual cómo esté escrita la ruta: en Windows, con la otra barra o
        // en otras mayúsculas es el mismo sitio.
        if cfg!(windows) {
            let otra = ruta.replace('\\', "/").to_uppercase();
            assert!(scan(a.raices(), &[otra]).is_empty());
        }
    }

    #[test]
    fn lo_recien_tocado_va_primero() {
        let a = Arbol::nuevo("orden");
        let viejo = a.repo("viejo", "main");
        std::thread::sleep(std::time::Duration::from_millis(20));
        a.repo("nuevo", "main");
        // Se le da al viejo una fecha de hace un año para no depender de la
        // resolución del reloj del sistema de ficheros.
        let hace_un_ano = SystemTime::now() - std::time::Duration::from_secs(365 * 24 * 3600);
        filetime(&viejo.join(".git"), hace_un_ano);

        assert_eq!(nombres(&scan(a.raices(), &[])), ["nuevo", "viejo"]);
    }

    /// Cambia la fecha de modificación de algo. No hay API estable en `std`, así
    /// que se hace con lo que da cada sistema.
    fn filetime(path: &Path, time: SystemTime) {
        let file = std::fs::OpenOptions::new()
            .write(true)
            .open(path)
            .or_else(|_| {
                // Un directorio no se abre en escritura; se toca su HEAD, que es
                // lo que de verdad mueve la fecha del `.git` en git.
                std::fs::OpenOptions::new()
                    .write(true)
                    .open(path.join("HEAD"))
            });
        if let Ok(file) = file {
            let _ = file.set_modified(time);
        }
    }

    #[test]
    fn la_raiz_de_una_unidad_nunca_es_una_raiz_de_busqueda() {
        // Quien abre una sesión en `C:\algo` no está pidiendo que se recorra el
        // disco entero.
        let raiz = if cfg!(windows) { r"C:\algo" } else { "/algo" };
        let candidatas = roots(&[raiz.to_owned()]);
        assert!(
            candidatas.iter().all(|p| p.parent().is_some()),
            "se coló la raíz de una unidad: {candidatas:?}"
        );
    }

    #[test]
    fn el_head_suelto_se_lee_como_lo_dice_git() {
        let a = Arbol::nuevo("head");
        let repo = a.repo("suelto", "main");
        std::fs::write(
            repo.join(".git").join("HEAD"),
            "9fceb02d0ae598e95dc970b74767f19372d61af8\n",
        )
        .unwrap();
        let encontrados = scan(a.raices(), &[]);
        assert_eq!(encontrados[0].branch.as_deref(), Some("9fceb02"));
    }
}
