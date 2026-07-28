pub mod bar;
pub mod chrome;
pub mod grain;
pub mod output;
pub mod prompt;
pub mod spawn;
pub mod themes;
pub mod tiles;
pub mod widgets;

/// Hacia dónde mover el foco dentro de la rejilla.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dir {
    Left,
    Right,
    Up,
    Down,
}

/// Intención producida por la UI y aplicada después por `Flow::apply`.
///
/// En modo inmediato la vista tiene prestado `&mut Flow` mientras dibuja, así
/// que no puede añadir ni quitar nada sobre la marcha. Devolver una acción y
/// resolverla al final del frame evita pelearse con el borrow checker y deja un
/// único sitio donde ocurren las mutaciones de verdad.
#[derive(Clone, Debug)]
pub enum Action {
    /// Cambia de sesión.
    Switch(u64),
    /// Cierra una sesión entera, con todos sus paneles.
    CloseSession(u64),
    /// Da el foco a un panel de la sesión actual.
    Focus(u64),
    /// Mueve el foco al panel vecino en esa dirección.
    FocusDir(Dir),
    /// Da el foco al panel n-ésimo (contando desde 0) de la sesión actual.
    FocusIndex(usize),
    Kill(u64),
    /// Cierra un panel. Si era el último, se lleva la sesión por delante.
    Close(u64),
    Restart(u64),
    /// Bytes para el panel con el foco, tal cual.
    ///
    /// Lo que se teclea **no** pasa por aquí: va directo del teclado al PTY en
    /// `Flow::type_into_pane`, sin dar la vuelta por una acción, porque no hay
    /// nada que decidir al final del frame y el eco tiene que ser inmediato.
    /// Esto es para los botones de la tira de abajo, `^C` y `ESC`.
    SendRaw(Vec<u8>),
    OpenSpawn(spawn::Kind),
    CancelSpawn,
    ConfirmSpawn,
    /// Abre el selector de temas.
    OpenThemes,
    /// Prueba el tema n-ésimo. Se aplica de verdad y a toda la app: elegir tema
    /// es mirarlo puesto.
    PickTheme(usize),
    /// Se queda con el que esté probando y lo deja escrito en el fichero.
    ConfirmThemes,
    /// Vuelve al que había antes de abrir el selector.
    CancelThemes,
}
