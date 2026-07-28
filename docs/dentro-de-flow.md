# Un agente dentro de flow

Un agente lanzado en flow es un proceso de terminal normal y corriente, así
que **no hay nada que integrar**. Lo que sigue es lo que flow le ofrece de más
a quien quiera aprovecharlo: saber dónde está y poder pedir que le abran un
panel al lado.

## Lo que le dice el entorno

Un agente lanzado aquí no sabe que está aquí: para él esto es una terminal
cualquiera. flow se lo dice por el entorno, y de paso le da algo que en una
terminal normal no existe —*ábreme esto al lado*—:

| Variable          | Qué es                                             |
| ----------------- | -------------------------------------------------- |
| `FLOW`            | `1`. Estás dentro de flow                          |
| `FLOW_SESSION`    | Nombre de tu sesión                                |
| `FLOW_SESSION_ID` | Su identificador                                   |
| `FLOW_DIR`        | El directorio que comparten sus paneles            |
| `FLOW_INBOX`      | El buzón: por aquí se piden paneles                |
| `FLOW_BIN`        | La ruta del propio flow, por si no está en el PATH |
| `FLOW_PANES`      | Cuántos caben por sesión                           |
| `FLOW_HOWTO`      | Todo lo anterior explicado en prosa, para el modelo |

### `flow run`

Para abrir un panel en tu propia sesión:

```
flow run cargo test
flow run npm run dev
```

Y si flow no está en el `PATH` —porque te copiaste el `.exe` a una carpeta
cualquiera, que es una forma legítima de tenerlo— la misma llamada por su ruta,
que llega en `FLOW_BIN`.

**`flow run` no ejecuta nada**: escribe la petición y se va. El que lanza el
proceso es flow, en su propio PTY, que es lo que lo convierte en un panel de
verdad y no en la salida de un proceso colgando de otro. De ahí lo único que hay
que entender para usarlo bien: **la salida no vuelve a quien lo pidió**, se ve en
el panel.

Eso reparte el trabajo solo:

- Lo corto, y cuya respuesta el agente **necesita leer** para seguir —`git
  diff`, un typecheck— se queda donde estaba: en su propia herramienta.
- Lo que dura o interesa mirar —un servidor, la suite larga, seguir un log, un
  subagente— va a `flow run`, y se ve trabajar al lado.

### Por debajo es un fichero

`flow run` solo escribe el comando en un fichero nuevo dentro de `FLOW_INBOX`, y
eso sigue siendo el protocolo: se puede hacer a mano.

```
echo cargo test > "%FLOW_INBOX%\1.cmd"     :: Windows
echo cargo test > "$FLOW_INBOX/1.cmd"      # el resto
```

flow lo lee cada 300 ms, borra el fichero y abre el comando como un panel más
al lado, en el mismo directorio y con el mismo entorno. Un fichero, un panel.

Es un directorio y no un puerto, un socket o un binario auxiliar porque
cualquier cosa sabe escribir un fichero —un agente, un script, un `echo` a
pelo— en cualquier lenguaje y en los dos sistemas, sin que flow tenga que abrir
nada al exterior. El buzón vive en el temporal del sistema, lleva el PID de flow
y se borra al cerrar la sesión.

El subcomando existe porque el `echo` se usaba poco y mal: había que acordarse de
la ruta del buzón, inventarse un nombre de fichero que no chocara con otro y
acertar con la redirección y las comillas, que no se escriben igual en `cmd`, en
PowerShell y en un shell de Unix. Y cuando fallaba, fallaba en silencio: el
fichero se escribía con el comando a medias y lo que aparecía al lado era un
panel con un error raro. `flow run` además escribe fuera del buzón y mueve el
fichero dentro de un tirón, así que flow no puede leerlo a medio escribir.

**Lo que flow no puede hacer** es obligar a un agente de terceros a usarlo:
puede ofrecer el mecanismo y anunciarlo, pero no meterse en el prompt de otro
programa. Para que lo use de verdad, dile en su fichero de contexto —el
`CLAUDE.md` o `AGENTS.md` del proyecto— algo así:

```markdown
Si la variable FLOW está puesta, estás dentro del orquestador flow. Lee
FLOW_HOWTO. Todo proceso que dure o que interese mirar (servidores, suites de
tests largas, builds, seguir un log, subagentes) lánzalo con `flow run <comando>`
—o "%FLOW_BIN%" run <comando>— para que se vea en una terminal de esta misma
sesión en vez de ejecutarlo donde nadie lo ve. Ojo: la salida de `flow run` no
vuelve a ti, se queda en su panel, así que lo que necesites leer para seguir
trabajando ejecútalo como siempre.
```

Un aviso: quien pueda escribir en el buzón puede hacer que flow lance procesos.
Está en el temporal del usuario, así que no da a nadie nada que no tuviera ya
—cualquier proceso tuyo puede lanzar procesos—, pero conviene saberlo.


---

Volver al [README](../README.md) · Cómo está montado flow: [por dentro](arquitectura.md)
