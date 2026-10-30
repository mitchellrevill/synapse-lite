//! Explicit one-pass macro playback, independently of the device worker.
//! One process-wide join handle owns admission; UI teardown requests Stop but
//! never detaches or forgets it. The next owner can reap a completed orphan.
use secbird_core::{
    native_host_launch as native,
    profile_compat::macro_library as library,
    source_macro::{Event, HostAction},
    Store,
};
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicU64, AtomicUsize, Ordering},
        Arc, Condvar, LazyLock, Mutex,
    },
    thread::{self, JoinHandle},
    time::Duration,
};

#[derive(Clone)]
pub(crate) struct Request {
    pub path: PathBuf,
    pub guid: String,
    pub revision: String,
    pub read_only: bool,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Outcome {
    Finished,
    Stopped,
    Failed(String),
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Report {
    pub outcome: Outcome,
    pub launches: usize,
    pub completed: usize,
    pub uncertain: bool,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Snapshot {
    pub total: usize,
    pub completed: usize,
    pub source_row: Option<usize>,
    pub launches: usize,
    pub stopping: bool,
}
pub(crate) enum Poll {
    Pending,
    Running(Snapshot),
    Finished(Report),
    Missing,
}
#[derive(Default)]
struct Progress {
    total: AtomicUsize,
    completed: AtomicUsize,
    row: AtomicUsize,
    launches: AtomicUsize,
}
impl Progress {
    fn new() -> Self {
        Self {
            row: AtomicUsize::new(usize::MAX),
            ..Default::default()
        }
    }
    fn snapshot(&self, stop: &Stop) -> Snapshot {
        let row = self.row.load(Ordering::Acquire);
        Snapshot {
            total: self.total.load(Ordering::Acquire),
            completed: self.completed.load(Ordering::Acquire),
            source_row: (row != usize::MAX).then_some(row),
            launches: self.launches.load(Ordering::Acquire),
            stopping: stop.requested(),
        }
    }
    fn report(&self, outcome: Outcome, uncertain: bool) -> Report {
        Report {
            outcome,
            launches: self.launches.load(Ordering::Acquire),
            completed: self.completed.load(Ordering::Acquire),
            uncertain,
        }
    }
}
#[derive(Default)]
struct StopState {
    requested: bool,
    native: Option<Arc<native::Control>>,
}
#[derive(Default)]
struct Stop {
    state: Mutex<StopState>,
    wake: Condvar,
}
impl Stop {
    fn request(&self) {
        let control = {
            let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());
            state.requested = true;
            state.native.clone()
        };
        if let Some(control) = control {
            control.cancel();
        }
        self.wake.notify_all();
    }
    fn requested(&self) -> bool {
        self.state
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .requested
    }
    fn wait(&self, duration: Duration) -> bool {
        let state = self.state.lock().unwrap_or_else(|p| p.into_inner());
        let (state, _) = self
            .wake
            .wait_timeout_while(state, duration, |s| !s.requested)
            .unwrap_or_else(|p| p.into_inner());
        !state.requested
    }
    fn native(&self) -> Arc<native::Control> {
        let control = Arc::new(native::Control::new());
        let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());
        state.native = Some(control.clone());
        if state.requested {
            control.cancel();
        }
        control
    }
    fn clear_native(&self) {
        self.state.lock().unwrap_or_else(|p| p.into_inner()).native = None;
    }
}
pub(crate) struct Session {
    id: u64,
    stop: Arc<Stop>,
}
impl Session {
    pub fn stop(&self) {
        self.stop.request();
    }
}
struct Run {
    id: u64,
    stop: Arc<Stop>,
    progress: Arc<Progress>,
    join: JoinHandle<Report>,
}
#[derive(Default)]
struct Registry {
    run: Mutex<Option<Run>>,
    sequence: AtomicU64,
}
static REGISTRY: LazyLock<Registry> = LazyLock::new(Registry::default);

impl Registry {
    fn spawn(
        &self,
        work: impl FnOnce(&Stop, &Progress) -> Report + Send + 'static,
    ) -> Result<Session, String> {
        let mut slot = match self.run.try_lock() {
            Ok(slot) => slot,
            Err(std::sync::TryLockError::Poisoned(p)) => p.into_inner(),
            Err(_) => return Err("Macro playback is busy; try again after it drains".into()),
        };
        if let Some(run) = slot.as_ref() {
            if !run.join.is_finished() {
                return Err("A macro is still running or stopping".into());
            }
        }
        //Only an actually finished thread is joined/reaped; no UI wait on an OS call.
        if let Some(old) = slot.take() {
            let _ = old.join.join();
        }
        let previous = self
            .sequence
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |n| n.checked_add(1))
            .map_err(|_| "Macro session limit reached")?;
        let id = previous + 1;
        let stop = Arc::new(Stop::default());
        let progress = Arc::new(Progress::new());
        let child_stop = stop.clone();
        let child_progress = progress.clone();
        let join = thread::Builder::new()
            .name("secbird-macro".into())
            .spawn(move || work(&child_stop, &child_progress))
            .map_err(|e| e.to_string())?;
        *slot = Some(Run {
            id,
            stop: stop.clone(),
            progress,
            join,
        });
        Ok(Session { id, stop })
    }
    fn poll(&self, session: &Session) -> Poll {
        let mut slot = match self.run.try_lock() {
            Ok(slot) => slot,
            Err(std::sync::TryLockError::Poisoned(p)) => p.into_inner(),
            Err(_) => return Poll::Pending,
        };
        let Some(run) = slot.as_ref().filter(|run| run.id == session.id) else {
            return Poll::Missing;
        };
        if !run.join.is_finished() {
            return Poll::Running(run.progress.snapshot(&run.stop));
        }
        let run = slot.take().unwrap();
        let result = run.join.join().unwrap_or_else(|_| {
            run.progress.report(
                Outcome::Failed(
                    "Macro worker failed; a launch may already have been accepted".into(),
                ),
                true,
            )
        });
        Poll::Finished(result)
    }
}
pub(crate) fn start(request: Request) -> Result<Session, String> {
    if request.read_only {
        return Err("Read-only mode refuses macro playback".into());
    }
    if request.guid.is_empty()
        || request.guid.len() > 128
        || request.revision.is_empty()
        || request.revision.len() > 128
    {
        return Err("Invalid macro selection identity".into());
    }
    REGISTRY.spawn(move |stop, progress| run_request(&request, stop, progress, &native::launch))
}
pub(crate) fn poll(session: &Session) -> Poll {
    REGISTRY.poll(session)
}

