use std::ffi::c_void;
use std::ptr::NonNull;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use libloading::Library;
use windows::Win32::System::EventLog::{
    EVT_HANDLE, EVT_SUBSCRIBE_NOTIFY_ACTION, EvtClose, EvtSubscribe, EvtSubscribeActionDeliver,
    EvtSubscribeActionError, EvtSubscribeToFutureEvents,
};
use windows::core::w;

const EVENT_LOG_CHANNEL: windows::core::PCWSTR =
    w!("Microsoft-Windows-PushNotification-Platform/Operational");
const EVENT_LOG_QUERY: windows::core::PCWSTR = w!(
    "*[System[EventID=3052] or (System[EventID=3053] and EventData[Data[@Name='NotificationType']='toast'])]"
);
const WNF_SHEL_TOAST_PUBLISHED: u64 = 0x0D83_063E_A3BD_0035;
const STATUS_SUCCESS: i32 = 0;

static DELIVERY_PENDING: AtomicBool = AtomicBool::new(false);
static ERROR_PENDING: AtomicBool = AtomicBool::new(false);
static ERROR_CODE: AtomicU32 = AtomicU32::new(0);

type WnfCallback = unsafe extern "system" fn(
    state_name: u64,
    change_stamp: u32,
    type_id: *const c_void,
    callback_context: *mut c_void,
    buffer: *const c_void,
    length: u32,
) -> i32;
type RtlQueryWnfStateData = unsafe extern "system" fn(
    change_stamp: *mut u32,
    state_name: u64,
    callback: WnfCallback,
    callback_context: *mut c_void,
    type_id: *const c_void,
) -> i32;
type RtlSubscribeWnfStateChangeNotification = unsafe extern "system" fn(
    subscription_handle: *mut *mut c_void,
    state_name: u64,
    change_stamp: u32,
    callback: WnfCallback,
    callback_context: *mut c_void,
    type_id: *const c_void,
    serialization_group: u32,
    flags: u32,
) -> i32;
type RtlUnsubscribeWnfStateChangeNotification =
    unsafe extern "system" fn(subscription_handle: *mut c_void) -> i32;

pub(super) struct NotificationEventSubscription {
    _wnf: Option<WnfToastSubscription>,
    _event_log: Option<EventLogSubscription>,
}

struct WnfToastSubscription {
    handle: NonNull<c_void>,
    unsubscribe: RtlUnsubscribeWnfStateChangeNotification,
    _library: Library,
}

struct EventLogSubscription {
    handle: EVT_HANDLE,
}

impl NotificationEventSubscription {
    pub(super) fn subscribe() -> Result<Self, String> {
        reset_signals();
        let wnf = match WnfToastSubscription::subscribe() {
            Ok(subscription) => {
                log::info!("Notification events are using WNF toast publication signals");
                Some(subscription)
            }
            Err(error) => {
                log::warn!("WNF toast subscription is unavailable: {error}");
                None
            }
        };
        let event_log = match EventLogSubscription::subscribe() {
            Ok(subscription) => Some(subscription),
            Err(error) if wnf.is_some() => {
                log::warn!("Notification Event Log reconciliation is unavailable: {error:?}");
                None
            }
            Err(error) => {
                return Err(format!(
                    "WNF and Event Log notification subscriptions are unavailable: {error:?}"
                ));
            }
        };
        Ok(Self {
            _wnf: wnf,
            _event_log: event_log,
        })
    }

    pub(super) fn has_wnf(&self) -> bool {
        self._wnf.is_some()
    }
}

