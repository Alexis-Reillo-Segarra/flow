# El diseño de flow

Por qué la interfaz es como es. Si solo quieres usar flow, con el
[README](../README.md) tienes de sobra: esto es el registro de las decisiones
y de lo que costó cada una.

La referencia no es una app de escritorio, es un **gestor de ventanas en
mosaico**: ventanas que se reparten el espacio solas, separadas por hueco y no
por marcos, con la activa marcada por su borde.


## La rejilla

El espacio no se reparte con una tabla de N×M sino cortando el rectángulo
disponible **por su lado largo**, una y otra vez, hasta que hay un hueco por
panel. La mitad con menos paneles va primero, así que el agente más antiguo se
queda el hueco grande.

De ahí sale la única propiedad que se le pide: **se adapta sola**. Los mismos
ocho paneles caen en 4×2 con la ventana en horizontal y en 2×4 si la estrechas,
sin nada que configurar y sin que la app sepa nada de resoluciones. Maximizada,
a media pantalla o en un monitor vertical es el mismo código.

Los paneles no saltan de sitio: cada uno persigue su hueco con un suavizado
exponencial de unos 165 ms. Abrir o cerrar uno reorganiza la rejilla deslizando,
que es la diferencia entre entender lo que pasó y encontrarte otra pantalla.

El tope de **ocho por sesión** no es técnico: repartida una pantalla normal entre
más, cada panel deja de tener columnas suficientes para que la salida de un CLI
se lea sin partirse. Sesiones puede haber las que quieras.

## Espacio

La estructura se lee por el **hueco**: los mismos 8 px separan un panel del de al
lado y del borde de la ventana. Un panel lleva 6 px de redondeo —lo justo para
que se lea como una ventana suelta y no como la celda de una tabla— y todo lo que
va dentro sigue con esquina viva, para que el redondeo signifique una cosa
concreta: esto es una ventana.

Quién tiene el foco lo dice el marco: el activo lo lleva en un degradado de las
dos claridades del blanco de marca, a 45°, y los demás en gris. Como refuerzo, el
**contenido** de los otros siete baja un 10% —solo la tinta; el marco y las
divisorias se quedan enteros, porque una rejilla con siete bordes medio borrados
se deshace—.

Ese 10% es deliberadamente poco. Atenuar de verdad siete procesos para destacar
uno sale más caro de leer de lo que vale: están los ocho en pantalla porque la
promesa es que los ves trabajar a todos. Y no sale gratis: los dos niveles de
gris más flojos se subieron para que **atenuados** sigan cumpliendo AA, así que
un panel apagado se lee hoy igual de bien que se leía toda la interfaz antes de
que esto existiera. Lo que el atenuado hace, en realidad, no es oscurecer siete:
es aclarar el que te escucha.

## Color

