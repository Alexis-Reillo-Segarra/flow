<p align="center">
  <img src="assets/brand/png/flow-icon-dark-256.png" alt="La marca de flow: cuatro barras inclinadas, de más a menos altura y de más a menos contraste, que dibujan la silueta de una F" width="128">
</p>

<h1 align="center">flow</h1>

**Lanza varios agentes de terminal a la vez y velos trabajar a todos en la misma
pantalla.**

`claude`, `codex`, `cargo test`, un servidor, un `git log` — cada uno en su
terminal de verdad, todos visibles al mismo tiempo. flow te dice de un vistazo
cuál está trabajando, cuál terminó y cuál lleva cinco minutos esperando que le
contestes.

Aplicación de escritorio nativa, escrita en Rust. **Un solo fichero**: sin
Electron, sin servidor, sin navegador, sin instalador.

![Cinco paneles en mosaico, cada uno con su terminal, y la columna de sesiones a la izquierda](docs/mosaico.png)

*Cinco procesos a la vez. El del borde blanco es el que te escucha. Arriba a la
derecha, `git` escribiendo en sus colores de siempre: la interfaz es gris, el
color es de los procesos.*

---

## Instalarlo

### Windows — descargar y abrir

**[⬇ Descargar `flow.exe`](https://github.com/Alexis-Reillo-Segarra/flow/releases/latest)**

Es un único fichero de 11 MB. Cópialo donde quieras —el escritorio, una carpeta
cualquiera— y ábrelo con doble clic. No hay instalador, no escribe en el
registro, no pide permisos de administrador. **Para desinstalarlo, borra el
fichero.**

Si quieres poder abrirlo escribiendo `flow` en cualquier terminal, déjalo en una
carpeta que esté en el `PATH`.

<details>
<summary>Dos cosas que pasan la primera vez, y son normales</summary>

- **Windows dirá que «ha protegido tu PC».** Es SmartScreen, y sale con
  cualquier programa sin firma digital, no porque este tenga nada raro:
  *Más información* → *Ejecutar de todas formas*. Quitarlo de en medio requiere
  un certificado de firma de código, que se paga.
- **El antivirus puede tardar un rato** en dejarlo abrir la primera vez. Es lo
  normal con binarios nuevos y sin reputación.

</details>

### Cualquier sistema — compilarlo

Solo hace falta [Rust](https://rustup.rs). No hay dependencias del sistema que
instalar, ni SDKs, ni `pkg-config`; las tipografías van dentro del binario.

```
git clone https://github.com/Alexis-Reillo-Segarra/flow.git
cd flow
cargo run --release        # probarlo sin más
cargo install --path .     # dejarlo instalado: luego se abre escribiendo `flow`
```

`cargo install` lo copia a `~/.cargo/bin`, que ya está en el `PATH`. Para
desinstalarlo, `cargo uninstall flow`. Para actualizarlo, `git pull` y vuelve a
lanzar `cargo install --path .`.

> **Sobre otros sistemas.** El código es multiplataforma, pero quien lo
> desarrolla y lo prueba trabaja en Windows, que es de donde salen los binarios
> publicados. En Linux y macOS se compila desde fuente; en Linux hacen falta las
> bibliotecas de ventanas habituales de cualquier GUI (`libxkbcommon`, X11 o
> Wayland). Si algo no va en tu sistema,
> [abre un issue](https://github.com/Alexis-Reillo-Segarra/flow/issues).

---

## Usarlo, en un minuto

Al abrirlo no hay nada: flow no adivina qué querías lanzar. **`Ctrl-N`** abre una
sesión nueva y el formulario pregunta tres cosas, de una en una. En cada paso hay
algo que pulsar, así que se puede recorrer entero sin escribir: `Enter` avanza, y
`Enter` en el último paso lanza.

**1 — Dónde.** Sobre qué carpeta se trabaja. Arriba, los proyectos que ya has
usado; debajo, los repositorios que flow ha encontrado solo en tu equipo. No hay
que teclear rutas.

![Primer paso: PROYECTO, REPOS y el campo DIR](docs/asistente-1.png)

**2 — Qué.** Qué se lanza ahí dentro. Solo salen los agentes que tienes de verdad
instalados, cada uno con su marca; pulsas uno y rellena el comando.

![Segundo paso: AGENTES, HERRAMIENTAS y el campo COMMAND](docs/asistente-2.png)

**3 — Cómo se llama.** El nombre del panel —si no pones nada, se llama como el
comando— y un resumen de lo que va a pasar antes de que pase.

![Tercer paso: NAME y el resumen de lo que se va a lanzar](docs/asistente-3.png)

Y ya está corriendo. A partir de ahí, **`Ctrl-T`** le añade más paneles —hasta
ocho, todos sobre la misma carpeta— y la rejilla se reparte sola.

**Para escribirle a uno, escribe.** Lo que teclees va al panel que tenga el foco
—el del borde claro—, tecla a tecla y sin campo de por medio: `Ctrl-C`
interrumpe, `Tab` completa, las flechas recorren el historial y una TUI a
pantalla completa responde como en cualquier terminal. Para cambiar de
destinatario, haz clic en otro panel o salta con `Alt` y las flechas.

### Los estados

Cada panel dice en su cabecera qué está haciendo, y no solo con el color: cada
estado lleva su palabra, su forma y su ritmo.

| Estado    | Qué significa                                     |
| --------- | -------------------------------------------------- |
| `WORKING` | Está escribiendo ahora mismo. Late despacio.        |
| `BLOCKED` | Lleva un rato callado esperando que le contestes. Parpadea: es el único que reclama atención. |
| `IDLE`    | Vivo, pero sin hacer nada visible.                  |
| `DONE`    | Terminó bien.                                       |
| `EXIT`    | Terminó con error.                                  |
| `FAILED`  | Ni siquiera pudo arrancar: el panel dice por qué.   |

### Atajos

**Todo el teclado es del proceso menos estas once combinaciones**, que son las
que mueven la aplicación. **Ctrl** se mueve entre sesiones y **Alt**, dentro de
la que estás mirando.

| Tecla          | Acción                                             |
| -------------- | -------------------------------------------------- |
| `Ctrl-N`       | Abrir una sesión nueva                             |
| `Ctrl-T`       | Añadir una terminal o un agente a esta sesión      |
| `Ctrl-Shift-T` | Cambiar de tema                                    |
| `Ctrl-W`       | Cerrar el panel con el foco                        |
| `Ctrl-Shift-W` | Cerrar la sesión entera, con todos sus procesos    |
| `Ctrl-1`…`9`   | Saltar a la sesión n-ésima                         |
| `Alt-1`…`8`    | Saltar al panel n-ésimo de la sesión               |
| `Alt`+flechas  | Mover el foco al panel de al lado                  |
| `Enter`        | En el formulario: pasa al siguiente paso, o lanza  |
| `Esc`          | Cerrar el formulario                               |

Lo demás llega entero al panel con el foco, `Ctrl-A`, `Ctrl-R` o `Ctrl-Z`
incluidos. El precio de las tres primeras hay que saberlo: **`Ctrl-W` cierra el
panel en vez de borrar la palabra anterior**, y lo mismo pasa con `Ctrl-N` y
`Ctrl-T` en un shell o en una TUI que los use.

---

## Qué hace

- **Cualquier CLI vale.** flow no integra ningún agente en concreto: todos son
  procesos de terminal. Puedes tener a la vez `claude`, `codex` y los tests
  corriendo, cada uno en el suyo. El formulario mira tu `PATH` al abrirse y solo
  ofrece los que de verdad tienes: `claude`, `codex`, `gemini`, `opencode`,
  `aider`, `cursor-agent`, `amp`, `goose`, `crush`, más `shell`, `tests` y `git`.
- **Ocho paneles a la vez, en mosaico.** Todos en pantalla al mismo tiempo, sin
  pestañas: si tienes cuatro procesos trabajando, ves trabajar a los cuatro. El
  reparto se recalcula solo con la forma de la ventana.
- **Sesiones.** Una sesión es un agente y lo que le hace falta alrededor: sus
  paneles comparten carpeta, así que mientras `claude` escribe código, al lado
  abres un shell en su mismo sitio y ves lo que está haciendo. La columna de la
  izquierda resume el estado de cada sesión, para que un `cargo test` que revienta
  en la tres se vea desde la uno.
- **PTYs de verdad.** No se captura la salida por una tubería: se abre un
  pseudo-terminal, así que los procesos se creen interactivos, emiten color y
  pueden pedirte datos. Funciona con cualquier programa sin integrarlo.
- **Emulador de terminal propio.** Scrollback, SGR completo (16 colores, 256 y
  truecolor), alt-screen y región de scroll. Aguanta tanto salida en flujo como
  una TUI a pantalla completa.
- **Se escribe dentro.** El teclado va al panel con el foco tal cual, así que un
  panel es una terminal de verdad y no una caja de salida: `Ctrl-C` interrumpe,
  `Tab` completa, las flechas recorren el historial y pegar varias líneas no las
  ejecuta si el proceso pidió el pegado entre corchetes. Abajo quedan los
  botones para lo que no se teclea cómodo: `^C`, `ESC` y `KILL`.
- **Los agentes pueden pedirte paneles.** Un agente que corre dentro de flow
  puede lanzar `flow run cargo test` y aparece un panel al lado con eso
  corriendo. Ver [dentro de flow](docs/dentro-de-flow.md).

---

## Temas

**`Ctrl-Shift-T`** abre la lista. Vienen cinco.

![El selector de temas, con los cinco incluidos y sus muestras de color](docs/temas.png)

| Tema         | Qué es                                                 |
| ------------ | ------------------------------------------------------ |
| `flow`       | El de casa: monocromo, del negro OLED (`#000000`) al blanco |
| `catppuccin` | Catppuccin Mocha: malva sobre base azulada             |
| `gruvbox`    | Gruvbox Dark: cálido, contrastado, retro               |
| `tokyonight` | Tokyo Night: azul noche                                |
| `nord`       | Nord: frío, gris azulado                               |

**Lo que eliges se ve mientras lo eliges**: moverte por la lista aplica el tema
de verdad a la app entera —los paneles, el fondo, la salida de los procesos que
ya estaban escritos—, no a una miniatura. Un tema se juzga con la terminal llena
de texto. `Enter` se queda con el que estés probando y `Esc` deja las cosas como
estaban, así que probar no cuesta nada.

**El de casa es monocromo**, y es la idea que ordena toda la interfaz: si algo
tiene color en pantalla, **no lo ha puesto flow, lo ha escrito un proceso**. Los
otros cuatro son los que ya tiene medio mundo en su editor, portados tono a tono.

Ninguno puede dejar la interfaz ilegible: todos pasan los mismos tests de
contraste, y son tests que fallan el `cargo test`, no buenas intenciones. El
porqué está en [el diseño de flow](docs/diseno.md).

### Escribirte uno

flow lee un fichero de texto donde puedes cambiar el tema activo o escribirte el
tuyo, heredando de uno de los cinco:

```ini
theme = nord

[theme mío]
base   = flow
bg     = #101014
accent = #d3869b
```

El fichero está en `%APPDATA%\flow\config` —o `$XDG_CONFIG_HOME/flow/config`
fuera de Windows— y **el propio flow te lo escribe comentado** la primera vez,
con todos los nombres de color que puedes tocar. El selector de temas enseña
abajo la ruta exacta.

---

## Dónde deja las cosas

flow escribe **dos ficheros de texto** en tu disco, y ninguno es crítico: si no
se pueden escribir, la app funciona igual y simplemente no recuerda.

| Qué                                | Dónde                                             |
| ---------------------------------- | ------------------------------------------------- |
| Los directorios que ya has usado   | `%APPDATA%\flow\projects`                         |
| Tu configuración y tus temas       | `%APPDATA%\flow\config`                           |
| Los buzones de las sesiones vivas  | El temporal del sistema, bajo el PID de flow      |

Fuera de Windows, `$XDG_CONFIG_HOME/flow/…` o `~/.config/flow/…`.

El buzón de una sesión se borra al cerrar la sesión. La carpeta que los agrupa
—`<temporal>\flow\<pid>`— se queda: son directorios vacíos, con el PID de una
ejecución que ya terminó, en el sitio del disco que el sistema limpia solo.

---

## Lo que flow no hace

Conviene saberlo antes de probarlo:

- **No sobrevive a cerrarse.** Al cerrar flow se matan todos los agentes. No hay
  *reattach* ni sesiones persistentes, a diferencia de tmux.
- **CJK y emoji descuadran las columnas**: la rejilla les da una celda y ocupan
  dos.
- **Ocho paneles por sesión como tope**, y el reparto no se toca a mano: no hay
  paneles flotantes ni se arrastra el borde entre dos.
- **Al redimensionar no hay reflow**: el contenido se recorta, no se reajusta.
- **Lo que un agente ejecuta con sus propias herramientas no se ve como un
  panel.** Para verlo tiene que pedirlo con `flow run`.

La lista completa y razonada está en [por dentro](docs/arquitectura.md).

---

## Más a fondo

| Documento                                        | De qué va                                                         |
| ------------------------------------------------ | ----------------------------------------------------------------- |
| [Un agente dentro de flow](docs/dentro-de-flow.md) | El entorno que ve un agente, `flow run` y el buzón                |
| [El diseño de flow](docs/diseno.md)              | Por qué la interfaz es así: color, tipografía, movimiento, accesibilidad |
| [Por dentro](docs/arquitectura.md)               | Arquitectura, rendimiento, limitaciones y lo que falta            |
| [AGENTS.md](AGENTS.md)                           | Orientación rápida del repositorio, para agentes y para humanos con prisa |

## Desarrollo

```
cargo test                                       # 245: emulador, estado, interfaz, paleta, config, marca y teclado
cargo clippy --all-targets                       # sin warnings
cargo run --example pty_probe -- "tu comando"    # sonda de la capa PTY
```

No hace falta nada más que Rust. `build.rs` le pega al `.exe` su icono y su ficha
de propiedades, y lo hace enlazando un recurso ya compilado que va en el
repositorio: compilar flow no depende del SDK de Windows ni de ninguna caja de
construcción. Fuera de `*-pc-windows-msvc` ese paso no hace nada.

Cada push y cada pull request pasan por `.github/workflows/ci.yml`: `cargo test`
y `cargo clippy -D warnings` en Windows. Publicar una versión es empujar una
etiqueta `vX.Y.Z`; `release.yml` compila y sube el `.exe` a la release.

## Licencias

Código bajo MIT. Las dos tipografías conservan la suya, y las dos son OFL:
[Inter](https://rsms.me/inter/) y
[JetBrains Mono](https://www.jetbrains.com/lp/mono/). Van embebidas en el binario
y su licencia viaja al lado del `.ttf`, en `assets/fonts/`.

La marca —las cuatro barras de arriba— también es del proyecto y está en
[`assets/brand/`](assets/brand/README.md), con su geometría escrita. Dentro de la
aplicación **se dibuja en código** (`src/logo.rs`): así se rasteriza al número
exacto de píxeles que va a ocupar sea cual sea la escala del sistema, y se pinta
con el acento del tema que tengas puesto. Los ficheros de `assets/brand/` son
para lo que flow no dibuja: el icono del `.exe` y esta página.
