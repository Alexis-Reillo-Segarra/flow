//! El fichero de configuración: qué tema está puesto y los temas propios.
//!
//! Es un fichero de texto y no JSON ni TOML, por lo mismo que la lista de
//! proyectos (ver `crate::projects`): no hay nada que estructurar más allá de
//! `clave = valor`, se abre con cualquier editor, y un fichero medio escrito se
//! lee igual de bien —la línea que no se entienda se descarta sola—. Añadir un
//! formato serio habría metido una dependencia entera para leer veinte colores.
//!
//! ```text
//! # Un comentario ocupa su línea entera; para una nota al final de una que ya
//! # dice algo, `;` — porque `#` ya está cogido: abre un color.
//! theme = catppuccin
//!
//! [theme mío]
//! base   = flow          ; de dónde hereda lo que no digas
//! bg     = #101014
//! accent = #ff9e64
//! ```
//!
//! **Lo que no digas se hereda**, y esa es la decisión de diseño del formato:
//! un tema son veintitantos colores, y obligar a escribirlos todos para cambiar
//! el acento habría hecho que nadie escribiera ninguno. Con `base` puesto a un
//! tema que ya cumple el contraste, cambiar dos colores no puede dejar la
//! interfaz ilegible por accidente.
//!
//! **Nada de esto es crítico.** Si el fichero no existe, no se puede leer o está
//! lleno de erratas, flow arranca con el tema de casa y sigue funcionando: los
//! avisos se escriben por la salida de errores —donde los ve quien lo lanzó
//! desde una terminal— y ninguno para la aplicación. Perder un color no vale una
//! app que no abre.
//!
//! **Los temas propios no pasan por los tests de contraste**, y no puede ser de
//! otra forma: son de quien los escribe y se leen al arrancar, no al compilar.
//! Los cinco incluidos sí (ver `crate::theme`), así que heredar de uno de ellos
//! es lo que hace que un tema propio empiece cumpliendo.

use std::path::PathBuf;

use egui::Color32;

use crate::theme::Palette;

/// Lo que sale de leer el fichero.
pub struct Config {
    /// El tema que se pidió, si se pidió alguno.
    pub theme: Option<String>,
    /// Los temas escritos a mano, ya resueltos sobre su `base`.
    pub customs: Vec<Palette>,
    /// Lo que no se entendió, con su número de línea. Ni un aviso interrumpe la
    /// carga: se apuntan todos y se cuentan al final.
    pub warnings: Vec<String>,
}

/// Lee la configuración. Un fichero que no existe es la respuesta normal la
/// primera vez que alguien abre flow, no un error.
///
/// `builtin` son los temas incluidos, de los que heredan los propios. Se pasan
/// en vez de pedírselos a `theme` para no forzar ahí la lista antes de tiempo:
/// esta función corre justo antes de instalarla, y la lista definitiva es
/// precisamente los incluidos **más** lo que salga de aquí.
pub fn load(builtin: &[Palette]) -> Config {
    let text = file()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .unwrap_or_default();
    parse(&text, builtin)
}

/// Deja escrito qué tema está puesto, **conservando el resto del fichero**.
///
/// Solo se toca la línea de `theme`; los temas propios, los comentarios y hasta
/// el orden se quedan como estaban. Reescribir el fichero entero desde la
/// estructura leída habría sido más corto y le habría borrado a alguien sus
/// comentarios la primera vez que cambiara de tema desde la app.
///
/// Si el fichero no existe todavía, se escribe con la plantilla comentada de
/// abajo: es el momento en que el usuario ha demostrado interés por los temas,
/// que es justo cuando sirve encontrarse el formato explicado.
pub fn save_theme(name: &str) {
    let Some(path) = file() else { return };
    let previo = std::fs::read_to_string(&path).ok();
    let text = match previo {
        Some(text) if text.lines().any(|l| key_of(l) == Some("theme")) => text
            .lines()
            .map(|l| {
                if key_of(l) == Some("theme") {
                    format!("theme = {name}")
                } else {
                    l.to_owned()
                }
            })
            .collect::<Vec<_>>()
            .join("\n"),
        Some(text) => format!("theme = {name}\n\n{text}"),
        None => TEMPLATE.replace("{theme}", name),
    };

    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    // Si falla, se calla: ver el comentario del módulo.
    let _ = std::fs::write(path, text);
}

