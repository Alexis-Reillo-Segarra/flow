---
name: flow-ui
description: Sistema de diseño y modo de trabajo para la interfaz de flow, el orquestador de agentes en Rust + egui. Úsala SIEMPRE que vayas a tocar cualquier cosa dentro de src/ui/, src/theme.rs, src/logo.rs o src/term.rs, y también cuando el usuario hable de la apariencia, la paleta, los colores, el negro OLED, el fondo, el grano o la textura, las sombras, los halos, la profundidad, la tipografía, el espaciado, las animaciones, el foco, los paneles, el mosaico, la barra superior, la columna de sesiones, los iconos o logotipos de los agentes, los temas, el aspecto "Hyprland" o "más bonito/moderno", o pida añadir un widget, una pantalla o un indicador nuevo. Aunque el cambio parezca trivial —mover dos píxeles, cambiar un gris— léela antes: flow tiene reglas escritas que no se deducen del código y que un cambio local puede romper sin que compile mal.
---

# La interfaz de flow

flow es un orquestador de agentes CLI: varios procesos a la vez, cada uno en un
PTY real, todos visibles en la misma pantalla. La interfaz existe para responder
una pregunta de un vistazo — **¿cuál de estos está trabajando, cuál terminó y
cuál lleva cinco minutos esperando que le contesten?**

La referencia estética no es una app de escritorio: es un **gestor de ventanas en
mosaico al estilo Hyprland**. Ventanas que se reparten el espacio solas,
separadas por hueco y no por marcos, con la activa marcada por su borde y
movimiento que explica lo que acaba de pasar. Todo lo que decidas tiene que
empujar hacia ahí.

Lo que **no** es: un dashboard de tarjetas, una app de navegación por pestañas,
ni nada que se parezca a Material o a Bootstrap. Si un cambio hace que la
pantalla se lea como una tabla de celdas o una pila de tarjetas, va en dirección
contraria aunque quede "limpio".

**Sobre la columna de la izquierda.** Aquí decía «ni una app con barra lateral»,
y hoy hay una: `ui::bar` lleva las sesiones en vertical. No es una contradicción,
es que la frase estaba mal escrita. Lo que no cabe en flow es el *chrome de
navegación* — pestañas, iconos de sección, un menú que se despliega, una barra
que existe para llevarte a otras pantallas. Una **columna de espacios de
trabajo** sí, porque es una seña del mosaico y no de una app de escritorio: una
fila por sesión, su estado en el canto, sin niveles ni desplegables, y se encoge
a los números cuando la ventana aprieta. Si lo que vas a añadir a esa columna
tiene hijos que se abren y se cierran, ya no es esto.

## Lo primero: el código ya explica el porqué

`src/theme.rs`, `src/ui/widgets.rs`, `src/ui/tiles.rs`, `src/ui/bar.rs` y
`src/ui/grain.rs` llevan comentarios de módulo largos que justifican cada
decisión. **Léelos antes de cambiar nada de lo que tocan.** No son documentación
de cortesía: son el registro de por qué las cosas están como están, y varias de
ellas parecen arbitrarias hasta que lees el motivo.

Cuando cambies una decisión, **actualiza el comentario que la defendía**. Un
comentario que defiende lo contrario de lo que hace el código es peor que no
tener comentario: el siguiente que pase se lo va a creer.

## El sistema visual

### Color

Tres colores llevan la estructura: el fondo, el gris de las divisorias de 1 px y
el verde de la marca. **La estructura se lee por las líneas y por el hueco, no
por bloques de tono.** No hay rellenos de color decorativos ni tarjetas grises.

El fondo es **`#000000` exacto, negro OLED**, y eso es una decisión tomada, no un
valor por defecto que nadie revisó: en un panel OLED ese valor es el píxel
apagado, un negro que ninguna otra pantalla sabe dar. **No lo subas.** Es la
restricción más dura de la paleta y de ella se derivan dos cosas que a primera
vista parecen caprichos: por qué no hay sombras (ver más abajo) y por qué existe
el grano.

