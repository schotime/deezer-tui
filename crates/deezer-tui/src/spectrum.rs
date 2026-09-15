//! Linux output-monitor capture and a lightweight logarithmic spectrum analyzer.
//!
//! PipeWire exposes a PulseAudio-compatible default monitor on Omarchy. `parec`
//! handles the device negotiation and gives us raw PCM without putting capture,
//! FFT work, or extra locking in Rodio's playback path.

use std::io::{self, Read};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::thread;

use rustfft::num_complex::Complex;
use rustfft::{Fft, FftPlanner};

pub const SPECTRUM_BANDS: usize = 32;
const SAMPLE_RATE: f32 = 48_000.0;
const WINDOW_FRAMES: usize = 2_048;
const CHANNELS: usize = 2;

pub struct SpectrumCapture {
    stop: Arc<AtomicBool>,
    receiver: mpsc::Receiver<Vec<f32>>,
}

impl SpectrumCapture {
    pub fn start() -> io::Result<Self> {
        let mut child = Command::new("parec")
            .args([
                "--device=@DEFAULT_MONITOR@",
                "--format=float32le",
                "--rate=48000",
                "--channels=2",
                "--raw",
                "--latency-msec=50",
                "--client-name=deezer-tui-visualizer",
                "--stream-name=Deezer TUI spectrum",
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| io::Error::other("parec did not expose stdout"))?;
        let stop = Arc::new(AtomicBool::new(false));
        let worker_stop = Arc::clone(&stop);
        let (sender, receiver) = mpsc::sync_channel(2);

        thread::Builder::new()
            .name("deezer-spectrum".into())
            .spawn(move || capture_loop(child, stdout, worker_stop, sender))?;

        Ok(Self { stop, receiver })
    }

    /// Return only the newest spectrum, discarding frames the renderer did not
    /// consume. This keeps visualization latency bounded when drawing stalls.
    pub fn latest(&self) -> Result<Option<Vec<f32>>, mpsc::TryRecvError> {
        let mut latest = None;
        loop {
            match self.receiver.try_recv() {
                Ok(levels) => latest = Some(levels),
                Err(mpsc::TryRecvError::Empty) => return Ok(latest),
                Err(mpsc::TryRecvError::Disconnected) if latest.is_some() => return Ok(latest),
                Err(err) => return Err(err),
            }
        }
    }
}

impl Drop for SpectrumCapture {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

fn capture_loop(
    mut child: Child,
    mut stdout: impl Read,
    stop: Arc<AtomicBool>,
    sender: mpsc::SyncSender<Vec<f32>>,
) {
    let mut bytes = vec![0_u8; WINDOW_FRAMES * CHANNELS * size_of::<f32>()];
    let mut samples = vec![0_f32; WINDOW_FRAMES];
    let mut analyzer = SpectrumAnalyzer::new(WINDOW_FRAMES, SPECTRUM_BANDS);

    while !stop.load(Ordering::Relaxed) {
        if stdout.read_exact(&mut bytes).is_err() {
            break;
        }
        for (frame, sample) in bytes.chunks_exact(8).zip(&mut samples) {
            let left = f32::from_le_bytes(frame[0..4].try_into().unwrap());
            let right = f32::from_le_bytes(frame[4..8].try_into().unwrap());
            *sample = if left.is_finite() && right.is_finite() {
                (left + right) * 0.5
            } else {
                0.0
            };
        }
        let levels = analyzer.analyze(&samples);
        let _ = sender.try_send(levels.to_vec());
    }

    let _ = child.kill();
    let _ = child.wait();
}

struct SpectrumAnalyzer {
    fft: Arc<dyn Fft<f32>>,
    buffer: Vec<Complex<f32>>,
    magnitudes: Vec<f32>,
    bin_counts: Vec<u16>,
    smoothed: Vec<f32>,
}

impl SpectrumAnalyzer {
    fn new(window_frames: usize, bands: usize) -> Self {
        let mut planner = FftPlanner::new();
        Self {
            fft: planner.plan_fft_forward(window_frames),
            buffer: vec![Complex::ZERO; window_frames],
            magnitudes: vec![0.0; bands],
            bin_counts: vec![0; bands],
            smoothed: vec![0.0; bands],
        }
    }

