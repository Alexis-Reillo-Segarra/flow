//! De teclas a bytes: lo que se escribe va al panel con el foco.
//!
//! Aquí vive la traducción entera —un evento de egui a los bytes que espera un
//! proceso al otro lado de un PTY— y vive sola, sin `Ui` ni `Context`, porque es
//! la parte con reglas de verdad y así se puede probar de una en una. Lo que la
//! usa (`app::Flow::type_into_pane`) no decide nada: coge los eventos del frame
//! y manda lo que salga.
//!
//! # Lo que no sale de aquí
//!
//! Un puñado de combinaciones son **de flow y no del proceso**: son las que
//! abren una sesión, cambian de panel o de tema. Están en [`reservada`] y esta
//! función las salta, así que hay un único sitio donde se decide quién se queda
//! cada tecla — si se decidiera en dos, un atajo nuevo llegaría al proceso
//! además de hacer lo suyo.
//!
//! El precio es real y conviene saberlo: `Ctrl-W` en un shell borra la palabra
//! anterior y aquí cierra el panel, y lo mismo pasa con `Ctrl-N` y `Ctrl-T`. Se
//! eligieron esas tres porque son las de "ventana" en cualquier programa con
//! pestañas, y porque lo que hacen —abrir y cerrar paneles— es lo que más se usa
//! en flow. El resto del teclado es del proceso.

use egui::{Event, Key, Modifiers};

/// Los modos del terminal que cambian lo que se manda al pulsar una tecla.
///
/// Los pone el proceso con secuencias de escape y los guarda `term::Term`, así
/// que esto es una copia de solo lectura del estado del panel que va a recibir.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Modes {
    /// DECCKM (`ESC[?1h`): las flechas van con `SS3` en vez de con `CSI`.
    ///
    /// Lo encienden casi todas las TUIs. Mandar siempre `CSI` funciona en
    /// muchas, pero no en las que leen la tecla por terminfo, que en ese modo
    /// espera `ESC O A` y se come el `[` como si fuera texto.
    pub app_cursor: bool,
    /// Pegado entre corchetes (`ESC[?2004h`): lo pegado va envuelto en marcas.
    ///
    /// Sin esto, pegar varias líneas en un shell **ejecuta** todas menos la
    /// última, que es de las pocas formas que tiene esta interfaz de hacer daño
    /// de verdad. Envuelto, el shell sabe que es texto pegado y lo deja en la
    /// línea de edición.
    pub bracketed_paste: bool,
}

/// ¿Se queda flow esta tecla en vez de mandarla al proceso?
///
/// Es la lista de `app::Flow::shortcuts` mirada desde el otro lado. Si añades un
/// atajo global, añádelo aquí: si no, hará lo suyo **y además** le llegará al
/// proceso.
pub fn reservada(key: Key, m: &Modifiers) -> bool {
    let digito = matches!(
        key,
        Key::Num1
            | Key::Num2
            | Key::Num3
            | Key::Num4
            | Key::Num5
            | Key::Num6
            | Key::Num7
            | Key::Num8
            | Key::Num9
    );
    let flecha = matches!(
        key,
        Key::ArrowLeft | Key::ArrowRight | Key::ArrowUp | Key::ArrowDown
    );

    if m.ctrl {
        // Ctrl-N, Ctrl-T y Ctrl-W —con o sin Shift— y Ctrl-1..9.
        return matches!(key, Key::N | Key::T | Key::W) || digito;
    }
    if m.alt {
        // Alt-1..8 salta de panel y Alt-flechas mueve el foco por la rejilla.
        //
        // Se reservan los nueve dígitos y no ocho: caben ocho paneles, así que
        // Alt-9 no tiene a dónde ir y `shortcuts` lo resuelve en nada. Se queda
        // igualmente porque la alternativa es peor —que Alt-9 sí le llegue al
        // proceso mientras Alt-8 no— y porque el día que `MAX_PANES` suba, el
        // atajo funciona sin tocar esta lista.
        return digito || flecha;
    }
    false
}

/// Traduce los eventos de un frame a los bytes que van al PTY.
///
/// Solo se le pasan los eventos cuando no hay ningún modal abierto: con el
/// formulario delante manda él, y sus campos de texto ya se quedan las teclas.
pub fn encode(events: &[Event], modes: Modes) -> Vec<u8> {
    let mut out = Vec::new();
    for ev in events {
        match ev {
            // El texto ya viene resuelto por el sistema: mayúsculas, acentos,
            // AltGr y teclas muertas incluidas. Traducirlo desde `Key` a mano
            // sería reimplementar la distribución de teclado de cada país.
            Event::Text(t) => out.extend_from_slice(t.as_bytes()),
            Event::Paste(t) => pegar(&mut out, t, modes),
            // `Ctrl-C` y `Ctrl-X` **no llegan como teclas**: egui los reconoce
            // como copiar y cortar y emite esto en su lugar, sin la tecla
            // (`egui_winit::is_copy_command`). Aquí eso no es una pérdida sino
            // el camino bueno, porque en una terminal significan otra cosa:
            // `Ctrl-C` interrumpe —es la tecla más importante que hay en esta
            // aplicación, tanto que tiene botón propio abajo— y `Ctrl-X` es de
            // quien esté corriendo, que en `nano` es salir.
            //
            // Y no se pierde ningún copiar: flow no tiene selección de texto que
            // copiar. El día que la tenga, será ella la que decida —copia si hay
            // algo seleccionado, interrumpe si no—, que es lo que hacen todos
            // los terminales.
            Event::Copy => out.push(0x03),
            Event::Cut => out.push(0x18),
            Event::Key {
                key,
                pressed: true,
                modifiers,
                ..
            } => tecla(&mut out, *key, modifiers, modes),
            _ => {}
        }
    }
    out
}