/// Dónde vive el fichero: al lado de la lista de proyectos, siguiendo la
/// costumbre de cada sistema en vez de dejarlo junto al ejecutable.
pub fn file() -> Option<PathBuf> {
    let base = if cfg!(windows) {
        std::env::var_os("APPDATA").map(PathBuf::from)
    } else {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
    }?;
    Some(base.join("flow").join("config"))
}

/// La clave de una línea `clave = valor`, si lo es.
fn key_of(line: &str) -> Option<&str> {
    let line = line.trim();
    if line.starts_with('#') || line.starts_with('[') {
        return None;
    }
    Some(line.split_once('=')?.0.trim())
}

/// `#rgb` o `#rrggbb`. La almohadilla es opcional porque medio mundo copia los
/// colores sin ella.
fn color(value: &str) -> Option<Color32> {
    let v = value.trim().trim_start_matches('#');
    let n = |s: &str| u8::from_str_radix(s, 16).ok();
    match v.len() {
        3 => {
            let d: Vec<u8> = v.chars().filter_map(|c| n(&c.to_string())).collect();
            (d.len() == 3).then(|| Color32::from_rgb(d[0] * 17, d[1] * 17, d[2] * 17))
        }
        6 => {
            let d: Vec<u8> = (0..3).filter_map(|i| n(&v[i * 2..i * 2 + 2])).collect();
            (d.len() == 3).then(|| Color32::from_rgb(d[0], d[1], d[2]))
        }
        _ => None,
    }
}

/// El corazón del módulo, aparte del disco para poder probarlo.
fn parse(text: &str, builtin: &[Palette]) -> Config {
    let mut cfg = Config {
        theme: None,
        customs: Vec::new(),
        warnings: Vec::new(),
    };
    // El tema que se está escribiendo ahora mismo, si estamos dentro de una
    // sección.
    let mut current: Option<Palette> = None;

    for (n, raw) in text.lines().enumerate() {
        // Una línea que empieza por `#` es un comentario, y de `;` en adelante
        // también. Son dos marcas y no una porque `#` ya está cogido: abre un
        // color, y `bg = #101014  # el fondo` tendría que decidir cuál de las
        // dos almohadillas manda. Con `;` no hay nada que decidir.
        let line = raw.split(';').next().unwrap_or("").trim();
        let n = n + 1;
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        // Cabecera de sección: cierra el tema anterior y abre uno nuevo.
        if let Some(head) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            if let Some(done) = current.take() {
                cfg.customs.push(done);
            }
            let name = head
                .trim()
                .strip_prefix("theme")
                .filter(|rest| rest.starts_with(char::is_whitespace))
                .map(str::trim);
            match name {
                Some(name) if !name.is_empty() => {
                    // Nace como copia del tema de casa. `base` lo cambia, y por
                    // eso `base` tiene que ir antes que los colores: lo dice la
                    // plantilla y lo avisa el parser si llega tarde.
                    let mut p = builtin[0].clone();
                    p.name = name.to_owned();
                    p.about = "tuyo, del fichero de configuración".to_owned();
                    current = Some(p);
                }
                _ => cfg
                    .warnings
                    .push(format!("línea {n}: '[{head}]' no es '[theme <nombre>]'")),
            }
            continue;
        }

        let Some((key, value)) = line.split_once('=') else {
            cfg.warnings
                .push(format!("línea {n}: '{line}' no es 'clave = valor'"));
            continue;
        };
        let (key, value) = (key.trim(), value.trim());

        let Some(theme) = current.as_mut() else {
            // Fuera de toda sección solo se admiten los ajustes generales, que
            // hoy son uno.
            match key {
                "theme" => cfg.theme = Some(value.to_owned()),
                _ => cfg
                    .warnings
                    .push(format!("línea {n}: '{key}' no es un ajuste de flow")),
            }
            continue;
        };

        if key == "base" {
            match builtin.iter().find(|p| p.name == value) {
                Some(b) => {
                    let name = std::mem::take(&mut theme.name);
                    let about = std::mem::take(&mut theme.about);
                    *theme = b.clone();
                    theme.name = name;
                    theme.about = about;
                }
                None => cfg.warnings.push(format!(
                    "línea {n}: no hay ningún tema incluido que se llame '{value}'"
                )),
            }
            continue;
        }

        match color(value) {
            Some(c) => {
                if !theme.set(key, c) {
                    cfg.warnings
                        .push(format!("línea {n}: '{key}' no es un color de la paleta"));
                }
            }
            None => cfg
                .warnings
                .push(format!("línea {n}: '{value}' no es un color #rrggbb")),
        }
    }

    if let Some(done) = current.take() {
        cfg.customs.push(done);
    }
    cfg
}

