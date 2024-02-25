use std::sync::mpsc::Receiver;
use std::sync::Arc;
use std::time::Duration;
use std::vec;

use parking_lot::RwLock;

pub struct RetroAudio {
    pub rx: Receiver<Vec<i16>>,
    pub current_frame: vec::IntoIter<i16>,
    pub base_sample_rate: f32,
    pub speed_factor: Arc<RwLock<f32>>,
}

impl rodio::Source for RetroAudio {
    fn current_frame_len(&self) -> Option<usize> {
        Some(self.current_frame.len().max(1))
    }

    fn channels(&self) -> u16 {
        2
    }

    fn sample_rate(&self) -> u32 {
        let speed_factor = *self.speed_factor.read();

        (speed_factor * self.base_sample_rate) as u32
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
