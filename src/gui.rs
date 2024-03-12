use std::path::PathBuf;
use std::sync::mpsc::Receiver;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};

use eframe::CreationContext;
use egui::epaint::ImageDelta;

use egui::widgets::Image;
use egui::{
    menu, CentralPanel, ColorImage, ImageData, TextureFilter, TextureHandle, TextureOptions,
    TextureWrapMode, TopBottomPanel,
};

use crate::core;
use crate::video::Frame;

mod input;

const CORE_TEXTURE_OPTIONS: TextureOptions = TextureOptions {
    magnification: TextureFilter::Nearest,
    minification: TextureFilter::Nearest,
    wrap_mode: TextureWrapMode::ClampToEdge,
};

pub fn run(core: PathBuf, rom: PathBuf) -> Result<()> {
    let native_options = eframe::NativeOptions {
        vsync: true,
        ..<_>::default()
    };

    eframe::run_native(
        "APE",
        native_options,
        Box::new(move |cc| Box::new(Gui::new(cc, core, rom))),
    )
    .map_err(|err| anyhow!("{err}"))
    .context("failed to run eframe")?;

    Ok(())
}

pub struct Gui {
    core_texture: TextureHandle,
    frame_rx: Receiver<Option<Frame>>,
    core_handle: core::Handle,
    save_state: Option<Vec<u8>>,
    show_menu: bool,
    fullscreen: bool,
}

impl Gui {
    fn new(cc: &CreationContext, core: PathBuf, rom: PathBuf) -> Self {
        let texture_name = "Core";
        let image = ImageData::from(ColorImage::example());
        let core_texture = cc
            .egui_ctx
            .load_texture(texture_name, image, CORE_TEXTURE_OPTIONS);

        let (frame_rx, core_handle) = super::run(core, rom, cc.egui_ctx.clone()).unwrap();

        Self {
            core_texture,
            frame_rx,
            core_handle,
            save_state: None,
            show_menu: false,
            fullscreen: false,
        }
    }
}

impl eframe::App for Gui {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.request_repaint_after(Duration::from_secs(1) / 60);

        self.handle_input(ctx);

        if self.show_menu {
            TopBottomPanel::top("top").show(ctx, |ui| {
                menu::bar(ui, |ui| {
                    ui.menu_button("File", |ui| {
                        if ui.button("Load ROM").clicked() {
                            println!("load rom!");
                            ui.close_menu();
                        }
                    });
                });
            });
        }

        // TopBottomPanel::bottom("bottom").show(ctx, |ui| {
        //     let rupees = self
        //         .core_handle
        //         .run(|core| core.get_memory(0xDB5D, 2))
        //         .unwrap();

        //     let label = format!("Rupee count: {:X}{:02X}", rupees[0], rupees[1]);
        //     ui.heading(label);
        // });

        let frame = egui::Frame::default();
        CentralPanel::default().frame(frame).show(ctx, |ui| {
            self.core_handle.run(|core| core.run()).unwrap();
            if let Ok(Some(frame)) = self.frame_rx.try_recv() {
                let pixels = frame.buffer_to_packed_rgb888();
                let size = [frame.width, frame.height];
                let image = ColorImage::from_rgb(size, &pixels);
                let image = ImageDelta::full(image, CORE_TEXTURE_OPTIONS);

                ctx.tex_manager().write().set(self.core_texture.id(), image);
            }

            let image = Image::new(&self.core_texture).fit_to_exact_size(ui.available_size());

            ui.add_sized(ui.available_size(), image);
        });
    }
}
