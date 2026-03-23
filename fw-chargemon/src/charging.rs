use std::fmt;

use ec_core::common::{CrosEcCommandV2, EcCmd, FullWriteV2Command, fire};

/// Intel-specific items
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub(crate) struct IntelBattDbpt {
    /// Battery's level of DBPT support: 0, 2
    support_level: u8,
    /// Maximum peak power from battery (10ms), Watts
    /// If DBPT is not supported, this is 0
    max_peak_power: u8,
    /// Maximum sustained power from battery, Watts
    /// If DBPT is not supported, this is 0
    sus_peak_power: u8,
}

impl const Default for IntelBattDbpt {
    fn default() -> Self {
        Self {
            support_level: 0,
            max_peak_power: 0,
            sus_peak_power: 0,
        }
    }
}

impl fmt::Display for IntelBattDbpt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.support_level != 0 {
            write!(
                f,
                "Level {:}; 10ms Peak @ {:} W, {:} W Sus",
                self.support_level, self.max_peak_power, self.sus_peak_power
            )
        } else {
            write!(f, "Unsupported")
        }
    }
}

/// v1 of EC_CMD_POWER_INFO
#[repr(u8)]
#[derive(Debug, Clone, Copy)]
pub(crate) enum SystemPowerSource {
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

impl const Default for SystemPowerSource {
    fn default() -> Self {
        Self::Unknown
    }
}

impl fmt::Display for SystemPowerSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::Unknown => "Unknown",
                Self::Battery => "Battery",
                Self::AC => "AC",
                Self::ACBattery => "ACBattery",
            }
        )
    }
}

#[repr(C, packed)]
#[derive(Debug, Default)]
pub(crate) struct EcResponsePowerInfoV1 {
    /// enum [`SystemPowerSource`]
    pub(crate) system_power_source: SystemPowerSource,
    /// Battery state-of-charge, 0-100, 0 if not present
    pub(crate) battery_soc: u8,
    /// AC Adapter 100% rating, Watts
    pub(crate) ac_adapter_100pct: u8,
    /// AC Adapter 10ms rating, Watts
    pub(crate) ac_adapter_10ms: u8,
    /// Battery 1C rating, derated
    pub(crate) battery_1cd: u8,
    /// Rest of Platform average, Watts
    pub(crate) rop_avg: u8,
    /// Rest of Platform peak, Watts
    pub(crate) rop_peak: u8,
    /// Nominal charger efficiency, %
    pub(crate) nominal_charger_eff: u8,
    /// Rest of Platform VR Average Efficiency, %
    pub(crate) rop_avg_eff: u8,
    /// Rest of Platform VR Peak Efficiency, %
    pub(crate) rop_peak_eff: u8,
    /// SoC VR Efficiency at Average level, %
    pub(crate) soc_avg_eff: u8,
    /// SoC VR Efficiency at Peak level, %
    pub(crate) soc_peak_eff: u8,
    /// Intel-specific items
    pub(crate) intel: IntelBattDbpt,
}
impl fmt::Display for EcResponsePowerInfoV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "---- System Power Source Information ----")?;

        writeln!(f, "Power Source:         {}", self.system_power_source)?;
        writeln!(f, "Battery SoC:          {}%", self.battery_soc)?;
        writeln!(f, "AC Adapter (100%):    {} W", self.ac_adapter_100pct)?;
        writeln!(f, "AC Adapter (10ms):    {} W", self.ac_adapter_10ms)?;
        writeln!(f, "Battery 1C Derated:   {}", self.battery_1cd)?;
        writeln!(f, "ROP Average:          {} W", self.rop_avg)?;
        writeln!(f, "ROP Peak:             {} W", self.rop_peak)?;
        writeln!(f, "Nominal Charger Eff:  {}%", self.nominal_charger_eff)?;
        writeln!(f, "ROP VR Avg Eff:       {}%", self.rop_avg_eff)?;
        writeln!(f, "ROP VR Peak Eff:      {}%", self.rop_peak_eff)?;
        writeln!(f, "SoC VR Avg Eff:       {}%", self.soc_avg_eff)?;
        writeln!(f, "SoC VR Peak Eff:      {}%", self.soc_peak_eff)?;

        writeln!(f, "Intel DBPT:           {}", self.intel)?;

        Ok(())
    }
}

/// Subcommands for the [`ec_core::common::EcCmd::ChargeState`] command
#[repr(C)]
pub(crate) enum ChargeStateCommand {
    CmdGetState,
    CmdGetParam,
    CmdSetParam,
    NumCmds,
}

/// Known param numbers are defined here. Ranges are reserved for board-specific
/// params, which are handled by the particular implementations.
#[repr(C)]
pub(crate) enum ChargeStateParams {
    /// charger voltage limit
    ParamChgVoltage,
    /// charger current limit
    ParamChgCurrent,
    /// charger input current limit
    ParamChgInputCurrent,
    /// charger-specific status
    ParamChgStatus,
    /// charger-specific options
    ParamChgOption,
    /// Check if power is limited due to low battery and / or a weak external
    /// charger. READ ONLY.
    ParamLimitPower,
    /// How many so far?
    NumBaseParams,
    /// Range for CONFIG_CHARGER_OVERRIDE params
    ParamCustomProfileMin = 0x100000,
    ParamCustomProfileMax = 0x1ffff,
}
