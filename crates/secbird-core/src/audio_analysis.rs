//! Bounded playback PCM decoding and analysis, independent of vendor FFT/effects.
use crate::{Error, Result};
use serde::Serialize;
pub const FFT_SIZE: usize = 2048;
const BINS: usize = FFT_SIZE / 2 + 1;
pub const MAX_PACKET_BYTES: usize = 16 * 1024 * 1024;
#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Encoding {
    Unsigned8,
    Signed16,
    Signed24,
    Signed32,
    Float32,
    Float64,
}
#[derive(Clone, Copy, Debug, Serialize)]
pub struct Format {
    sample_rate: u32,
    channels: u16,
    block_align: u16,
    encoding: Encoding,
}
fn invalid<T>(message: &str) -> Result<T> {
    Err(Error::InvalidArg(message.into()))
}
impl Format {
    pub fn sample_rate(self) -> u32 {
        self.sample_rate
    }
    pub fn channels(self) -> u16 {
        self.channels
    }
    pub fn block_align(self) -> u16 {
        self.block_align
    }
    pub fn from_wave_format(bytes: &[u8]) -> Result<Self> {
        if bytes.len() < 18 {
            return invalid("truncated WAVEFORMATEX");
        }
        let u16at = |at| u16::from_le_bytes([bytes[at], bytes[at + 1]]);
        let u32at = |at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        let (mut tag, channels, rate, align, bits, extra) = (
            u16at(0),
            u16at(2),
            u32at(4),
            u16at(12),
            u16at(14),
            u16at(16),
        );
        if tag == 0xfffe {
            if extra < 22 || bytes.len() < 40 {
                return invalid("truncated extensible audio format");
            }
            let valid = u16at(18);
            if valid == 0 || valid > bits {
                return invalid("invalid valid-bit count");
            }
            // KSDATAFORMAT_SUBTYPE_{PCM,IEEE_FLOAT}, GUID little-endian storage.
            if bytes[28..40] != [0, 0, 16, 0, 128, 0, 0, 170, 0, 56, 155, 113] {
                return invalid("unsupported audio subtype GUID");
            }
            let subtype = u32at(24);
            if ![1, 3].contains(&subtype) {
                return invalid("unsupported extensible audio subtype");
            }
            tag = subtype as u16;
            if tag == 3 && valid != bits {
                return invalid("float valid bits differ from container");
            }
        } else if extra != 0 {
            return invalid("unsupported non-extensible format extension");
        }
        if !(1..=32).contains(&channels)
            || !(8000..=384000).contains(&rate)
            || ![8, 16, 24, 32, 64].contains(&bits)
            || align != channels * (bits / 8)
            || u32at(8) != rate * u32::from(align)
        {
            return invalid("unsupported or inconsistent PCM geometry");
        }
        let encoding = match (tag, bits) {
            (1, 8) => Encoding::Unsigned8,
            (1, 16) => Encoding::Signed16,
            (1, 24) => Encoding::Signed24,
            (1, 32) => Encoding::Signed32,
            (3, 32) => Encoding::Float32,
            (3, 64) => Encoding::Float64,
            _ => return invalid("unsupported PCM encoding"),
        };
        Ok(Self {
            sample_rate: rate,
            channels,
            block_align: align,
            encoding,
        })
    }
    fn sample(self, bytes: &[u8]) -> Result<f64> {
        let value = match self.encoding {
            Encoding::Unsigned8 => (f64::from(bytes[0]) - 128.) / 128.,
            Encoding::Signed16 => f64::from(i16::from_le_bytes(bytes.try_into().unwrap())) / 32768.,
            Encoding::Signed24 => {
                let value = i32::from_le_bytes([0, bytes[0], bytes[1], bytes[2]]) >> 8;
                f64::from(value) / 8388608.
            }
            Encoding::Signed32 => {
                f64::from(i32::from_le_bytes(bytes.try_into().unwrap())) / 2147483648.
            }
            Encoding::Float32 => f64::from(f32::from_le_bytes(bytes.try_into().unwrap())),
            Encoding::Float64 => f64::from_le_bytes(bytes.try_into().unwrap()),
        };
        if !value.is_finite() || value.abs() > 16. {
            return invalid("nonfinite or excessive audio sample");
        }
        Ok(value)
    }
}
#[derive(Clone, Debug, Serialize)]
pub struct MeterFrame {
    pub sample_rate: u32,
    pub fft_size: usize,
    pub bin_hz: f64,
    pub rms: f64,
    pub peak: f64,
    pub magnitudes: Box<[f64]>,
}
#[derive(Clone, Copy, Default)]
struct Complex {
    re: f64,
    im: f64,
}
pub struct Analyzer {
    format: Format,
    ring: Box<[f64]>,
    window: Box<[f64]>,
    scratch: Box<[Complex]>,
    position: usize,
    filled: usize,
    hop: usize,
    window_sum: f64,
    stride: usize,
}
impl Analyzer {
    pub fn new(format: Format) -> Self {
        let window: Box<[_]> = (0..FFT_SIZE)
            .map(|i| 0.5 - 0.5 * (std::f64::consts::TAU * i as f64 / (FFT_SIZE - 1) as f64).cos())
            .collect();
        let window_sum = window.iter().sum();
        Self {
            format,
            ring: vec![0.; FFT_SIZE * usize::from(format.channels)].into_boxed_slice(),
            window,
            scratch: vec![Complex::default(); FFT_SIZE].into_boxed_slice(),
            position: 0,
            filled: 0,
            hop: 0,
            window_sum,
            stride: FFT_SIZE / 2,
        }
    }
    /// Cap meter analysis work at25fps while retaining the endpoint sample rate.
    pub fn for_meter(format: Format) -> Self {
        let mut analyzer = Self::new(format);
        analyzer.stride = (format.sample_rate as usize / 25).max(FFT_SIZE / 2);
        analyzer
    }
    pub fn reset(&mut self) {
        self.ring.fill(0.);
        self.position = 0;
        self.filled = 0;
        self.hop = 0;
    }
    pub fn storage_bytes(&self) -> usize {
        self.ring.len() * 8
            + self.window.len() * 8
            + self.scratch.len() * std::mem::size_of::<Complex>()
    }
    pub fn push(
        &mut self,
        bytes: &[u8],
        frames: u32,
        silent: bool,
        mut sink: impl FnMut(MeterFrame),
    ) -> Result<()> {
        let length = (frames as usize)
            .checked_mul(usize::from(self.format.block_align))
            .ok_or_else(|| Error::InvalidArg("audio packet size overflow".into()))?;
        if length > MAX_PACKET_BYTES || (!silent && bytes.len() != length) {
            return invalid("audio packet byte count mismatch or resource limit");
        }
        let channels = usize::from(self.format.channels);
        let sample_bytes = usize::from(self.format.block_align) / channels;
        for frame in 0..frames as usize {
            for channel in 0..channels {
                let offset = frame * usize::from(self.format.block_align) + channel * sample_bytes;
                self.ring[self.position * channels + channel] = if silent {
                    0.
                } else {
                    self.format.sample(&bytes[offset..offset + sample_bytes])?
                };
            }
            self.position = (self.position + 1) % FFT_SIZE;
            if self.filled < FFT_SIZE {
                self.filled += 1;
                if self.filled == FFT_SIZE {
                    sink(self.analyze());
                    self.hop = 0;
                }
            } else {
                self.hop += 1;
                if self.hop == self.stride {
                    sink(self.analyze());
                    self.hop = 0;
                }
            }
        }
        Ok(())
    }
    fn analyze(&mut self) -> MeterFrame {
        let channels = usize::from(self.format.channels);
        let mut squares = 0.;
        let mut peak = 0.0_f64;
        let mut powers = vec![0.; BINS];
        for channel in 0..channels {
            for i in 0..FFT_SIZE {
                let sample = self.ring[((self.position + i) % FFT_SIZE) * channels + channel];
                squares += sample * sample;
                peak = peak.max(sample.abs());
                self.scratch[i] = Complex {
                    re: sample * self.window[i],
                    im: 0.,
                };
            }
            fft(&mut self.scratch);
            for (bin, power) in powers.iter_mut().enumerate() {
                let value = self.scratch[bin];
                let scale = if bin == 0 || bin == FFT_SIZE / 2 {
                    1.
                } else {
                    2.
                };
                let magnitude = value.re.hypot(value.im) * scale / self.window_sum;
                *power += magnitude * magnitude / channels as f64;
            }
        }
        MeterFrame {
            sample_rate: self.format.sample_rate,
            fft_size: FFT_SIZE,
            bin_hz: self.format.sample_rate as f64 / FFT_SIZE as f64,
            rms: (squares / (FFT_SIZE * channels) as f64).sqrt(),
            peak,
            magnitudes: powers.into_iter().map(f64::sqrt).collect(),
        }
    }
}
fn fft(values: &mut [Complex]) {
    let n = values.len();
    let mut reverse = 0;
    for index in 1..n {
        let mut bit = n / 2;
        while reverse & bit != 0 {
            reverse ^= bit;
            bit /= 2;
        }
        reverse ^= bit;
        if index < reverse {
            values.swap(index, reverse);
        }
    }
    let mut width = 2;
    while width <= n {
        let angle = -std::f64::consts::TAU / width as f64;
        let step = Complex {
            re: angle.cos(),
            im: angle.sin(),
        };
        for start in (0..n).step_by(width) {
            let mut weight = Complex { re: 1., im: 0. };
            for offset in 0..width / 2 {
                let a = values[start + offset];
                let b = values[start + offset + width / 2];
                let b = Complex {
                    re: b.re * weight.re - b.im * weight.im,
                    im: b.re * weight.im + b.im * weight.re,
                };
                values[start + offset] = Complex {
                    re: a.re + b.re,
                    im: a.im + b.im,
                };
                values[start + offset + width / 2] = Complex {
                    re: a.re - b.re,
                    im: a.im - b.im,
                };
                weight = Complex {
                    re: weight.re * step.re - weight.im * step.im,
                    im: weight.re * step.im + weight.im * step.re,
                };
            }
        }
        width *= 2;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn wave(tag: u16, bits: u16, channels: u16, rate: u32) -> Vec<u8> {
        let align = channels * (bits / 8);
        let mut bytes = vec![];
        bytes.extend(tag.to_le_bytes());
        bytes.extend(channels.to_le_bytes());
        bytes.extend(rate.to_le_bytes());
        bytes.extend((rate * u32::from(align)).to_le_bytes());
        bytes.extend(align.to_le_bytes());
        bytes.extend(bits.to_le_bytes());
        bytes.extend(0u16.to_le_bytes());
        bytes
    }
    #[test]
    fn all_bins_and_overlap_match_independent_numpy_reference_without_antiphase_cancellation() {
        let reference: serde_json::Value = serde_json::from_str(include_str!(
            "../tests/fixtures/audio-analysis-reference.json"
        ))
        .unwrap();
        for case in reference["cases"].as_array().unwrap() {
            let channels = case["channels"].as_u64().unwrap() as u16;
            let rate = case["sample_rate"].as_u64().unwrap() as u32;
            let format = Format::from_wave_format(&wave(3, 32, channels, rate)).unwrap();
            let mut analyzer = Analyzer::new(format);
            let mut state = 123456789u32;
            let mut bytes = vec![];
            for frame in 0..3072 {
                for channel in 0..channels {
                    let sample = match case["kind"].as_str().unwrap() {
                        "antiphase" => {
                            0.5 * (std::f64::consts::TAU * 137. * frame as f64 / 2048.).sin()
                                * if channel == 0 { 1. } else { -1. }
                        }
                        "dc" => 0.25,
                        "multitone" => {
                            (0.1 + 0.025 * f64::from(channel))
                                * (std::f64::consts::TAU
                                    * f64::from(30 + 31 * channel)
                                    * frame as f64
                                    / 2048.)
                                    .sin()
                        }
                        _ => {
                            state = state.wrapping_mul(1664525).wrapping_add(1013904223);
                            (f64::from(state >> 8) / 16777216. - 0.5) * 0.25
                        }
                    } as f32;
                    bytes.extend(sample.to_le_bytes());
                }
            }
            let mut output = vec![];
            let mut at = 0;
            // Deliberately split windows at irregular packet boundaries.
            for frames in [17, 1024, 700, 1331] {
                let end = at + frames * usize::from(format.block_align());
                analyzer
                    .push(&bytes[at..end], frames as u32, false, |frame| {
                        output.push(frame)
                    })
                    .unwrap();
                at = end;
            }
            assert_eq!(output.len(), 2);
            for (actual, expected) in output.iter().zip(case["expected"].as_array().unwrap()) {
                assert!((actual.rms - expected["rms"].as_f64().unwrap()).abs() < 1e-9);
                assert!((actual.peak - expected["peak"].as_f64().unwrap()).abs() < 1e-9);
                for (index, (a, e)) in actual
                    .magnitudes
                    .iter()
                    .zip(expected["magnitudes"].as_array().unwrap())
                    .enumerate()
                {
                    assert!(
                        (a - e.as_f64().unwrap()).abs() < 1e-9,
                        "{} bin{index}: {a} != {e}",
                        case["kind"]
                    );
                }
                assert_eq!(actual.bin_hz, f64::from(rate) / 2048.);
            }
            assert!(analyzer.storage_bytes() < 600000);
        }
    }
    #[test]
    fn pcm_integer_float_extensible_and_invalid_geometry_are_explicit() {
        for (tag, bits, bytes, expected) in [
            (1, 8, vec![0], -1.),
            (1, 16, (-32768i16).to_le_bytes().to_vec(), -1.),
            (1, 24, vec![0, 0, 128], -1.),
            (1, 32, i32::MIN.to_le_bytes().to_vec(), -1.),
            (3, 32, 0.75f32.to_le_bytes().to_vec(), 0.75),
            (3, 64, (-0.5f64).to_le_bytes().to_vec(), -0.5),
        ] {
            let format = Format::from_wave_format(&wave(tag, bits, 1, 48000)).unwrap();
            assert_eq!(format.sample(&bytes).unwrap(), expected);
        }
        let mut ext = wave(0xfffe, 32, 2, 48000);
        ext[16..18].copy_from_slice(&22u16.to_le_bytes());
        ext.extend(24u16.to_le_bytes());
        ext.extend(3u32.to_le_bytes());
        ext.extend([1, 0, 0, 0, 0, 0, 16, 0, 128, 0, 0, 170, 0, 56, 155, 113]);
        assert!(Format::from_wave_format(&ext).is_ok());
        for offset in [2, 4, 12, 14] {
            let mut bytes = wave(3, 32, 2, 48000);
            bytes[offset] = 255;
            bytes[offset + 1] = 255;
            assert!(Format::from_wave_format(&bytes).is_err());
        }
        assert!(Format::from_wave_format(&ext[..39]).is_err());
        ext[39] = 0;
        assert!(Format::from_wave_format(&ext).is_err());
        let format = Format::from_wave_format(&wave(3, 32, 1, 48000)).unwrap();
        assert!(format.sample(&f32::NAN.to_le_bytes()).is_err());
        assert!(format.sample(&17f32.to_le_bytes()).is_err());
    }
    #[test]
    fn silence_never_reads_bytes_and_reset_discards_partial_windows() {
        let format = Format::from_wave_format(&wave(3, 32, 2, 48000)).unwrap();
        let mut analyzer = Analyzer::new(format);
        let mut count = 0;
        analyzer.push(&[], 1024, true, |_| count += 1).unwrap();
        assert_eq!(count, 0);
        analyzer.reset();
        analyzer.push(&[], 1024, true, |_| count += 1).unwrap();
        assert_eq!(count, 0);
        analyzer
            .push(&[], 1024, true, |frame| {
                count += 1;
                assert_eq!(frame.peak, 0.);
                assert_eq!(frame.rms, 0.);
                assert!(frame.magnitudes.iter().all(|value| *value == 0.));
            })
            .unwrap();
        assert_eq!(count, 1);
        assert!(analyzer
            .push(&[1], 1, false, |_| panic!("invalid packet published"))
            .is_err());
    }
    #[test]
    fn meter_cadence_bounds_fft_work_at_high_sample_rates() {
        for rate in [48000, 384000] {
            let format = Format::from_wave_format(&wave(3, 32, 1, rate)).unwrap();
            let mut analyzer = Analyzer::for_meter(format);
            let mut frames = 0;
            analyzer.push(&[], rate, true, |_| frames += 1).unwrap();
            assert!(
                (24..=25).contains(&frames),
                "rate{rate}: {frames} analyses in one second"
            );
        }
    }
}