Aparte de eso hay cuatro colores que **solo** hablan de estado — verde, ámbar,
rojo, gris. La regla que los hace útiles: *si ves color en flow, significa algo*.
En cuanto uses el ámbar para adornar un separador, el ámbar deja de querer decir
"esto está bloqueado esperándote" y pierdes el idioma entero.

El acento tiene dos caras y no son intercambiables:

- `ACCENT` — **solo como superficie o trazo**: bordes, rellenos, marcos. Cumple
  el 3:1 que la WCAG pide a un componente de interfaz, pero no el 4,5:1 de un
  texto.
- `ACCENT_TEXT` — la misma marca aclarada, para cuando el acento **es una letra**
  o un glifo fino.

Confundirlas es el error más fácil de cometer aquí, y hay un test que lo caza.

Los 16 slots ANSI son para el output de los procesos y **conservan su nombre
semántico**: el slot 6 es un cian de verdad, no el color de la marca. Un proceso
que escribe en cian espera cian; atarlo al acento significaría que cambiar la
marca repinta la salida ajena.

### Tipografía

Dos familias con un trabajo cada una, y no hay una tercera:

- **Sans (Inter)** — nombres, estados, botones, rótulos, etiquetas. Neutro a
  propósito: el carácter lo pone la rejilla, no la letra.
- **Mono (JetBrains Mono)** — todo lo que sale de un proceso, más rutas,
  comandos y cifras.

Esa última parte importa y se olvida: **los datos van en mono aunque estén en el
chrome**. Un tiempo en vida, un contador `3/8`, una ruta — mono, y en el cuerpo
pequeño, porque son datos de apoyo y compiten con el nombre que llevan al lado si
van al mismo tamaño.

Usa `theme::sans(…)` y `theme::mono(…)` con las constantes de tamaño ya
definidas. Si te hace falta un tamaño que no existe, pregúntate primero por qué:
cuatro tamaños cubren toda la interfaz hoy.

### Espacio

`GAP` es la **única** unidad de aire de la interfaz: el mismo valor separa un
panel del de al lado y del borde de la ventana. Ese hueco uniforme es
exactamente lo que hace que la pantalla se lea como ventanas en mosaico. No
inventes espaciados sueltos junto a los paneles; si necesitas otra medida, sale
de `GAP` (`GAP * 2`, `GAP / 2`), no de un número mágico nuevo.

`RADIUS` **solo lo lleva el panel de la rejilla**. Todo lo que va dentro
—botones, campos, marcas de estado— y todo lo que es una fila de una lista —las
sesiones de la columna— va a esquina viva, para que el redondeo signifique una
cosa concreta: *esto es una ventana*. Si lo llevara todo, no distinguiría nada. El
borde exterior de la app también va a esquina viva: es el corte de la ventana,
no una superficie flotante.

## Las señas de Hyprland que flow adopta

Estas cuatro son canon y **las cuatro están implementadas ya**. Este apartado no
es una lista de deseos: es lo que hay, con el porqué, para que no lo deshagas sin
saber lo que costó.

### 1. El foco se dice con el borde

La ventana activa lleva un **degradado** entre las dos caras del acento a lo largo
del rectángulo, a 45°; las demás, el gris de divisoria liso.
`widgets::gradient_border` recorre el contorno guardando en cada punto su normal
hacia fuera y saca una tira de triángulos con el color interpolado por vértice.

Dos cosas que hay que saber antes de tocarlo:

- **Va de `ACCENT` a `ACCENT_TEXT`, no a un tercer verde.** Son los dos tonos que
  ya existen, comparten tono, y los dos pasan el 3:1 de un trazo, así que el
  borde cumple en todo su recorrido y no solo en un extremo.
