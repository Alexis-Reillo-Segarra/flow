// En release no queremos que Windows abra una consola detrás de la ventana.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod agent;
mod app;
mod logo;
mod presets;
mod projects;
mod session;
mod term;
mod theme;
mod ui;

fn main() -> eframe::Result<()> {
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