/// Lo pegado, envuelto si el proceso lo pidió.
fn pegar(out: &mut Vec<u8>, texto: &str, modes: Modes) {
    // Los saltos de línea de un portapapeles de Windows vienen en CRLF y el
    // terminal quiere CR: sin esto, cada línea pegada llega dos veces.
    let texto = texto.replace("\r\n", "\r").replace('\n', "\r");
    if modes.bracketed_paste {
        out.extend_from_slice(b"\x1b[200~");
        out.extend_from_slice(texto.as_bytes());
        out.extend_from_slice(b"\x1b[201~");
    } else {
        out.extend_from_slice(texto.as_bytes());
    }
}

fn tecla(out: &mut Vec<u8>, key: Key, m: &Modifiers, modes: Modes) {
    if reservada(key, m) {
        return;
    }

    // Ctrl + letra son los códigos de control de toda la vida: Ctrl-A es 0x01 y
    // así hasta Ctrl-Z.
    //
    // Tres no pasan por aquí en Windows y no es un olvido: `Ctrl-C`, `Ctrl-X` y
    // `Ctrl-V` los intercepta egui antes y los convierte en `Copy`, `Cut` y
    // `Paste`, que se tratan arriba. Este camino sigue siendo el que los coge en
    // un macOS, donde el atajo de copiar es `Cmd-C` y `Ctrl-C` llega entero.
    if m.ctrl && !m.alt {
        if let Some(n) = letra(key) {
            out.push(n + 1);
            return;
        }
        // Los tres controles que no son letras y se usan a diario.
        match key {
            Key::Space => out.push(0x00),
            Key::Backspace => out.push(0x08),
            Key::OpenBracket => out.push(0x1b),
            _ => {}
        }
        return;
    }

    // Alt + tecla es la convención de siempre: un ESC delante de lo que fuera.
    // Es como el shell recibe `Alt-B` o `Alt-.` para moverse por palabras.
    if m.alt {
        if let Some(n) = letra(key) {
            out.extend_from_slice(&[0x1b, b'a' + n]);
        }
        return;
    }

    let ss3 = modes.app_cursor;
    match key {
        Key::Enter => out.push(b'\r'),
        Key::Tab => out.push(b'\t'),
        Key::Escape => out.push(0x1b),
        // DEL y no BS: es lo que manda un terminal moderno, y es lo que esperan
        // `readline` y compañía. Con `\x08` el shell borra pero no repinta.
        Key::Backspace => out.push(0x7f),
        Key::ArrowUp => flecha(out, b'A', ss3),
        Key::ArrowDown => flecha(out, b'B', ss3),
        Key::ArrowRight => flecha(out, b'C', ss3),
        Key::ArrowLeft => flecha(out, b'D', ss3),
        Key::Home => flecha(out, b'H', ss3),
        Key::End => flecha(out, b'F', ss3),
        Key::Insert => out.extend_from_slice(b"\x1b[2~"),
        Key::Delete => out.extend_from_slice(b"\x1b[3~"),
        Key::PageUp => out.extend_from_slice(b"\x1b[5~"),
        Key::PageDown => out.extend_from_slice(b"\x1b[6~"),
        _ => {}
    }
}

/// Flechas, `Home` y `End`: `CSI` en modo normal, `SS3` en modo aplicación.
fn flecha(out: &mut Vec<u8>, letra: u8, ss3: bool) {
    out.extend_from_slice(if ss3 { b"\x1bO" } else { b"\x1b[" });
    out.push(letra);
}