- **La tira lleva un reborde de un píxel que se desvanece, repartido medio píxel
  a cada lado.** Sin él, las curvas de las esquinas salen en escalera: los trazos
  de egui se suavizan solos y una malla puesta a mano no. Y si lo reparte mal
  —un píxel entero a cada lado sobre el grosor completo— el marco engorda y el
  foco se acaba diciendo dos veces, por color y por grosor.

Que el foco se diga por el borde tiene una consecuencia: **no se dice también por
otras cinco cosas**. Hoy son dos señales —el borde y el atenuado de abajo— y ahí
se acaban. El panel con foco **no** lleva fondo propio (hubo un relleno `SEL` en
su cabecera y se quitó), ni el nombre en otro color, ni tipografía distinta.

### 2. Más aire y más redondeo

`GAP` está en 8 y `RADIUS` en 6. Se movieron hacia los valores de Hyprland
—`gaps_out 20`, `rounding 10`— sin llegar a ellos, y no van a llegar: flow mete
hasta ocho paneles en pantalla y el aire que en un WM de escritorio es holgura,
aquí se come columnas de terminal. Con cuatro columnas, cada punto de `GAP` son
cinco puntos de ancho que el terminal no ve.

Si los subes más, hazlo en pasos y mirando el resultado **con ocho paneles**
abiertos, no con dos.

### 3. Atenuar los paneles sin foco (`dim_inactive`)

`theme::DIM_INACTIVE` está en **0,90**, y ese número no está elegido a ojo.

Se apaga **solo la tinta** —los rótulos de la cabecera y la salida del proceso—.
El marco y las divisorias van enteros: son el esqueleto del mosaico, y una
rejilla con siete bordes medio borrados se deshace. La estructura se queda nítida
en los ocho; lo que retrocede es lo que hay dentro.

El atenuado es leve porque el argumento que había en contra sigue en pie a
medias: los ocho paneles están en pantalla porque la promesa de flow es que los
ves trabajar a todos a la vez, y siete apagados de verdad son siete que ya no
puedes leer. Un `dim_inactive` al estilo de un WM de escritorio rompe el
producto, no solo el contraste.

**El coste ya está pagado, y conviene entender cómo**, porque es el patrón a
seguir si algún día lo cambias. Atenuar tumbaba a `TEXT_FAINT` de 4,51:1 a
3,79:1, o sea por debajo de AA. En vez de bajar el listón se subieron los dos
niveles de gris más flojos hasta que **atenuados** siguen cumpliendo. El
resultado tiene una propiedad que merece la pena conservar: un panel apagado se
lee hoy exactamente igual que se leía toda la interfaz antes de que esto
existiera, así que el atenuado no oscurece siete — **aclara el que te escucha**.
Si tocas `DIM_INACTIVE`, recalcula los niveles; hay un test que te para.

### 4. Profundidad: halo, no sombra

Hyprland trae sombra por defecto. **flow no la lleva, y no es un olvido: es que
no puede.** Una sombra oscurece lo que tiene debajo, y sobre `#000000` no queda
nada que oscurecer. Una sombra negra sobre negro no es nada.

La salida no es subir el fondo. El negro OLED es una decisión anterior y más
importante (ver *Color*), así que la profundidad se da **al revés**: un halo de
luz muy tenue que se desvanece hacia fuera del panel. `widgets::panel_halo`, con
`epaint::Shadow` y una alfa muy baja. Para el ojo funciona igual —lo que separa
dos superficies es que entre ellas haya un gradiente, dé igual hacia qué lado— y
consigue lo mismo que busca la sombra de allí: que la ventana se lea despegada
del fondo y no recortada sobre él.

El halo del panel con foco va teñido de verde, como el `col.shadow` coloreado de
la ventana activa de Hyprland. **No es una señal más**: es el borde derramándose,
el mismo acento del apartado 1.

Si alguien te pide «sombras de verdad», esto es lo que hay que contarle: se puede,
pero cuesta el negro OLED. Es un intercambio, no una mejora, y lo decide quien
manda en el proyecto, no tú.

