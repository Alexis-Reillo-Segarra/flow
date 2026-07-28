//! Una ventana de mentira para probar lo que dibuja.
//!
//! Media aplicación es código de interfaz, y hasta ahora ninguna línea de `ui/`
//! entraba en un test: el argumento tácito era que para dibujar hace falta una
//! ventana. No hace falta. `egui` es de modo inmediato y su contexto no depende
//! del sistema de ventanas: se le da un rectángulo y una lista de eventos, corre
//! un frame entero —reparto, respuestas, clics, foco— y devuelve lo que se
//! habría pintado. Lo único que no ocurre es que alguien lo suba a una GPU.
//!
//! Eso convierte «probar la interfaz» en algo tan barato como probar el resto, y
//! lo que se prueba con esto no es cosmético: que un clic en la pastilla de la
//! sesión tres devuelva `Switch(3)` y no `Switch(2)`, que el formulario no deje
//! lanzar sin directorio, que el velo de un modal se coma el clic que iba al
//! panel de debajo. Son las decisiones que la vista toma sola.
//!
//! El contexto lleva **las fuentes de verdad** —no las vacías de
//! `egui::__run_test_ui`— porque medir texto es la mitad de lo que hace esta
//! interfaz: el ancho de la columna de sesiones, el recorte de un nombre largo y
//! el número de filas de una terminal salen todos de una medida de galley.

use egui::{Context, Event, Key, Modifiers, PointerButton, Pos2, RawInput, Rect, Ui, Vec2};

use crate::agent::{Agent, State};
use crate::session::Session;

/// Una ventana de mentira: un contexto de `egui` con el tema de flow puesto.
pub struct Ventana {
    ctx: Context,
    rect: Rect,
    eventos: Vec<Event>,
    /// Las teclas muertas del frame que viene.
    ///
    /// Van aparte de los eventos porque `egui` las lee de los dos sitios y no
    /// significan lo mismo: el evento dice «se pulsó Ctrl-N» y esto dice «Ctrl
    /// está bajado ahora». Los atajos preguntan por lo segundo, así que una
    /// tecla con modificadores puesta solo en el evento no dispara ninguno.
    modificadores: Modifiers,
    reloj: f64,
}

impl Ventana {
    /// Del tamaño de casa, el que trae `main.rs` en su `ViewportBuilder`.
    pub fn nueva() -> Self {
        Self::de(1480.0, 900.0)
    }

    /// De un tamaño concreto: la rejilla y la columna de sesiones cambian de
    /// forma con la ventana, así que hay tests que necesitan una estrecha.
    pub fn de(ancho: f32, alto: f32) -> Self {
        let ctx = Context::default();
        crate::theme::install(&ctx);
        Self {
            ctx,
            rect: Rect::from_min_size(Pos2::ZERO, Vec2::new(ancho, alto)),
            eventos: Vec::new(),
            modificadores: Modifiers::NONE,
            reloj: 0.0,
        }
    }

    pub fn ctx(&self) -> &Context {
        &self.ctx
    }

    pub fn rect(&self) -> Rect {
        self.rect
    }

    /// Corre un frame y devuelve lo que la vista haya devuelto.
    ///
    /// Los eventos encolados se gastan aquí: valen para un frame, como los de
    /// verdad. El reloj avanza 16 ms, que es lo que tarda un frame a 60 Hz, para
    /// que lo que se anima con el tiempo —el latido de `WORKING`, el reparto de
    /// la rejilla— avance en vez de quedarse congelado en el instante cero.
    pub fn frame<T>(&mut self, mut vista: impl FnMut(&mut Ui) -> T) -> T {
        let entrada = RawInput {
            screen_rect: Some(self.rect),
            events: std::mem::take(&mut self.eventos),
            modifiers: std::mem::replace(&mut self.modificadores, Modifiers::NONE),
            time: Some(self.reloj),
            predicted_dt: 1.0 / 60.0,
            ..Default::default()
        };
        self.reloj += 1.0 / 60.0;

        let mut salida = None;
        let _ = self.ctx.run_ui(entrada, |ui| {
            salida = Some(vista(ui));
        });
        salida.expect("el frame no llegó a correr la vista")
    }

    /// Para las vistas que piden `&Context` en vez de `&mut Ui` —los modales—.
    /// Se corre igual dentro de un frame: fuera de uno no hay nada que medir.
    pub fn frame_ctx<T>(&mut self, mut vista: impl FnMut(&Context) -> T) -> T {
        self.frame(|ui| {
            let ctx = ui.ctx().clone();
            vista(&ctx)
        })
    }

    /// Corre un frame sin mirar lo que devuelve. Sirve para el frame de
    /// calentamiento: `egui` no sabe dónde cae un widget hasta que lo ha
    /// dibujado una vez, así que un clic a ciegas en el primer frame no acierta
    /// nada. Se dibuja, y al siguiente ya se puede pinchar.
    pub fn calienta(&mut self, vista: impl FnMut(&mut Ui)) {
        self.frame(vista);
    }

