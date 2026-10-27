//! Unsigned build experiment, never loaded or attached to a device.
//!
//! Requests retain their class-connect payload and are forwarded to the lower
//! target. This deliberately implements no Razer selector or input injection.
#![no_std]
#![deny(unsafe_op_in_unsafe_fn)]

extern crate wdk_panic;

use core::{mem::size_of, ptr, sync::atomic::{AtomicBool, AtomicU64, Ordering}};
use wdk_sys::{
    call_unsafe_wdf_function_binding as wdf, NTSTATUS, PCUNICODE_STRING,
    PDRIVER_OBJECT, WDFDEVICE, WDFDEVICE_INIT, WDFDRIVER, WDFOBJECT, WDFQUEUE,
    WDFREQUEST, WDF_DRIVER_CONFIG, WDF_OBJECT_ATTRIBUTES,
    WDF_OBJECT_CONTEXT_TYPE_INFO, WDF_IO_QUEUE_CONFIG, WDF_REQUEST_SEND_OPTIONS,
    _WDF_EXECUTION_LEVEL::WdfExecutionLevelInheritFromParent,
    _WDF_SYNCHRONIZATION_SCOPE::WdfSynchronizationScopeInheritFromParent,
    _WDF_IO_QUEUE_DISPATCH_TYPE::WdfIoQueueDispatchParallel,
    _WDF_TRI_STATE::WdfUseDefault,
    _WDF_REQUEST_SEND_OPTIONS_FLAGS::WDF_REQUEST_SEND_OPTION_SEND_AND_FORGET,
};

const STATUS_INVALID_DEVICE_STATE: NTSTATUS = 0xC000_0184u32 as i32;
const STATUS_DELETE_PENDING: NTSTATUS = 0xC000_0056u32 as i32;

// Checked against the exact generated x64 WDK types, not hand-written FFI.
const _: () = {
    assert!(size_of::<WDF_DRIVER_CONFIG>() == 32);
    assert!(size_of::<WDF_OBJECT_ATTRIBUTES>() == 56);
    assert!(size_of::<WDF_OBJECT_CONTEXT_TYPE_INFO>() == 40);
    assert!(size_of::<WDF_IO_QUEUE_CONFIG>() == 96);
    assert!(size_of::<WDF_REQUEST_SEND_OPTIONS>() == 16);
};

#[repr(C)]
struct DeviceContext {
    closing: AtomicBool,
    forwarding_attempts: AtomicU64,
    failed_sends: AtomicU64,
}

// WDF's type identifier has static lifetime. Its pointers refer to immutable
// static metadata only; the framework does not mutate this descriptor.
struct ContextDescriptor(WDF_OBJECT_CONTEXT_TYPE_INFO);
unsafe impl Sync for ContextDescriptor {}

static DEVICE_CONTEXT_TYPE: ContextDescriptor = ContextDescriptor(WDF_OBJECT_CONTEXT_TYPE_INFO {
    Size: size_of::<WDF_OBJECT_CONTEXT_TYPE_INFO>() as u32,
    ContextName: c"RZ_PASSTHROUGH_CONTEXT".as_ptr().cast_mut(),
    ContextSize: size_of::<DeviceContext>(),
    UniqueType: ptr::null(),
    EvtDriverGetUniqueContextType: None,
});

unsafe fn context(object: WDFOBJECT) -> *mut DeviceContext {
    // The same static type descriptor is supplied at device creation.
    unsafe { wdf!(WdfObjectGetTypedContextWorker, object, &DEVICE_CONTEXT_TYPE.0) }.cast()
}

#[unsafe(export_name = "DriverEntry")]
pub unsafe extern "system" fn driver_entry(
    driver: PDRIVER_OBJECT,
    registry_path: PCUNICODE_STRING,
) -> NTSTATUS {
    let mut config = WDF_DRIVER_CONFIG {
        Size: size_of::<WDF_DRIVER_CONFIG>() as u32,
        EvtDriverDeviceAdd: Some(device_add),
        ..Default::default()
    };
    // WDF owns driver lifetime; no custom unload or global allocations exist.
    unsafe { wdf!(WdfDriverCreate, driver, registry_path, ptr::null_mut(), &mut config, ptr::null_mut()) }
}