## El grano del fondo

Esto no es una seña de Hyprland —allí no existe—, pero va con lo anterior porque
responde al mismo problema y sale de la misma restricción. Un negro absoluto y
liso a pantalla completa no le da al ojo ninguna referencia, y deja de saber si
mira una superficie o un agujero. `ui::grain` pinta un mosaico de ruido de 128×128
que se repite, anclado a la ventana para que no se arrastre cuando un panel se
mueve.

Las dos reglas que lo mantienen compatible con todo lo demás:

- **No aclara el negro, lo motea.** Las motas son blanco a un 6% de opacidad como
  mucho, así que la inmensa mayoría de los píxeles del fondo siguen siendo negro
  exacto y en un OLED siguen apagados. Si le subes la amplitud, deja de ser
  textura y pasa a ser ruido de televisión.
- **Vive debajo de los paneles, nunca dentro.** Los paneles de la rejilla se
  rellenan de negro liso encima —para eso existe ese `rect_filled`, no por
  color—, así que la salida de un proceso no se lee jamás sobre ruido. El grano
  se queda en los huecos, la barra y la columna, donde no hay nada que leer.

Corolario técnico: los rellenos de los paneles de egui (`Panel::top`,
`Panel::left`, `CentralPanel`) van **transparentes**, porque el fondo con grano
ya está pintado debajo. Si le pones un `fill` opaco a uno, tapas el grano en esa
franja y aparece una costura.

## Las marcas de los agentes

Cada agente conocido lleva una forma —el destello, el anillo, los corchetes— en
su botón del formulario y en la cabecera de su panel. Es lo que hace que en una
rejilla de ocho se vea de un golpe cuál es el `claude` y cuál el shell, sin ir
leyendo cabeceras. Están en `presets::Mark` y las dibuja
`widgets::paint_agent`; añadir un agente al catálogo es una línea con su marca.

**No son los logotipos oficiales, y no deben serlo.** flow no empaqueta recursos
de marca ajenos: son formas geométricas propias, elegidas para *evocar* a cada
agente y, sobre todo, para distinguirse entre ellas a diez píxeles, que es el
trabajo que tienen que hacer. Si alguien pide «los iconos oficiales», eso
requiere que aporte los ficheros; no los inventes ni los descargues por tu
cuenta.

Van dibujadas con segmentos, no con emoji ni con una fuente de iconos, igual que
todo lo demás. Todas caben en el mismo círculo imaginario y comparten grosor de
trazo, para que puestas en columna se lean como una familia y no como un
muestrario.

## Lo destructivo no se pone al alcance del ratón

Esto salió de un fallo concreto y vale como regla. Había una X para cerrar una
sesión entera, de 11×11, que aparecía al pasar por encima y estaba clavada **en
la esquina** de su pastilla: se salía por arriba invadiendo la barra de título,
quedaba cortada, y ponía «matar todos los procesos de esta sesión» a un píxel del
clic con el que se cambia de sesión.

La regla: **una acción que mata varios procesos y no tiene deshacer va por atajo**
—`Ctrl-Shift-W`—, no en un objetivo diminuto que aparece solo al acercar el ratón.
Cerrar un panel suelto sí puede tener su X en la cabecera, porque afecta a uno y
la cabecera tiene sitio de sobra. La diferencia no es de estilo, es de cuánto
duele equivocarse.

Y si vas a colocar algo a mano dentro de un rectángulo: **cabe dentro o no está**.
Centrar un control en el borde de su contenedor lo parte por la mitad a la primera
que el contenedor cambie de alto.

## Temas

La paleta deja de ser fija: flow admite temas intercambiables (Catppuccin,
Gruvbox, Tokyo Night, el verde de casa). El contrato que **cualquier** tema tiene
que cumplir para entrar:

1. Los cuatro niveles de texto cumplen 4,5:1 contra el fondo del tema — **y
   también atenuados**, porque con `dim_inactive` el nivel más tenue de un panel
   sin foco es el peor caso real de la interfaz.
