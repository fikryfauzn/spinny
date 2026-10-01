use std::{error::Error, fmt, path::Path};

use crate::{
    BurnedDisc,
    format::{FormatError, TrackEntry},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerState {
    Empty,
    Stopped,
    Playing,
    Paused,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerAction {
    Insert,
    Eject,
    Play,
    Pause,
    Stop,
    Next,
    Previous,
    Seek,
    TrackFinished,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerBoundary {
    StartOfDisc,
    EndOfDisc,
}

#[derive(Debug)]
pub enum PlayerError {
    InvalidTransition {
        action: PlayerAction,
        state: PlayerState,
    },
    Boundary {
        action: PlayerAction,
        boundary: PlayerBoundary,
    },
    SeekOutOfRange {
        requested_ms: u64,
        duration_ms: u64,
    },
    Disc(FormatError),
}

impl fmt::Display for PlayerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidTransition { action, state } => {
                write!(f, "player action {action:?} is invalid while {state:?}")
            }
            Self::Boundary { action, boundary } => {
                write!(f, "player action {action:?} reached {boundary:?}")
            }
            Self::SeekOutOfRange {
                requested_ms,
                duration_ms,
            } => write!(
                f,
                "seek target {requested_ms} ms exceeds track duration {duration_ms} ms"
            ),
            Self::Disc(error) => write!(f, "could not insert VDISC: {error}"),
        }
    }
}

impl Error for PlayerError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Disc(error) => Some(error),
            _ => None,
        }
    }
}

impl From<FormatError> for PlayerError {
    fn from(error: FormatError) -> Self {
        Self::Disc(error)
    }
}

pub type PlayerResult<T> = std::result::Result<T, PlayerError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TransportState {
    Stopped,
    Playing,
    Paused,
}

impl TransportState {
    fn public(self) -> PlayerState {
        match self {
            Self::Stopped => PlayerState::Stopped,
            Self::Playing => PlayerState::Playing,
            Self::Paused => PlayerState::Paused,
        }
    }
}

#[derive(Debug)]
struct LoadedPlayer {
    disc: BurnedDisc,
    transport: TransportState,
    track_index: usize,
    position_ms: u64,
}

#[derive(Debug)]
enum PlayerInner {
    Empty,
    Loaded(Box<LoadedPlayer>),
}

/// Pure player behavior for one VDISC. No audio device or decoder is used here.
///
/// State errors are transactional: a failed command never partially mutates
/// track selection, position, transport state, or the inserted disc.
#[derive(Debug)]
pub struct Player {
    inner: PlayerInner,
}

impl Default for Player {
    fn default() -> Self {
        Self::new()
    }
}

impl Player {
    pub fn new() -> Self {
        Self {
            inner: PlayerInner::Empty,
        }
    }

    pub fn state(&self) -> PlayerState {
        match &self.inner {
            PlayerInner::Empty => PlayerState::Empty,
            PlayerInner::Loaded(loaded) => loaded.transport.public(),
        }
    }

    pub fn disc(&self) -> Option<&BurnedDisc> {
        match &self.inner {
            PlayerInner::Empty => None,
            PlayerInner::Loaded(loaded) => Some(&loaded.disc),
        }
    }

    pub fn track_count(&self) -> usize {
        self.disc().map_or(0, BurnedDisc::track_count)
    }

    pub fn current_track_index(&self) -> Option<usize> {
        match &self.inner {
            PlayerInner::Empty => None,
            PlayerInner::Loaded(loaded) => Some(loaded.track_index),
        }
    }

    pub fn current_track(&self) -> Option<&TrackEntry> {
        match &self.inner {
            PlayerInner::Empty => None,
            PlayerInner::Loaded(loaded) => loaded.disc.tracks().get(loaded.track_index),
        }
    }

    pub fn position_ms(&self) -> u64 {
        match &self.inner {
            PlayerInner::Empty => 0,
            PlayerInner::Loaded(loaded) => loaded.position_ms,
        }
    }

    /// Insert and fully verify one VDISC. Failed verification leaves the player
    /// empty. Inserting while another disc is loaded is an invalid transition.
    pub fn insert(&mut self, path: impl AsRef<Path>) -> PlayerResult<()> {
        if !matches!(&self.inner, PlayerInner::Empty) {
            return Err(self.invalid(PlayerAction::Insert));
        }

        let disc = BurnedDisc::open(path)?;
        self.inner = PlayerInner::Loaded(Box::new(LoadedPlayer {
            disc,
            transport: TransportState::Stopped,
            track_index: 0,
            position_ms: 0,
        }));
        Ok(())
    }

    /// Eject from any loaded state, including while playing or paused.
    pub fn eject(&mut self) -> PlayerResult<()> {
        if matches!(&self.inner, PlayerInner::Empty) {
            return Err(self.invalid(PlayerAction::Eject));
        }

        self.inner = PlayerInner::Empty;
        Ok(())
    }

