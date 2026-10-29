//! Run-key registration with exact command readback. Tests inject Backend and never touch HKCU.
use super::*;

pub trait Backend {
    fn read(&mut self) -> Result<Option<String>, String>;
    fn write(&mut self, command: Option<&str>) -> Result<(), String>;
}

pub struct Registry;
fn command() -> Result<String, String> {
    Ok(format!(
        "\"{}\" --startup",
        std::env::current_exe()
            .map_err(|e| e.to_string())?
            .display()
    ))
}
fn owned(command: &str) -> Result<bool, String> {
    let expected = self::command()?;
    Ok(command == expected || command == expected.replace(" --startup", " --tray"))
}
pub fn enabled(backend: &mut dyn Backend) -> Result<bool, String> {
    match backend.read()? {
        None => Ok(false),
        Some(value) if owned(&value)? => Ok(true),
        Some(_) => Err("Auto Launch points to another command. Original registration retained; resolve it in Windows Startup settings.".into()),
    }
}
pub fn set(backend: &mut dyn Backend, enable: bool) -> Result<(), String> {
    enabled(backend)?; // Never overwrite or delete another installation's command.
    let expected = command()?;
    backend.write(enable.then_some(expected.as_str()))?;
    let actual = backend.read()?;
    if actual.as_deref() != enable.then_some(expected.as_str()) {
        return Err(
            "Auto Launch write did not match registry readback; refresh before retrying".into(),
        );
    }
    Ok(())
}
pub fn autostart_enabled() -> Result<bool, String> {
    enabled(&mut Registry)
}
pub fn set_autostart(enable: bool) -> Result<(), String> {
    super::change_local("autostart", enable, 0, "", false).map(|_| ())
}
fn status(code: u32) -> Result<(), String> {
    if code == ERROR_SUCCESS {
        Ok(())
    } else {
        Err(format!(
            "Windows Auto Launch: {}",
            std::io::Error::from_raw_os_error(code as i32)
        ))
    }
}
struct Key(HKEY);
impl Drop for Key {
    fn drop(&mut self) {
        unsafe {
            RegCloseKey(self.0);
        }
    }
}
impl Backend for Registry {
    fn read(&mut self) -> Result<Option<String>, String> {
        unsafe {
            let mut key = null_mut();
            let code = RegOpenKeyExW(
                HKEY_CURRENT_USER,
                wide(RUN_KEY).as_ptr(),
                0,
                KEY_QUERY_VALUE,
                &mut key,
            );
            if code == ERROR_FILE_NOT_FOUND {
                return Ok(None);
            }
            status(code)?;
            let key = Key(key);
            let mut kind = 0;
            let mut size = 0;
            let code = RegQueryValueExW(
                key.0,
                wide(APP_NAME).as_ptr(),
                null(),
                &mut kind,
                null_mut(),
                &mut size,
            );
            if code == ERROR_FILE_NOT_FOUND {
                return Ok(None);
            }
            status(code)?;
            if kind != REG_SZ || !(2..=65536).contains(&size) || size % 2 != 0 {
                return Err("Unsupported Auto Launch value type or size; original retained".into());
            }
            let mut buffer = vec![0u16; size as usize / 2];
            status(RegQueryValueExW(
                key.0,
                wide(APP_NAME).as_ptr(),
                null(),
                &mut kind,
                buffer.as_mut_ptr().cast(),
                &mut size,
            ))?;
            if kind != REG_SZ || size < 2 || size as usize > buffer.len() * 2 || size % 2 != 0 {
                return Err("Auto Launch changed during read".into());
            }
            buffer.truncate(size as usize / 2);
            if buffer.pop() != Some(0) || buffer.contains(&0) {
                return Err("Malformed Auto Launch command; original retained".into());
            }
            String::from_utf16(&buffer)
                .map(Some)
                .map_err(|e| e.to_string())
        }
    }
    fn write(&mut self, command: Option<&str>) -> Result<(), String> {
        unsafe {
            let mut key = null_mut();
            status(RegCreateKeyExW(
                HKEY_CURRENT_USER,
                wide(RUN_KEY).as_ptr(),
                0,
                null(),
                0,
                KEY_SET_VALUE,
                null(),
                &mut key,
                null_mut(),
            ))?;
            let key = Key(key);
            let code = if let Some(command) = command {
                let bytes = wide(command);
                RegSetValueExW(
                    key.0,
                    wide(APP_NAME).as_ptr(),
                    0,
                    REG_SZ,
                    bytes.as_ptr().cast(),
                    (bytes.len() * 2) as u32,
                )
            } else {
                RegDeleteValueW(key.0, wide(APP_NAME).as_ptr())
            };
            if command.is_none() && code == ERROR_FILE_NOT_FOUND {
                return Ok(());
            }
            status(code)
        }
    }
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;
    #[derive(Default)]
    pub struct Fake {
        pub value: Option<String>,
        pub fail: bool,
        pub read_fail: bool,
        pub ignore: bool,
        pub writes: usize,
    }
    impl Backend for Fake {
        fn read(&mut self) -> Result<Option<String>, String> {
            if self.read_fail {
                return Err("Read access denied".into());
            }
            Ok(self.value.clone())
        }
        fn write(&mut self, value: Option<&str>) -> Result<(), String> {
            self.writes += 1;
            if self.fail {
                return Err("Access denied".into());
            }
            if !self.ignore {
                self.value = value.map(str::to_owned);
            }
            Ok(())
        }
    }
    #[test]
    fn startup_readback_ownership_and_failures() {
        let mut fake = Fake {
            read_fail: true,
            ..Fake::default()
        };
        assert!(enabled(&mut fake).is_err());
        assert!(set(&mut fake, true).is_err());
        assert_eq!(fake.writes, 0);
        fake.read_fail = false;
        set(&mut fake, true).unwrap();
        assert!(enabled(&mut fake).unwrap());
        assert!(fake.value.as_ref().unwrap().ends_with(" --startup"));
        fake.value = Some(command().unwrap().replace(" --startup", " --tray"));
        assert!(enabled(&mut fake).unwrap());
        set(&mut fake, false).unwrap();
        assert!(!enabled(&mut fake).unwrap());
        fake.fail = true;
        assert!(set(&mut fake, true).unwrap_err().contains("Access denied"));
        fake.fail = false;
        fake.ignore = true;
        assert!(set(&mut fake, true).is_err());
        fake.value = Some("another-installation.exe --tray".into());
        let writes = fake.writes;
        assert!(set(&mut fake, false).is_err());
        assert_eq!(fake.writes, writes);
    }
}