    /// Encola un clic. Se gasta en el siguiente `frame`.
    pub fn clic(&mut self, pos: Pos2) {
        self.eventos.push(Event::PointerMoved(pos));
        self.eventos.push(Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed: true,
            modifiers: Modifiers::NONE,
        });
        self.eventos.push(Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed: false,
            modifiers: Modifiers::NONE,
        });
    }

    /// Deja el puntero encima de un sitio, sin pulsar.
    pub fn puntero(&mut self, pos: Pos2) {
        self.eventos.push(Event::PointerMoved(pos));
    }

    /// Dos clics seguidos en el mismo sitio, que es otra cosa que uno: en la
    /// barra de título, maximizar.
    pub fn doble_clic(&mut self, pos: Pos2) {
        self.clic(pos);
        self.clic(pos);
    }

    /// Pulsa, mueve y suelta: es como se arrastra la ventana por su barra.
    pub fn arrastra(&mut self, desde: Pos2, hasta: Pos2) {
        self.eventos.push(Event::PointerMoved(desde));
        self.eventos.push(Event::PointerButton {
            pos: desde,
            button: PointerButton::Primary,
            pressed: true,
            modifiers: Modifiers::NONE,
        });
        self.eventos.push(Event::PointerMoved(hasta));
        self.eventos.push(Event::PointerButton {
            pos: hasta,
            button: PointerButton::Primary,
            pressed: false,
            modifiers: Modifiers::NONE,
        });
    }

    /// Mueve la rueda del ratón sobre un sitio. Positivo sube por el contenido,
    /// que es lo que despega la vista del final de una terminal.
    pub fn rueda(&mut self, pos: Pos2, delta: f32) {
        self.eventos.push(Event::PointerMoved(pos));
        self.eventos.push(Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: Vec2::new(0.0, delta),
            // `Move` es lo que manda un ratón de rueda; las otras fases son de
            // un panel táctil, que aquí no cambian nada.
            phase: egui::TouchPhase::Move,
            modifiers: Modifiers::NONE,
        });
    }

    /// Encola una tecla pulsada, con sus modificadores bajados.
    pub fn tecla(&mut self, key: Key, modifiers: Modifiers) {
        self.modificadores = modifiers;
        self.eventos.push(Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        });
    }

    /// Encola texto escrito, que es lo que manda el sistema al teclear.
    pub fn escribe(&mut self, texto: &str) {
        self.eventos.push(Event::Text(texto.to_owned()));
    }
}

/// Un proceso de mentira que no termina hasta que lo maten.
///
/// Es un proceso **de verdad** —el `Agent` habla con un PTY real y no hay forma
/// honesta de fingirlo— pero elegido para que no haga nada: lee de su entrada y
/// se calla. Así el test decide cuándo muere.
pub fn quieto() -> &'static str {
    if cfg!(windows) {
        // `pause` sin consola de por medio se queda esperando una tecla.
        "cmd /C pause"
    } else {
        "cat"
    }
}

/// Un proceso que escribe algo y termina bien.
pub fn saluda() -> &'static str {
    if cfg!(windows) {
        "cmd /C echo hola"
    } else {
        "echo hola"
    }
}

/// Un agente lanzado de verdad, con el directorio actual y sin entorno.
pub fn agente(id: u64, nombre: &str, cmd: &str) -> Agent {
    Agent::spawn(
        id,
        nombre.to_owned(),
        cmd.to_owned(),
        ".".to_owned(),
        80,
        24,
        &[],
    )
}

/// Un agente al que ya se le ha dejado terminar, con el estado que se pida.
///
/// Espera a que el proceso muera de verdad antes de forzar el estado: si no, el
/// hilo de espera manda su `Exit` un frame después y pisa lo que el test quería
/// probar.
pub fn agente_terminado(id: u64, nombre: &str, estado: State) -> Agent {
    let mut a = agente(id, nombre, saluda());
    espera_a_que_termine(&mut a);
    a.state = estado;
    a
}

/// Bombea hasta que el proceso termine, o hasta rendirse.
///
/// El tope no es una carrera contra el reloj de nadie: es que un test que se
/// cuelga es peor que un test que falla.
pub fn espera_a_que_termine(a: &mut Agent) {
    for _ in 0..600 {
        a.pump();
        if !a.state.is_running() {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    panic!("el proceso no terminó en 6 s: {}", a.cmdline);
}

/// Bombea un rato sin exigir que termine, para que llegue lo que haya escrito.
pub fn deja_hablar(a: &mut Agent) {
    for _ in 0..60 {
        a.pump();
        if !a.term().last_nonempty_line().trim().is_empty() {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

/// Una sesión con los paneles que se le pasen.
pub fn sesion(id: u64, nombre: &str, paneles: Vec<Agent>) -> Session {
    let mut paneles = paneles.into_iter();
    let primero = paneles.next().expect("una sesión nace con un panel");
    let foco = primero.id;
    let mut s = Session::new(
        id,
        nombre.to_owned(),
        ".".to_owned(),
        std::env::temp_dir()
            .join("flow-test-inbox")
            .join(id.to_string()),
        primero,
    );
    for p in paneles {
        s.panes.push(p);
    }
    s.focused = Some(foco);
    s
}
