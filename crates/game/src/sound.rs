//! The sound engine: plays the WAV files that `tools assetgen` makes (D-034).
//!
//! The sounds are part of the program (`include_bytes!`). A sound that cannot load or play
//! never stops the game: the engine goes silent for that sound. The mixer has one volume for
//! each group (effects, ambience, music) and a master volume. Effects use a short cool-down so
//! a crowd of colonists does not make a wall of noise.

use macroquad::audio::{self, PlaySoundParams};
use sim::{Alert, AlertKind, AlertLevel};

/// A sound of the game. The WAV file has the name of the sound.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SoundId {
    /// A new alert of level `Info`.
    AlertInfo,
    /// A new alert of level `Warning`.
    AlertWarning,
    /// A new alert of level `Danger`.
    AlertDanger,
    /// The player pressed a button.
    UiClick,
    /// A colonist reached a goal.
    Footstep,
    /// A colonist finished cutting a tree.
    ToolChop,
    /// A colonist finished mining a tile.
    ToolMine,
    /// A workshop finished a recipe.
    CraftDone,
}

/// Every sound, in file order.
pub const ALL_SOUNDS: [SoundId; 8] = [
    SoundId::AlertInfo,
    SoundId::AlertWarning,
    SoundId::AlertDanger,
    SoundId::UiClick,
    SoundId::Footstep,
    SoundId::ToolChop,
    SoundId::ToolMine,
    SoundId::CraftDone,
];

/// A group of sounds with one volume.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Group {
    /// Short sounds of actions and alerts.
    Effects,
    /// Long loops of weather and places (S16). Not used yet.
    #[allow(dead_code)]
    Ambience,
    /// Music (S15). Not used yet.
    #[allow(dead_code)]
    Music,
}

impl SoundId {
    /// The embedded WAV file.
    fn bytes(self) -> &'static [u8] {
        match self {
            Self::AlertInfo => include_bytes!("../../../assets/sounds/alert_info.wav"),
            Self::AlertWarning => include_bytes!("../../../assets/sounds/alert_warning.wav"),
            Self::AlertDanger => include_bytes!("../../../assets/sounds/alert_danger.wav"),
            Self::UiClick => include_bytes!("../../../assets/sounds/ui_click.wav"),
            Self::Footstep => include_bytes!("../../../assets/sounds/footstep.wav"),
            Self::ToolChop => include_bytes!("../../../assets/sounds/tool_chop.wav"),
            Self::ToolMine => include_bytes!("../../../assets/sounds/tool_mine.wav"),
            Self::CraftDone => include_bytes!("../../../assets/sounds/craft_done.wav"),
        }
    }

    /// The group of the sound.
    pub fn group(self) -> Group {
        Group::Effects
    }

    /// Frames that must pass before the same sound can play again.
    fn cooldown(self) -> u32 {
        match self {
            Self::UiClick => 3,
            Self::Footstep => 12,
            Self::ToolChop | Self::ToolMine | Self::CraftDone => 8,
            Self::AlertInfo | Self::AlertWarning | Self::AlertDanger => 30,
        }
    }

    fn index(self) -> usize {
        ALL_SOUNDS.iter().position(|&s| s == self).unwrap_or(0)
    }
}

/// The volumes of the mixer, in percent (0 to 100).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Mixer {
    /// Master volume.
    pub master: u8,
    /// Volume of the effects.
    pub effects: u8,
    /// Volume of the ambience.
    pub ambience: u8,
    /// Volume of the music.
    pub music: u8,
}

impl Default for Mixer {
    fn default() -> Self {
        Self {
            master: 70,
            effects: 100,
            ambience: 60,
            music: 50,
        }
    }
}

impl Mixer {
    /// The playback volume (0.0 to 1.0) of a group.
    pub fn gain(&self, group: Group) -> f32 {
        let part = match group {
            Group::Effects => self.effects,
            Group::Ambience => self.ambience,
            Group::Music => self.music,
        };
        f32::from(self.master.min(100)) / 100.0 * f32::from(part.min(100)) / 100.0
    }
}

/// The alert identity: the same alert has the same key when its numbers change.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AlertKey(u8, Option<(u16, u16)>, Option<u32>);

fn alert_key(alert: &Alert) -> AlertKey {
    let (tag, colonist) = match alert.kind {
        AlertKind::LowFuel { .. } => (0, None),
        AlertKind::ColdPeriod { .. } => (1, None),
        AlertKind::Hunger {
            colonist, starving, ..
        } => (2 + u8::from(starving), Some(colonist.index())),
        AlertKind::Hypothermia { colonist, .. } => (4, Some(colonist.index())),
        AlertKind::Heatstroke { colonist, .. } => (5, Some(colonist.index())),
        AlertKind::LowMood { colonist, .. } => (6, Some(colonist.index())),
        AlertKind::Death { .. } => (7, None),
    };
    // A colonist alert is the same alert when the colonist moves; a tile alert is not.
    let tile = if colonist.is_some() {
        None
    } else {
        alert.tile()
    };
    AlertKey(tag, tile, colonist)
}

/// The keys of the alerts now.
pub fn alert_keys(alerts: &[Alert]) -> Vec<AlertKey> {
    alerts.iter().map(alert_key).collect()
}

