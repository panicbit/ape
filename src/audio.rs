use std::sync::mpsc::Receiver;
use std::time::Duration;
use std::vec;

pub struct RetroAudio {
    pub rx: Receiver<Vec<i16>>,
    pub current_frame: vec::IntoIter<i16>,
    pub sample_rate: u32,
}

impl rodio::Source for RetroAudio {
    fn current_frame_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> u16 {
        2
    }

    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    fn total_duration(&self) -> Option<Duration> {
        None
    }
}

impl Iterator for RetroAudio {
    type Item = i16;

    fn next(&mut self) -> Option<Self::Item> {
        match self.current_frame.next() {
            Some(sample) => Some(sample),
            None => {
                self.current_frame = match self.rx.recv() {
                    Ok(current_frame) => current_frame.into_iter(),
                    Err(err) => {
                        eprintln!("Failed to receive audio frames: {err}");
                        return None;
                    }
                };
                self.current_frame.next()
            }
        }
    }
}
