use std::sync::mpsc::Receiver;
use std::sync::{Arc, RwLock};
use std::time::Duration;
use std::vec;

pub struct RetroAudio {
    pub rx: Receiver<Vec<i16>>,
    pub current_frame: vec::IntoIter<i16>,
    pub sample_rate: Arc<RwLock<u32>>,
}

impl rodio::Source for RetroAudio {
    fn current_frame_len(&self) -> Option<usize> {
        Some(self.current_frame.len().max(1))
    }

    fn channels(&self) -> u16 {
        2
    }

    fn sample_rate(&self) -> u32 {
        *self.sample_rate.read().unwrap()
    }

    fn total_duration(&self) -> Option<Duration> {
        None
    }
}

impl Iterator for RetroAudio {
    type Item = i16;

    fn next(&mut self) -> Option<Self::Item> {
        let sample = match self.current_frame.next() {
            Some(sample) => Some(sample),
            None => {
                println!("should not happen more than once");
                self.current_frame = match self.rx.recv() {
                    Ok(current_frame) => current_frame.into_iter(),
                    Err(err) => {
                        eprintln!("Failed to receive audio frames: {err}");
                        return None;
                    }
                };

                self.current_frame.next()
            }
        };

        if self.current_frame.len() == 0 {
            self.current_frame = match self.rx.recv() {
                Ok(current_frame) => current_frame.into_iter(),
                Err(err) => {
                    eprintln!("Failed to receive audio frames: {err}");
                    return None;
                }
            };
        }

        if sample.is_none() {
            eprintln!("returning empty sample!");
        }

        sample
    }
}
