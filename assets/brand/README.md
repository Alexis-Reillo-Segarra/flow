# Marca de flow

Icono definitivo: la silueta de una **F** formada por cuatro barras inclinadas
(`skewX(-16°)`), alineadas arriba, de más a menos altura y de más a menos
contraste. La caída de contraste es la que da la sensación de *flow*.

Origen: proyecto «Flow Logo Design» en Claude Design
(`699d769a-9a01-4c66-a1c1-9b24890613a5`, fichero `Flow Icon.dc.html`).

## Ficheros

| Fichero | Uso |
| --- | --- |
| `flow-icon-dark.svg` | Icono completo sobre fondo oscuro, 280×280, radio 44 |
| `flow-icon-light.svg` | Icono completo sobre fondo claro, con borde de 1px |
| `flow-mark-dark.svg` | Solo la marca, fondo transparente, grises para oscuro |
| `flow-mark-light.svg` | Solo la marca, fondo transparente, grises para claro |
| `flow-favicon.svg` | Versión ajustada a 32px (barras más gruesas en proporción, radio 7) |
| `flow.ico` | Icono de Windows: 16/32/48 desde el favicon, 256 desde el icono grande |
| `png/` | Rasterizados: iconos a 1024/512/256/128/64, marcas a 512, favicon a 16/32/48 |
| `windows/` | El recurso que le da al `.exe` su icono y su ficha de propiedades |

Los SVG son la fuente. Los PNG y el ICO se derivan de ellos, no al revés.

## Dónde se usa cada cosa

Nada de esto lo carga la aplicación en tiempo de ejecución. **Dentro de flow la
marca se dibuja en código**, en [`src/logo.rs`](../../src/logo.rs), que es esta
misma geometría escrita en Rust: así se rasteriza al número exacto de píxeles que
va a ocupar sea cual sea la escala del sistema, y se puede pintar con el acento
del tema activo, que es lo que hace en la barra de título.

Los ficheros de aquí son para lo que flow no dibuja:

| Fichero | Quién lo lee |
| --- | --- |
| `windows/flow.res` (que lleva dentro `flow.ico`) | Windows, para el icono del `.exe` en el Explorador y en la barra de tareas anclada. Lo enlaza `build.rs` |
| `png/flow-icon-dark-256.png` | La cabecera del README |
| Los SVG | La fuente de todo lo anterior, y cualquier sitio que necesite la marca |

Que la geometría esté escrita dos veces es a propósito —los consumidores son
distintos— y lo que las ata es un test:
`logo::tests::la_marca_tiene_la_proporcion_de_los_svg` comprueba que la caja de
la marca dibujada sigue siendo la de `flow-mark-dark.svg` y la de
`flow-favicon.svg`. Si cambias la geometría aquí, cámbiala allí.

### Regenerar los rasterizados

Con [`@resvg/resvg-js`](https://github.com/yisibl/resvg-js) fuera del repo (esto
es un proyecto Rust, no metemos `node_modules` aquí):

```
npm install @resvg/resvg-js
node render.js
```

El ICO se ensambla a mano: cabecera `ICONDIR`, una `ICONDIRENTRY` por tamaño y
los PNG concatenados detrás (Windows admite PNG dentro de ICO desde Vista).
El campo de ancho/alto va a `0` para el tamaño de 256.

## Geometría (lienzo de 280)

Barras de 26 de ancho, separación de 8, todas empezando en `y = 80`.
Alturas 120 / 88 / 56 / 30. La inclinación desplaza cada barra
`tan(16°) · altura / 2` a la derecha arriba y lo mismo a la izquierda abajo,
por lo que la marca ocupa de `x = 58.8` a `x = 208.3`.

En 32px la proporción cambia a propósito: barras de 3.5, separación 1.5,
alturas 15 / 11 / 7 / 4. A ese tamaño las proporciones grandes se empastan.

## Color

Grises puros. Los valores de diseño están en OKLCH con croma 0; aquí queda la
conversión a sRGB que usan los SVG.

| | Fondo | Barra 1 | Barra 2 | Barra 3 | Barra 4 |
| --- | --- | --- | --- | --- | --- |
| Oscuro | `#090909` (L 0.14) | `#eeeeee` (0.95) | `#ababab` (0.74) | `#717171` (0.55) | `#424242` (0.38) |
| Claro | `#f5f5f5` (0.97) | `#0b0b0b` (0.15) | `#424242` (0.38) | `#7a7a7a` (0.58) | `#ababab` (0.74) |
| Favicon | `#090909` | `#eeeeee` | `#b1b1b1` (0.76) | `#7a7a7a` (0.58) | `#4d4d4d` (0.42) |

Borde del icono claro: `#cecece` (L 0.85).

## Nota

La marca no está centrada ópticamente en el lienzo de 280: por la inclinación
queda unos 6.5px a la izquierda del centro geométrico. Es lo que produce el
diseño aprobado y se ha respetado tal cual. Si algún día molesta, se corrige
desplazando el grupo `+6.45` en `x`.
