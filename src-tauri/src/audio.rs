use std::{
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

/// Owns the mute flag and a lightweight LCG seed for variant/threshold randomness.
/// Lives exclusively in the poll thread; `muted` is cloned into Tauri state for the command.
pub struct AudioPlayer {
    pub muted: Arc<AtomicBool>,
    seed: u64,
}

impl AudioPlayer {
    pub fn new() -> Self {
        use std::time::{SystemTime, UNIX_EPOCH};
        let seed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .subsec_nanos() as u64;
        Self {
            muted: Arc::new(AtomicBool::new(false)),
            seed,
        }
    }

    /// Pseudo-random integer in 1..=5 for variant selection (LCG).
    pub fn rand_1_5(&mut self) -> u8 {
        self.seed = self
            .seed
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        ((self.seed >> 33) % 5 + 1) as u8
    }

    pub fn rand_1_5_avoiding(&mut self, last_variant: Option<u8>) -> u8 {
        let variant = self.rand_1_5();
        match last_variant {
            Some(last) if (1..=5).contains(&last) && variant == last => (variant % 5) + 1,
            _ => variant,
        }
    }

    /// Returns a pseudo-random play threshold in 8..=14.
    /// The poll loop increments a request counter each time fill_pct rises;
    /// when the counter hits this threshold an audio line plays and the counter resets.
    /// This keeps clips from getting too sparse while still avoiding frequent chatter,
    /// giving roughly 40–70 s between cues at a 5 s poll interval.
    pub fn rand_threshold(&mut self) -> u8 {
        self.seed = self
            .seed
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        ((self.seed >> 33) % 7 + 8) as u8
    }

    /// Spawn a detached thread that plays `<state>_<variant>.mp3`.
    /// No-op when muted or state is stale/unavailable.
    /// If muted mid-playback, the sink is stopped immediately.
    pub fn play(&self, state: &str, variant: u8, resource_dir: &Path) {
        if self.muted.load(Ordering::Relaxed) {
            return;
        }
        if matches!(state, "stale" | "unavailable") {
            return;
        }
        let path = resource_dir
            .join("assets")
            .join("audio")
            .join(format!("{}_{}.mp3", state, variant));
        let muted = self.muted.clone();
        std::thread::spawn(move || {
            if let Err(e) = play_mp3_blocking(&path, muted) {
                eprintln!("[audio] playback error: {e}");
            }
        });
    }
}

pub fn play_mp3_blocking(
    path: &std::path::Path,
    muted: Arc<AtomicBool>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let (_stream, handle) = rodio::OutputStream::try_default()?;
    let sink = rodio::Sink::try_new(&handle)?;
    let file = std::fs::File::open(path)?;
    let source = rodio::Decoder::new(std::io::BufReader::new(file))?;
    sink.append(source);
    // Poll every 100ms so muting takes effect within one tick.
    while !sink.empty() {
        if muted.load(Ordering::Relaxed) {
            sink.stop();
            return Ok(());
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    Ok(())
}
