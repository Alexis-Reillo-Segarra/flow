# AGENTS.md — orientación rápida del repositorio

Este fichero existe para que un agente (o una persona con prisa) entienda de qué
va **flow** y cómo está montado sin tener que leerse el código. Es un mapa, no un
tutorial: para usar la aplicación está el [README](README.md).

---

## En una frase

**flow es un orquestador de agentes de CLI**: lanza varios procesos a la vez,
cada uno en un pseudo-terminal real, y los enseña todos en mosaico en la misma
pantalla, diciendo de un vistazo cuál trabaja, cuál terminó y cuál está
bloqueado esperando una respuesta.

- **Qué es técnicamente:** una aplicación de escritorio nativa, un solo binario.
- **Lenguaje:** Rust 2021. `flow v0.1.0`.
- **GUI:** [`egui`](https://github.com/emilk/egui) / `eframe` (modo inmediato).
- **PTY:** `portable-pty`. Emulador de terminal **propio**, no una biblioteca.
- **Plataforma:** multiplataforma en el código; **Windows es donde se desarrolla,
  se prueba y se publica**. Linux y macOS compilan desde fuente.
- **Sin red.** flow no abre puertos, no habla con ningún servicio y no envía
  telemetría. Todo lo que escribe en disco son dos ficheros de texto.
- **Idioma del proyecto:** el código, los comentarios, los nombres de los tests
  y la documentación están **en español**. Si contribuyes, sigue en español.

## Qué **no** es

Aclarar esto ahorra la mayoría de los malentendidos:

- **No es un multiplexor de terminal.** No hay persistencia ni *reattach*: al
  cerrar flow mueren todos los procesos. tmux resuelve otro problema.
- **No es un cliente de ningún modelo ni de ninguna API.** flow no habla con
  Anthropic, OpenAI ni nadie. Lanza procesos. Que uno de esos procesos sea
  `claude` o `codex` le da exactamente igual.
- **No integra ningún agente en concreto.** Cualquier CLI vale. El «catálogo» de
  agentes es una lista de nombres que se buscan en el `PATH` para rellenar un
  formulario, nada más.
- **No es una app web ni de Electron.** No hay JavaScript en el producto.

## Vocabulario

| Término     | Qué es en el código                                                        |
| ----------- | -------------------------------------------------------------------------- |
| **Sesión**  | Un directorio y hasta 8 paneles que lo comparten. `src/session.rs`          |
| **Panel**   | Un proceso corriendo en su PTY, con su emulador y su estado. `src/agent.rs` |
| **Estado**  | `WORKING`, `BLOCKED`, `IDLE`, `DONE`, `EXIT`, `FAILED`. Deducido, no consultado |
| **Buzón**   | Un directorio del temporal; un fichero dentro = una petición de panel       |
| **Preset**  | Una entrada del catálogo de agentes/herramientas. `src/presets.rs`          |
| **Paleta**  | Los colores de un tema. `theme::Palette`                                    |

## Mapa del repositorio

```
build.rs       el icono y la ficha de propiedades del .exe de Windows
src/
  main.rs      arranque de eframe y opciones de ventana
  run.rs       `flow run`: pedir un panel desde dentro de una sesión
  app.rs       estado global, bucle de frame, atajos, escala automática y buzones
  session.rs   una sesión: sus paneles, su directorio y el entorno que ven
  agent.rs     un panel = un proceso en un PTY, con dos hilos y heurística de estado
  term.rs      emulador de terminal (rejilla, scrollback, ANSI, alt-screen)
  theme.rs     los temas: paleta, contrato de contraste, fuentes, espacio y estilo
  config.rs    el fichero de configuración: tema activo y temas propios
  projects.rs  los directorios que ya has usado
  repos.rs     búsqueda de repositorios en el equipo
  presets.rs   catálogo de agentes y detección en el PATH
  keys.rs      de teclas a bytes, y qué teclas se queda flow
  logo.rs      la marca: cuatro barras inclinadas, rasterizadas en código
  testkit.rs   solo en tests: una ventana de mentira para probar lo que dibuja
  ui/
    mod.rs       el enum `Action` y el pegamento entre vistas
    chrome.rs    barra superior, botones de ventana y bordes de resize
    bar.rs       la columna de sesiones de la izquierda
    grain.rs     el grano del fondo
    tiles.rs     el mosaico: reparto del espacio y marco de cada panel
    output.rs    la terminal de un panel
    prompt.rs    la tira de abajo: destinatario y botones ^C/ESC/KILL
    spawn.rs     el formulario de lanzamiento
    themes.rs    el selector de temas
    widgets.rs   primitivas de dibujo
docs/          documentación larga (ver abajo)
examples/      pty_probe: sonda de la capa PTY
tests/cli.rs   la línea de comandos, ejecutando el binario de verdad
assets/brand/  la marca: SVG fuente, PNG e ICO derivados, y el recurso del .exe
assets/fonts/  Inter y JetBrains Mono, embebidas en el binario
.claude/skills/flow-ui/SKILL.md   guía de la interfaz para quien la toque
```

## Invariantes: lo que romperás sin darte cuenta

Cada una de estas está defendida por un test, por un comentario largo, o por las
dos cosas. **Léelas antes de tocar el módulo correspondiente.**

1. **La UI no muta: devuelve intención.** En modo inmediato la vista tiene
   prestado `&mut Flow` mientras dibuja. El patrón es que cada función de UI
   devuelve un `ui::Action` y `Flow::apply` lo resuelve al final del frame. No
   uses `RefCell` ni canales para saltártelo.
2. **De `ui/` no sale ningún `Color32` literal.** Todo el color viene de
   `theme::pal()`. Un color escrito a pelo no cambia al cambiar de tema. Las
   excepciones son tres y están declaradas: el blanco de subir una textura sin
   teñirla (`ui/grain.rs`, y la marca en `ui/chrome.rs`), el negro del velo de un
   modal (`ui/widgets.rs`) y el blanco del aspa de cerrar cuando su botón está
   en rojo (`ui/chrome.rs`), que no sale de la paleta porque no se dibuja sobre
   el fondo del tema sino sobre `pal().red`.
3. **El terminal guarda el slot ANSI, no el color.** `term::Ink` distingue
   `Ansi(u8)` de `Rgb` y se resuelve al dibujar. Guardar un color ya resuelto
   deja el scrollback pintado con el tema que hubiera cuando llegó el texto.
4. **Los temas pasan un contrato de contraste**, y los tests de `theme.rs` lo
   recorren **para todos los temas**, no solo el activo: 4,5:1 para texto —y
   también con el panel atenuado—, dos caras del acento del mismo tono, estados
   separables y ningún slot ANSI igual al acento. Si añades un color de texto a
   `Palette`, añádelo a la lista `textos()` de los tests.
5. **El tema de casa es monocromo, estados incluidos.** Lo único con tono en
   pantalla son los 16 colores ANSI, es decir, la salida ajena. Hay un test.
6. **Nada de emoji ni fuentes de iconos.** Los símbolos se dibujan con
   rectángulos y segmentos, para que queden nítidos a 6 px y no dependan de que
   la fuente traiga el glifo.
7. **Lo dibujado a mano se le declara a AccessKit** con `response.widget_info(…)`,
   o no existe para un lector de pantalla.
8. **El estado nunca se dice solo con color.** Siempre va con su palabra, su
   forma y su ritmo. Es lo que permite que el tema de casa sea gris.
9. **Ocho paneles por sesión** (`session::MAX_PANES`). Lo que se pide al buzón
   con la sesión llena se descarta en silencio, y es a propósito.
10. **El teclado es del proceso salvo lista corta.** Lo que se teclea va al panel
   con el foco (`app::Flow::type_into_pane`), y las combinaciones que se queda la
   aplicación están **dos veces**: en `app::Flow::shortcuts`, que las ejecuta, y
   en `keys::reservada`, que impide que además le lleguen al proceso. Si añades
   un atajo y te olvidas de la segunda, el atajo hará lo suyo **y** escribirá
   basura en la terminal. Hay un test que recorre la lista.

## Cómo se prueba

**La interfaz se prueba, y no hace falta una ventana para hacerlo.** `egui` es de
modo inmediato y su contexto no depende del sistema de ventanas: se le da un
rectángulo y una lista de eventos, corre un frame entero —reparto, respuestas,
clics, foco— y devuelve lo que se habría pintado. Eso es `src/testkit.rs`, y es
lo que permite probar que un clic en la pastilla de la sesión tres devuelve
`Switch(3)` y no `Switch(2)`, o que el velo de un modal se come el clic que iba
al panel de debajo.

Tres cosas que conviene saber antes de escribir uno:

1. **Un clic necesita un frame de calentamiento.** `egui` resuelve la
   interacción contra los rectángulos del frame **anterior**, así que hay que
   dibujar una vez antes de pinchar. Y si lo que se prueba es un modal, el modal
   tiene que estar ya dibujado en ese primer frame.
2. **Los modificadores van en dos sitios.** El evento dice «se pulsó Ctrl-N» y
   `RawInput::modifiers` dice «Ctrl está bajado». Los atajos preguntan por lo
   segundo; `Ventana::tecla` pone los dos.
3. **Ningún test escribe en los ficheros del usuario.** El de configuración se
   redirige por hilo con `config::redirigir_para_test`, y la lista de proyectos
   tiene `Projects::en_memoria`. Si añades algo que escriba en disco, dale su
   costura antes de probarlo.

Lo que **no** está cubierto, y por qué: `main.rs` abre una ventana de verdad, y
las ramas de error del PTY en `agent.rs` piden que falle el sistema operativo.
Todo lo demás está por encima del 93% de líneas.

## Cómo se construye y se comprueba

```
cargo build                    # depuración
cargo run --release            # ejecutarlo
cargo test                     # 245 tests, 95% de las líneas. Ver «Cómo se prueba»
cargo clippy --all-targets     # CI lo pasa con -D warnings
cargo install --path .         # instalar en ~/.cargo/bin
```

Para verlo con la pantalla llena sin lanzar nada a mano, hay tres variables de
entorno de desarrollo, leídas en `app.rs`:

| Variable      | Efecto                                                     |
| ------------- | ---------------------------------------------------------- |
| `FLOW_DEMO=8` | Abre una sesión con 8 paneles de shell                     |
| `FLOW_FORM=session` \| `pane` | Arranca con el formulario abierto           |
| `FLOW_PICKER=1` | Arranca con el selector de temas abierto                 |

CI: `.github/workflows/ci.yml` corre tests y clippy en Windows en cada push y
cada PR. `release.yml` publica el `.exe` al empujar una etiqueta `vX.Y.Z`.

## Interoperar con flow desde dentro

Si un proceso ve la variable `FLOW` puesta, está corriendo **dentro** de una
sesión de flow. El contrato completo está en
[docs/dentro-de-flow.md](docs/dentro-de-flow.md), pero lo esencial:

- `FLOW_HOWTO` lleva las instrucciones en prosa, pensadas para que las lea un
  modelo. Es el primer sitio donde mirar.
- **`flow run <comando>`** abre el comando en un panel nuevo de la misma sesión.
  Sirve para lo que dura o interesa mirar: servidores, suites largas, logs,
  subagentes.
- **La salida de `flow run` no vuelve a quien lo pidió**, se queda en su panel.
  Lo que necesites leer para seguir trabajando, ejecútalo como siempre.
- Por debajo es un fichero: `flow run` solo escribe el comando dentro de
  `FLOW_INBOX` y se va. Se puede hacer con un `echo`.

## Documentación larga

| Documento                                          | De qué va                                                   |
| -------------------------------------------------- | ----------------------------------------------------------- |
| [README.md](README.md)                             | Qué es, cómo se instala y cómo se usa                       |
| [docs/dentro-de-flow.md](docs/dentro-de-flow.md)   | El entorno que ve un agente, `flow run` y el buzón          |
| [docs/diseno.md](docs/diseno.md)                   | El sistema visual entero y el contrato de accesibilidad     |
| [docs/arquitectura.md](docs/arquitectura.md)       | Arquitectura, rendimiento, limitaciones y mejoras pendientes |
| [.claude/skills/flow-ui/SKILL.md](.claude/skills/flow-ui/SKILL.md) | Guía operativa para tocar la interfaz sin romper el idioma |

**Los comentarios del código son la fuente de verdad.** `theme.rs`,
`ui/widgets.rs`, `ui/tiles.rs`, `ui/bar.rs` y `ui/grain.rs` llevan comentarios de
módulo largos que justifican cada decisión, y varias parecen arbitrarias hasta
que lees el motivo. Si cambias una decisión, actualiza el comentario que la
defendía: uno que dice lo contrario de lo que hace el código es peor que ninguno.