/// The sound for the most urgent alert that is not in `before`. `None` if no alert is new.
pub fn new_alert_sound(before: &[AlertKey], alerts: &[Alert]) -> Option<SoundId> {
    let level = alerts
        .iter()
        .filter(|a| !before.contains(&alert_key(a)))
        .map(Alert::level)
        .max()?;
    Some(match level {
        AlertLevel::Info => SoundId::AlertInfo,
        AlertLevel::Warning => SoundId::AlertWarning,
        AlertLevel::Danger => SoundId::AlertDanger,
    })
}

/// The loaded sounds and the mixer.
pub struct Engine {
    sounds: Vec<Option<audio::Sound>>,
    /// The mixer volumes.
    pub mixer: Mixer,
    frame: u32,
    last: [Option<u32>; ALL_SOUNDS.len()],
    muted: bool,
}

impl Engine {
    /// Load every sound. A sound that does not load stays silent. `PCS_MUTE=1` mutes the game.
    pub async fn load() -> Self {
        let mut sounds = Vec::with_capacity(ALL_SOUNDS.len());
        for id in ALL_SOUNDS {
            sounds.push(audio::load_sound_from_bytes(id.bytes()).await.ok());
        }
        Self {
            sounds,
            mixer: Mixer::default(),
            frame: 0,
            last: [None; ALL_SOUNDS.len()],
            muted: std::env::var("PCS_MUTE").is_ok_and(|v| v == "1"),
        }
    }

    /// Call once for each frame. It drives the cool-downs.
    pub fn next_frame(&mut self) {
        self.frame = self.frame.wrapping_add(1);
    }

    /// Play a sound once. It is skipped inside its cool-down.
    pub fn play(&mut self, id: SoundId) {
        let i = id.index();
        if self.muted {
            return;
        }
        if let Some(t) = self.last[i]
            && self.frame.wrapping_sub(t) < id.cooldown()
        {
            return;
        }
        let volume = self.mixer.gain(id.group());
        if volume <= 0.0 {
            return;
        }
        if let Some(Some(sound)) = self.sounds.get(i) {
            audio::play_sound(
                sound,
                PlaySoundParams {
                    looped: false,
                    volume,
                },
            );
            self.last[i] = Some(self.frame);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wav_data_len(bytes: &[u8]) -> Option<usize> {
        if bytes.get(0..4)? != b"RIFF" || bytes.get(8..12)? != b"WAVE" {
            return None;
        }
        let rate = u32::from_le_bytes(bytes.get(24..28)?.try_into().ok()?);
        let bits = u16::from_le_bytes(bytes.get(34..36)?.try_into().ok()?);
        let channels = u16::from_le_bytes(bytes.get(22..24)?.try_into().ok()?);
        (rate == 44_100 && bits == 16 && channels == 1).then_some(bytes.len() - 44)
    }

    #[test]
    fn every_sound_is_a_16_bit_mono_wav_that_is_not_empty() {
        for id in ALL_SOUNDS {
            let n = wav_data_len(id.bytes());
            assert!(n.is_some_and(|n| n > 100), "{id:?} is not a valid WAV");
        }
    }

    #[test]
    fn indexes_follow_the_sound_list() {
        for (i, id) in ALL_SOUNDS.iter().enumerate() {
            assert_eq!(id.index(), i);
        }
    }

    #[test]
    fn master_and_group_volume_multiply_and_clamp() {
        let m = Mixer {
            master: 50,
            effects: 50,
            ambience: 200,
            music: 0,
        };
        assert!((m.gain(Group::Effects) - 0.25).abs() < 1e-6);
        assert!((m.gain(Group::Ambience) - 0.5).abs() < 1e-6);
        assert_eq!(m.gain(Group::Music), 0.0);
    }

    fn alert(kind: AlertKind) -> Alert {
        Alert { kind }
    }

    fn cold() -> Alert {
        alert(AlertKind::ColdPeriod {
            days_ahead: 0,
            offset: -80,
        })
    }

    fn death() -> Alert {
        alert(AlertKind::Death {
            x: 1,
            y: 2,
            cause: sim::DeathCause::Starvation,
            hours_ago: 0,
        })
    }

    #[test]
    fn no_sound_if_no_alert_is_new() {
        let now = [cold()];
        assert_eq!(new_alert_sound(&alert_keys(&now), &now), None);
        assert_eq!(new_alert_sound(&[], &[]), None);
    }

    #[test]
    fn a_new_alert_plays_the_sound_of_its_level() {
        assert_eq!(new_alert_sound(&[], &[cold()]), Some(SoundId::AlertInfo));
        assert_eq!(new_alert_sound(&[], &[death()]), Some(SoundId::AlertDanger));
    }

    fn fuel(hours: u32, x: u16) -> Alert {
        alert(AlertKind::LowFuel { x, y: 0, hours })
    }

    #[test]
    fn the_most_urgent_new_alert_wins() {
        let before = alert_keys(&[cold()]);
        assert_eq!(
            new_alert_sound(&before, &[cold(), fuel(2, 1), death()]),
            Some(SoundId::AlertDanger)
        );
    }

    #[test]
    fn a_changed_number_is_not_a_new_alert_but_a_new_tile_is() {
        let before = alert_keys(&[fuel(5, 1)]);
        assert_eq!(new_alert_sound(&before, &[fuel(4, 1)]), None);
        assert_eq!(
            new_alert_sound(&before, &[fuel(4, 1), fuel(4, 2)]),
            Some(SoundId::AlertInfo)
        );
    }
}