/// Lo que se escribe la primera vez, para que el fichero se explique solo.
const TEMPLATE: &str = r#"# Configuración de flow.
#
# El tema activo. Los incluidos son: flow, catppuccin, gruvbox, tokyonight y
# nord. También vale el nombre de cualquier tema que definas aquí abajo.
# Ctrl-Shift-T lo cambia desde la app y reescribe esta línea.
theme = {theme}

# ── Temas propios ────────────────────────────────────────────────────────────
#
# Quita el `#` de las líneas de abajo para tener un tema tuyo. Lo que no
# escribas se hereda de `base`, así que basta con poner lo que quieras cambiar.
# `base` tiene que ir antes que los colores.
#
# Los colores van en #rrggbb (o #rgb). El acento se declara una vez, por su cara
# clara —la que se usa cuando es texto—, y flow saca de ahí la oscura del marco
# conservando el tono.
#
# [theme mío]
# base   = flow
#
# bg     = #101014      ; el fondo de todo
# raised = #17171c      ; campos y cajas
# sel    = #202028      ; fila seleccionada
# hover  = #191920
# line   = #3c424b      ; las divisorias de 1 px
# line_hi = #5a626d     ; borde exterior y campo con foco
#
# text       = #c6cbd2  ; el nivel normal
# text_hi    = #ffffff  ; lo que destaca
# text_dim   = #97a0ab
# text_faint = #7b838e  ; el más flojo: sigue teniendo que leerse
#
# accent = #30cf97      ; la marca: foco, cursor, logo
# accent_stroke = #1e825f   ; opcional: la cara oscura, si no te vale la que sale sola
#
# green = #6ee787       ; WORKING y DONE
# amber = #f0b45c       ; BLOCKED: el único estado que reclama atención
# red   = #f2696e       ; EXIT != 0 y FAILED
# slate = #7c8695       ; IDLE
#
# Los 16 colores del terminal, ansi0 … ansi15. Los que no toques se heredan.
# ansi0  = #14161a
# ansi1  = #f2696e
# ansi6  = #35d6f5
# ansi15 = #ffffff
#
# Un aviso que flow no puede comprobarte: los temas incluidos pasan tests de
# contraste (WCAG AA sobre su propio fondo, también con el panel atenuado) y el
# tuyo no, porque se lee al arrancar. Si heredas de uno de ellos y cambias poco,
# empiezas cumpliendo.
"#;

#[cfg(test)]
mod tests {
    use super::*;

