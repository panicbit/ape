use std::sync::mpsc::{self, sync_channel, Receiver, SyncSender};

use anyhow::{anyhow, Result};

use crate::core::Core;

type CoreRunFn = Box<dyn FnOnce(&mut Core) + Send>;

pub struct Host {
    rx: Receiver<CoreRunFn>,
    tx: SyncSender<CoreRunFn>,
}

impl Host {
    pub fn new() -> Self {
        let (tx, rx) = sync_channel(0);

        Self { rx, tx }
    }

    pub fn handle(&self) -> Handle {
        Handle {
            tx: self.tx.clone(),
        }
    }

    pub fn run(&self, core: &mut Core) {
        if let Ok(run_fn) = self.rx.recv() {
            run_fn(core);
        }
    }
}

#[derive(Clone)]
pub struct Handle {
    tx: SyncSender<CoreRunFn>,
}

impl Handle {
    pub fn run<F, R>(&self, f: F) -> Result<R>
    where
        F: FnOnce(&mut Core) -> R + Send + 'static,
        R: Send + 'static,
    {
        let (result_tx, result_rx) = mpsc::sync_channel(0);
        let run_fn: CoreRunFn = Box::new(move |core| {
            let result = f(core);

            result_tx
                .send(result)
                .expect("BUG: core run fn result sender closed");
        });

        self.tx
            .send(run_fn)
            .map_err(|_| anyhow!("core run fn channel closed"))?;

        let result = result_rx
            .recv()
            .expect("BUG: core run fn result receiver closed");

        Ok(result)
    }
}