enum Step {
    Delay(u32),
    Launch(native::Request),
}
struct Plan {
    steps: Vec<(usize, Step)>,
}
fn prepare(request: &Request) -> Result<Plan, String> {
    if request.read_only {
        return Err("Read-only mode refuses macro playback".into());
    }
    let store = Store::load(&request.path).map_err(|e| e.to_string())?;
    let entries = library::read(&store).map_err(|e| e.to_string())?;
    let entry = entries
        .entries
        .iter()
        .find(|entry| entry.guid == request.guid)
        .ok_or("The selected macro no longer exists")?;
    if entry.revision != request.revision {
        return Err("The macro changed outside this editor; refresh before playing".into());
    }
    if entry.events.iter().all(|row| row["Type"] == "actionBar") {
        return Err("Add an action or delay before playing.".into());
    }
    //An explicit editor action is one pass; it does not alter a device binding.
    let binding = serde_json::json!({"guid":request.guid,"macroPlaybackOption":"Once"}).to_string();
    let prepared = library::planning::prepare(&store, &binding).map_err(|e| e.to_string())?;
    let mut steps = Vec::with_capacity(prepared.plan().events().len());
    for (row, event) in prepared.source_rows().iter().zip(prepared.plan().events()) {
        let step = match event {
            Event::DelayMs(ms) => Step::Delay(*ms),
            Event::Host(action) => match action.as_ref() {
                HostAction::Launch { .. } => {
                    Step::Launch(native::Request::from_action(action).map_err(|e| e.to_string())?)
                }
                HostAction::ClipboardText(_) => {
                    return Err("Text playback is not available yet. Nothing was run.".into())
                }
            },
            _ => {
                return Err(
                    "Keyboard and mouse playback is not available yet. Nothing was run.".into(),
                )
            }
        };
        steps.push((*row, step));
    }
    Ok(Plan { steps })
}
fn run_request(
    request: &Request,
    stop: &Stop,
    progress: &Progress,
    launch: &impl Fn(&native::Request, &native::Control) -> Result<native::Receipt, native::Error>,
) -> Report {
    if stop.requested() {
        return progress.report(Outcome::Stopped, false);
    }
    let plan = match prepare(request) {
        Ok(plan) => plan,
        Err(error) => {
            return progress.report(Outcome::Failed(error.chars().take(1024).collect()), false)
        }
    };
    progress.total.store(plan.steps.len(), Ordering::Release);
    for (row, step) in plan.steps {
        if stop.requested() {
            return progress.report(Outcome::Stopped, false);
        }
        progress.row.store(row, Ordering::Release);
        match step {
            Step::Delay(ms) => {
                if !stop.wait(Duration::from_millis(ms.into())) {
                    return progress.report(Outcome::Stopped, false);
                }
            }
            Step::Launch(request) => {
                let control = stop.native();
                let result = launch(&request, &control);
                stop.clear_native();
                match result {
                    Ok(_) => {
                        progress.launches.fetch_add(1, Ordering::AcqRel);
                    }
                    Err(native::Error::Cancelled) if stop.requested() => {
                        return progress.report(Outcome::Stopped, false)
                    }
                    Err(error) => {
                        return progress.report(Outcome::Failed(execution_error(error)), false)
                    }
                }
            }
        }
        progress.completed.fetch_add(1, Ordering::AcqRel);
    }
    progress.report(
        if stop.requested() {
            Outcome::Stopped
        } else {
            Outcome::Finished
        },
        false,
    )
}

fn execution_error(error: native::Error) -> String {
    match error {
        native::Error::PathNotFound => "The program or document could not be found.".into(),
        native::Error::InvalidCommand => {
            "The command is too long or contains unsupported characters.".into()
        }
        native::Error::LaunchFailed {
            shell_error: 31, ..
        } => "Windows has no app registered to open this file.".into(),
        native::Error::LaunchFailed { shell_error: 5, .. } => {
            "Windows denied access to the program or document.".into()
        }
        native::Error::LaunchFailed { shell_error, .. } => {
            format!("Windows could not open the program or document (error {shell_error}).")
        }
        _ => error.to_string(),
    }
}

#[cfg(test)]
mod tests;