    /// Window and transform every sample, then combine the FFT bins into
    /// logarithmic bands so bass has the same visual room as higher ranges.
    fn analyze(&mut self, samples: &[f32]) -> &[f32] {
        if samples.len() != self.buffer.len() || self.smoothed.is_empty() {
            return &self.smoothed;
        }
        let count = samples.len() as f32;
        for (index, (sample, output)) in samples.iter().zip(&mut self.buffer).enumerate() {
            let window = 0.5 - 0.5 * (std::f32::consts::TAU * index as f32 / (count - 1.0)).cos();
            *output = Complex::new(sample * window, 0.0);
        }
        self.fft.process(&mut self.buffer);
        self.magnitudes.fill(0.0);
        self.bin_counts.fill(0);

        let low_hz = 45.0_f32;
        let high_hz = 16_000.0_f32;
        let log_span = (high_hz / low_hz).ln();
        for (bin, value) in self
            .buffer
            .iter()
            .take(samples.len() / 2)
            .enumerate()
            .skip(2)
        {
            let frequency = bin as f32 * SAMPLE_RATE / count;
            if !(low_hz..=high_hz).contains(&frequency) {
                continue;
            }
            let position = (frequency / low_hz).ln() / log_span;
            let band =
                ((position * self.magnitudes.len() as f32) as usize).min(self.magnitudes.len() - 1);
            let magnitude = value.norm() / (count * 0.5);
            self.magnitudes[band] = self.magnitudes[band].max(magnitude);
            self.bin_counts[band] += 1;
        }

        fill_empty_bands(&mut self.magnitudes, &self.bin_counts);

        for (magnitude, output) in self.magnitudes.iter().zip(&mut self.smoothed) {
            let decibels = 20.0 * magnitude.max(1.0e-7).log10();
            let mut level = ((decibels + 65.0) / 58.0).clamp(0.0, 1.0);
            if level < 0.035 {
                level = 0.0;
            }
            let smoothing = if level > *output { 0.72 } else { 0.18 };
            *output += (level - *output) * smoothing;
        }
        &self.smoothed
    }
}

/// At low frequencies a logarithmic band can be narrower than one FFT bin.
/// Interpolate those structural holes from their nearest sampled neighbours;
/// the separate bin counts ensure genuine zero-energy bands stay at zero.
fn fill_empty_bands(magnitudes: &mut [f32], bin_counts: &[u16]) {
    for empty in 0..magnitudes.len() {
        if bin_counts.get(empty).copied().unwrap_or_default() != 0 {
            continue;
        }
        let left = (0..empty).rev().find(|index| bin_counts[*index] != 0);
        let right = (empty + 1..magnitudes.len()).find(|index| bin_counts[*index] != 0);
        magnitudes[empty] = match (left, right) {
            (Some(left), Some(right)) => {
                let position = (empty - left) as f32 / (right - left) as f32;
                magnitudes[left] + (magnitudes[right] - magnitudes[left]) * position
            }
            (Some(left), None) => magnitudes[left],
            (None, Some(right)) => magnitudes[right],
            (None, None) => 0.0,
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn silence_produces_no_visible_bands() {
        let mut analyzer = SpectrumAnalyzer::new(WINDOW_FRAMES, SPECTRUM_BANDS);
        let levels = analyzer.analyze(&vec![0.0; WINDOW_FRAMES]);
        assert!(levels.iter().all(|level| *level == 0.0));
    }

    #[test]
    fn sine_wave_peaks_near_its_frequency() {
        let frequency = 1_000.0;
        let samples = (0..WINDOW_FRAMES)
            .map(|index| {
                (std::f32::consts::TAU * frequency * index as f32 / SAMPLE_RATE).sin() * 0.5
            })
            .collect::<Vec<_>>();
        let mut analyzer = SpectrumAnalyzer::new(WINDOW_FRAMES, SPECTRUM_BANDS);
        let levels = analyzer.analyze(&samples);
        let peak = levels
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.total_cmp(b))
            .map(|(index, _)| index)
            .unwrap();
        let peak_hz = 45.0 * (16_000.0_f32 / 45.0).powf(peak as f32 / 31.0);
        assert!((700.0..1_400.0).contains(&peak_hz));
        assert!(levels[peak] > 0.5);
    }

    #[test]
    fn fft_resolution_holes_are_interpolated_without_changing_real_silence() {
        let mut magnitudes = vec![0.2, 0.0, 0.6, 0.0];
        fill_empty_bands(&mut magnitudes, &[1, 0, 1, 1]);
        assert!((magnitudes[1] - 0.4).abs() < f32::EPSILON);
        assert_eq!(magnitudes[3], 0.0, "a sampled silent band stays silent");
    }
}
