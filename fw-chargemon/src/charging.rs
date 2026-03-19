/// Intel-specific items
#[repr(C)]
struct IntelBattDbpt {
    /// Battery's level of DBPT support: 0, 2
    support_level: u8,
    /// Maximum peak power from battery (10ms), Watts
    /// If DBPT is not supported, this is 0
    max_peak_power: u8,
    /// Maximum sustained power from battery, Watts
    /// If DBPT is not supported, this is 0
    sus_peak_power: u8,
}

/// v1 of EC_CMD_POWER_INFO
#[repr(C)]
enum SystemPowerSource {
    /// Haven't established which power source is used yet,
    /// or no presence signals are avaliable
    Unknown = 0,
    /// System is running on battery alone
    Battery = 1,
    /// System is running on A/C alone
    AC = 2,
    // System is running on A/C and battery
    ACBattery = 3,
}

#[repr(C, packed)]
struct EcResponsePowerInfoV1 {
    /// enum [`SystemPowerSource`]
    system_power_source: u8,
    /// Battery state-of-charge, 0-100, 0 if not present
    battery_soc: u8,
    /// AC Adapter 100% rating, Watts
    ac_adapter_100pct: u8,
    /// AC Adapter 10ms rating, Watts
    ac_adapter_10ms: u8,
    /// Battery 1C rating, derated
    battery_1cd: u8,
    /// Rest of Platform average, Watts
    rop_avg: u8,
    /// Rest of Platform peak, Watts
    rop_peak: u8,
    /// Nominal charger efficiency, %
    nominal_charger_eff: u8,
    /// Rest of Platform VR Average Efficiency, %
    rop_avg_eff: u8,
    /// Rest of Platform VR Peak Efficiency, %
    rop_peak_eff: u8,
    /// SoC VR Efficiency at Average level, %
    soc_avg_eff: u8,
    /// SoC VR Efficiency at Peak level, %
    soc_peak_eff: u8,
    /// Intel-specific items
    intel: IntelBattDbpt,
}
