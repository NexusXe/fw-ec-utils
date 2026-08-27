use std::{ffi::c_char, fmt};

use ec_core::{EcCmd, EcCommand, EcError, MemMapRegion};

#[repr(C)]
#[allow(unused)]
enum EcBatteryVendorParamMode {
    Get = 0,
    Set,
}

// struct is __packed
#[repr(C, packed)]
#[allow(unused)]
struct EcParamsBatteryVendorParam {
    param: u32,
    value: u32,
    mode: u8,
}

// struct is __packed __aligned(4)
#[repr(C)]
#[allow(unused)]
struct EcResponseBatteryVendorParam {
    value: u32,
}

// /// Battery static info parameters
// #[repr(C, packed)]
// struct EcParamsBatteryStaticInfo {
//     /// Battery index.
//     index: u8,
// }

const EC_COMM_TEXT_MAX: usize = 8;

// // struct is __packed __aligned(4)
// #[repr(C)]
// /// Battery static info response
// struct EcResponseBatteryStaticInfo {
//     /// Battery Design Capacity (mAh)
//     design_capacity: u16,
//     /// Battery Design Voltage (mV)
//     design_voltage: u16,
//     /// Battery Manufacturer String
//     manufacturer: [u8; EC_COMM_TEXT_MAX],
//     /// Battery Model Number String
//     model: [u8; EC_COMM_TEXT_MAX],
//     /// Battery Serial Number String
//     serial: [u8; EC_COMM_TEXT_MAX],
//     /// Battery Type String
//     r#type: [u8; EC_COMM_TEXT_MAX],
//     /// Battery Cycle Count
//     cycle_count: u32,
// }

// struct is __packed
#[repr(C, packed)]
#[derive(Clone, Copy)]
/// Battery dynamic info parameters
struct EcParamsBatteryDynamicInfo {
    /// Battery index.
    index: u8,
}

// struct is __packed __aligned(2)
#[repr(C)]
#[derive(Debug, Clone, Copy)]
/// Battery dynamic info response
pub(crate) struct EcResponseBatteryDynamicInfo {
    /// Battery voltage (mV)
    pub(crate) actual_voltage: i16,
    /// Battery current (mA); negative=discharging
    pub(crate) actual_current: i16,
    /// Remaining capacity (mAh)
    pub(crate) remaining_capacity: i16,
    /// Full capacity (mAh, might change occasionally)
    pub(crate) full_capacity: i16,
    /// Flags, see [`EcBattFlag`]
    pub(crate) flags: i16,
    // Charging voltage desired by battery (mV)
    pub(crate) desired_voltage: i16,
    // Charging current desired by battery (mA)
    pub(crate) desired_current: i16,
}

/// Get battery dynamic information, i.e. information that is likely to change
/// every time it is read.
struct BatteryGetDynamic;

impl EcCommand for BatteryGetDynamic {
    type Request = EcParamsBatteryDynamicInfo;
    type Response = EcResponseBatteryDynamicInfo;
    const CMD: EcCmd = EcCmd::BatteryGetDynamic;
}

pub(crate) fn get_battery_dynamic_info() -> Result<EcResponseBatteryDynamicInfo, EcError> {
    BatteryGetDynamic::call(EcParamsBatteryDynamicInfo { index: 0 })
}
/// Battery bit flags at EC_MMAP_BATT_FLAG.
#[derive(Debug)]
pub(crate) struct EcBattFlags(pub(crate) u8);

impl fmt::Display for EcBattFlags {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        const BATT_FLAG_DEFS: [&str; 8] = [
            "AC_PRESENT",
            "BATT_PRESENT",
            "DISCHARGING",
            "CHARGING",
            "LEVEL_CRITICAL",
            "INVALID_DATA",
            "UNKNOWN_6",
            "UNKNOWN_7",
        ];

        (0..u8::BITS)
            .filter(|&i| (self.0 >> i) & 1 == 1)
            .try_for_each(|i| {
                write!(
                    f,
                    "{}{}",
                    if i > 0 { ", " } else { "" },
                    BATT_FLAG_DEFS[i as usize]
                )
            })
    }
}

/// A helper struct representing the very helpfully 64-byte aligned and
/// pre-padded data available from EC.
#[repr(C, align(64))]
#[derive(Debug)]
pub(crate) struct MemMappedBatteryInfo {
    /// Battery Present Voltage
    pub(crate) volt: u32,
    /// Battery Present Rate
    pub(crate) rate: u32,
    /// Battery Remaining Capacity
    pub(crate) cap: u32,
    /// Battery State
    pub(crate) state: EcBattFlags,
    /// Battery Count
    pub(crate) count: u8,
    /// Current Battery Data Index
    pub(crate) index: u8,
    /// Battery Design Capacity
    pub(crate) dcap: u32,
    /// Battery Design Voltage
    pub(crate) dvlt: u32,
    /// Battery Last Full Charge Capacity
    pub(crate) lfcc: u32,
    /// Battery Cycle Count
    pub(crate) ccnt: u32,
    /// Battery Manufacturer String
    pub(crate) mfgr: [c_char; EC_COMM_TEXT_MAX],
    /// Battery Model Number String
    pub(crate) model: [c_char; EC_COMM_TEXT_MAX],
    /// Battery Serial Number String
    pub(crate) serial: [c_char; EC_COMM_TEXT_MAX],
    /// Battery Type String
    pub(crate) r#type: [c_char; EC_COMM_TEXT_MAX],
}

impl fmt::Display for MemMappedBatteryInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "---- Stats for battery at index {:} ----", self.index)?;

        // there has to be a better way of doing this
        let display_c_str = |c_array: &[c_char]| {
            let bytes =
                unsafe { std::slice::from_raw_parts(c_array.as_ptr() as *const u8, c_array.len()) };
            let len = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
            String::from_utf8_lossy(&bytes[..len])
        };

        writeln!(f, "Manufacturer:  {}", display_c_str(&self.mfgr))?;
        writeln!(f, "Model:         {}", display_c_str(&self.model))?;
        writeln!(f, "Serial:        {}", display_c_str(&self.serial))?;
        writeln!(f, "Type:          {}", display_c_str(&self.r#type))?;

        writeln!(f, "State:         {}", self.state)?;

        writeln!(f, "Count:         {}", self.count)?;
        writeln!(f, "Cycle Count:   {}", self.ccnt)?;

        // Grouping the capacity and voltage stats for readability
        writeln!(
            f,
            "Design Volt:   {}.{:03} V",
            self.dvlt / 1000,
            self.dvlt % 1000
        )?;
        writeln!(
            f,
            "Present Volt:  {}.{:03} V",
            self.volt / 1000,
            self.volt % 1000
        )?;
        writeln!(
            f,
            "Present Curr:  {}.{:03} A",
            self.rate / 1000,
            self.rate % 1000
        )?;
        writeln!(f, "Remain. mAh:   {}", self.cap)?;
        writeln!(f, "Design mAh:    {}", self.dcap)?;
        writeln!(f, "Last Full mAh: {}", self.lfcc)?;

        Ok(())
    }
}

// SAFETY: `MemMappedBatteryInfo` is `#[repr(C)]`, holds only integers and
// `c_char` arrays -- so every bit pattern is valid -- and its fields line up
// with the battery block of the EC memory map (see `ec_mmap_offsets::Batt`).
unsafe impl MemMapRegion for MemMappedBatteryInfo {
    const OFFSET: u32 = crate::ec_mmap_offsets::Batt::Volt as u32;
}

pub(crate) fn get_memmapped_battery_info() -> Result<MemMappedBatteryInfo, EcError> {
    MemMappedBatteryInfo::read()
}
