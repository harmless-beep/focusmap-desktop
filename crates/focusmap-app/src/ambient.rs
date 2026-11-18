//! Optional ambient drone, synthesised in memory and played by the OS.
//!
//! No audio files, no decoder, no network. The eight-second loop is made of
//! sines whose frequencies are whole multiples of 1/8 Hz, so it joins cleanly.

#[cfg(windows)]
use windows_sys::Win32::Media::Audio::{
    PlaySoundW, SND_ASYNC, SND_LOOP, SND_MEMORY, SND_NODEFAULT,
};

const RATE: u32 = 22_050;
const SECONDS: usize = 8;
const SAMPLES: usize = (RATE as usize) * SECONDS;

#[derive(Default)]
pub(crate) struct Ambient {
    wav: Vec<u8>,
    playing: bool,
}

impl Ambient {
    /// Start or stop the loop. The WAV bytes stay owned by the app because
    /// `SND_MEMORY` plays them straight from that buffer.
    pub(crate) fn set(&mut self, on: bool) {
        if on == self.playing {
            return;
        }
        if on {
            if self.wav.is_empty() {
                self.wav = wav();
            }
            #[cfg(windows)]
            unsafe {
                let bytes = self.wav.as_ptr() as windows_sys::core::PCWSTR;
                PlaySoundW(
                    bytes,
                    std::ptr::null_mut(),
                    SND_MEMORY | SND_ASYNC | SND_LOOP | SND_NODEFAULT,
                );
            }
        } else {
            #[cfg(windows)]
            unsafe {
                PlaySoundW(std::ptr::null(), std::ptr::null_mut(), 0);
            }
        }
        self.playing = on;
    }

    pub(crate) fn stop(&mut self) {
        self.set(false);
    }
}

fn wav() -> Vec<u8> {
    let data_len = SAMPLES * 2;
    let mut out = Vec::with_capacity(44 + data_len);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36u32 + data_len as u32).to_le_bytes());
    out.extend_from_slice(b"WAVE");
    out.extend_from_slice(b"fmt ");
    out.extend_from_slice(&16u32.to_le_bytes()); // PCM header size
    out.extend_from_slice(&1u16.to_le_bytes()); // PCM
    out.extend_from_slice(&1u16.to_le_bytes()); // mono
    out.extend_from_slice(&RATE.to_le_bytes());
    out.extend_from_slice(&(RATE * 2).to_le_bytes()); // bytes per second
    out.extend_from_slice(&2u16.to_le_bytes()); // block align
    out.extend_from_slice(&16u16.to_le_bytes()); // bits per sample
    out.extend_from_slice(b"data");
    out.extend_from_slice(&(data_len as u32).to_le_bytes());

    let tau = std::f32::consts::TAU;
    for i in 0..SAMPLES {
        let t = i as f32 / RATE as f32;
        let breath = 0.60 + 0.40 * (tau * 0.125 * t).sin();
        let mut value = 0.16 * (tau * 55.0 * t).sin()
            + 0.10 * (tau * 82.5 * t + 0.7).sin()
            + 0.07 * (tau * 110.0 * t + 1.4).sin()
            + 0.05 * (tau * 165.0 * t + 0.3).sin();
        value += 0.025 * (tau * 440.0 * t).sin() * breath;
        value *= breath;
        let sample = (value.clamp(-1.0, 1.0) * 12_000.0) as i16;
        out.extend_from_slice(&sample.to_le_bytes());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_a_valid_short_wav() {
        let bytes = wav();
        assert_eq!(&bytes[..4], b"RIFF");
        assert_eq!(&bytes[8..12], b"WAVE");
        let data_len = u32::from_le_bytes(bytes[40..44].try_into().unwrap()) as usize;
        assert_eq!(data_len, SAMPLES * 2);
        assert_eq!(bytes.len(), 44 + data_len);
        let rate = u32::from_le_bytes(bytes[24..28].try_into().unwrap());
        assert_eq!(rate, RATE);
    }
}