    /// Start from stopped or resume from paused. Calling play while already
    /// playing is rejected rather than treated as an implicit no-op.
    pub fn play(&mut self) -> PlayerResult<()> {
        let state = self.state();
        let Some(loaded) = self.loaded_mut() else {
            return Err(PlayerError::InvalidTransition {
                action: PlayerAction::Play,
                state,
            });
        };

        match loaded.transport {
            TransportState::Stopped | TransportState::Paused => {
                loaded.transport = TransportState::Playing;
                Ok(())
            }
            TransportState::Playing => Err(PlayerError::InvalidTransition {
                action: PlayerAction::Play,
                state,
            }),
        }
    }

    pub fn pause(&mut self) -> PlayerResult<()> {
        let state = self.state();
        let Some(loaded) = self.loaded_mut() else {
            return Err(PlayerError::InvalidTransition {
                action: PlayerAction::Pause,
                state,
            });
        };

        if loaded.transport != TransportState::Playing {
            return Err(PlayerError::InvalidTransition {
                action: PlayerAction::Pause,
                state,
            });
        }

        loaded.transport = TransportState::Paused;
        Ok(())
    }

    /// Stop retains the selected track and rewinds that track to its beginning.
    /// Repeated stop while already stopped is intentionally idempotent.
    pub fn stop(&mut self) -> PlayerResult<()> {
        let state = self.state();
        let Some(loaded) = self.loaded_mut() else {
            return Err(PlayerError::InvalidTransition {
                action: PlayerAction::Stop,
                state,
            });
        };

        loaded.transport = TransportState::Stopped;
        loaded.position_ms = 0;
        Ok(())
    }

    /// Move to the next track without wrapping. Transport state is preserved.
    pub fn next_track(&mut self) -> PlayerResult<()> {
        let state = self.state();
        let Some(loaded) = self.loaded_mut() else {
            return Err(PlayerError::InvalidTransition {
                action: PlayerAction::Next,
                state,
            });
        };

        let next = loaded.track_index + 1;
        if next >= loaded.disc.track_count() {
            return Err(PlayerError::Boundary {
                action: PlayerAction::Next,
                boundary: PlayerBoundary::EndOfDisc,
            });
        }

        loaded.track_index = next;
        loaded.position_ms = 0;
        Ok(())
    }

    /// Move to the previous track without wrapping. Transport state is preserved.
    pub fn previous(&mut self) -> PlayerResult<()> {
        let state = self.state();
        let Some(loaded) = self.loaded_mut() else {
            return Err(PlayerError::InvalidTransition {
                action: PlayerAction::Previous,
                state,
            });
        };

        let Some(previous) = loaded.track_index.checked_sub(1) else {
            return Err(PlayerError::Boundary {
                action: PlayerAction::Previous,
                boundary: PlayerBoundary::StartOfDisc,
            });
        };

        loaded.track_index = previous;
        loaded.position_ms = 0;
        Ok(())
    }

    /// Seek within the selected track while playing or paused.
    ///
    /// If VDISC metadata includes a duration, targets beyond it are rejected.
    /// Duration is optional in V1, so an absent duration does not invent a
    /// boundary the state layer cannot know; Objective 17's decoder remains
    /// responsible for resolving the real media position.
    pub fn seek(&mut self, position_ms: u64) -> PlayerResult<()> {
        let state = self.state();
        let Some(loaded) = self.loaded_mut() else {
            return Err(PlayerError::InvalidTransition {
                action: PlayerAction::Seek,
                state,
            });
        };

        if !matches!(
            loaded.transport,
            TransportState::Playing | TransportState::Paused
        ) {
            return Err(PlayerError::InvalidTransition {
                action: PlayerAction::Seek,
                state,
            });
        }

        if let Some(duration_ms) = loaded.disc.tracks()[loaded.track_index].duration_ms
            && position_ms > duration_ms
        {
            return Err(PlayerError::SeekOutOfRange {
                requested_ms: position_ms,
                duration_ms,
            });
        }

        loaded.position_ms = position_ms;
        Ok(())
    }

    /// Backend event for Objective 17: the currently playing track reached EOF.
    ///
    /// Intermediate tracks advance and keep playing. Finishing the last track
    /// stops the disc and returns selection to track 1 at position zero.
    pub fn track_finished(&mut self) -> PlayerResult<()> {
        let state = self.state();
        let Some(loaded) = self.loaded_mut() else {
            return Err(PlayerError::InvalidTransition {
                action: PlayerAction::TrackFinished,
                state,
            });
        };

        if loaded.transport != TransportState::Playing {
            return Err(PlayerError::InvalidTransition {
                action: PlayerAction::TrackFinished,
                state,
            });
        }

        if loaded.track_index + 1 < loaded.disc.track_count() {
            loaded.track_index += 1;
            loaded.position_ms = 0;
        } else {
            loaded.track_index = 0;
            loaded.position_ms = 0;
            loaded.transport = TransportState::Stopped;
        }

        Ok(())
    }

    fn invalid(&self, action: PlayerAction) -> PlayerError {
        PlayerError::InvalidTransition {
            action,
            state: self.state(),
        }
    }

    fn loaded_mut(&mut self) -> Option<&mut LoadedPlayer> {
        match &mut self.inner {
            PlayerInner::Empty => None,
            PlayerInner::Loaded(loaded) => Some(loaded),
        }
    }
}
