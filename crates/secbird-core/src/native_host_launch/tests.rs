use super::*;
use std::{collections::VecDeque, sync::Arc};

struct Fake {
    calls: Vec<String>,
    exists: bool,
    create: Result<u32, u32>,
    shells: VecDeque<Result<Option<u32>, u32>>,
    stop_at: Option<(&'static str, Arc<Control>)>,
}
impl Default for Fake {
    fn default() -> Self {
        Self {
            calls: vec![],
            exists: true,
            create: Ok(42),
            shells: VecDeque::new(),
            stop_at: None,
        }
    }
}
impl Fake {
    fn call(&mut self, stage: &str, detail: String) {
        self.calls.push(detail);
        if let Some((at, control)) = &self.stop_at {
            if *at == stage {
                control.cancel();
            }
        }
    }
}
impl Backend for Fake {
    fn path_exists(&mut self, path: &str) -> bool {
        self.call("path", format!("path:{path}"));
        self.exists
    }
    fn create(&mut self, command: &str, hidden: bool) -> Result<u32, u32> {
        self.call("create", format!("create:{hidden}:{command}"));
        self.create
    }
    fn shell(
        &mut self,
        file: &str,
        args: Option<&str>,
        hidden: bool,
    ) -> Result<Option<u32>, ShellError> {
        self.call("shell", format!("shell:{hidden}:{file}:{args:?}"));
        self.shells
            .pop_front()
            .expect("unexpected retry")
            .map_err(ShellError::Win32)
    }
}
fn request(command: &str) -> Request {
    Request::from_action(&HostAction::Launch {
        path: command.into(),
        start_hidden: true,
        path_to_check: None,
    })
    .unwrap()
}

#[test]
fn launch_contract_attempts_native_routes_in_order_without_waiting_for_child_exit() {
    let r = request("\"C:/Own Fixture.exe\" --list");
    let mut f = Fake::default();
    assert_eq!(
        execute(&r, &Control::new(), &mut f).unwrap(),
        Receipt {
            route: Route::CreateProcess,
            process_id: Some(42)
        }
    );
    assert_eq!(f.calls, vec!["create:true:\"C:/Own Fixture.exe\" --list"]);
    let mut f = Fake {
        create: Err(193),
        shells: VecDeque::from([Ok(None)]),
        ..Default::default()
    };
    assert_eq!(
        execute(&r, &Control::new(), &mut f).unwrap(),
        Receipt {
            route: Route::ShellWholeCommand,
            process_id: None
        }
    );
    assert_eq!(f.calls.len(), 2);
    for error in [2, 3] {
        let mut f = Fake {
            create: Err(2),
            shells: VecDeque::from([Err(error), Ok(Some(77))]),
            ..Default::default()
        };
        assert_eq!(
            execute(&r, &Control::new(), &mut f).unwrap(),
            Receipt {
                route: Route::ShellSplitCommand,
                process_id: Some(77)
            }
        );
        assert_eq!(
            f.calls.last().unwrap(),
            "shell:true:C:/Own Fixture.exe:Some(\"--list\")"
        );
    }
}
#[test]
fn launch_contract_only_retries_source_not_found_errors_and_preserves_error_codes() {
    for error in [0, 5, 8, 31, 1223] {
        let mut f = Fake {
            create: Err(193),
            shells: VecDeque::from([Err(error)]),
            ..Default::default()
        };
        assert_eq!(
            execute(&request("literal"), &Control::new(), &mut f),
            Err(Error::LaunchFailed {
                create_error: 193,
                shell_error: error
            })
        );
        assert_eq!(f.calls.len(), 2);
    }
    let mut f = Fake {
        create: Err(2),
        shells: VecDeque::from([Err(3), Err(31)]),
        ..Default::default()
    };
    assert_eq!(
        execute(&request("literal --args"), &Control::new(), &mut f),
        Err(Error::LaunchFailed {
            create_error: 2,
            shell_error: 31
        })
    );
}
#[test]
fn launch_split_preserves_the_native_first_token_rule_without_argv_requoting() {
    for (command, file, args) in [
        (
            "  \t\"C:/One Two.exe\"  \t--flag \"a b\"",
            "C:/One Two.exe",
            Some("--flag \"a b\""),
        ),
        ("own.exe\t--arg  value", "own.exe", Some("--arg  value")),
        ("\"unclosed path", "unclosed path", None),
        ("\"one\"suffix", "one", Some("suffix")),
        ("  \t  ", "", None),
        ("雪😀.exe", "雪😀.exe", None),
        ("own\u{a0}file.exe", "own\u{a0}file.exe", None),
    ] {
        assert_eq!(split_command(command), (file, args));
    }
}
#[test]
fn launch_control_cancel_before_start_makes_no_backend_calls_and_never_replays() {
    let c = Control::new();
    let mut f = Fake::default();
    assert_eq!(c.cancel(), CancelOutcome::BeforeStart);
    assert_eq!(execute(&request("data"), &c, &mut f), Err(Error::Cancelled));
    assert!(f.calls.is_empty());
    let c = Control::new();
    execute(&request("data"), &c, &mut f).unwrap();
    assert_eq!(
        execute(&request("data"), &c, &mut f),
        Err(Error::AlreadyUsed)
    );
    assert_eq!(c.cancel(), CancelOutcome::AlreadyFinished);
    assert_eq!(f.calls.len(), 1);
}
#[test]
fn launch_control_stop_between_attempts_prevents_retry_but_never_hides_success() {
    for stage in ["path", "create", "shell"] {
        let c = Arc::new(Control::new());
        let mut f = Fake {
            create: Err(2),
            shells: VecDeque::from([Err(2)]),
            stop_at: Some((stage, c.clone())),
            ..Default::default()
        };
        let mut r = request("own --arg");
        r.path_to_check = Some("exists".into());
        assert_eq!(execute(&r, &c, &mut f), Err(Error::Cancelled));
        assert_eq!(
            f.calls.len(),
            match stage {
                "path" => 1,
                "create" => 2,
                _ => 3,
            }
        );
    }
    let c = Arc::new(Control::new());
    let mut f = Fake {
        stop_at: Some(("create", c.clone())),
        ..Default::default()
    };
    assert_eq!(
        execute(&request("data"), &c, &mut f).unwrap().process_id,
        Some(42)
    );
    assert_eq!(f.calls.len(), 1); //No terminate/retry operation exists.
}
#[test]
fn launch_path_check_and_native_utf16_nul_bounds_refuse_before_execution() {
    let mut r = request("own");
    r.path_to_check = Some("absent".into());
    let mut f = Fake {
        exists: false,
        ..Default::default()
    };
    assert_eq!(
        execute(&r, &Control::new(), &mut f),
        Err(Error::PathNotFound)
    );
    assert_eq!(f.calls, vec!["path:absent"]);
    r.path_to_check = Some(String::new());
    let mut f = Fake::default();
    execute(&r, &Control::new(), &mut f).unwrap();
    assert_eq!(f.calls.len(), 1);
    for path in [
        String::new(),
        "x\0y".into(),
        "x\ny".into(),
        "x".repeat(32767),
        "😀".repeat(16384),
    ] {
        assert_eq!(
            Request::from_action(&HostAction::Launch {
                path,
                start_hidden: false,
                path_to_check: None
            }),
            Err(Error::InvalidCommand)
        );
    }
    assert!(Request::from_action(&HostAction::Launch {
        path: "x".repeat(32766),
        start_hidden: false,
        path_to_check: None
    })
    .is_ok());
    assert_eq!(
        Request::from_action(&HostAction::ClipboardText("inert".into())),
        Err(Error::NotLaunch)
    );
}

#[test]
fn launch_control_concurrent_duplicate_cannot_dispatch_a_second_request() {
    use std::sync::Barrier;
    let control = Control::new();
    let entered = Barrier::new(2);
    let release = Barrier::new(2);
    struct Paused<'a>(&'a Barrier, &'a Barrier);
    impl Backend for Paused<'_> {
        fn path_exists(&mut self, _: &str) -> bool {
            panic!("no path check expected")
        }
        fn create(&mut self, _: &str, _: bool) -> Result<u32, u32> {
            self.0.wait();
            self.1.wait();
            Ok(42)
        }
        fn shell(&mut self, _: &str, _: Option<&str>, _: bool) -> Result<Option<u32>, ShellError> {
            panic!("no shell expected")
        }
    }
    std::thread::scope(|scope| {
        let join =
            scope.spawn(|| execute(&request("own"), &control, &mut Paused(&entered, &release)));
        entered.wait();
        let mut other = Fake::default();
        assert_eq!(
            execute(&request("different"), &control, &mut other),
            Err(Error::AlreadyUsed)
        );
        assert!(other.calls.is_empty());
        release.wait();
        assert_eq!(join.join().unwrap().unwrap().process_id, Some(42));
    });
}