unsafe extern "C" fn device_add(_driver: WDFDRIVER, mut init: *mut WDFDEVICE_INIT) -> NTSTATUS {
    // Mark as a filter before creation so unhandled request types retain WDF's
    // filter forwarding behavior. No physical interface IDs are registered.
    unsafe { wdf!(WdfFdoInitSetFilter, init) };
    let mut attributes = WDF_OBJECT_ATTRIBUTES {
        Size: size_of::<WDF_OBJECT_ATTRIBUTES>() as u32,
        EvtCleanupCallback: Some(device_cleanup),
        ExecutionLevel: WdfExecutionLevelInheritFromParent,
        SynchronizationScope: WdfSynchronizationScopeInheritFromParent,
        ContextTypeInfo: &DEVICE_CONTEXT_TYPE.0,
        ..Default::default()
    };
    let mut device: WDFDEVICE = ptr::null_mut();
    let status = unsafe { wdf!(WdfDeviceCreate, &mut init, &mut attributes, &mut device) };
    if status < 0 { return status; }

    let state = unsafe { context(device.cast()) };
    if state.is_null() { return STATUS_INVALID_DEVICE_STATE; }
    // Initialize before creating the queue that can deliver request callbacks.
    unsafe { state.write(DeviceContext {
        closing: AtomicBool::new(false),
        forwarding_attempts: AtomicU64::new(0),
        failed_sends: AtomicU64::new(0),
    }) };

    let mut queue = WDF_IO_QUEUE_CONFIG {
        Size: size_of::<WDF_IO_QUEUE_CONFIG>() as u32,
        DispatchType: WdfIoQueueDispatchParallel,
        PowerManaged: WdfUseDefault,
        DefaultQueue: 1,
        EvtIoDefault: Some(forward_request),
        ..Default::default()
    };
    // Match WDF_IO_QUEUE_CONFIG_INIT_DEFAULT_QUEUE's parallel queue setting.
    queue.Settings.Parallel.NumberOfPresentedRequests = u32::MAX;
    // Parallel delivery; context counters/state are atomic. The device owns
    // the queue and WDF retains handles during each framework callback.
    unsafe { wdf!(WdfIoQueueCreate, device, &mut queue, ptr::null_mut(), ptr::null_mut()) }
}

unsafe extern "C" fn device_cleanup(object: WDFOBJECT) {
    let state = unsafe { context(object) };
    if let Some(state) = unsafe { state.as_ref() } {
        state.closing.store(true, Ordering::Release);
    }
    // No custom service callback, timer, target, queue or allocation is owned
    // outside the WDF object tree. No key-up or hardware operation is issued.
}

unsafe extern "C" fn forward_request(queue: WDFQUEUE, request: WDFREQUEST) {
    let device = unsafe { wdf!(WdfIoQueueGetDevice, queue) };
    let state = unsafe { context(device.cast()) };
    let Some(state) = (unsafe { state.as_ref() }) else {
        unsafe { wdf!(WdfRequestComplete, request, STATUS_INVALID_DEVICE_STATE) };
        return;
    };
    if state.closing.load(Ordering::Acquire) {
        unsafe { wdf!(WdfRequestComplete, request, STATUS_DELETE_PENDING) };
        return;
    }
    state.forwarding_attempts.fetch_add(1, Ordering::Relaxed);
    let target = unsafe { wdf!(WdfDeviceGetIoTarget, device) };
    let mut options = WDF_REQUEST_SEND_OPTIONS {
        Size: size_of::<WDF_REQUEST_SEND_OPTIONS>() as u32,
        Flags: WDF_REQUEST_SEND_OPTION_SEND_AND_FORGET as u32,
        ..Default::default()
    };
    unsafe { wdf!(WdfRequestFormatRequestUsingCurrentType, request) };
    // Successful send-and-forget transfers request ownership to the lower
    // stack. Do not access or complete request after that successful send.
    if unsafe { wdf!(WdfRequestSend, request, target, &mut options) } == 0 {
        state.failed_sends.fetch_add(1, Ordering::Relaxed);
        let status = unsafe { wdf!(WdfRequestGetStatus, request) };
        unsafe { wdf!(WdfRequestComplete, request, status) };
    }
}
