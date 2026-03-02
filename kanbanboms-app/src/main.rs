#![warn(clippy::all, rust_2018_idioms)]
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result<()> {
    env_logger::init();
    let args: Vec<String> = std::env::args().collect();
    let import_path = args
        .windows(2)
        .find(|w| w[0] == "--import" || w[0] == "-i")
        .and_then(|w| w.get(1).cloned());
    let mut native_options = eframe::NativeOptions::default();
    native_options.viewport = egui::ViewportBuilder::default().with_inner_size([900.0, 600.0]);
    eframe::run_native(
        "Kanban BOMs",
        native_options,
        Box::new(move |cc| {
            let mut app = kanbanboms_app::KanbanBomsApp::new(cc);
            if let Some(ref path) = import_path {
                if let Err(e) = app.load_json_file(path) {
                    log::error!("Failed to import JSON from {}: {}", path, e);
                    app.status_flash = Some((format!("Import failed: {}", e), true));
                    app.status_flash_at = None;
                }
            }
            Ok(Box::new(app))
        }),
    )
}

#[cfg(target_arch = "wasm32")]
fn main() {
    use eframe::wasm_bindgen::JsCast;

    eframe::WebLogger::init(log::LevelFilter::Debug).ok();
    let web_options = eframe::WebOptions::default();

    wasm_bindgen_futures::spawn_local(async {
        let document = web_sys::window()
            .expect("No window")
            .document()
            .expect("No document");

        let canvas = document
            .get_element_by_id("the_canvas_id")
            .expect("Failed to find the_canvas_id")
            .dyn_into::<web_sys::HtmlCanvasElement>()
            .expect("the_canvas_id was not a HtmlCanvasElement");

        let start_result = eframe::WebRunner::new()
            .start(
                canvas,
                web_options,
                Box::new(|cc| Ok(Box::new(kanbanboms_app::KanbanBomsApp::new(cc)))),
            )
            .await;

        if let Some(loading_text) = document.get_element_by_id("loading_text") {
            match start_result {
                Ok(_) => {
                    loading_text.remove();
                }
                Err(e) => {
                    loading_text.set_inner_html(&format!(
                        "<p style='color:red'>The app crashed. See console for details.</p>"
                    ));
                    panic!("Failed to start eframe: {e:?}");
                }
            }
        }
    });
}