2. El acento tiene sus dos caras: una de trazo (≥3:1) y una de texto (≥4,5:1),
   **con el mismo tono**, para que se lean como el mismo color y no como dos.
3. Los cuatro colores de estado cumplen 4,5:1 y siguen siendo distinguibles
   entre sí: verde, ámbar, rojo y gris tienen que separarse también para quien
   no distingue rojo de verde — por eso el estado **nunca** va solo en color, y
   siempre lleva su palabra (`WORKING`, `BLOCKED`) o su forma al lado.
4. Los slots ANSI conservan su nombre: el cian del tema es cian.

Los tests de `theme.rs` son la puerta de entrada, no un trámite. **Al añadir un
tema, los tests tienen que recorrer todos los temas**, no solo el activo — si
siguen comprobando constantes sueltas, dejan de proteger nada en cuanto haya un
segundo tema.

## El idioma egui de este proyecto

### La UI devuelve intención, no muta

En modo inmediato la vista tiene prestado `&mut Flow` mientras dibuja, así que no
puede añadir ni quitar nada sobre la marcha. El patrón es: **la función de UI
devuelve un `Action`, y `Flow::apply` lo resuelve al final del frame.** Eso evita
pelearse con el borrow checker y deja un único sitio donde ocurren las
mutaciones de verdad.

Si añades una interacción nueva, añade su variante a `Action` y trátala en
`apply`. No busques atajos con `RefCell` ni canales: el patrón ya existe y es el
que hace legible el flujo.

### Pintar a mano vs. usar widgets

Las marcas de estado, la X de cerrar, los botones de ventana y las divisorias van
**dibujados con rectángulos y segmentos, no con glifos** (`● ◐ ○ ✕`). Dos
razones: un rectángulo se clava en píxel exacto a cualquier escala —y a 6×6
puntos eso es la diferencia entre una marca nítida y una mancha gris— y no
depende de que la fuente traiga el símbolo.

Corolario que aplica a casi todo lo que te van a pedir: **no metas emoji ni
iconos de fuente en la interfaz.** Si hace falta un símbolo nuevo, se dibuja.

Cuando dibujes a mano, dos cosas que se olvidan siempre:

- **Redondea el centro al píxel** antes de trazar líneas de 1 px. Si el centro
  cae en medio de un píxel, la línea sale gris en vez de nítida.
- **Declárale el widget a AccessKit** con `response.widget_info(…)`. Algo pintado
  a mano no existe para un lector de pantalla hasta que le dices qué es y cómo se
  llama. Todos los controles dibujados del proyecto lo hacen; el tuyo también.

### Repintado

egui solo repinta cuando pasa algo. Si tu elemento se anima, **pide el repintado
tú**: `request_repaint_after` con la cadencia que necesites, o `request_repaint`
mientras dure una transición. Si no lo pides, tu animación se congela hasta que
el usuario mueva el ratón. Y al revés: no pidas repintado continuo para algo que
está quieto — con ocho PTYs vivos, los frames de más se notan.

## Movimiento

El movimiento en flow **explica**, no decora. Cada animación que existe responde
a una pregunta concreta:

- **El panel se desliza a su hueco** en vez de saltar, porque abrir o cerrar uno
  reorganiza la rejilla entera: deslizando entiendes lo que pasó, saltando te
  encuentras otra pantalla.
- **`WORKING` late** despacio y en onda suave: se nota movimiento sin distraer.
- **`BLOCKED` parpadea** en onda cuadrada, no senoidal, porque es un aviso y no
  una respiración. Es el único estado que reclama atención.
- **Un panel nuevo nace** algo más pequeño y translúcido, y entra **entero** —
  marco, cabecera y contenido a la vez. Si solo se atenúa el marco, el contenido
  aparece de golpe dentro de un marco todavía translúcido y se lee como un fallo
  de dibujado en vez de como algo naciendo.