impl WnfToastSubscription {
    fn subscribe() -> Result<Self, String> {
        // SAFETY: ntdll.dll is a trusted Windows system library and remains loaded for the process
        // lifetime. All resolved symbols are called with their native ABI signatures below.
        let library = unsafe { Library::new("ntdll.dll") }
            .map_err(|error| format!("ntdll.dll could not be loaded: {error}"))?;
        // SAFETY: The symbol types match the Windows native API declarations.
        let query = unsafe {
            *library
                .get::<RtlQueryWnfStateData>(b"RtlQueryWnfStateData\0")
                .map_err(|error| format!("RtlQueryWnfStateData is unavailable: {error}"))?
        };
        // SAFETY: The symbol types match the Windows native API declarations.
        let subscribe = unsafe {
            *library
                .get::<RtlSubscribeWnfStateChangeNotification>(
                    b"RtlSubscribeWnfStateChangeNotification\0",
                )
                .map_err(|error| {
                    format!("RtlSubscribeWnfStateChangeNotification is unavailable: {error}")
                })?
        };
        // SAFETY: The symbol types match the Windows native API declarations.
        let unsubscribe = unsafe {
            *library
                .get::<RtlUnsubscribeWnfStateChangeNotification>(
                    b"RtlUnsubscribeWnfStateChangeNotification\0",
                )
                .map_err(|error| {
                    format!("RtlUnsubscribeWnfStateChangeNotification is unavailable: {error}")
                })?
        };

        let mut change_stamp = 0;
        // SAFETY: All pointers are valid for the synchronous query. The callback ignores the
        // optional state buffer, and the state name was verified on supported Windows versions.
        let status = unsafe {
            query(
                &mut change_stamp,
                WNF_SHEL_TOAST_PUBLISHED,
                wnf_query_callback,
                std::ptr::null_mut(),
                std::ptr::null(),
            )
        };
        if status < STATUS_SUCCESS {
            return Err(format!(
                "state query returned NTSTATUS 0x{:08X}",
                status as u32
            ));
        }

        let mut handle = std::ptr::null_mut();
        // SAFETY: The output pointer is valid, callbacks use no caller-owned context, and the
        // resolved function pointer has the native RtlSubscribeWnfStateChangeNotification ABI.
        let status = unsafe {
            subscribe(
                &mut handle,
                WNF_SHEL_TOAST_PUBLISHED,
                change_stamp,
                wnf_notification_callback,
                std::ptr::null_mut(),
                std::ptr::null(),
                0,
                0,
            )
        };
        if status < STATUS_SUCCESS {
            return Err(format!(
                "subscription returned NTSTATUS 0x{:08X}",
                status as u32
            ));
        }
        let Some(handle) = NonNull::new(handle) else {
            return Err("subscription returned a null handle".to_string());
        };
        Ok(Self {
            handle,
            unsubscribe,
            _library: library,
        })
    }
}

impl Drop for WnfToastSubscription {
    fn drop(&mut self) {
        // SAFETY: This instance owns the subscription handle and the function pointer remains
        // valid because its ntdll library handle is retained until after this destructor runs.
        let status = unsafe { (self.unsubscribe)(self.handle.as_ptr()) };
        if status < STATUS_SUCCESS {
            log::debug!(
                "WNF toast subscription could not be closed: NTSTATUS 0x{:08X}",
                status as u32
            );
        }
    }
}

impl EventLogSubscription {
    fn subscribe() -> windows::core::Result<Self> {
        // SAFETY: The channel and query are static null-terminated strings. The callback does not
        // retain either the event handle or user context, and the returned handle is owned here.
        let handle = unsafe {
            EvtSubscribe(
                None,
                None,
                EVENT_LOG_CHANNEL,
                EVENT_LOG_QUERY,
                None,
                None,
                Some(event_log_callback),
                EvtSubscribeToFutureEvents.0,
            )?
        };
        Ok(Self { handle })
    }
}

impl Drop for EventLogSubscription {
    fn drop(&mut self) {
        // SAFETY: This instance owns the valid subscription handle returned by EvtSubscribe and
        // closes it exactly once while being dropped.
        if let Err(error) = unsafe { EvtClose(self.handle) } {
            log::debug!("Notification event subscription could not be closed: {error:?}");
        }
    }
}

pub(super) fn take_delivery() -> bool {
    DELIVERY_PENDING.swap(false, Ordering::AcqRel)
}

pub(super) fn take_error() -> Option<u32> {
    ERROR_PENDING
        .swap(false, Ordering::AcqRel)
        .then(|| ERROR_CODE.load(Ordering::Acquire))
}

pub(super) fn reset_signals() {
    DELIVERY_PENDING.store(false, Ordering::Release);
    ERROR_PENDING.store(false, Ordering::Release);
    ERROR_CODE.store(0, Ordering::Release);
}

unsafe extern "system" fn wnf_query_callback(
    _state_name: u64,
    _change_stamp: u32,
    _type_id: *const c_void,
    _callback_context: *mut c_void,
    _buffer: *const c_void,
    _length: u32,
) -> i32 {
    STATUS_SUCCESS
}

unsafe extern "system" fn wnf_notification_callback(
    _state_name: u64,
    _change_stamp: u32,
    _type_id: *const c_void,
    _callback_context: *mut c_void,
    _buffer: *const c_void,
    _length: u32,
) -> i32 {
    DELIVERY_PENDING.store(true, Ordering::Release);
    super::wake();
    STATUS_SUCCESS
}

unsafe extern "system" fn event_log_callback(
    action: EVT_SUBSCRIBE_NOTIFY_ACTION,
    _user_context: *const c_void,
    event: EVT_HANDLE,
) -> u32 {
    if action == EvtSubscribeActionDeliver {
        DELIVERY_PENDING.store(true, Ordering::Release);
    } else if action == EvtSubscribeActionError {
        ERROR_CODE.store(event.0 as u32, Ordering::Release);
        ERROR_PENDING.store(true, Ordering::Release);
    } else {
        return 0;
    }
    super::wake();
    0
}
