//! Opt-in playback capture lifetime. Only derived analysis reaches the caller.
use crate::audio_endpoints::Role;
use secbird_core::audio_analysis::{Format, MeterFrame};
use serde::Serialize;
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver},
        Arc, Mutex, OnceLock,
    },
    time::{Duration, Instant},
};
#[derive(Clone)]
pub(crate) enum Choice {
    Default(Role),
    // TODO: retain explicit endpoint selection until the desktop exposes an opt-in selector.
    // The backend contract is exercised by capture fixtures; no integrated selector is claimed.
    #[allow(dead_code)]
    Exact(String),
}
#[derive(Clone)]
pub(crate) struct Config {
    pub choice: Choice,
    pub duration: Option<Duration>,
}
impl Config {
    fn validate(&self) -> Result<(), String> {
        if let Choice::Exact(id) = &self.choice {
            if id.is_empty() || id.contains('\0') || id.encode_utf16().count() > 4095 {
                return Err("Invalid playback endpoint identity".into());
            }
        }
        if self
            .duration
            .is_some_and(|duration| duration.is_zero() || duration > Duration::from_secs(86400))
        {
            return Err("Capture duration must be positive and at most24 hours".into());
        }
        Ok(())
    }
}
#[derive(Debug, Serialize)]
pub(crate) struct Report {
    pub endpoint_id: String,
    pub format: Format,
    pub native_buffer_frames: u32,
    pub packets: u64,
    pub sample_frames: u64,
    pub silent_frames: u64,
    pub analysis_frames: u64,
    pub discontinuities: u64,
    pub timestamp_errors: u64,
    pub stop_succeeded: bool,
    pub analysis_storage_bytes: usize,
    pub packet_storage_bytes: usize,
}
struct Admission(Arc<AtomicBool>);
impl Drop for Admission {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}
type Latest = Arc<Mutex<Option<(Instant, MeterFrame)>>>;
pub(crate) struct Capture {
    cancel: Arc<AtomicBool>,
    latest: Latest,
    completion: Receiver<Result<Report, String>>,
    finished: bool,
}
impl Capture {
    pub fn start(config: Config) -> Result<Self, String> {
        static BUSY: OnceLock<Arc<AtomicBool>> = OnceLock::new();
        Self::spawn(
            config,
            BUSY.get_or_init(|| Arc::new(AtomicBool::new(false)))
                .clone(),
            |config, cancel, sink| crate::audio_endpoints::capture(config, cancel, sink),
        )
    }
    fn spawn(
        config: Config,
        busy: Arc<AtomicBool>,
        work: impl FnOnce(&Config, &AtomicBool, &mut dyn FnMut(MeterFrame)) -> Result<Report, String>
            + Send
            + 'static,
    ) -> Result<Self, String> {
        config.validate()?;
        if busy
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return Err("Previous playback capture is still running or draining".into());
        }
        let admission = Admission(busy);
        let cancel = Arc::new(AtomicBool::new(false));
        let latest: Latest = Arc::new(Mutex::new(None));
        let (sender, completion) = mpsc::sync_channel(1);
        let stop = cancel.clone();
        let frames = latest.clone();
        std::thread::Builder::new()
            .name("audio-loopback".into())
            .stack_size(1024 * 1024)
            .spawn(move || {
                let result = work(&config, &stop, &mut |frame| {
                    if let Ok(mut latest) = frames.lock() {
                        if !stop.load(Ordering::Acquire) {
                            *latest = Some((Instant::now(), frame));
                        }
                    }
                });
                if let Ok(mut latest) = frames.lock() {
                    *latest = None;
                }
                drop(admission);
                let _ = sender.send(result);
            })
            .map_err(|error| error.to_string())?;
        Ok(Self {
            cancel,
            latest,
            completion,
            finished: false,
        })
    }
    pub fn stop(&self) {
        self.cancel.store(true, Ordering::Release);
        if let Ok(mut latest) = self.latest.lock() {
            *latest = None;
        }
    }
    // TODO: connect a consented meter consumer; preserve bounded, stale-frame-filtered readback.
    // Keeping this API does not claim a production audio-to-lighting runtime.
    #[allow(dead_code)]
    pub fn latest(&self) -> Option<MeterFrame> {
        if self.cancel.load(Ordering::Acquire) {
            return None;
        }
        self.latest
            .lock()
            .ok()?
            .as_ref()
            .filter(|(time, _)| time.elapsed() < Duration::from_millis(250))
            .map(|(_, frame)| frame.clone())
    }
    pub fn try_finish(&mut self) -> Option<Result<Report, String>> {
        if self.finished {
            return None;
        }
        match self.completion.try_recv() {
            Ok(result) => {
                self.finished = true;
                Some(result)
            }
            Err(mpsc::TryRecvError::Empty) => None,
            Err(_) => {
                self.finished = true;
                Some(Err("Playback worker ended without a cleanup result".into()))
            }
        }
    }
}
impl Drop for Capture {
    fn drop(&mut self) {
        self.stop();
    }
}
pub(crate) fn diagnostic(args: &[String]) -> Result<(), String> {
    use std::io::Write;
    if args.len() != 3 || args[0] != "--audio-capture-default" || args[2].starts_with("--") {
        return Err(
            "Use --audio-capture-default SECONDS NEW_REPORT.json alone (1..30 seconds)".into(),
        );
    }
    let seconds = args[1]
        .parse::<u64>()
        .ok()
        .filter(|value| (1..=30).contains(value))
        .ok_or("Capture diagnostic requires1..30 seconds")?;
    let mut output = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&args[2])
        .map_err(|error| error.to_string())?;
    let mut capture = Capture::start(Config {
        choice: Choice::Default(Role::Multimedia),
        duration: Some(Duration::from_secs(seconds)),
    })?;
    let deadline = Instant::now() + Duration::from_secs(seconds + 5);
    let result = loop {
        if let Some(result) = capture.try_finish() {
            break result;
        }
        if Instant::now() >= deadline {
            capture.stop();
            break Err("Playback capture timed out; OS cleanup is not confirmed".into());
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    let value = match &result {
        Ok(report) => serde_json::to_value(report).map_err(|error| error.to_string())?,
        Err(error) => serde_json::json!({"error":error}),
    };
    output
        .write_all(&serde_json::to_vec_pretty(&value).map_err(|error| error.to_string())?)
        .map_err(|error| error.to_string())?;
    output.write_all(b"\n").map_err(|error| error.to_string())?;
    result.map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn format() -> Format {
        Format::from_wave_format(&[3, 0, 2, 0, 128, 187, 0, 0, 0, 220, 5, 0, 8, 0, 32, 0, 0, 0])
            .unwrap()
    }
    fn frame() -> MeterFrame {
        MeterFrame {
            sample_rate: 48000,
            fft_size: 2048,
            bin_hz: 23.4375,
            rms: 0.1,
            peak: 0.2,
            magnitudes: vec![0.1; 1025].into_boxed_slice(),
        }
    }
    fn report() -> Report {
        Report {
            endpoint_id: "test".into(),
            format: format(),
            native_buffer_frames: 480,
            packets: 1,
            sample_frames: 480,
            silent_frames: 0,
            analysis_frames: 1,
            discontinuities: 0,
            timestamp_errors: 0,
            stop_succeeded: true,
            analysis_storage_bytes: 1,
            packet_storage_bytes: 1,
        }
    }
    fn config() -> Config {
        Config {
            choice: Choice::Default(Role::Multimedia),
            duration: None,
        }
    }
    fn finish(capture: &mut Capture) -> Result<Report, String> {
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            if let Some(result) = capture.try_finish() {
                return result;
            }
            if Instant::now() > deadline {
                return Err("fake capture failed to drain".into());
            }
            std::thread::sleep(Duration::from_millis(1));
        }
    }
    #[test]
    fn stop_hides_late_frames_and_waits_for_cleanup_before_readmitting_a_capture() {
        let busy = Arc::new(AtomicBool::new(false));
        let (ready, started) = mpsc::channel();
        let (release, drain) = mpsc::channel();
        let mut first = Capture::spawn(config(), busy.clone(), move |_, stop, sink| {
            sink(frame());
            ready.send(()).unwrap();
            while !stop.load(Ordering::Acquire) {
                std::thread::sleep(Duration::from_millis(1));
            }
            sink(frame());
            drain.recv().unwrap();
            Ok(report())
        })
        .unwrap();
        started.recv_timeout(Duration::from_secs(2)).unwrap();
        assert!(first.latest().is_some());
        if let Some((time, _)) = first.latest.lock().unwrap().as_mut() {
            *time = Instant::now() - Duration::from_millis(251);
        }
        assert!(first.latest().is_none());
        first.stop();
        assert!(first.latest().is_none());
        assert!(first.try_finish().is_none());
        assert!(
            Capture::spawn(config(), busy.clone(), |_, _, _| panic!("duplicate worker")).is_err()
        );
        assert!(busy.load(Ordering::Acquire));
        release.send(()).unwrap();
        assert!(finish(&mut first).unwrap().stop_succeeded);
        assert!(first.try_finish().is_none());
        let (ready, started) = mpsc::channel();
        let mut second = Capture::spawn(config(), busy.clone(), move |_, stop, sink| {
            sink(frame());
            ready.send(()).unwrap();
            while !stop.load(Ordering::Acquire) {
                std::thread::sleep(Duration::from_millis(1));
            }
            Ok(report())
        })
        .unwrap();
        started.recv_timeout(Duration::from_secs(2)).unwrap();
        drop(first);
        assert!(
            second.latest().is_some(),
            "old handle stopped the new capture"
        );
        second.stop();
        assert!(finish(&mut second).is_ok());
        assert!(!busy.load(Ordering::Acquire));
    }
    #[test]
    fn malformed_choices_and_duration_refuse_before_worker_admission() {
        for id in [String::new(), "bad\0id".into(), "x".repeat(4096)] {
            let busy = Arc::new(AtomicBool::new(false));
            assert!(Capture::spawn(
                Config {
                    choice: Choice::Exact(id),
                    duration: None
                },
                busy.clone(),
                |_, _, _| panic!("invalid source opened")
            )
            .is_err());
            assert!(!busy.load(Ordering::Acquire));
        }
        let mut invalid = config();
        invalid.duration = Some(Duration::ZERO);
        assert!(invalid.validate().is_err());
        assert!(diagnostic(&[
            "--audio-capture-default".into(),
            "0".into(),
            "ignored.json".into()
        ])
        .is_err());
    }
    #[test]
    #[ignore = "explicitly captures playback-derived analysis briefly; no raw audio is saved"]
    fn live_default_and_exact_playback_capture_stop_and_reopen() {
        let mut default = Capture::start(Config {
            choice: Choice::Default(Role::Multimedia),
            duration: Some(Duration::from_millis(200)),
        })
        .unwrap();
        let first = finish(&mut default).unwrap();
        assert!(first.stop_succeeded);
        assert!(first.analysis_storage_bytes < 600000);
        assert!(default.latest().is_none());
        let mut exact = Capture::start(Config {
            choice: Choice::Exact(first.endpoint_id.clone()),
            duration: None,
        })
        .unwrap();
        std::thread::sleep(Duration::from_millis(100));
        exact.stop();
        assert!(exact.latest().is_none());
        let second = finish(&mut exact).unwrap();
        assert!(second.stop_succeeded);
        assert_eq!(second.endpoint_id, first.endpoint_id);
        eprintln!(
            "Playback lifecycle: default packets={}, exact packets={}, stop/reopen confirmed",
            first.packets, second.packets
        );
    }
}
