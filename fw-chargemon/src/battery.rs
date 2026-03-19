use crate::ec_mmap_offsets;

#[repr(C)]
enum EcBatteryVendorParamMode {
    Get = 0,
    Set,
}

// struct is __packed
#[repr(C, packed)]
struct EcParamsBatteryVendorParam {
    param: u32,
    value: u32,
    mode: u8,
}

// struct is __packed __aligned(4)
#[repr(C)]
struct EcResponseBatteryVendorParam {
    value: u32,
}

/// Battery static info parameters
#[repr(C, packed)]
struct EcParamsBatteryStaticInfo {
    /// Battery index.
    index: u8,
}

const EC_COMM_TEXT_MAX: usize = 8;

// struct is __packed __aligned(4)
#[repr(C)]
/// Battery static info response
struct EcResponseBatteryStaticInfo {
    /// Battery Design Capacity (mAh)
    design_capacity: u16,
    /// Battery Design Voltage (mV)
    design_voltage: u16,
    /// Battery Manufacturer String
    manufacturer: [u8; EC_COMM_TEXT_MAX],
    /// Battery Model Number String
    model: [u8; EC_COMM_TEXT_MAX],
    /// Battery Serial Number String
    serial: [u8; EC_COMM_TEXT_MAX],
    /// Battery Type String
    r#type: [u8; EC_COMM_TEXT_MAX],
    /// Battery Cycle Count
    cycle_count: u32,
}

// struct is __packed
#[repr(C, packed)]
/// Battery dynamic info parameters
struct EcParamsBatteryDynamicInfo {
    /// Battery index.
    index: u8,
}

/// Battery bit flags at EC_MMAP_BATT_FLAG.
enum EcBattFlag {
    ACPresent = 1 << 0,
    BattPresent = 1 << 1,
    Discharging = 1 << 2,
    Charging = 1 << 3,
    LevelCritical = 1 << 4,
    /// Set if some of the static/dynamic data is invalid (or outdated).
    InvalidData = 1 << 5,
}

// struct is __packed __aligned(2)
#[repr(C)]
/// Battery dynamic info response
struct EcResponseBatteryDynamicInfo {
    /// Battery voltage (mV)
    actual_voltage: i16,
    /// Battery current (mA); negative=discharging
    actual_current: i16,
    /// Remaining capacity (mAh)
    remaining_capacity: i16,
    /// Full capacity (mAh, might change occasionally)
    full_capacity: i16,
    /// Flags, see [`EcBattFlag`]
    flags: i16,
    // Charging voltage desired by battery (mV)
    desired_voltage: i16,
    // Charging current desired by battery (mA)
    desired_current: i16,
}