Dos reglas técnicas al animar:

- **Usa el `dt` real** (`input.stable_dt`), no un paso fijo por frame, para que
  la animación dure lo mismo a 60 Hz que a 144.
- **Para de animar cuando llegues.** Hay una tolerancia por debajo de la cual no
  se distingue en pantalla; seguir interpolando solo gasta frames.

Si te piden una animación que no responde a ninguna pregunta —un fundido al pasar
el ratón, un rebote al abrir— la respuesta por defecto es no. Hyprland es rápido
y seco; no es una app de móvil.

## Accesibilidad: es un test, no una intención

Esto no es un apartado de buenas prácticas. `theme.rs` **tiene tests que fallan**
si lo rompes, y están ahí porque son fáciles de romper sin darte cuenta:

- Todo lo que es texto cumple 4,5:1 contra el fondo. **Incluido el nivel más
  tenue**: el gris flojo existe para dar jerarquía, no para esconder texto. Si se
  puede leer, tiene que poder leerse.
- **Y lo cumple también atenuado**, que es el caso real peor de la interfaz: el
  nivel más flojo de uno de los siete paneles sin foco. Tiene su propio test,
  `el_texto_de_un_panel_apagado_tambien_cumple_aa`, y es el que fija el techo de
  `DIM_INACTIVE` — `TEXT_FAINT` se queda en 4,58:1 al apagarse, a un pelo del
  mínimo. Un tercer test guarda el propio 0,90 dentro de un rango acordado.
- El acento de trazo cumple 3:1 y **se queda por debajo de 4,5:1 a propósito** —
  el test lo comprueba, porque si subiera, su variante de texto sobraría.
- Las dos caras del acento comparten tono, y **las dos** pasan el 3:1, porque las
  dos son extremos del degradado del foco: un degradado solo cumple si cumplen
  sus dos puntas.
- Nada parpadea por encima de 3 destellos por segundo (WCAG 2.3.1). El indicador
  de bloqueo va muy por debajo a propósito, porque puede estar parpadeando
  durante minutos.

Hay **una lista única de colores de texto** (`TEXTOS`, en los tests de
`theme.rs`) que recorren los dos tests de contraste. Si añades un color que va a
ser texto, va ahí: un color que nadie comprueba es un color que en dos meses no
cumple.

Dos excepciones declaradas, y solo dos:

- Las **divisorias** se quedan por debajo de 3:1 porque son decoración
  estructural y no transmiten estado.
- Los **halos** no pasan por ningún test porque no son texto, no dicen estado y
  no hay que poder leerlos. Son las únicas superficies de la interfaz que no
  significan nada.

Todo lo que **sí** informa cumple AA. Cuando cambies un color, **ejecuta
`cargo test`**.

## Antes de dar por terminado un cambio de UI

- ¿`cargo test` pasa? Los de contraste son los que importan aquí, y ahora hay dos
  —entero y atenuado—.
- **¿Lo has mirado?** Un cambio visual que solo has razonado no está terminado.
  Lanza la app (`FLOW_DEMO=8`) y mírala; para lo fino —una curva de 1 px, el peso
  de un trazo— haz una captura y amplíala, que a tamaño real no se distingue.
- ¿Se ve bien **con ocho paneles**, no solo con dos? Es el caso que rompe cosas.
- ¿Y con la ventana estrecha en vertical? El reparto se recalcula solo, la columna
  se encoge a números y lo que cabía deja de caber.
- ¿Lo que dibujaste a mano tiene su `widget_info`?
- ¿Metiste algún color que no signifique nada?
- ¿Tapaste el grano con algún relleno opaco nuevo?
- ¿Sigue leyéndose como ventanas en mosaico, o se ha convertido en una tabla?
- ¿Lo destructivo sigue lejos del ratón?
- ¿Actualizaste el comentario que defendía la decisión que acabas de cambiar?
