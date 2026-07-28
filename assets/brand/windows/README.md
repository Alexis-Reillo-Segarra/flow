# Recursos del ejecutable de Windows

`flow.rc` describe dos cosas que Windows lee **del fichero `.exe`**, sin abrirlo:
el icono que sale en el Explorador, en el escritorio y en la barra de tareas
anclada, y la ficha de *Propiedades → Detalles*. No tiene nada que ver con el
icono de la ventana, que lo dibuja `src/logo.rs` en tiempo de ejecución.

`flow.res` es ese mismo fichero ya compilado, y **va versionado a propósito**:
así `cargo build` sigue sin necesitar el SDK de Windows ni ninguna caja de
construcción. `build.rs` se lo pasa al enlazador con `rustc-link-arg-bins`
cuando el objetivo es `*-pc-windows-msvc`.

## Regenerarlo

Hace falta cuando cambia el icono (`../flow.ico`) o cuando cambia la versión del
`Cargo.toml`. Lo segundo lo comprueba `build.rs` y falla el build si se olvida;
lo primero no lo puede comprobar nadie, así que va aquí escrito.

```
rc.exe /nologo /fo flow.res flow.rc
```

`rc.exe` viene con el SDK de Windows y no está en el `PATH`. Sale de:

```
C:\Program Files (x86)\Windows Kits\10\bin\<versión>\x64\rc.exe
```

o directamente en el `PATH` si abres una consola *Developer Command Prompt for
VS*. Después, comprueba que el `.exe` recién compilado lleva el icono:

```
cargo build --release
explorer target\release
```
