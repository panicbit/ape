use std::sync::mpsc::{self, sync_channel, Receiver, SyncSender};

use anyhow::{anyhow, Ok, Result};

use crate::core::Core;

type CoreHookFn = Box<dyn FnOnce(&mut Core) + Send>;

pub struct Host {
    rx: Receiver<CoreHookFn>,
    tx: SyncSender<CoreHookFn>,
    buffer_size: usize,
}

impl Host {
    pub fn new() -> Self {
        let buffer_size = 100;
        let (tx, rx) = sync_channel(buffer_size);

        Self {
            rx,
            tx,
            buffer_size,
        }
    }

    pub fn handle(&self) -> Handle {
        Handle {
            tx: self.tx.clone(),
        }
    }

    pub fn run(&self, core: &mut Core) {
        for hook_fn in self.rx.try_iter().take(self.buffer_size) {
            hook_fn(core);
        }
    }
}

#[derive(Clone)]
pub struct Handle {
    tx: SyncSender<CoreHookFn>,
}

impl Handle {
    pub fn run<F, R>(&self, f: F) -> Result<R>
    where
        F: FnOnce(&mut Core) -> R + Send + 'static,
        R: Send + 'static,
    {
        let (result_tx, result_rx) = mpsc::sync_channel(0);
        let hook_fn: CoreHookFn = Box::new(move |core| {
            let result = f(core);

            result_tx
                .try_send(result)
                .expect("BUG: hook result sender closed");
        });

        self.tx
            .send(hook_fn)
            .map_err(|_| anyhow!("hook channel closed"))?;

        let result = result_rx.recv().expect("BUG: hook result receiver closed");

        Ok(result)
    }
}
