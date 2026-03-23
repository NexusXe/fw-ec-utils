/// Battery values are all 32 bits, unless otherwise noted.
pub(crate) enum Batt {
    /// Battery Present Voltage
    Volt = 0x40,
    /// Battery Present Rate
    Rate = 0x44,
    /// Battery Remaining Capacity
    Cap = 0x48,
    /// Battery State, see [`crate::battery::EcBattFlag`] (8-bit)
    Flag = 0x4c,
    /// Battery Count (8-bit)
    Count = 0x4d,
    /// Current Battery Data Index (8-bit)
    Index = 0x4e,
    /// Battery Design Capacity
    Dcap = 0x50,
    /// Battery Design Voltage
    Dvlt = 0x54,
    /// Battery Last Full Charge Capacity
    Lfcc = 0x58,
    /// Battery Cycle Count
    Ccnt = 0x5c,
    /// Battery Manufacturer String
    Mfgr = 0x60,
    /// Battery Model Number String
    Model = 0x68,
    /// Battery Serial Number String
    Serial = 0x70,
    /// Battery Type String
    Type = 0x78,
}
