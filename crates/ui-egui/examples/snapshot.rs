//! Renders the MK Sigma window offscreen (wgpu, no window) to a PNG, to look at the UI without
//! touching the screen.
//!
//! `cargo run -p mksigma-ui-egui --example snapshot -- [--in data.sav] [--variables] [--labels]
//!  [--run 'SYNTAX'] [--disable N]... [--select ITEM:PART] [--pin ITEM]... [--dashboard]
//!  [--size 1280x800] out.png` (`--disable` switches off applied step N, counting from 1, after
//!  `--run`; `--select` cross-filters by category PART of Output item ITEM, both counting from 1:
//!  a bar, bin or box, or a table row; `--pin` pins Output item ITEM to the dashboard and
//!  `--dashboard` shows it)

use mksigma_engine::output::Item;
use mksigma_ui_egui::{SigmaApp, View};

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut app = SigmaApp::default();
    let mut size = (1280.0f32, 800.0f32);
    let mut out = "snapshot.png".to_string();
    let mut run = None;
    let mut disable: Vec<usize> = Vec::new();
    let mut select = None;
    let mut pins: Vec<usize> = Vec::new();
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
            "--disable" => {
                i += 1;
                disable.push(args.get(i).and_then(|n| n.parse::<usize>().ok()).filter(|n| *n > 0).ok_or("--disable needs a step number")? - 1);
            }
            "--select" => {
                i += 1;
                let pair = args.get(i).and_then(|s| s.split_once(':'));
                let pair = pair.and_then(|(a, b)| Some((a.parse::<usize>().ok()?.checked_sub(1)?, b.parse::<usize>().ok()?.checked_sub(1)?)));
                select = Some(pair.ok_or("--select needs ITEM:PART")?);
            }
            "--pin" => {
                i += 1;
                pins.push(args.get(i).and_then(|n| n.parse::<usize>().ok()).and_then(|n| n.checked_sub(1)).ok_or("--pin needs an item number")?);
            }
            "--dashboard" => app.output_state.dashboard = true,
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
    for step in disable {
        app.session.set_step_enabled(step, false).map_err(|e| e.to_string())?;
    }
    for item in pins {
        app.session.toggle_pin(item).map_err(|e| e.to_string())?;
    }
    if let Some((item, part)) = select {
        let category = match app.session.output.items.get(item) {
            Some(Item::Chart(c)) => c.category(part),
            Some(Item::Table(t)) => t.category(part),
            _ => None,
        };
        let category = category.cloned().ok_or("--select: that item has no such category")?;
        app.session.select_category(item, category).map_err(|e| e.to_string())?;
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
