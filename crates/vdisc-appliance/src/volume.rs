use std::{error::Error, fmt};

use crate::state::{D_E200_AVLS_VOLUME_CEILING, De200Controller, Volume};

/// Long-MENU action owned by the AVLS policy layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AvlsAction {
    Toggle,
}

/// Why an AVLS command was rejected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AvlsTransitionErrorKind {
    HoldEnabled,
}

/// A long-MENU AVLS action was blocked by appliance policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AvlsTransitionError {
    action: AvlsAction,
    hold_enabled: bool,
    kind: AvlsTransitionErrorKind,
}

impl AvlsTransitionError {
    pub const fn action(self) -> AvlsAction {
        self.action
    }

    pub const fn hold_enabled(self) -> bool {
        self.hold_enabled
    }

    pub const fn kind(self) -> AvlsTransitionErrorKind {
        self.kind
    }
}

impl fmt::Display for AvlsTransitionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "AVLS action {:?} is invalid while hold is {}: {:?}",
            self.action, self.hold_enabled, self.kind
        )
    }
}

impl Error for AvlsTransitionError {}

impl De200Controller {
    /// Apply a requested normalized machine volume.
    ///
    /// Volume remains operable while HOLD is enabled. This is an explicit VDISC
    /// Objective 10 rule: HOLD locks button/MENU transport controls, while the
    /// separate volume control remains adjustable. When AVLS is enabled, a
    /// request above the configured ceiling is clamped deterministically.
    pub fn request_set_volume(&mut self, requested: Volume) -> Volume {
        let applied = if self.avls_enabled()
            && requested.normalized() > D_E200_AVLS_VOLUME_CEILING.normalized()
        {
            D_E200_AVLS_VOLUME_CEILING
        } else {
            requested
        };

        self.commit_volume(applied);
        applied
    }

    /// Handle long MENU by toggling AVLS.
    ///
    /// HOLD locks MENU, so long-MENU is rejected while HOLD is active. Enabling
    /// AVLS immediately clamps an already-higher current volume. Disabling AVLS
    /// restores the permitted range but deliberately does not restore any prior
    /// louder value.
    pub fn request_toggle_avls(&mut self) -> Result<bool, AvlsTransitionError> {
        if self.ensure_controls_unlocked().is_err() {
            return Err(AvlsTransitionError {
                action: AvlsAction::Toggle,
                hold_enabled: true,
                kind: AvlsTransitionErrorKind::HoldEnabled,
            });
        }

        let enabled = !self.avls_enabled();
        self.commit_avls_enabled(enabled);

        if enabled && self.volume().normalized() > D_E200_AVLS_VOLUME_CEILING.normalized() {
            self.commit_volume(D_E200_AVLS_VOLUME_CEILING);
        }

        Ok(enabled)
    }
}