#[cfg(windows)]
fn owned_fixture_path() -> std::path::PathBuf {
    let path = std::path::PathBuf::from(
        std::env::var("SECBIRD_HOST_LAUNCH_FIXTURE").expect("explicit fixture path required"),
    )
    .canonicalize()
    .unwrap();
    assert_eq!(path.file_name().unwrap(), "host-launch-fixture.exe");
    assert!(path.is_file());
    path
}

#[cfg(windows)]
#[test]
#[ignore = "Explicit owned executable fixture only; forces initial CreateProcess failure to exercise actual ShellExecuteEx"]
fn launch_owned_windows_fixture_through_shell_routes() {
    let path = owned_fixture_path();
    for (command, route) in [
        (path.to_str().unwrap().to_owned(), Route::ShellWholeCommand),
        (
            format!("\"{}\" --owned-fixture-argument", path.display()),
            Route::ShellSplitCommand,
        ),
    ] {
        let receipt =
            super::windows::observed_owned_fixture(&request(&command), &Control::new(), true, 0)
                .unwrap();
        assert_eq!(receipt.route, route);
        println!(
            "OWNED_SHELL route={:?} returned_process_identity={}",
            receipt.route,
            receipt.process_id.is_some()
        );
    }
}

#[cfg(windows)]
#[test]
#[ignore = "Explicit owned fixture only: build host-launch-fixture, set SECBIRD_HOST_LAUNCH_FIXTURE; never imported commands"]
fn launch_owned_windows_fixture_and_close_returned_handles() {
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, GetProcessHandleCount};
    let path = std::path::PathBuf::from(
        std::env::var("SECBIRD_HOST_LAUNCH_FIXTURE").expect("explicit fixture path required"),
    )
    .canonicalize()
    .unwrap();
    assert_eq!(path.file_name().unwrap(), "host-launch-fixture.exe");
    assert!(path.is_file());
    let mut count_before = 0;
    unsafe {
        assert_ne!(
            GetProcessHandleCount(GetCurrentProcess(), &mut count_before),
            0
        );
    }
    for _ in 0..4 {
        let mut r = request(&format!(
            "\"{}\" --expect-temp-working-directory",
            path.display()
        ));
        r.path_to_check = Some(path.to_str().unwrap().into());
        let receipt =
            super::windows::observed_owned_fixture(&r, &Control::new(), false, 42).unwrap();
        assert_eq!(receipt.route, Route::CreateProcess);
    }
    let mut count_after = 0;
    unsafe {
        assert_ne!(
            GetProcessHandleCount(GetCurrentProcess(), &mut count_after),
            0
        );
    }
    assert!(
        count_after <= count_before + 2,
        "parent handle count grew: {count_before}->{count_after}"
    );
    println!(
        "OWNED_CREATE_PROCESS handles_before={count_before} handles_after={count_after} launches=4"
    );
}

#[cfg(windows)]
#[test]
#[ignore = "Explicit owned executable fixture only; validates incompatible COM context without launching through shell"]
fn launch_owned_fixture_from_mta_and_refuse_incompatible_shell_apartment() {
    use windows_sys::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_MULTITHREADED};
    let hr = unsafe { CoInitializeEx(std::ptr::null(), COINIT_MULTITHREADED as u32) };
    assert!(hr >= 0);
    struct Mta;
    impl Drop for Mta {
        fn drop(&mut self) {
            unsafe {
                CoUninitialize();
            }
        }
    }
    let _mta = Mta;
    let path = owned_fixture_path();
    let r = request(&format!(
        "\"{}\" --expect-temp-working-directory",
        path.display()
    ));
    let receipt = super::windows::observed_owned_fixture(&r, &Control::new(), false, 42).unwrap();
    assert_eq!(receipt.route, Route::CreateProcess);
    assert_eq!(
        super::windows::observed_owned_fixture(&r, &Control::new(), true, 0),
        Err(Error::Com(0x80010106u32 as i32))
    );
    println!("OWNED_MTA direct_create=accepted shell=RPC_E_CHANGED_MODE");
}