Lo que sigue describe el tema de casa. Los otros cambian los valores, pero no la
estructura: los papeles son los mismos en los cinco y ninguno estrena un color
que no signifique nada (ver [temas](../README.md#temas)).

El tema de casa es **monocromo entero**: no tiene un solo color de interfaz.
Tres valores llevan el peso: **negro puro** de fondo, **gris** en las divisorias
de 1 px y el **blanco de la marca** (`#FFFFFF`) en todo lo que es flow hablando
—el logo, el marco del panel con el foco, el cursor—. No hay tarjetas grises ni
rellenos decorativos: la estructura se lee por las líneas, no por bloques de
tono.

No es una elección de gusto suelta: es la regla «si ves color en flow, significa
algo» llevada a donde ya no hace falta enunciarla. Lo único con tono que puede
salir en pantalla son los dieciséis colores del terminal, así que **si ves
color, no lo ha puesto flow: lo ha escrito un proceso**. Lo comprueba
`theme::tests::el_tema_de_casa_no_tiene_un_solo_color_de_interfaz`.

Los grises tampoco tienen tono. Los de antes tiraban a azul un par de puntos
—`#c6cbd2`, `#3c424b`—, que es un matiz que funciona cuando la marca aporta el
color y se nota en cuanto no: sin nada a lo que arrimarse, un gris azulado no se
lee como neutro con carácter, se lee como una desviación.

El negro es `#000000` exacto, y es una decisión, no una casualidad: en un panel
OLED ese valor es el píxel **apagado**, un negro que ninguna otra pantalla sabe
dar. Subirlo aunque fuera un punto lo encendería entero. Es también la razón de
que el tema de casa siga siendo el de fábrica y de que ninguno de los otros
cuatro le toque el fondo: un tema es justo eso, y este no lo cambia.

## Grano y profundidad

Un negro absoluto y liso a pantalla completa no le da al ojo ninguna referencia,
y deja de saber si mira una superficie o un agujero. Por eso el fondo lleva una
capa de **grano**: motas de blanco a un 6% de opacidad como mucho, así que la
inmensa mayoría de los píxeles siguen siendo negro exacto y el OLED sigue
apagado. El grano vive solo en el fondo —huecos, barra y columna—; los paneles se
rellenan de negro liso encima, y la salida de un proceso no se lee jamás sobre
ruido.

Los paneles llevan además un **halo**: la sombra de un gestor de ventanas, pero
invertida. Hay una razón física para ello —una sombra oscurece lo que tiene
debajo, y sobre `#000000` no queda nada que oscurecer—, así que la profundidad se
da al revés, con un desvanecido de luz hacia fuera del panel. El efecto para el
ojo es el mismo que busca la sombra de Hyprland: que la ventana se lea despegada
del fondo y no recortada sobre él. El del panel con foco lleva el acento, como la
sombra coloreada de la ventana activa de allí; no es una señal más, es el borde
derramándose.

## La marca

Cuatro barras inclinadas 16°, alineadas arriba, **de más a menos altura y de más
a menos contraste**. Se leen como la silueta de una `F` y como algo que se
desvanece hacia la derecha; esa caída es la idea entera, y es la misma que
gobierna la interfaz: aquí también hay cuatro niveles de texto y lo que importa
es el primero.

Tres decisiones, y las tres son la regla de la casa aplicada a un logotipo:

- **Sin color.** La marca vive en la escala de grises, como el tema de casa. En
  la barra de título se pinta con el acento del tema —blanco en flow, mauve en
  Catppuccin— y las cuatro barras son ese mismo color a cuatro opacidades, así
  que la caída se conserva en cualquier tema sin tener cuatro colores que
  mantener. La tinta es `accent_text`, la cara **clara** del acento: la marca ya
  trae su propia caída dentro, y empezarla en la cara de trazo la apagaría dos
  veces y dejaría las dos últimas barras invisibles sobre el negro.
- **Sin letras.** No hay ninguna tipografía dentro: si la hubiera, la marca
  dependería de que esa fuente esté, y a 16 px una letra de contorno se empasta.
  La `F` la dibuja el hueco.
- **Distinta a 16 px que a 256.** La versión pequeña engorda la barra y abre la
  separación (3,5 y 1,5 sobre 32, contra 26 y 8 sobre 280), porque las
  proporciones del icono grande se empastan a ese tamaño. Es la corrección que
  hace cualquier tipografía entre un titular y un cuerpo de 8, y `logo.rs` cambia
  de juego de proporciones solo, según los píxeles que le pidan.

La geometría está escrita dos veces —en los SVG de `assets/brand/` y en
`src/logo.rs`— porque los consumidores son distintos: el `.exe` necesita un
fichero y la ventana necesita píxeles a la escala del sistema. Un test comprueba
que la caja de la marca dibujada sigue siendo la del SVG, que es lo que las ata.

## Las marcas de los agentes

Cada agente conocido lleva una forma —el destello, el anillo, los corchetes— en
su botón del formulario y en la cabecera de su panel, para que en una rejilla de
ocho se vea de un golpe cuál es el `claude` y cuál el shell.

**No son los logotipos oficiales.** flow no empaqueta recursos de marca ajenos:
son formas geométricas propias, dibujadas con segmentos como todo lo demás,
elegidas para evocar a cada agente y, sobre todo, para distinguirse entre ellas a
diez píxeles, que es el trabajo que tienen que hacer.

El blanco de marca vive en dos claridades, que se eligen por lo que va a pintar,
no por gusto:

| Campo         | Valor     | Contraste | Dónde                                                          |
| ------------- | --------- | --------- | -------------------------------------------------------------- |
| `accent`      | `#6E6E6E` | 4,12:1    | Marcos, trazos, rellenos; extremo oscuro del degradado          |
| `accent_text` | `#FFFFFF` | 21,00:1   | Rótulos, glifos finos, el bloque del cursor, la marca; extremo claro |

La razón es de contraste: la WCAG le pide 3:1 a un componente de interfaz y
4,5:1 a un texto. `#6E6E6E` cumple lo primero pero no lo segundo, así que en
cuanto el acento tiene que ser una letra se sube a la variante clara. El test
`theme::tests::el_acento_de_flow_cumple_lo_de_un_componente_de_interfaz` impide
que alguien lo use de color de texto sin enterarse.

Las dos caras no se escriben por separado: la oscura es la clara escalada por
0,43, y de ahí que compartan tono exacto —escalar los tres canales por igual no
lo mueve—. Un tema propio declara solo `accent` y la otra sale sola. En un tema
monocromo eso es trivialmente cierto, pero en los cuatro portados es lo único que
impide que alguien acabe con dos acentos que no se leen como el mismo color.

El degradado del panel con foco va de una a la otra: los dos extremos pasan de
sobra el 3:1 de un trazo, así que el marco cumple **en todo su recorrido** y no
solo en un extremo. Lo comprueba
`theme::tests::el_degradado_del_foco_es_trazo_valido_de_punta_a_punta`.

Que el acento sea blanco puro tiene dos consecuencias, y las dos son a propósito:

- **`text_hi` baja a `#E6E6E6`.** El blanco pasa a ser de la marca; si lo llevara
  también el texto destacado, no quedaría nada que separase el foco de un rótulo
  cualquiera. En un tema con acento de color esa separación la hace el tono; aquí
  solo puede hacerla la claridad, así que el resto de la interfaz se queda por
  debajo del blanco.
- **El slot ANSI 15 se mueve a `#F2F2F2`.** Ningún slot puede ser exactamente el
  acento —lo comprueba `theme::tests::ningun_slot_ansi_es_el_de_la_marca`—, y
  aquí el que choca es el "blanco brillante" del terminal. Se mueve el slot y no
  el acento, como en los temas portados: dos puntos de blanco no se ven en la
  salida de un proceso, y el acento sí tiene que ser el blanco de verdad.

Los cuatro estados también son grises, ordenados por cuánto te reclama cada uno.
Los campos siguen llamándose `green`, `amber` y `red` porque nombran el papel y
no el tono —y porque son el formato del fichero de configuración, donde los otros
cuatro temas sí ponen verde, ámbar y rojo de verdad—:

| Campo   | Valor     | Contraste | Estado                                           |
| ------- | --------- | --------- | ------------------------------------------------ |
| `red`   | `#F0F0F0` | 18,43:1   | `EXIT` con código ≠ 0 y `FAILED`                 |
| `amber` | `#C0C0C0` | 11,54:1   | `BLOCKED` — el único estado que reclama atención |
| `green` | `#969696` | 7,10:1    | `WORKING` (latiendo) y `DONE` (sólido)           |
| `slate` | `#868686` | 5,77:1    | `IDLE`                                           |

Que un estado se pueda decir en gris no es una concesión: el color **nunca** fue
la única señal. Van cuatro canales a la vez, y el color siempre fue el
prescindible —es el que no le llega a quien no distingue rojo de verde—:

- **La palabra**, siempre al lado: `WORKING`, `BLOCKED`, `IDLE`, `DONE`, `EXIT`,
  `FAILED`.
- **La forma**: `IDLE` va hueco y el resto sólido, que es lo que separa `DONE` de
  `IDLE` ahora que son dos grises parecidos.
- **El ritmo**: `WORKING` late, `BLOCKED` parpadea, los terminales están quietos.
- **La claridad**, que es lo que aquí sustituye al tono, ordenada por daño: el
  error arriba, el aviso debajo, luego lo que va bien y `IDLE` el más apagado.

Ese orden no es el que parece obvio. Mirando solo las marcas de estado saldría al
revés —quien te reclama es `BLOCKED`, no un proceso que ya murió—, pero estos dos
campos no pintan solo marcas: `red` es además el botón `KILL`, la X de cerrar y
los mensajes de error, y `amber` es el `^C` y los avisos del formulario. Con tono
se podían tener las dos cosas, porque el rojo y el ámbar se separaban sin
ordenarse; en una sola dimensión hay que elegir, y pierde la marca de estado,
que es la que tiene con qué compensar: `BLOCKED` parpadea, y el parpadeo gana a
cualquier claridad.

El par que de verdad depende de la claridad es `DONE` contra `EXIT`/`FAILED`: los
dos son sólidos y quietos, así que solo los separan la claridad y la palabra. Por
eso son los dos que más se han apartado, 2,60:1 entre ellos.

Y por eso el test de estados acepta ahora dos formas de cumplir, que son la misma
idea sobre paletas distintas: **25° de tono** —lo que hacen los cuatro temas de
color, y se pide tono y no claridad porque quien no separa rojo de verde los
tiene casi a la misma luminancia: en Nord se quedan en 1,02:1 entre ellos— **o
1,5:1 de claridad**, lo único que le queda a un tema sin tono. Lo que no vale es
no cumplir ninguna de las dos.

## Tipografía

Dos familias, cada una con un trabajo:

| Familia                                                     | Uso                                 | Licencia |
| ----------------------------------------------------------- | ----------------------------------- | -------- |
| [Inter](https://rsms.me/inter/)                             | Nombres, estados, botones, rótulos  | OFL      |
| [JetBrains Mono](https://www.jetbrains.com/lp/mono/)        | Salida de procesos, rutas, comandos | OFL      |

Inter es un grotesco neutro dibujado para leerse en pantalla a cuerpos pequeños.
No aporta carácter, y eso es exactamente lo que se le pide: el carácter lo pone
la rejilla, no la letra. JetBrains Mono lleva todo lo que sale de un proceso,
porque el terminal exige monoespaciada, y de paso las cifras y las rutas del
chrome, donde importa distinguir `l` de `1`.

Las dos traen Latin-1 completo, así que no hay texto que no se pueda escribir, y
las dos son de contorno: no hay retícula de píxeles que respetar.

## Por qué la escala la decide el sistema

Aquí hubo antes dos fuentes de pixel-art atadas a su ladrillo, y de ahí salía
media docena de reglas raras: la escala tenía que ser un número entero, existía
un selector `1× 2× 3×` en la barra de título y luego una heurística que lo
elegía por el tamaño de la ventana. Con Inter y JetBrains Mono, que son de
contorno, nada de eso hace falta.

Hoy `app::auto_scale` hace lo que hace cualquier aplicación: **seguir al
sistema**. Si tienes el escritorio al 150%, flow va al 150%.

La única corrección es para pantallas grandes con el escalado del sistema en
100% —un 4K donde 13 puntos son 13 píxeles de nada—: ahí se agranda un 50%. Se
mira el tamaño del **monitor** y no el de la ventana a propósito, porque es un
dato que no cambia mientras arrastras el borde: redimensionar tiene que enseñar
más terminal, no letra más grande. Los tres casos están fijados en
`app::tests`.

La marca sí se sigue rasterizando en código, y ahora por dos razones más
simples. Una es de nitidez: en la barra de título ocupa 16 puntos, donde cada
barra mide dos o tres píxeles y el hueco entre ellas uno, y escalar una imagen
para llegar ahí lo convertiría en una mancha gris. Se genera al número exacto de
píxeles que va a ocupar, sea cual sea el factor, y se cachea. La otra es de
color: ahí la marca se pinta **con el acento del tema**, así que no hay un color
que se pueda hornear en un fichero.

Los ficheros de `assets/brand/` —los SVG, sus PNG y el `.ico`— son para lo que
flow no dibuja: el icono del `.exe`, que lo lee el Explorador de Windows sin
abrir el programa, y el README. La geometría está escrita en los dos sitios y hay
un test que comprueba que la caja de la marca dibujada es la del SVG.

## Movimiento

Hay exactamente seis cosas que se mueven, y ninguna es decorativa: o dicen dónde
estaba algo que se ha movido, o dicen que un proceso está vivo.

| Qué                     | Ritmo                            | Para qué                                    |
| ----------------------- | -------------------------------- | ------------------------------------------- |
| Reparto de la rejilla   | τ = 55 ms (95% en ~165 ms)       | Ver a dónde se fue cada panel                |
| Panel recién abierto    | 160 ms, de 94% a 100% y opacidad | Que nazca en vez de aparecer                 |
| Marca `WORKING`         | Seno, ciclo de 1,6 s             | Latido: hay algo pasando                     |
| Marca `BLOCKED`         | Cuadrada, 1,25 Hz                | Aviso: te está esperando                     |
| Subrayado de la pastilla | Cuadrada, 1,25 Hz                | Lo mismo, para sesiones que no estás mirando |
| Cursor del panel activo | 1,6 Hz                           | Dónde vas a escribir                         |

Tres decisiones que sostienen esto:

- **El deslizamiento va con el `dt` real**, no con un paso fijo por frame, así
  que dura lo mismo a 60 Hz que a 144.
- **Los parpadeos van a 1,25 Hz**, muy por debajo del límite de 3 destellos/s de
  la WCAG 2.3.1. Importa porque un `BLOCKED` puede estar en pantalla durante
  minutos.
- **El cursor solo parpadea en el panel con el foco.** En los otros siete se
  queda como una marca apagada al 30%: ocho cursores parpadeando a destiempo es
  justo el ruido que esta interfaz intenta no tener.

Mientras algo se mueve se pide repintado; cuando todo llega a su sitio y ningún
proceso está vivo, la app deja de dibujar y no consume CPU. El PTY no se
redimensiona hasta que el panel ha dejado de moverse: sería un `ioctl` y una
rejilla nueva por frame de animación para acabar exactamente donde iba a acabar.

## Accesibilidad

**Contraste.** Todo lo que es texto cumple WCAG AA (4,5:1) contra el fondo, y no
por confianza: lo comprueba `theme::tests::todo_lo_que_es_texto_cumple_aa`, que
recorre la paleta entera de **cada tema** y calcula la luminancia relativa. Si
alguien oscurece un color por debajo del mínimo, falla el `cargo test`.

Y lo cumple **también atenuado**, que es el caso real peor: el nivel más flojo de
uno de los siete paneles sin foco. Eso lo comprueba un segundo test,
`el_texto_de_un_panel_apagado_tambien_cumple_aa`, y es el que fija el techo del
atenuado: `text_faint` se queda en 4,69:1 al apagarse en el tema de casa, a un
pelo del mínimo y el peor de los cinco temas. Un tercer test guarda el propio
0,90 dentro de un rango acordado, para que bajarlo haga ruido al pasar los tests
y no dos semanas después.

Atenuar no es oscurecer: `dim_inactive` baja también la alfa, así que lo que se
ve es el color **mezclado con el fondo del panel**. Sobre el negro de casa daba
lo mismo —mezclar con negro es oscurecer—, pero sobre el fondo de un tema con
color no, y el test mide lo que se ve.

Las divisorias se quedan en 2,09:1 a propósito: son decoración, no transmiten
información, y subirlas más convertiría la rejilla en lo más ruidoso de la
pantalla. El acento se queda en 4,12:1 por la misma lógica —es marco y relleno,
nunca letra— y tiene su propio test que lo mantiene fuera del texto.

Los números de abajo son los del tema de casa. Los otros cuatro pasan los mismos
mínimos con los suyos:

|                             | vs negro                    | apagado                     |                            |
| --------------------------- | --------------------------- | --------------------------- | -------------------------- |
| `text`                      | 12,94:1                     | 10,47:1                     | AA                         |
| `text_hi`                   | 16,83:1                     | 13,48:1                     | AA                         |
| `text_dim`                  | 8,03:1                      | 6,58:1                      | AA                         |
| `text_faint`                | 5,61:1                      | 4,69:1                      | AA (mínimo permitido)      |
| `accent_text`               | 21,00:1                     | 16,83:1                     | AA                         |
| `accent`                    | 4,12:1                      | —                           | AA solo como componente    |
| `red` / `amber` / `green` / `slate` | 18,43 / 11,54 / 7,10 / 5,77 | 14,73 / 9,36 / 5,85 / 4,82 | AA           |
| `line`                      | 2,09:1                      | —                           | decoración, no informa      |

`accent` y `line` no llevan columna de apagado porque nunca la ven: el acento
solo pinta el marco del panel con foco, que por definición no se atenúa, y las
divisorias son estructura y se quedan enteras en los ocho.

**El color nunca va solo**, y en el tema de casa ni siquiera hay color: cada
estado se comunica por cuatro canales a la vez —la palabra al lado, la forma
(relleno o hueco), el ritmo (late, parpadea o está quieto) y la claridad—. Quien
no distinga verde de ámbar sigue leyendo `WORKING` y `BLOCKED`, y ahora lo tiene
igual de fácil que todo el mundo, porque la señal que quedaba fuera de su alcance
ya no existe.

**Parpadeo.** El indicador de `BLOCKED` parpadea a 1,25 Hz, muy por debajo del
límite de 3 destellos/segundo de la WCAG 2.3.1. Importa porque puede estar en
pantalla durante minutos.

**Lectores de pantalla.** Media interfaz está pintada a mano con el `Painter`, y
eso para AccessKit no existe. Los botones, los controles de ventana, las
pastillas de la barra y las cabeceras de panel declaran su semántica
explícitamente; un panel se anuncia como "3, cargo, BLOCKED, 12s", con el número
y el estado dentro del nombre.

**Limitación conocida:** no hay navegación por `Tab`, y aquí no la puede haber
en el sentido de siempre: `Tab` es del proceso —completa rutas— desde que se
escribe directamente en el panel. Los widgets pintados a mano tampoco entran en
el orden de foco de egui. Las rutas principales sí son accesibles por teclado
(`Ctrl-N`, `Ctrl-T`, `Ctrl-Shift-T`, `Ctrl-W`, `Ctrl-1`…`9` —`Cmd` en macOS—,
`Alt-1`…`8`, `Alt`+flechas, y escribir va siempre al panel con el foco), pero
llegar a los botones de la barra inferior requiere ratón. Los dos que más importan tienen
tecla propia dentro del proceso, que es donde se esperan: `^C` es `Ctrl-C` y
`ESC` es `Esc`. El selector de temas se maneja entero con flechas, `Enter` y
`Esc`.


## Un tema es un contrato, no una lista de gustos

Los cinco pasan los mismos tests, y **todos los temas** los pasan, no solo el
activo:

1. Los cuatro niveles de texto cumplen 4,5:1 contra **su** fondo, y también
   atenuados —con `dim_inactive`, el nivel más tenue de un panel sin foco es el
   peor caso real de la interfaz—.
2. El acento tiene dos caras, una de trazo (≥3:1) y una de texto (≥4,5:1), **con
   el mismo tono**. No se escriben las dos: la de trazo sale de escalar la otra,
   y escalar los tres canales por igual no mueve el tono.
3. Los cuatro colores de estado cumplen 4,5:1 y se separan entre sí por tono
   (25°) **o** por claridad (1,5:1). Un tema de color se separa por tono, porque
   quien no distinga rojo de verde los tiene casi a la misma luminancia; uno
   monocromo no tiene tono que separar y le queda la claridad.
4. Ningún slot ANSI es exactamente el acento, y el cian sigue siendo cian.

De ahí salen las únicas licencias que se han tomado con las paletas originales:
el rojo de Gruvbox y el de Nord no llegaban a 4,5:1 sobre su propio fondo y se
han aclarado lo justo. El acento de Gruvbox tampoco es su naranja de siempre,
porque se quedaba a 13° de tono de su propio amarillo de aviso y el acento habría
dejado de distinguirse de un panel `BLOCKED`; se usa el aqua, que está a 100°.

## Escribirse uno

El fichero es `%APPDATA%\flow\config` —o `~/.config/flow/config` fuera de
Windows, macOS incluido—, y el selector te enseña su ruta abajo del todo. flow lo
escribe la primera vez que guardas un tema, ya comentado con todas las claves.

```ini
theme = mío

[theme mío]
base   = gruvbox      ; hereda todo lo suyo
accent = #d3869b      ; menos el acento, que aquí es rosa
```

**Lo que no digas se hereda de `base`**, y esa es toda la gracia del formato: un
tema son veintitantos colores, y obligar a escribirlos todos para cambiar uno
habría hecho que nadie escribiera ninguno. Las claves son las de la paleta —`bg`,
`raised`, `sel`, `hover`, `line`, `line_hi`, `text`, `text_hi`, `text_dim`,
`text_faint`, `accent`, `accent_stroke`, `green`, `amber`, `red`, `slate` y
`ansi0`…`ansi15`—, los valores van en `#rrggbb` o `#rgb`, un `#` al principio de
línea es un comentario y `;` abre una nota al final de una que ya dice algo.

Dos cosas que conviene saber:

- **`base` tiene que ir antes que los colores**, porque es lo que decide desde
  dónde se parte.
- **Tu tema no pasa por los tests de contraste**, y no puede: se lee al arrancar,
  no al compilar. Por eso heredar de uno de los cinco y cambiar poco es lo que
  hace que empieces cumpliendo. Si el fichero tiene una errata, flow no se cae:
  la línea se descarta, el aviso sale por la salida de errores —lo ves si lo
  lanzaste desde una terminal— y todo lo demás se carga igual.


---

Volver al [README](../README.md) · Cómo está montado flow: [por dentro](arquitectura.md)
