use super::*;
use std::{
    mem::{size_of, zeroed},
    ptr::null,
};
use windows_sys::Win32::{
    Foundation::{CloseHandle, GetLastError, HANDLE},
    Storage::FileSystem::{GetFileAttributesW, GetTempPathW, INVALID_FILE_ATTRIBUTES},
    System::{
        Com::{CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE},
        Threading::{
            CreateProcessW, GetProcessId, CREATE_UNICODE_ENVIRONMENT, PROCESS_INFORMATION,
            STARTUPINFOW,
        },
    },
    UI::Shell::{ShellExecuteExW, SEE_MASK_FLAG_NO_UI, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW},
};
fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(Some(0)).collect()
}
struct Handle(HANDLE);
impl Drop for Handle {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe {
                CloseHandle(self.0);
            }
        }
    }
}
struct Apartment;
impl Drop for Apartment {
    fn drop(&mut self) {
        unsafe {
            CoUninitialize();
        }
    }
}

/// Blocking API for an owned worker thread. Shell fallback requires a compatible
/// COM apartment; direct CreateProcess does not. Never call from a UI callback.
/// OS launch calls are not interruptible;
/// Stop prevents queued/retry attempts and does not terminate launched user apps.
/// This is not connected to saved profile activation or the input scheduler.
pub fn launch(request: &Request, control: &Control) -> Result<Receipt, Error> {
    execute(request, control, &mut Windows::default())
}

#[cfg(test)]
pub(super) fn observed_owned_fixture(
    request: &Request,
    control: &Control,
    force_shell: bool,
    expected_exit_code: u32,
) -> Result<Receipt, Error> {
    struct ForceCreateFailure(Windows);
    impl Backend for ForceCreateFailure {
        fn path_exists(&mut self, path: &str) -> bool {
            self.0.path_exists(path)
        }
        fn create(&mut self, _: &str, _: bool) -> Result<u32, u32> {
            Err(2)
        }
        fn shell(
            &mut self,
            file: &str,
            args: Option<&str>,
            hidden: bool,
        ) -> Result<Option<u32>, ShellError> {
            self.0.shell(file, args, hidden)
        }
    }
    let mut backend = Windows::default();
    let receipt = if force_shell {
        let mut forced = ForceCreateFailure(backend);
        let result = execute(request, control, &mut forced);
        backend = forced.0;
        result?
    } else {
        execute(request, control, &mut backend)?
    };
    assert_eq!(
        backend.observed.len(),
        1,
        "owned fixture must return one observable process"
    );
    for handle in &backend.observed {
        use windows_sys::Win32::System::Threading::{GetExitCodeProcess, WaitForSingleObject};
        assert_eq!(Some(unsafe { GetProcessId(handle.0) }), receipt.process_id);
        let wait = unsafe { WaitForSingleObject(handle.0, 5000) };
        let mut code = 0;
        let observed = unsafe { GetExitCodeProcess(handle.0, &mut code) };
        assert_eq!(wait, 0, "owned fixture did not exit");
        assert_ne!(observed, 0);
        assert_eq!(
            code, expected_exit_code,
            "owned fixture reported a directory/argument contract mismatch"
        );
    }
    Ok(receipt)
}
#[derive(Default)]
struct Windows {
    apartment: Option<Apartment>,
    #[cfg(test)]
    observed: Vec<Handle>,
}
#[cfg(test)]
impl Windows {
    //Only a test build retains a duplicate for authoritative exit observation.
    //Production handles still close immediately, before the receipt returns.
    fn observe(&mut self, process: HANDLE) {
        use windows_sys::Win32::{
            Foundation::{DuplicateHandle, DUPLICATE_SAME_ACCESS},
            System::Threading::GetCurrentProcess,
        };
        if process.is_null() {
            return;
        }
        let mut duplicate = std::ptr::null_mut();
        let ok = unsafe {
            DuplicateHandle(
                GetCurrentProcess(),
                process,
                GetCurrentProcess(),
                &mut duplicate,
                0,
                0,
                DUPLICATE_SAME_ACCESS,
            )
        };
        assert_ne!(ok, 0);
        self.observed.push(Handle(duplicate));
    }
}
impl Backend for Windows {
    fn path_exists(&mut self, path: &str) -> bool {
        unsafe { GetFileAttributesW(wide(path).as_ptr()) != INVALID_FILE_ATTRIBUTES }
    }
    fn create(&mut self, command: &str, hidden: bool) -> Result<u32, u32> {
        let mut command = wide(command);
        let mut info: STARTUPINFOW = unsafe { zeroed() };
        info.cb = size_of::<STARTUPINFOW>() as u32;
        info.dwFlags = 1; //STARTF_USESHOWWINDOW, source launch_win.cc
        info.wShowWindow = u16::from(!hidden); //SW_HIDE0 / SW_SHOWNORMAL1
        let mut process: PROCESS_INFORMATION = unsafe { zeroed() };
        // Source helper1801b1ef0 supplies GetTempPathW(MAX_PATH) as the
        // child directory. Failure/overflow leaves the source option empty.
        let mut directory = [0u16; 260];
        let length = unsafe { GetTempPathW(directory.len() as u32, directory.as_mut_ptr()) };
        let directory_pointer = if (1..260).contains(&length) {
            directory.as_ptr()
        } else {
            null()
        };
        let ok = unsafe {
            CreateProcessW(
                null(),
                command.as_mut_ptr(),
                null(),
                null(),
                0,
                CREATE_UNICODE_ENVIRONMENT,
                null(),
                directory_pointer,
                &info,
                &mut process,
            )
        };
        if ok == 0 {
            return Err(unsafe { GetLastError() });
        }
        let _process = Handle(process.hProcess);
        let _thread = Handle(process.hThread);
        #[cfg(test)]
        self.observe(process.hProcess);
        Ok(process.dwProcessId)
    }
    fn shell(
        &mut self,
        file: &str,
        arguments: Option<&str>,
        hidden: bool,
    ) -> Result<Option<u32>, ShellError> {
        // Direct process creation needs no COM work or apartment restriction.
        // Initialize once only when this invocation reaches the shell fallback.
        if self.apartment.is_none() {
            let hr = unsafe {
                CoInitializeEx(
                    null(),
                    (COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE) as u32,
                )
            };
            if hr < 0 {
                return Err(ShellError::Com(hr));
            }
            self.apartment = Some(Apartment);
        }
        let file = wide(file);
        let arguments = arguments.map(wide);
        let verb = wide("open");
        let mut info: SHELLEXECUTEINFOW = unsafe { zeroed() };
        info.cbSize = size_of::<SHELLEXECUTEINFOW>() as u32;
        info.fMask = SEE_MASK_NOCLOSEPROCESS | SEE_MASK_FLAG_NO_UI;
        info.lpVerb = verb.as_ptr();
        info.lpFile = file.as_ptr();
        info.lpParameters = arguments.as_ref().map_or(null(), |s| s.as_ptr());
        info.nShow = i32::from(!hidden);
        let ok = unsafe { ShellExecuteExW(&mut info) };
        if ok == 0 {
            return Err(ShellError::Win32(unsafe { GetLastError() }));
        }
        let _process = Handle(info.hProcess);
        #[cfg(test)]
        self.observe(info.hProcess);
        let id = if info.hProcess.is_null() {
            0
        } else {
            unsafe { GetProcessId(info.hProcess) }
        };
        Ok((id != 0).then_some(id))
    }
}
