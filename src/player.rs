//! Audio output, wrapping rodio's device sink and player.

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use rodio::{MixerDeviceSink, Player, Source};

use crate::cli::Output;

/// The audio device and the player feeding it.
///
/// The sink must outlive the player: dropping it stops the sound. Opening
/// the device is deferred until the first track plays, so a machine without
/// one can still browse its library.
pub struct Audio {
    /// The open output device.
    _device: Device,
    /// The queue feeding the device.
    player: Player,
    /// Whether playback was stopped, so the position reads zero until the
    /// next track starts.
    stopped: AtomicBool,
}

/// The output the player is feeding.
///
/// Held only so it is dropped with the player, which closes the device.
#[allow(dead_code)]
enum Device {
    /// cpal's ALSA stream, the default on every platform.
    Alsa(MixerDeviceSink),
    /// A stream opened in the sound server directly, on Linux.
    #[cfg(target_os = "linux")]
    Pulse(crate::pulse::Sink),
}

impl Audio {
    /// Opens the output device `output` names.
    pub fn open(volume: f32, output: Output) -> Result<Self, String> {
        match output {
            Output::Alsa => Self::open_alsa(volume),
            #[cfg(target_os = "linux")]
            Output::Pulse => Self::open_pulse(volume),
            #[cfg(not(target_os = "linux"))]
            Output::Pulse => Err("PulseAudio output is only available on Linux".to_string()),
        }
    }

    /// Opens cpal's default stream.
    fn open_alsa(volume: f32) -> Result<Self, String> {
        let sink = rodio::DeviceSinkBuilder::open_default_sink()
            .map_err(|error| format!("no audio output: {error}"))?;
        let player = Player::connect_new(sink.mixer());
        player.set_volume(volume);
        Ok(Self {
            _device: Device::Alsa(sink),
            player,
            stopped: AtomicBool::new(true),
        })
    }

    /// Opens a stream in the sound server itself.
    ///
    /// rodio's mixer is the same one the ALSA path uses, but the samples are
    /// pulled by [`crate::pulse`] rather than by a cpal callback.
    #[cfg(target_os = "linux")]
    fn open_pulse(volume: f32) -> Result<Self, String> {
        let (mixer, source) = rodio::mixer::mixer(
            rodio::ChannelCount::new(crate::pulse::CHANNELS).expect("two is a channel count"),
            rodio::SampleRate::new(crate::pulse::RATE).expect("48 kHz is a sample rate"),
        );
        let player = Player::connect_new(&mixer);
        player.set_volume(volume);
        let sink = crate::pulse::Sink::start(source)?;
        Ok(Self {
            _device: Device::Pulse(sink),
            player,
            stopped: AtomicBool::new(true),
        })
    }

    /// Replaces whatever plays with `path`, and starts it.
    ///
    /// Returns the file's duration as the decoder reports it, which is more
    /// exact than the tag's and what the seek bar is scaled against.
    pub fn play_file(&self, path: &Path) -> Result<Option<Duration>, String> {
        self.player.clear();
        self.stopped.store(false, Ordering::Relaxed);

        // A surround stream is folded to stereo here rather than by the
        // mixer, which would keep the front pair and drop the rest of the
        // mix with it.
        let source = crate::downmix::stereo(crate::decode::open(path)?);
        let duration = source.total_duration();
        self.player.append(source);
        self.player.play();
        Ok(duration)
    }

    /// Pauses when playing and resumes when paused.
    pub fn toggle(&self) {
        if self.player.is_paused() {
            self.player.play();
        } else {
            self.player.pause();
        }
    }

    /// Whether playback is paused, or has nothing loaded.
    pub fn is_paused(&self) -> bool {
        self.player.is_paused() || self.player.empty()
    }

    /// Stops playback and rewinds to the start.
    ///
    /// The device stays open, so the next [`play_file`](Self::play_file)
    /// starts without reopening it. This does not wait for the audio thread,
    /// so a wedged device cannot block the window.
    pub fn stop(&self) {
        self.stopped.store(true, Ordering::Relaxed);
        self.player.stop();
        self.player.pause();
    }

    /// Moves playback to `position`, clamped by the decoder to the track end.
    ///
    /// Returns why the decoder refused, which is worth showing: a format it
    /// cannot seek would otherwise look like a click that did nothing.
    pub fn seek(&self, position: Duration) -> Result<(), String> {
        self.player
            .try_seek(position)
            .map_err(|error| error.to_string())
    }

    /// Where playback is now.
    ///
    /// A stopped player reports zero rather than where the track was cut
    /// off, which is what the seek bar and the table should show.
    pub fn position(&self) -> Duration {
        if self.stopped.load(Ordering::Relaxed) {
            Duration::ZERO
        } else {
            self.player.get_pos()
        }
    }

    /// Whether the queue has run out, which is how a finished track ends.
    pub fn is_empty(&self) -> bool {
        self.player.empty()
    }

    /// Sets the output volume, from 0 to 1.
    pub fn set_volume(&self, volume: f32) {
        self.player.set_volume(volume);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The volume a device would be given is the one the app stores, so a
    /// slider move that reaches `Audio::set_volume` is applied. A machine
    /// without an output device skips this rather than failing.
    #[test]
    fn setting_the_volume_reaches_the_player() {
        let Ok(audio) = Audio::open(0.5, Output::Alsa) else {
            return;
        };
        audio.set_volume(0.25);
        assert_eq!(audio.player.volume(), 0.25);
    }
}
