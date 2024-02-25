use std::sync::mpsc::Receiver;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};

use eframe::CreationContext;
use egui::epaint::ImageDelta;

use egui::widgets::Image;
use egui::{
    menu, CentralPanel, ColorImage, ImageData, Key, KeyboardShortcut, Modifiers, TextureFilter,
    TextureHandle, TextureOptions, TextureWrapMode, TopBottomPanel, ViewportCommand,
};

use crate::core;
use crate::video::Frame;

use super::Cli;

const CORE_TEXTURE_OPTIONS: TextureOptions = TextureOptions {
    magnification: TextureFilter::Nearest,
    minification: TextureFilter::Nearest,
    wrap_mode: TextureWrapMode::ClampToEdge,
};

pub fn run(cli: Cli) -> Result<()> {
    let native_options = eframe::NativeOptions {
        vsync: true,
        ..<_>::default()
    };

    eframe::run_native(
        "APE",
        native_options,
        Box::new(move |cc| Box::new(Gui::new(cc, cli))),
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
    fn new(cc: &CreationContext, cli: Cli) -> Self {
        let texture_name = "Core";
        let image = ImageData::from(ColorImage::example());
        let core_texture = cc
            .egui_ctx
            .load_texture(texture_name, image, CORE_TEXTURE_OPTIONS);

        let (frame_rx, core_handle) = super::run(&cli.core, &cli.rom, cc.egui_ctx.clone()).unwrap();

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
        ctx.input_mut(|input| {
            if input.consume_key(Modifiers::SHIFT, Key::F1) {
                let save_state = self.core_handle.run(|core| core.state()).unwrap().unwrap();
                self.save_state = Some(save_state);
            }

            if input.consume_key(Modifiers::NONE, Key::F1) {
                if let Some(save_state) = &self.save_state {
                    let save_state = save_state.clone();
                    self.core_handle
                        .run(move |core| core.restore_state(&save_state))
                        .unwrap()
                        .unwrap();
                }
            }

            if input.consume_key(Modifiers::NONE, Key::Escape) {
                self.show_menu = !self.show_menu;
            }

            // if input.consume_key(Modifiers::NONE, Key::F11) {
            //     self.fullscreen = !self.fullscreen;
            //     ctx.send_viewport_cmd(ViewportCommand::Fullscreen(self.fullscreen));
            // }
        });

        // ctx.request_repaint_after(Duration::from_secs(1) / 60);

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

        TopBottomPanel::bottom("bottom").show(ctx, |ui| {
            let rupees = self
                .core_handle
                .run(|core| core.get_memory(0xDB5D, 2))
                .unwrap();

            let label = format!("Rupee count: {:X}{:02X}", rupees[0], rupees[1]);
            ui.heading(label);
        });

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
