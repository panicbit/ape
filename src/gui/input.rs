use std::thread;

use egui::{Key, Modifiers, ViewportCommand};

impl super::Gui {
    pub(super) fn handle_input(&mut self, ctx: &egui::Context) {
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

            if input.consume_key(Modifiers::NONE, Key::F11) {
                self.fullscreen = !self.fullscreen;
                let cmd = ViewportCommand::Fullscreen(self.fullscreen);
                let ctx = ctx.clone();

                thread::spawn(move || {
                    ctx.send_viewport_cmd(cmd);
                });
            }
        });
    }
}
