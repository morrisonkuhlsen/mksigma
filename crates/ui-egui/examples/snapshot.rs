//! Renders the MK Sigma window offscreen (wgpu, no window) to a PNG, to look at the UI without
//! touching the screen.
//!
//! `cargo run -p mksigma-ui-egui --example snapshot -- [--in data.sav] [--variables] [--labels]
//!  [--run 'SYNTAX'] [--size 1280x800] out.png`

use mksigma_ui_egui::{SigmaApp, View};

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut app = SigmaApp::default();
    let mut size = (1280.0f32, 800.0f32);
    let mut out = "snapshot.png".to_string();
    let mut run = None;
    let mut i = 0;
    while let Some(arg) = args.get(i) {
        match arg.as_str() {
            "--in" => {
                i += 1;
                let path = args.get(i).ok_or("--in needs a path")?;
                app.open_path(std::path::Path::new(path));
            }
            "--variables" => app.view = View::Variables,
            "--labels" => app.value_labels = true,
            "--run" => {
                i += 1;
                run = args.get(i).cloned();
            }
            "--size" => {
                i += 1;
                if let Some((w, h)) = args.get(i).and_then(|s| s.split_once('x')) {
                    size = (w.parse().map_err(|_| "bad width")?, h.parse().map_err(|_| "bad height")?);
                }
            }
            other => out = other.to_string(),
        }
        i += 1;
    }
    if let Some(syntax) = run {
        app.syntax = syntax;
        let _ = app.session.run_syntax(&app.syntax);
    }
    let mut harness = egui_kittest::Harness::builder()
        .with_size(egui::vec2(size.0, size.1))
        .with_pixels_per_point(1.0)
        .wgpu()
        .build_ui_state(|ui, app: &mut SigmaApp| app.ui(ui), app);
    harness.run_steps(6);
    let image = harness.render().map_err(|e| e.to_string())?;
    image.save(&out).map_err(|e| e.to_string())?;
    println!("{out}");
    Ok(())
}
