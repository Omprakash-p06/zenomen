//! Reusable action button component for GPUI-CE.

use crate::core::events::AppCommand;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonVariant {
    Primary,
    Secondary,
    Danger,
    Ghost,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ActionButton {
    pub label: String,
    pub variant: ButtonVariant,
    pub is_enabled: bool,
    pub is_loading: bool,
    pub command: Option<AppCommand>,
}

impl ActionButton {
    pub fn new(label: impl Into<String>, variant: ButtonVariant) -> Self {
        Self {
            label: label.into(),
            variant,
            is_enabled: true,
            is_loading: false,
            command: None,
        }
    }

    pub fn with_command(mut self, cmd: AppCommand) -> Self {
        self.command = Some(cmd);
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.is_enabled = !disabled;
        self
    }

    pub fn loading(mut self, loading: bool) -> Self {
        self.is_loading = loading;
        self
    }
}
