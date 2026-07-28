// En release no queremos que Windows abra una consola detrás de la ventana.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod agent;
mod app;
mod config;
mod keys;
mod logo;
mod presets;
mod projects;
mod repos;
mod run;
mod session;
mod term;
#[cfg(test)]
mod testkit;
mod theme;
mod ui;

fn main() -> eframe::Result<()> {
    // `flow run` y compañía, antes que nada: son subcomandos que escriben una
    // línea y se van, y no tienen por qué pagar el arranque de una ventana ni
    // leer la configuración de temas de nadie.
    if let run::Cli::Done(code) = run::cli() {
        std::process::exit(code);
    }

    // Los temas, antes de que nada pinte: la lista es los incluidos más los que
    // haya escritos en el fichero de configuración, y se instala una sola vez
    // porque un índice de tema tiene que significar lo mismo durante toda la
    // ejecución.
    //
    // Lo que no se entienda del fichero se dice por la salida de errores y no
    // para nada. Es visible para quien lanzó flow desde una terminal —que es
    // quien acaba de editar el fichero— y no le estropea la ventana a nadie más.
    let base = theme::builtin();
    let cfg = config::load(&base);
    let mut themes = base;
    themes.extend(cfg.customs);
    theme::install_themes(themes);
    for aviso in &cfg.warnings {
        eprintln!("flow: config: {aviso}");
    }
    if let Some(name) = cfg.theme.as_deref() {
        match theme::index_of(name) {
            Some(i) => theme::set_active(i),
            None => eprintln!("flow: config: no hay ningún tema que se llame '{name}'"),
        }
    }

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("flow")
            // Ancha de casa: la rejilla corta por el lado largo, así que el
            // ancho es lo que decide cuántos agentes caben uno al lado del otro
            // sin que ninguno se quede sin columnas. La altura se queda corta a
            // propósito, para que en una pantalla de 1080p la barra de entrada
            // no acabe debajo de la barra de tareas.
            //
            // El mínimo es mucho menor porque por debajo la interfaz se aprieta
            // pero sigue funcionando entera: los paneles se reordenan solos (ver
            // `ui::tiles::split`) y la columna de sesiones se queda en números.
            .with_inner_size([1480.0, 900.0])
            .with_min_inner_size([760.0, 460.0])
            .with_icon(logo::icon())
            // Chrome propio: la barra de título del sistema rompería la
            // estética. `ui::chrome` reimplementa arrastre, botones y
            // redimensionado por los bordes.
            //
            // Sin decoración se comporta igual en los tres sistemas, con una
            // diferencia que está resuelta en `ui::chrome::resize_handles`: en
            // macOS la ventana conserva el `Resizable` de AppKit y es el sistema
            // el que redimensiona por los bordes, así que allí los tiradores
            // propios sobran. Arrastrar y maximizar sí funcionan en los tres.
            .with_decorations(false)
            .with_resizable(true),
        ..Default::default()
    };

    eframe::run_native(
        "flow",
        options,
        Box::new(|cc| {
            theme::install(&cc.egui_ctx);
            Ok(Box::new(app::Flow::new().demo()))
        }),
    )
}
