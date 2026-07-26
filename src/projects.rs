//! Los proyectos: los directorios sobre los que trabajas.
//!
//! Una sesión ya vivía atada a un directorio —es lo que comparten sus paneles y
//! la razón de que la sesión exista—, pero ese directorio había que teclearlo
//! entero cada vez. Un proyecto no es nada más que eso: una ruta que flow ya te
//! ha visto usar, para que abrir sesión sobre ella sea pulsar su nombre.
//!
//! Se guardan **entre ejecuciones**, y es lo único que flow escribe en tu disco
//! aparte de los buzones del temporal. El formato es un fichero de texto con una
//! ruta por línea, la más reciente arriba. Ni JSON ni TOML a propósito: no hay
//! nada que estructurar, se puede abrir con cualquier editor, y un fichero medio
//! escrito por un apagón se lee igual de bien —las líneas que sobrevivieron son
//! rutas válidas y las demás se descartan solas al comprobar que existen—.
//!
//! Nada de esto es crítico: si el directorio de configuración no se puede
//! escribir, flow funciona igual y simplemente no recuerda. Perder la lista de
//! recientes no vale ni un mensaje de error, y desde luego no vale una sesión
//! que no se abre.

use std::path::{Path, PathBuf};

/// Cuántos proyectos se recuerdan. Pasado ese punto la lista deja de ser «lo
/// que tengo entre manos» y pasa a ser un archivo histórico que hay que leerse.
const MAX: usize = 12;

pub struct Projects {
    /// Rutas, de la más reciente a la más antigua.
    dirs: Vec<String>,
    /// Dónde se guardan. `None` si no hubo forma de averiguarlo, en cuyo caso
    /// esto funciona en memoria y se olvida al cerrar.
    file: Option<PathBuf>,
}

impl Projects {
    /// Lee la lista del disco. Las rutas que ya no existen se caen aquí: un
    /// proyecto que borraste o un disco que no está montado no tiene por qué
    /// seguir ocupando sitio en la lista.
    pub fn load() -> Self {
        let file = config_file();
        let dirs = file
            .as_ref()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .map(|text| {
                text.lines()
                    .map(str::trim)
                    .filter(|l| !l.is_empty())
                    .filter(|l| Path::new(l).is_dir())
                    .map(str::to_owned)
                    .take(MAX)
                    .collect()
            })
            .unwrap_or_default();
        Self { dirs, file }
    }

    pub fn dirs(&self) -> &[String] {
        &self.dirs
    }

    /// Sube `dir` a la cabeza de la lista y la guarda.
    ///
    /// Se llama al abrir una sesión, que es el momento en que un directorio
    /// demuestra ser un proyecto. Ordenar por lo último usado y no
    /// alfabéticamente es lo que hace útil una lista de recientes: lo que
    /// abriste hace un minuto es lo que vas a volver a abrir.
    pub fn touch(&mut self, dir: &str) {
        let dir = dir.trim();
        if dir.is_empty() {
            return;
        }
        self.dirs.retain(|d| d != dir);
        self.dirs.insert(0, dir.to_owned());
        self.dirs.truncate(MAX);
        self.save();
    }

    fn save(&self) {
        let Some(file) = &self.file else { return };
        if let Some(parent) = file.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        // Si falla, se calla: ver el comentario del módulo.
        let _ = std::fs::write(file, self.dirs.join("\n"));
    }
}

/// El nombre corto de un proyecto: el último tramo de la ruta, que es como lo
/// llamarías de viva voz («el flow», «el web-api»).
pub fn name_of(dir: &str) -> String {
    let raw = dir.trim();
    let trimmed = raw.trim_end_matches(['/', '\\']);
    // La raíz se dice a sí misma. Quitarle las barras la deja en nada, y un
    // botón sin texto en el formulario no habría forma de pulsarlo a sabiendas.
    if trimmed.is_empty() {
        return raw.to_owned();
    }
    trimmed
        .rsplit(['/', '\\'])
        .find(|s| !s.is_empty())
        .unwrap_or(trimmed)
        .to_owned()
}

/// Dónde vive el fichero, siguiendo la costumbre de cada sistema en vez de
/// dejarlo junto al ejecutable: en Windows `%APPDATA%`, y fuera el
/// `XDG_CONFIG_HOME` de la especificación, con `~/.config` de respaldo.
fn config_file() -> Option<PathBuf> {
    let base = if cfg!(windows) {
        std::env::var_os("APPDATA").map(PathBuf::from)
    } else {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
    }?;
    Some(base.join("flow").join("projects"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn el_nombre_es_el_ultimo_tramo_de_la_ruta() {
        assert_eq!(name_of(r"C:\Repos\projects\flow"), "flow");
        assert_eq!(name_of("/home/alexi/web-api"), "web-api");
        // Una barra de más al final no debe dejar el nombre en blanco: es lo
        // que sale de copiar la ruta desde un explorador de archivos.
        assert_eq!(name_of("/home/alexi/notas/"), "notas");
        assert_eq!(name_of(r"D:\notas\\"), "notas");
    }

    #[test]
    fn una_ruta_sin_tramos_se_dice_a_si_misma() {
        // Sin esto, la raíz de una unidad se quedaría sin nombre y el botón del
        // formulario saldría vacío.
        assert_eq!(name_of("/"), "/");
        assert_eq!(name_of("flow"), "flow");
    }

    #[test]
    fn lo_ultimo_usado_manda_y_no_se_repite() {
        let mut p = Projects {
            dirs: vec!["/a".into(), "/b".into()],
            file: None, // Sin fichero: `save` no toca el disco del que ejecute los tests.
        };
        p.touch("/b");
        assert_eq!(p.dirs(), ["/b", "/a"], "lo recién usado va primero");
        p.touch("/c");
        assert_eq!(p.dirs(), ["/c", "/b", "/a"]);
        // Repetir no duplica.
        p.touch("/c");
        assert_eq!(p.dirs(), ["/c", "/b", "/a"]);
    }

    #[test]
    fn la_lista_no_crece_sin_fin() {
        let mut p = Projects {
            dirs: Vec::new(),
            file: None,
        };
        for i in 0..MAX * 2 {
            p.touch(&format!("/p{i}"));
        }
        assert_eq!(p.dirs().len(), MAX);
        // Y lo que se cae es lo más viejo, no lo más nuevo.
        assert_eq!(p.dirs()[0], format!("/p{}", MAX * 2 - 1));
    }
}
