//! Sounds: library audio files (kept as their original bytes) and the sound
//! a keyframe starts, as in Animate's Properties ▸ Sound. Decoding, mixing,
//! waveforms and lip-sync analysis live in the `ac-audio` crate.

use serde::{Deserialize, Serialize};

pub type SoundId = u32;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SoundAsset {
    pub id: SoundId,
    pub name: String,
    /// The imported file (WAV, MP3, OGG Vorbis or FLAC), base64 in JSON.
    #[serde(serialize_with = "crate::bitmap::b64_ser", deserialize_with = "crate::bitmap::b64_de")]
    pub data: Vec<u8>,
}

/// Animate's Sync setting.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum SoundSync {
    /// Plays to its end on its own, whatever the timeline does.
    #[default]
    Event,
    /// Like Event, but not if the same sound is already playing.
    Start,
    /// Silences the sound.
    Stop,
    /// Tied to the timeline: plays only while its frames do, cut at the span end.
    Stream,
}

impl SoundSync {
    pub const ALL: [SoundSync; 4] = [SoundSync::Event, SoundSync::Start, SoundSync::Stop, SoundSync::Stream];
    pub fn name(self) -> &'static str {
        match self {
            SoundSync::Event => "Event",
            SoundSync::Start => "Start",
            SoundSync::Stop => "Stop",
            SoundSync::Stream => "Stream",
        }
    }
    pub fn from_name(s: &str) -> Option<SoundSync> {
        SoundSync::ALL.into_iter().find(|x| x.name().eq_ignore_ascii_case(s.trim()))
    }
}

/// The sound a keyframe starts (or stops).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FrameSound {
    pub sound: SoundId,
    #[serde(default)]
    pub sync: SoundSync,
    /// Repeat count (1 = play once); ignored when `forever`.
    #[serde(default = "one")]
    pub repeat: u32,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub forever: bool,
}

fn one() -> u32 {
    1
}

impl FrameSound {
    pub fn new(sound: SoundId) -> Self {
        FrameSound { sound, sync: SoundSync::Event, repeat: 1, forever: false }
    }
}

/// Mouth shapes for Auto Lip-Sync (Animate's viseme set, simplified).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Viseme {
    Rest,
    Mbp,
    Ai,
    E,
    O,
    U,
    Fv,
    L,
    Wq,
    Etc,
}

impl Viseme {
    pub const ALL: [Viseme; 10] = [Viseme::Rest, Viseme::Mbp, Viseme::Ai, Viseme::E, Viseme::O, Viseme::U, Viseme::Fv, Viseme::L, Viseme::Wq, Viseme::Etc];
    pub fn name(self) -> &'static str {
        match self {
            Viseme::Rest => "Neutral",
            Viseme::Mbp => "MBP",
            Viseme::Ai => "Ai",
            Viseme::E => "E",
            Viseme::O => "O",
            Viseme::U => "U",
            Viseme::Fv => "FV",
            Viseme::L => "L",
            Viseme::Wq => "WQ",
            Viseme::Etc => "etc",
        }
    }
    pub fn from_name(s: &str) -> Option<Viseme> {
        Viseme::ALL.into_iter().find(|v| v.name().eq_ignore_ascii_case(s.trim()))
    }
}

/// Mouth-shape analysis is provided by the audio crate at startup (ac-core
/// stays free of decoders): (sound, start seconds, fps, frames) → shapes.
pub type Analyzer = fn(&SoundAsset, f32, f32, usize) -> Option<Vec<Viseme>>;
static ANALYZER: std::sync::OnceLock<Analyzer> = std::sync::OnceLock::new();
/// Sound length in seconds, also from the audio crate.
pub type Measurer = fn(&SoundAsset) -> Option<f32>;
static MEASURER: std::sync::OnceLock<Measurer> = std::sync::OnceLock::new();

pub fn set_analyzer(a: Analyzer, m: Measurer) {
    let _ = ANALYZER.set(a);
    let _ = MEASURER.set(m);
}
pub fn analyze(s: &SoundAsset, t0: f32, fps: f32, n: usize) -> Option<Vec<Viseme>> {
    ANALYZER.get().and_then(|a| a(s, t0, fps, n))
}
/// True once the audio crate has registered its decoders.
pub fn has_decoder() -> bool {
    MEASURER.get().is_some()
}
pub fn seconds(s: &SoundAsset) -> Option<f32> {
    MEASURER.get().and_then(|m| m(s))
}
