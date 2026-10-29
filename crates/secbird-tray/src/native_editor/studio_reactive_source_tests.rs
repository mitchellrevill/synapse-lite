use super::*;
use secbird_core::{
    lighting_pixels::Pixel,
    studio::{
        reactive_input::{LedAddress, ReactiveInputMap},
        reactive_replay::{PulseRecording, ReplayRegion, Retrigger},
    },
    Rgb,
};
use std::{
    ptr::{null, null_mut},
    sync::OnceLock,
};
use windows_sys::Win32::{
    Foundation::*, System::LibraryLoader::GetModuleHandleW, UI::WindowsAndMessaging::*,
};

fn packet(handle: usize, code: u16, flags: u16) -> RAWINPUT {
    let mut input: RAWINPUT = unsafe { std::mem::zeroed() };
    input.header.dwType = RIM_TYPEKEYBOARD;
    input.header.hDevice = handle as _;
    input.header.dwSize =
        (std::mem::size_of::<RAWINPUTHEADER>() + std::mem::size_of::<RAWKEYBOARD>()) as u32;
    input.data.keyboard.MakeCode = code;
    input.data.keyboard.Flags = flags;
    input.data.keyboard.VKey = 65;
    input.data.keyboard.Message = if flags & 1 == 0 { WM_KEYDOWN } else { WM_KEYUP };
    input
}
fn replay() -> ReactiveReplay {
    let map = ReactiveInputMap::from_device_json(
        r#"{"id":"k","leds":[{"row":2,"col":3,"inputType":"kbd","inputData":"[30,0]"}]}"#,
    )
    .unwrap();
    ReactiveReplay::new(
        map,
        &[LedAddress { row: 2, col: 3 }],
        vec![ReplayRegion {
            leds: vec![LedAddress { row: 2, col: 3 }],
            pulse: PulseRecording::new(40, &[Pixel::opaque(Rgb(200, 40, 0)), Pixel::EMPTY])
                .unwrap(),
        }],
        Retrigger::Restart,
    )
    .unwrap()
}
#[test]
fn exact_collection_decoder_refuses_foreign_fake_malformed_and_conflicting_records() {
    assert_eq!(
        decode(&packet(7, 30, 0), 7),
        Some((KeyboardKey::new(30, 0).unwrap(), true))
    );
    assert_eq!(
        decode(&packet(7, 30, 3), 7),
        Some((KeyboardKey::new(30, 2).unwrap(), false))
    );
    for input in [
        packet(8, 30, 0),
        packet(7, 0, 0),
        packet(7, 255, 0),
        packet(7, 30, 6),
        packet(7, 30, 8),
    ] {
        assert!(decode(&input, 7).is_none());
    }
    let mut input = packet(7, 30, 0);
    input.header.dwSize -= 1;
    assert!(decode(&input, 7).is_none());
    input = packet(7, 30, 0);
    input.header.dwType = 0;
    assert!(decode(&input, 7).is_none());
    input = packet(7, 30, 0);
    input.data.keyboard.Message = WM_KEYUP;
    assert!(decode(&input, 7).is_none());
    input = packet(7, 30, 0);
    input.data.keyboard.VKey = 255;
    assert!(decode(&input, 7).is_none());
    assert!(decode(&packet(0, 30, 0), 0).is_none());
}

struct Harness {
    replay: ReactiveReplay,
    input: ReplayInput,
    now: u64,
    accepted: usize,
}
const RECORD: u32 = WM_APP + 0x531;
const LOST: u32 = WM_APP + 0x532;
unsafe extern "system" fn procedure(hwnd: HWND, message: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    if message == WM_NCCREATE {
        let create = &*(l as *const CREATESTRUCTW);
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, create.lpCreateParams as isize);
    }
    let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Harness;
    if !ptr.is_null() {
        let state = &mut *ptr;
        if message == RECORD {
            // Synchronous private WM_APP record, never counterfeit WM_INPUT or
            // GetRawInputData and never a generated keyboard message.
            state.accepted = state
                .input
                .observe(&mut state.replay, &*(l as *const RAWINPUT), state.now)
                .unwrap();
            return 0;
        }
        if message == LOST {
            state.input.source_lost(&mut state.replay);
            return 0;
        }
        if message == WM_NCDESTROY {
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
        }
    }
    DefWindowProcW(hwnd, message, w, l)
}
#[test]
fn owned_message_window_routes_only_explicit_synthetic_records_and_stops_on_loss() {
    static CLASS: OnceLock<Vec<u16>> = OnceLock::new();
    let class = CLASS.get_or_init(|| {
        let name: Vec<u16> = "SynapseLiteReactiveReplayTest\0".encode_utf16().collect();
        let definition = WNDCLASSW {
            lpfnWndProc: Some(procedure),
            hInstance: unsafe { GetModuleHandleW(null()) },
            lpszClassName: name.as_ptr(),
            ..unsafe { std::mem::zeroed() }
        };
        assert_ne!(unsafe { RegisterClassW(&definition) }, 0);
        name
    });
    let mut plan = replay();
    let session = plan.start_local_preview();
    let mut state = Box::new(Harness {
        replay: plan,
        input: ReplayInput::diagnostic(session.clone(), "k".into(), 7),
        now: 0,
        accepted: 0,
    });
    unsafe {
        let hwnd = CreateWindowExW(
            0,
            class.as_ptr(),
            class.as_ptr(),
            0,
            0,
            0,
            0,
            0,
            HWND_MESSAGE,
            null_mut(),
            GetModuleHandleW(null()),
            (&mut *state as *mut Harness).cast(),
        );
        assert!(!hwnd.is_null());
        let foreign = packet(8, 30, 0);
        SendMessageW(hwnd, RECORD, 0, &foreign as *const RAWINPUT as isize);
        assert_eq!(state.accepted, 0);
        let own = packet(7, 30, 0);
        SendMessageW(hwnd, RECORD, 0, &own as *const RAWINPUT as isize);
        assert_eq!(state.accepted, 1);
        let mut frame = [Rgb(0, 0, 0)];
        assert!(state.replay.blend(&session, 0, 100, &mut frame).unwrap());
        assert_eq!(frame, [Rgb(200, 40, 0)]);
        SendMessageW(hwnd, LOST, 0, 0);
        SendMessageW(hwnd, RECORD, 0, &own as *const RAWINPUT as isize);
        assert_eq!(state.accepted, 0);
        assert!(!state.replay.blend(&session, 40, 100, &mut frame).unwrap());
        let fresh = state.replay.start_local_preview();
        SendMessageW(hwnd, LOST, 0, 0);
        SendMessageW(hwnd, RECORD, 0, &own as *const RAWINPUT as isize);
        assert_eq!(state.accepted, 0);
        frame.fill(Rgb(0, 0, 0));
        assert!(state.replay.blend(&fresh, 0, 100, &mut frame).unwrap());
        assert_eq!(frame, [Rgb(0, 0, 0)]);
        assert_ne!(DestroyWindow(hwnd), 0);
    }
}
