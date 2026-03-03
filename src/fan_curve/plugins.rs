use std::{
    collections::HashMap,
    ffi::{CStr, CString, c_char},
    sync::{LazyLock, Mutex},
};

use crate::{
    temp::{
        EC_TEMP_SENSOR_ENTRIES, EcResponseTempSensorGetInfo, NUM_TEMP_SENSORS, TempSensorVector,
        get_temperatures_v, probe_sensor,
    },
    warn,
};

pub(crate) static ALL_SENSORS: LazyLock<[EcResponseTempSensorGetInfo; EC_TEMP_SENSOR_ENTRIES]> =
    LazyLock::new(|| {
        let mut sensors = [EcResponseTempSensorGetInfo::default(); EC_TEMP_SENSOR_ENTRIES];

        let limit = NUM_TEMP_SENSORS.min(EC_TEMP_SENSOR_ENTRIES as u8);

        for id in 0..limit {
            if let Ok(info) = probe_sensor(id) {
                sensors[id as usize] = info;
            } else {
                warn!("Failed to probe sensor {id:}, despite EC reporting {limit:} sensors.");
            }
        }

        sensors
    });

#[repr(C)]
pub enum PluginGetStatus {
    /// The buffer was large enough and the data was successfully copied.
    Success = 0,
    /// The key does not exist or arguments were null.
    NotFound = 1,
    /// The key exists, but the provided buffer was null or too small.
    /// The required size has been written to the length out-parameter.
    BufferTooSmall = 2,
}

static PLUGIN_STATE: LazyLock<Mutex<HashMap<CString, Box<[u8]>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

pub(crate) extern "C" fn plugin_set(key: *const c_char, data: *const u8, len: usize) -> bool {
    // Null key is invalid. Null data is only valid if length is 0.
    if key.is_null() || (data.is_null() && len > 0) {
        return false;
    }

    let c_str = unsafe { CStr::from_ptr(key) };
    let c_string = CString::from(c_str);

    // SAFETY: We checked for null. We trust the C caller that `data` points to
    // at least `len` initialized bytes and does not alias mutably.
    let data_slice = if len == 0 {
        &[] // Handle zero-length allocations gracefully
    } else {
        unsafe { std::slice::from_raw_parts(data, len) }
    };

    // Copy the C buffer into a Rust-owned Box so it survives the function call
    let owned_data = data_slice.to_vec().into_boxed_slice();

    PLUGIN_STATE.lock().unwrap().insert(c_string, owned_data);

    true
}

pub(crate) extern "C" fn plugin_get(
    key: *const c_char,
    buffer: *mut u8,
    buffer_len: *mut usize,
) -> PluginGetStatus {
    if key.is_null() || buffer_len.is_null() {
        return PluginGetStatus::NotFound;
    }

    let c_str = unsafe { CStr::from_ptr(key) };

    if let Some(val) = PLUGIN_STATE.lock().unwrap().get(c_str) {
        let required_len = val.len();

        // Read the capacity the C plugin provided
        let provided_capacity = unsafe { *buffer_len };

        // Always tell the caller the exact size of the stored data
        unsafe { *buffer_len = required_len };

        // If the C buffer is null, or smaller than required, reject the copy.
        if buffer.is_null() || provided_capacity < required_len {
            return PluginGetStatus::BufferTooSmall;
        }

        // SAFETY: We verified the buffer is not null and is large enough.
        // We must trust the C caller that `buffer` is valid and unaliased for `required_len` bytes.
        unsafe {
            std::ptr::copy_nonoverlapping(val.as_ptr(), buffer, required_len);
        }

        return PluginGetStatus::Success; // Lock is dropped here
    }

    PluginGetStatus::NotFound
}

#[repr(C)]
pub struct PluginStateMethods {
    pub set: extern "C" fn(*const c_char, *const u8, usize) -> bool,
    pub get: extern "C" fn(*const c_char, *mut u8, *mut usize) -> PluginGetStatus,
}

#[repr(C)]
pub(crate) struct PluginCallData {
    sensors: *const EcResponseTempSensorGetInfo,
    temps: *const TempSensorVector,
    state: PluginStateMethods,
}

/// Unfinished plugin interface. Calls a shared object file
pub(crate) fn call_plugin() -> Result<(), Box<dyn std::error::Error>> {
    let readings = get_temperatures_v()?;

    let readings_ptr = readings.as_array().as_ptr();
    let sensors_ptr = ALL_SENSORS.as_ptr();

    todo!()
}