/// `0` para `A`, `25` para `Z`; `None` si la tecla no es una letra.
fn letra(key: Key) -> Option<u8> {
    let n = key.name().as_bytes();
    match n {
        [c @ b'A'..=b'Z'] if n.len() == 1 => Some(c - b'A'),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pulsar(key: Key, modifiers: Modifiers) -> Event {
        Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        }
    }

    fn bytes(events: &[Event]) -> Vec<u8> {
        encode(events, Modes::default())
    }

    #[test]
    fn lo_que_escribes_llega_tal_cual() {
        // El texto viene ya resuelto por el sistema, acentos incluidos.
        let ev = [Event::Text("héllo".to_owned())];
        assert_eq!(bytes(&ev), "héllo".as_bytes());
    }

    #[test]
    fn enter_manda_retorno_y_borrar_manda_del() {
        assert_eq!(bytes(&[pulsar(Key::Enter, Modifiers::NONE)]), b"\r");
        // DEL (0x7f) y no BS (0x08): es lo que espera `readline`.
        assert_eq!(bytes(&[pulsar(Key::Backspace, Modifiers::NONE)]), b"\x7f");
    }

    #[test]
    fn ctrl_c_es_el_byte_de_interrumpir() {
        let ev = [pulsar(Key::C, Modifiers::CTRL)];
        assert_eq!(bytes(&ev), b"\x03");
        // Y Ctrl-D, que es el fin de fichero con el que se sale de un REPL.
        assert_eq!(bytes(&[pulsar(Key::D, Modifiers::CTRL)]), b"\x04");
    }

    #[test]
    fn copiar_y_cortar_son_interrumpir_y_su_control() {
        // El camino de verdad de `Ctrl-C` en Windows: egui lo reconoce como el
        // atajo de copiar y **no emite la tecla**, solo esto. Si no se tratara
        // aquí, la tecla más importante de la aplicación no llegaría al proceso
        // — y no se nota en ningún test que mire teclas, porque no hay ninguna.
        assert_eq!(bytes(&[Event::Copy]), b"\x03");
        assert_eq!(bytes(&[Event::Cut]), b"\x18");
    }

    #[test]
    fn pegar_no_arrastra_el_control_de_la_tecla() {
        // En un macOS `Ctrl-V` sí llega como tecla y vale 0x16; lo que no puede
        // pasar es que salgan los dos y lo pegado llegue precedido de un byte
        // que nadie escribió. En Windows solo llega el pegado.
        let ev = [Event::Paste("ls -la".to_owned())];
        assert_eq!(bytes(&ev), b"ls -la");
    }

    #[test]
    fn las_flechas_cambian_con_el_modo_del_proceso() {
        let ev = [pulsar(Key::ArrowUp, Modifiers::NONE)];
        assert_eq!(encode(&ev, Modes::default()), b"\x1b[A");
        let app = Modes {
            app_cursor: true,
            ..Default::default()
        };
        assert_eq!(encode(&ev, app), b"\x1bOA");
    }

    #[test]
    fn pegar_varias_lineas_va_envuelto_si_el_proceso_lo_pidio() {
        let ev = [Event::Paste("uno\r\ndos".to_owned())];
        // Sin el modo, el CRLF de Windows se normaliza a CR y va suelto: es lo
        // que hace cualquier terminal, y lo que ejecuta la primera línea.
        assert_eq!(encode(&ev, Modes::default()), b"uno\rdos");
        // Con el modo, envuelto, y el shell lo trata como texto pegado.
        let bp = Modes {
            bracketed_paste: true,
            ..Default::default()
        };
        assert_eq!(encode(&ev, bp), b"\x1b[200~uno\rdos\x1b[201~".to_vec());
    }

    #[test]
    fn alt_letra_va_con_su_escape_delante() {
        // Es como el shell recibe Alt-B para irse una palabra atrás.
        assert_eq!(bytes(&[pulsar(Key::B, Modifiers::ALT)]), b"\x1bb");
    }

    #[test]
    fn los_atajos_de_flow_no_le_llegan_al_proceso() {
        // Estos hacen lo suyo en la app; si además salieran por el PTY, abrir un
        // panel escribiría basura en el que estabas mirando.
        for (key, m) in [
            (Key::N, Modifiers::CTRL),
            (Key::T, Modifiers::CTRL),
            (Key::T, Modifiers::CTRL.plus(Modifiers::SHIFT)),
            (Key::W, Modifiers::CTRL),
            (Key::Num1, Modifiers::CTRL),
            (Key::Num3, Modifiers::ALT),
            (Key::ArrowLeft, Modifiers::ALT),
        ] {
            assert!(
                reservada(key, &m),
                "{key:?} con {m:?} tendría que ser de flow"
            );
            assert!(
                bytes(&[pulsar(key, m)]).is_empty(),
                "{key:?} con {m:?} se le ha colado al proceso"
            );
        }
    }

    #[test]
    fn lo_demas_del_teclado_es_del_proceso() {
        // El contrapunto del test de arriba: reservar de más deja al usuario sin
        // teclas dentro del proceso, y `Ctrl-A`, `Ctrl-E` o `Ctrl-R` se usan
        // constantemente en cualquier shell.
        for (key, m) in [
            (Key::A, Modifiers::CTRL),
            (Key::E, Modifiers::CTRL),
            (Key::R, Modifiers::CTRL),
            (Key::Z, Modifiers::CTRL),
            (Key::ArrowUp, Modifiers::NONE),
            (Key::Tab, Modifiers::NONE),
        ] {
            assert!(!reservada(key, &m), "{key:?} no es de flow");
            assert!(
                !bytes(&[pulsar(key, m)]).is_empty(),
                "{key:?} con {m:?} no le llega al proceso"
            );
        }
    }
}
