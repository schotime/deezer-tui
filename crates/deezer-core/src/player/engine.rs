use std::io::Cursor;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink};
use tracing::{debug, info};

use crate::api::models::{AudioQuality, DeezerError, TrackData};
use crate::player::state::{PlaybackStatus, PlayerState};

pub struct PlayerEngine {
    state: Arc<Mutex<PlayerState>>,
    _stream: OutputStream,
    stream_handle: OutputStreamHandle,
    sink: Sink,
}

impl PlayerEngine {
    /// Builds the engine on top of an existing shared state handle, so callers that
    /// already handed out `Arc` clones (e.g. MPRIS) keep seeing live updates instead
    /// of a snapshot frozen before the engine existed.
    pub fn new(_master_key: [u8; 16], state: Arc<Mutex<PlayerState>>) -> Result<Self, DeezerError> {
        let (stream, stream_handle) =
            OutputStream::try_default().map_err(|e| DeezerError::Playback(e.to_string()))?;

        let sink =
            Sink::try_new(&stream_handle).map_err(|e| DeezerError::Playback(e.to_string()))?;

        Ok(Self {
            state,
            _stream: stream,
            stream_handle,
            sink,
        })
    }

    pub fn state(&self) -> Arc<Mutex<PlayerState>> {
        Arc::clone(&self.state)
    }

    /// Play pre-fetched and decrypted audio data.
    /// Called on the main thread with audio bytes from a background fetch.
    pub fn play_decoded(
        &mut self,
        audio_data: Vec<u8>,
        track: &TrackData,
        quality: AudioQuality,
    ) -> Result<(), DeezerError> {
        self.load_decoded(audio_data, track, quality, Duration::ZERO, false)
    }

    /// Load decoded audio at a saved position, optionally leaving the sink
    /// paused. Pausing before returning prevents session restore from emitting
    /// audio unexpectedly.
    pub fn load_decoded(
        &mut self,
        audio_data: Vec<u8>,
        track: &TrackData,
        quality: AudioQuality,
        position: Duration,
        start_paused: bool,
    ) -> Result<(), DeezerError> {
        let cursor = Cursor::new(audio_data);
        let source = Decoder::new(cursor)
            .map_err(|e| DeezerError::Playback(format!("Failed to decode audio: {e}")))?;

        // Preserve current volume before recreating the sink
        let current_volume = self.state.lock().unwrap().volume;

        // Clear the current sink and create a fresh one (Sink can't be reused after stop)
        self.sink.stop();
        self.sink =
            Sink::try_new(&self.stream_handle).map_err(|e| DeezerError::Playback(e.to_string()))?;
        self.sink.set_volume(current_volume);
        if start_paused {
            // Pause the empty sink before attaching the source, so no sample can
            // escape between append/seek and the later state update.
            self.sink.pause();
        }
        self.sink.append(source);
        let mut position_secs = position.as_secs().min(track.duration_secs());
        if position_secs > 0 {
            // Some formats may not support seeking; loading from the beginning
            // is preferable to discarding an otherwise valid restored session.
            if let Err(error) = self.sink.try_seek(Duration::from_secs(position_secs)) {
                debug!(%error, "Could not seek restored track");
                position_secs = 0;
            }
        }
        if !start_paused {
            self.sink.play();
        }

        info!(
            title = %track.title,
            artist = %track.artist,
            quality = quality.as_api_format(),
            "Now playing"
        );

        {
            let mut state = self.state.lock().unwrap();
            state.status = if start_paused {
                PlaybackStatus::Paused
            } else {
                PlaybackStatus::Playing
            };
            state.current_track = Some(track.clone());
            state.duration_secs = track.duration_secs();
            state.position_secs = position_secs;
            state.quality = quality;
        }

        Ok(())
    }

    pub fn pause(&self) {
        self.sink.pause();
        let mut state = self.state.lock().unwrap();
        state.status = PlaybackStatus::Paused;
        debug!("Paused");
    }

    pub fn resume(&self) {
        self.sink.play();
        let mut state = self.state.lock().unwrap();
        state.status = PlaybackStatus::Playing;
        debug!("Resumed");
    }

    pub fn toggle_pause(&self) {
        let status = self.state.lock().unwrap().status;
        match status {
            PlaybackStatus::Playing => self.pause(),
            PlaybackStatus::Paused => self.resume(),
            _ => {}
        }
    }

    pub fn stop(&mut self) {
        self.sink.stop();
        // Recreate sink for future use
        if let Ok(new_sink) = Sink::try_new(&self.stream_handle) {
            self.sink = new_sink;
        }
        let mut state = self.state.lock().unwrap();
        state.status = PlaybackStatus::Stopped;
        state.current_track = None;
        state.position_secs = 0;
        state.duration_secs = 0;
        debug!("Stopped");
    }

    pub fn set_volume(&self, volume: f32) {
        let volume = volume.clamp(0.0, 1.0);
        self.sink.set_volume(volume);
        self.state.lock().unwrap().volume = volume;
    }

    pub fn volume(&self) -> f32 {
        self.state.lock().unwrap().volume
    }

    pub fn try_seek(&self, pos: Duration) -> Result<(), DeezerError> {
        self.sink
            .try_seek(pos)
            .map_err(|e| DeezerError::Playback(format!("Seek failed: {e}")))
    }

    pub fn is_finished(&self) -> bool {
        self.sink.empty()
    }
}
