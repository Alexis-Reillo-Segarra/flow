# Por dentro

Cómo está montado flow, qué se cuidó para que vaya rápido, qué **no** hace y
qué falta. Para usarlo no hace falta nada de esto; para tocarlo, sí.

## Rendimiento

La app está parada la mayor parte del tiempo: solo repinta cuando llega salida
de un proceso o cuando hay algo animándose. Sin agentes vivos no consume CPU.

Dónde se cuidó de verdad:

- **Render virtualizado.** `show_rows` solo construye las filas visibles, así que
  da igual que el scrollback tenga 5000 líneas: se montan ~40 `LayoutJob` por
  frame, no 5000.
- **Tramos fusionados.** Cada fila junta las celdas contiguas con el mismo
  estilo, así que una línea sin color es un solo tramo de texto en vez de 200.
- **Estado cacheado.** Analizar la última línea para decidir si es un prompt se
  hacía en cada frame. Ahora se hace una vez por ráfaga de salida y se invalida
  cuando llegan bytes nuevos: mientras el proceso siga callado, la respuesta no
  puede cambiar.
- **Búsqueda acotada.** `last_nonempty_line` se rinde tras 64 líneas en blanco.
  Antes, una pantalla vacía recorría las 5000 del scrollback montando un
  `String` por cada una.
- **Scrollback recortado.** Las líneas que salen por arriba pierden sus blancos
  finales antes de archivarse, que en salida de terminal es la mayor parte.

## Arquitectura

```
build.rs      le pega al .exe de Windows su icono y su ficha de propiedades
src/
  main.rs     arranque de eframe y opciones de ventana
  run.rs      `flow run`: pedir un panel desde dentro de una sesión
  app.rs      estado global, bucle de frame, escala automática y buzones
  session.rs  una sesión: sus paneles, su directorio y el entorno que ven
  agent.rs    un panel = un proceso en un PTY, con dos hilos y heurística de estado
  term.rs     emulador de terminal (rejilla, scrollback, ANSI)
  keys.rs     de teclas a bytes: lo que escribes va al panel con el foco
  logo.rs     la marca: cuatro barras inclinadas, rasterizadas en código
  presets.rs  catálogo de agentes y detección en el PATH
  theme.rs    los temas: paleta, contrato, fuentes, espacio y estilo
  config.rs   el fichero de configuración: tema activo y temas propios
  projects.rs los directorios que ya has usado
  repos.rs    búsqueda de repositorios en el equipo
  ui/
    mod.rs      el enum `Action` y el pegamento entre vistas
    chrome.rs   barra superior, botones de ventana y bordes de resize
    bar.rs      la columna de sesiones de la izquierda
    grain.rs    el grano del fondo
    tiles.rs    el mosaico: reparto del espacio y marco de cada panel
    output.rs   la terminal de un panel
    prompt.rs   la tira de abajo: a quién le escribes y los botones ^C/ESC/KILL
    spawn.rs    el formulario de lanzamiento
    themes.rs   el selector de temas
    widgets.rs  primitivas de dibujo
```

La UI es de modo inmediato, así que no puede añadir ni quitar paneles mientras
dibuja. En vez de pelearse con el borrow checker, cada vista devuelve un
`ui::Action` y `Flow::apply` los resuelve al final del frame: un único sitio donde
ocurren las mutaciones de verdad.

### Detección de estado

No hay forma general de preguntarle a un proceso arbitrario "¿me estás
esperando?", así que se deduce del ritmo de la salida y de la forma de la última
línea —igual que un humano mirando la terminal de reojo:

- Sale texto ahora mismo → `WORKING`
- Lleva >1,2 s callado y la última línea parece un prompt (`?`, `:`, `>`,
  `(y/n)`, `password`…) → `BLOCKED`
- Lleva >10 s callado → `IDLE`

Es una heurística y a veces se equivocará. Los tests de `agent.rs` fijan los
casos que importan.

## Lo que no es igual en los tres sistemas

flow corre en Windows, en Linux y en macOS, y CI lo comprueba en los tres. Lo
que cambia son **cinco sitios**, cada uno con su `cfg` y su test; fuera de ellos
el código es el mismo. Están aquí juntos porque la lista completa es corta y es
lo que hay que mirar antes de añadir una decisión que dependa del sistema.

