#[allow(non_snake_case)]

use std::process::Command;
use eframe::egui;

fn main() -> Result<(), eframe::Error>
{
    // Command::new("target/debug/simulator")
    //     .spawn()
    //     .expect("Failed to execute simulator.");
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([320.0, 240.0]),
        ..Default::default()
    };
    eframe::run_native(
        "My egui App",
        options,
        Box::new(|cc| {
            // This gives us image support:
            egui_extras::install_image_loaders(&cc.egui_ctx);

            Box::<TestApp>::default()
        }),
    )
}

struct TestApp
{
    title: String,
    value: i32,
}

impl Default for TestApp {
    fn default() -> Self {
        Self {
            title: String::from("Default Title"),
            value: 0,
        }
    }
}


impl eframe::App for TestApp
{
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame)
    {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading(self.title.as_str());
            ui.add(egui::Slider::new(&mut self.value, 0..=10000).text("value"));
        });
    }
}
