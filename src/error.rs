//! [`FitsError`]: CFITSIO status plus an optional message stack.

use std::cell::RefCell;
use std::error::Error;
use std::fmt;

use crate::status;

thread_local! {
    static MSG_STACK: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

/// Push a message onto the per-thread analogue of CFITSIO's `ffpmsg` stack.
pub fn push_err_msg(msg: impl Into<String>) {
    MSG_STACK.with(|s| s.borrow_mut().push(msg.into()));
}

/// Pop the oldest message (`fits_read_errmsg` / `ffgmsg`).
pub fn pop_err_msg() -> Option<String> {
    MSG_STACK.with(|s| {
        let mut v = s.borrow_mut();
        if v.is_empty() {
            None
        } else {
            Some(v.remove(0))
        }
    })
}

/// Clear the message stack (`fits_clear_errmsg` / `ffcmsg`).
pub fn clear_err_msg() {
    MSG_STACK.with(|s| s.borrow_mut().clear());
}

/// An error identified by a CFITSIO status code.
///
/// `status` is the `int` CFITSIO would have written through `int *status`.
/// `messages` is the per-call analogue of the `ffpmsg` stack (not yet a
/// process-global stack).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FitsError {
    /// CFITSIO status code.
    pub status: i32,
    messages: Vec<String>,
}

impl FitsError {
    /// Error with a status code and an empty message stack.
    #[must_use]
    pub fn new(status: i32) -> Self {
        Self {
            status,
            messages: Vec::new(),
        }
    }

    /// Error with a status code and one stack message.
    #[must_use]
    pub fn with_message(status: i32, msg: impl Into<String>) -> Self {
        Self {
            status,
            messages: vec![msg.into()],
        }
    }

    /// Short `ffgerr` text for [`Self::status`].
    #[must_use]
    pub fn status_text(&self) -> &'static str {
        status::status_text(self.status)
    }

    /// Messages that would appear on CFITSIO's error stack for this call.
    #[must_use]
    pub fn messages(&self) -> &[String] {
        &self.messages
    }

    /// Append a stack message (analogue of `ffpmsg`).
    pub fn push_message(&mut self, msg: impl Into<String>) {
        self.messages.push(msg.into());
    }
}

impl fmt::Display for FitsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({})", self.status_text(), self.status)?;
        for msg in &self.messages {
            write!(f, ": {msg}")?;
        }
        Ok(())
    }
}

impl Error for FitsError {}

/// Alias used throughout the crate.
pub type Result<T> = std::result::Result<T, FitsError>;
