//! Explicit host launch execution; never invoked by import or macro admission.
//! Completion means the OS accepted launch, not that the child exited.
use crate::source_macro::HostAction;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use windows::launch;
#[cfg(test)]
mod tests;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    command: String,
    hidden: bool,
    path_to_check: Option<String>,
}
impl Request {
    pub fn from_action(action: &HostAction) -> Result<Self, Error> {
        let HostAction::Launch {
            path,
            start_hidden,
            path_to_check,
        } = action
        else {
            return Err(Error::NotLaunch);
        };
        // CreateProcessW's32767 UTF16 units include the final NUL.
        if path.is_empty()
            || !crate::source_macro::host::text(path, 32766, false)
            || path_to_check
                .as_ref()
                .is_some_and(|p| !crate::source_macro::host::text(p, 32766, false))
        {
            return Err(Error::InvalidCommand);
        }
        Ok(Self {
            command: path.clone(),
            hidden: *start_hidden,
            path_to_check: path_to_check.clone(),
        })
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Route {
    CreateProcess,
    ShellWholeCommand,
    ShellSplitCommand,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Receipt {
    pub route: Route,
    /// Shell association activation may return no new process.
    pub process_id: Option<u32>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    NotLaunch,
    InvalidCommand,
    Cancelled,
    AlreadyUsed,
    PathNotFound,
    Com(i32),
    LaunchFailed { create_error: u32, shell_error: u32 },
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Host launch: {self:?}")
    }
}
impl std::error::Error for Error {}

/// Fresh for each request. No reset/replay of an already consumed invocation.
pub struct Control {
    state: AtomicU8, //0 queued,1 dispatching,2 cancelled before start,3 terminal
    stop: AtomicBool,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CancelOutcome {
    BeforeStart,
    RequestedDuringDispatch,
    AlreadyFinished,
}
impl Default for Control {
    fn default() -> Self {
        Self::new()
    }
}
impl Control {
    pub fn new() -> Self {
        Self {
            state: AtomicU8::new(0),
            stop: AtomicBool::new(false),
        }
    }
    pub fn cancel(&self) -> CancelOutcome {
        self.stop.store(true, Ordering::Release);
        match self
            .state
            .compare_exchange(0, 2, Ordering::AcqRel, Ordering::Acquire)
        {
            Ok(_) | Err(2) => CancelOutcome::BeforeStart,
            Err(3) => CancelOutcome::AlreadyFinished,
            Err(_) => CancelOutcome::RequestedDuringDispatch,
        }
    }
    fn check(&self) -> Result<(), Error> {
        if self.stop.load(Ordering::Acquire) {
            Err(Error::Cancelled)
        } else {
            Ok(())
        }
    }
    fn begin(&self) -> Result<Finish<'_>, Error> {
        match self
            .state
            .compare_exchange(0, 1, Ordering::AcqRel, Ordering::Acquire)
        {
            Ok(_) => Ok(Finish(self)),
            Err(2) => Err(Error::Cancelled),
            _ => Err(Error::AlreadyUsed),
        }
    }
}
struct Finish<'a>(&'a Control);
impl Drop for Finish<'_> {
    fn drop(&mut self) {
        self.0.state.store(3, Ordering::Release);
    }
}

trait Backend {
    fn path_exists(&mut self, path: &str) -> bool;
    fn create(&mut self, command: &str, hidden: bool) -> Result<u32, u32>;
    fn shell(
        &mut self,
        file: &str,
        arguments: Option<&str>,
        hidden: bool,
    ) -> Result<Option<u32>, ShellError>;
}
enum ShellError {
    Win32(u32),
    Com(i32),
}
fn execute(
    request: &Request,
    control: &Control,
    backend: &mut impl Backend,
) -> Result<Receipt, Error> {
    let _finish = control.begin()?;
    control.check()?;
    if let Some(path) = request.path_to_check.as_deref().filter(|p| !p.is_empty()) {
        if !backend.path_exists(path) {
            return Err(Error::PathNotFound);
        }
    }
    control.check()?;
    let create_error = match backend.create(&request.command, request.hidden) {
        Ok(id) => {
            return Ok(Receipt {
                route: Route::CreateProcess,
                process_id: Some(id),
            })
        }
        Err(error) => error,
    };
    control.check()?;
    let mut shell_error = match backend.shell(&request.command, None, request.hidden) {
        Ok(process_id) => {
            return Ok(Receipt {
                route: Route::ShellWholeCommand,
                process_id,
            })
        }
        Err(ShellError::Win32(error)) => error,
        Err(ShellError::Com(error)) => return Err(Error::Com(error)),
    };
    if matches!(shell_error, 2 | 3) {
        control.check()?;
        let (file, arguments) = split_command(&request.command);
        shell_error = match backend.shell(file, arguments, request.hidden) {
            Ok(process_id) => {
                return Ok(Receipt {
                    route: Route::ShellSplitCommand,
                    process_id,
                })
            }
            Err(ShellError::Win32(error)) => error,
            Err(ShellError::Com(error)) => return Err(Error::Com(error)),
        };
    }
    Err(Error::LaunchFailed {
        create_error,
        shell_error,
    })
}

// Native helper's first-token split, not CommandLineToArgvW or shell parsing.
// Only ASCII space/tab separate tokens. Quoted token ends at the next quote;
// the remainder is retained literally and is never reconstructed as argv.
fn split_command(command: &str) -> (&str, Option<&str>) {
    let command = command.trim_start_matches([' ', '\t']);
    let (file, tail) = if let Some(quoted) = command.strip_prefix('"') {
        match quoted.find('"') {
            Some(end) => (&quoted[..end], &quoted[end + 1..]),
            None => (quoted, ""),
        }
    } else {
        let end = command.find([' ', '\t']).unwrap_or(command.len());
        (&command[..end], &command[end..])
    };
    let tail = tail.trim_start_matches([' ', '\t']);
    (file, (!tail.is_empty()).then_some(tail))
}