    fn temas() -> &'static [Palette] {
        crate::theme::themes()
    }

    #[test]
    fn un_fichero_vacio_no_dice_nada_ni_se_queja() {
        let cfg = parse("", temas());
        assert!(cfg.theme.is_none());
        assert!(cfg.customs.is_empty());
        assert!(cfg.warnings.is_empty());
    }

    #[test]
    fn lee_el_tema_activo_con_comentarios_de_por_medio() {
        let cfg = parse(
            "# mi config\n\n  theme = gruvbox   ; el de siempre\n",
            temas(),
        );
        assert_eq!(cfg.theme.as_deref(), Some("gruvbox"));
        assert!(cfg.warnings.is_empty(), "{:?}", cfg.warnings);
    }

    #[test]
    fn un_tema_propio_hereda_lo_que_no_dice() {
        let cfg = parse(
            "[theme mío]\nbase = catppuccin\naccent = #ff9e64\n",
            temas(),
        );
        assert!(cfg.warnings.is_empty(), "{:?}", cfg.warnings);
        let p = &cfg.customs[0];
        assert_eq!(p.name, "mío");
        // Lo que dijo, dicho.
        assert_eq!(p.accent_text, Color32::from_rgb(0xff, 0x9e, 0x64));
        // Y lo que no, heredado de su base y no del tema de casa.
        let base = temas().iter().find(|t| t.name == "catppuccin").unwrap();
        assert_eq!(p.bg, base.bg);
        assert_eq!(p.text, base.text);
    }

    #[test]
    fn sin_base_se_hereda_del_tema_de_casa() {
        let cfg = parse("[theme oled]\nbg = #000000\n", temas());
        assert_eq!(cfg.customs[0].text, temas()[0].text);
    }

    #[test]
    fn dos_temas_seguidos_no_se_mezclan() {
        let cfg = parse(
            "[theme uno]\nbg = #010101\n\n[theme dos]\nbg = #020202\n",
            temas(),
        );
        assert_eq!(cfg.customs.len(), 2);
        assert_eq!(cfg.customs[0].bg, Color32::from_rgb(1, 1, 1));
        assert_eq!(cfg.customs[1].bg, Color32::from_rgb(2, 2, 2));
    }

    #[test]
    fn lo_que_no_se_entiende_se_dice_y_no_para_lo_demas() {
        // Cada línea rota se apunta, pero la de después se sigue leyendo: un
        // fichero con una errata no puede dejar a nadie sin el resto de su tema.
        let cfg = parse(
            "theme = flow\nesto no es nada\ncolorines = azul\n\n[theme x]\nbg = naranja\nfulano = #fff\nbase = inventado\ntext = #ffffff\n",
            temas(),
        );
        assert_eq!(cfg.theme.as_deref(), Some("flow"));
        assert_eq!(cfg.warnings.len(), 5, "{:?}", cfg.warnings);
        assert!(cfg.warnings[0].contains("línea 2"));
        assert_eq!(cfg.customs[0].text, Color32::WHITE);
    }

    #[test]
    fn el_color_se_escribe_como_uno_quiera() {
        assert_eq!(color("#ff9e64"), Some(Color32::from_rgb(0xff, 0x9e, 0x64)));
        assert_eq!(color("ff9e64"), Some(Color32::from_rgb(0xff, 0x9e, 0x64)));
        assert_eq!(
            color("  #FF9E64 "),
            Some(Color32::from_rgb(0xff, 0x9e, 0x64))
        );
        // La forma corta se expande como en CSS: #f0a → #ff00aa.
        assert_eq!(color("#f0a"), Some(Color32::from_rgb(0xff, 0x00, 0xaa)));
        assert_eq!(color("#12345"), None);
        assert_eq!(color("azul"), None);
        assert_eq!(color(""), None);
    }

    #[test]
    fn el_nombre_de_un_tema_propio_puede_llevar_espacios() {
        // El de un tema incluido no, porque se escribe en `theme = `, pero la
        // cabecera es una línea entera y aquí no hay ambigüedad.
        let cfg = parse("[theme mi tema]\nbg = #000000\n", temas());
        assert_eq!(cfg.customs[0].name, "mi tema");
    }

    #[test]
    fn una_seccion_que_no_es_un_tema_se_avisa() {
        let cfg = parse("[cosas]\nbg = #000000\n", temas());
        assert_eq!(cfg.warnings.len(), 2, "{:?}", cfg.warnings);
        assert!(cfg.customs.is_empty());
    }
}