| Qué                       | Dónde                   | Por qué no puede ser igual                                                                 |
| ------------------------- | ----------------------- | ------------------------------------------------------------------------------------------ |
| El shell del panel        | `presets::shell`        | En Windows `cmd`; fuera, el de `$SHELL` —`zsh` en macOS— porque no todo el mundo tiene `bash` |
| Qué se puede ejecutar     | `presets::ejecutable`   | En Windows lo dice la extensión (`PATHEXT`); en Unix, el bit de ejecución                    |
| La tecla de los atajos    | `keys::reservada`       | En macOS es Cmd, y así el `Ctrl` entero se queda para el proceso                            |
| Qué separa una ruta       | `projects::name_of`, `repos::same_path` | Fuera de Windows `\` es un carácter válido de nombre, y las mayúsculas cuentan |
| Redimensionar por el borde| `ui::chrome::resize_handles` | En macOS lo hace AppKit; la orden de winit ni siquiera existe allí                     |

Y dos cosas que **no** cambian y podrían parecer que sí: el fichero de
configuración va a `~/.config/flow` también en macOS —es texto para editar a
mano, y ahí es donde lo buscan las herramientas de terminal— y la ventana va sin
decoración del sistema en los tres, con la misma barra propia.

Lo que sigue sin estar cubierto por los tests es lo de siempre: `main.rs` abre
una ventana de verdad, y ninguna máquina de CI la abre.

## Limitaciones conocidas

- **Sin persistencia.** Al cerrar flow se matan todos los agentes. No hay
  reattach ni sesiones que sobrevivan, a diferencia de un multiplexor de
  terminal.
- **Sin caracteres de doble ancho.** CJK y emoji ocupan una celda en la rejilla,
  así que descuadran las columnas.
- **El comando pasa por el shell** (`cmd.exe /C` en Windows, `$SHELL -c` fuera).
  Es lo que hace que funcionen los shims `.cmd` de Node y las tuberías, pero
  significa que hay un proceso intermedio entre flow y el agente.
- **Sin reflow al redimensionar.** El contenido se recorta, no se reajusta. Al
  encoger, las filas en blanco de abajo se tiran antes que archivar por arriba,
  así que la salida corta de un proceso no se va de la vista.
- **Ocho paneles por sesión como tope**, y el reparto no se puede tocar a mano:
  no hay paneles flotantes, ni intercambiar dos de sitio, ni cambiar la
  proporción de un corte arrastrando su borde. Un panel tampoco se puede mover
  de una sesión a otra.
- **Lo que se pide al buzón con la sesión llena se descarta**, y desde dentro
  del agente no hay forma de enterarse: flow no le contesta. `flow run` tampoco
  puede avisar, porque el que descarta es flow y para entonces el subcomando ya
  terminó; por eso dice «pedido un panel» y no «abierto».
- **Lo que el agente ejecuta con sus propias herramientas no se ve como un
  panel.** Ese proceso lo lanza él, como hijo suyo y con la salida enganchada a
  sí mismo; flow solo ve los bytes que el agente escribe en su PTY, así que lo
  verás como el agente decida enseñártelo. Para verlo de verdad tiene que
  pedirlo con `flow run`. Meterse en medio exigiría interceptarle la creación de
  procesos —enganchar `CreateProcess`, inyectar una DLL, suplantarle el shell— y
  eso es otra clase de programa.

## Mejoras propuestas de interfaz

Ninguna está implementada. Van ordenadas por lo que aportan frente a lo que
cuestan, y cada una dice dónde tocaría.

> Aquí había una primera entrada —separar el verde de marca del verde de estado,
> que se quedaban a 27° de tono— y ya no hace falta: el tema de casa es
> monocromo, así que el acento no tiene tono con el que confundirse con nada. La
> deuda se pagó quitando el color, no moviéndolo.
>
> Y había otra —avisar de que un panel está desenganchado del final— que **ya
> está hecha**: si subes por el scrollback, en la cabecera aparece una flecha
> abajo que además es el botón que te devuelve al final. Va sin número de líneas
> nuevas, y eso fue una decisión, no un olvido: ver `agent::follow`.
>
> Y una tercera —selección de texto en el panel— que **se descarta**. La idea
> era la regla de cualquier terminal: `Ctrl-C` copia si hay algo seleccionado e
> interrumpe si no. El problema es lo que cuesta aquí: la salida se dibuja fila a
> fila desde la rejilla del emulador con render virtualizado, así que una
> selección tiene que vivir en coordenadas de celda —no de texto—, sobrevivir al
> scroll, al reparto de la rejilla, al redimensionado que reescribe la rejilla
> entera y al alt-screen. Y encima le disputaría `Ctrl-C` al proceso, que es la
> tecla más importante de esta aplicación. Se queda fuera: para copiar lo que
> escribió un agente, la salida sigue estando en su terminal de origen.

**2. Animar el cierre de un panel.** Abrir desliza y crece; cerrar desaparece de
golpe y los demás se reordenan. Cerrar el ciclo pide que `Tiling` conserve el
panel muerto unos 120 ms con su opacidad bajando, lo que implica una lista de
"fantasmas" aparte de `panes`, porque el `Agent` ya no existe cuando toca
dibujarlo.

**3. Navegación por `Tab`.** Es la limitación de accesibilidad que reconoce
[el diseño de flow](diseno.md#accesibilidad). Media interfaz está pintada con el
`Painter` y no entra en el orden de foco de egui, pero las respuestas ya existen
(`ui.interact` en cabeceras, pastillas y botones): falta marcarlas como
enfocables y pintarles un anillo de foco. Con eso, llegar a los botones de la
barra inferior dejaría de exigir ratón.

**4. Respetar "reducir movimiento" del sistema.** Los parpadeos cumplen la WCAG
2.3.1 por frecuencia, pero quien haya pedido menos animación en su escritorio
sigue viéndolos. Lo correcto sería leer esa preferencia y degradar a estático:
`WORKING` sólido, `BLOCKED` alternando color en vez de parpadeando, y el reparto
de la rejilla saltando directo a su sitio. egui no lo expone, así que habría que
consultarlo al sistema.

**5. Decir que una sesión que no estás mirando ha escrito.** El subrayado de la
pastilla resume el *estado*, no la *novedad*: una sesión que pasó a `IDLE`
después de soltar cien líneas se ve igual que una que lleva parada media hora.
Un punto junto al número, que se apague al visitarla, cubriría el hueco.


---

Si lo que buscabas era **cómo se usa**, está en el [README](../README.md). Si es
**por qué la interfaz es así**, en [el diseño de flow](diseno.md).
