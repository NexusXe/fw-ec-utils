use nix::ioctl_readwrite;
use std::ffi::c_int;
use std::fs::{File, OpenOptions};
use std::num::NonZero;
use std::os::fd::AsRawFd;
use std::sync::LazyLock;

pub static CROS_EC_FILE: LazyLock<Result<File, Box<dyn std::error::Error + Send + Sync>>> = LazyLock::new(|| {
    let ec = OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/cros_ec");
    if ec.is_ok() {
        println!("[INFO]: Got EC file handle.");
    }
    Ok(ec?)
});

#[allow(dead_code)]
pub enum EcCmd {
    /// Get protocol version, used to deal with non-backward compatible protocol changes.
    ProtoVersion = 0x0000,
    /// Hello. This is a simple command to test the EC is responsive to commands.
    Hello = 0x0001,
    /// Get version number.
    GetVersion = 0x0002,
    /// Read test.
    ReadTest = 0x0003,
    /// Get build information. Response is null-terminated string.
    GetBuildInfo = 0x0004,
    /// Get chip info.
    GetChipInfo = 0x0005,
    /// Get board HW version.
    GetBoardVersion = 0x0006,
    /// Read memory-mapped data. This is an alternate interface to memory-mapped data for bus protocols which don't support direct-mapped memory - I2C, SPI, etc. Response is params.size bytes of data.
    ReadMemmap = 0x0007,
    /// Read versions supported for a command.
    GetCmdVersions = 0x0008,
    /// Check EC communications status (busy). This is needed on i2c/spi but not on lpc since it has its own out-of-band busy indicator. lpc must read the status from the command register. Attempting this on lpc will overwrite the args/parameter space and corrupt its data.
    GetCommsStatus = 0x0009,
    /// Fake a variety of responses, purely for testing purposes.
    TestProtocol = 0x000A,
    /// Get protocol information.
    GetProtocolInfo = 0x000B,
    /// More than one command can use these structs to get/set parameters.
    GsvPauseInS5 = 0x000C,
    /// List the features supported by the firmware.
    GetFeatures = 0x000D,
    /// Get the board's SKU ID from EC.
    GetSkuId = 0x000E,
    /// Set SKU ID from AP.
    SetSkuId = 0x000F,
    /// Get flash info.
    FlashInfo = 0x0010,
    /// Read flash. Response is params.size bytes of data.
    FlashRead = 0x0011,
    /// Write flash.
    FlashWrite = 0x0012,
    /// Erase flash.
    FlashErase = 0x0013,
    /// Get/set flash protection. If mask!=0, sets/clear the requested bits of flags. Depending on the firmware write protect GPIO, not all flags will take effect immediately; some flags require a subsequent hard reset to take effect. Check the returned flags bits to see what actually happened. If mask=0, simply returns the current flags state.
    FlashProtect = 0x0015,
    /// Get the region offset/size.
    FlashRegionInfo = 0x0016,
    /// Read/write VbNvContext.
    VbnvContext = 0x0017,
    /// Get SPI flash information.
    FlashSpiInfo = 0x0018,
    /// Select flash during flash operations.
    FlashSelect = 0x0019,
    /// Request random numbers to be generated and returned. Can be used to test the random number generator is truly random.
    RandNum = 0x001A,
    /// Get information about the key used to sign the RW firmware.
    RwsigInfo = 0x001B,
    /// Get information about the system, such as reset flags, locked state, etc.
    Sysinfo = 0x001C,
    /// Get fan target RPM.
    PwmGetFanTargetRpm = 0x0020,
    /// Set target fan RPM.
    PwmSetFanTargetRpm = 0x0021,
    /// Get keyboard backlight. OBSOLETE - Use EC_CMD_PWM_SET_DUTY.
    PwmGetKeyboardBacklight = 0x0022,
    /// Set keyboard backlight. OBSOLETE - Use EC_CMD_PWM_SET_DUTY.
    PwmSetKeyboardBacklight = 0x0023,
    /// Set target fan PWM duty cycle.
    PwmSetFanDuty = 0x0024,
    PwmSetDuty = 0x0025,
    PwmGetDuty = 0x0026,
    /// Lightbar commands. Since we only use one HOST command to say "talk to the lightbar", we put the "and tell it to do X" part into a subcommand.
    LightbarCmd = 0x0028,
    /// LED control commands.
    LedControl = 0x0029,
    /// Verified boot hash command.
    VbootHash = 0x002A,
    /// Motion sense commands. We'll make separate structs for sub-commands with different input args, so that we know how much to expect.
    MotionSenseCmd = 0x002B,
    /// Make lid event always open.
    ForceLidOpen = 0x002C,
    /// Configure the behavior of the power button.
    ConfigPowerButton = 0x002D,
    /// Set USB port charging mode.
    UsbChargeSetMode = 0x0030,
    /// Get persistent storage info.
    PstoreInfo = 0x0040,
    /// Read persistent storage. Response is params.size bytes of data.
    PstoreRead = 0x0041,
    /// Write persistent storage.
    PstoreWrite = 0x0042,
    /// These use ec_response_rtc.
    RtcGetValue = 0x0044,
    /// These use ec_response_rtc.
    RtcGetAlarm = 0x0045,
    /// These all use ec_params_rtc.
    RtcSetValue = 0x0046,
    /// These all use ec_params_rtc.
    RtcSetAlarm = 0x0047,
    /// Get last port80 code from previous boot.
    Port80Read = 0x0048,
    /// Get persistent storage info.
    VstoreInfo = 0x0049,
    /// Read temporary secure storage. Response is EC_VSTORE_SLOT_SIZE bytes of data.
    VstoreRead = 0x004A,
    /// Write temporary secure storage and lock it.
    VstoreWrite = 0x004B,
    ThermalSetThreshold = 0x0050,
    ThermalGetThreshold = 0x0051,
    /// Toggle automatic fan control.
    ThermalAutoFanCtrl = 0x0052,
    /// Get/Set TMP006 calibration data.
    Tmp006GetCalibration = 0x0053,
    /// Get/Set TMP006 calibration data.
    Tmp006SetCalibration = 0x0054,
    /// Read raw TMP006 data.
    Tmp006GetRaw = 0x0055,
    /// Read key state. Returns raw data for keyboard cols; see ec_response_mkbp_info.cols for expected response size. NOTE: This has been superseded by EC_CMD_MKBP_GET_NEXT_EVENT.
    MkbpState = 0x0060,
    /// Provide information about various MKBP things. See enum ec_mkbp_info_type.
    MkbpInfo = 0x0061,
    /// Simulate key press.
    MkbpSimulateKey = 0x0062,
    GetKeyboardId = 0x0063,
    /// Configure keyboard scanning.
    MkbpSetConfig = 0x0064,
    /// Configure keyboard scanning.
    MkbpGetConfig = 0x0065,
    /// Run the key scan emulation.
    KeyscanSeqCtrl = 0x0066,
    /// Get the next pending MKBP event. Returns EC_RES_UNAVAILABLE if there is no event pending.
    GetNextEvent = 0x0067,
    /// Run keyboard factory test scanning.
    KeyboardFactoryTest = 0x0068,
    MkbpWakeMask = 0x0069,
    /// Read temperature sensor info.
    TempSensorGetInfo = 0x0070,
    /// ACPI Read Embedded Controller. This reads from ACPI memory space on the EC (EC_ACPI_MEM_*).
    AcpiRead = 0x0080,
    /// ACPI Write Embedded Controller. This reads from ACPI memory space on the EC (EC_ACPI_MEM_*).
    AcpiWrite = 0x0081,
    /// ACPI Burst Enable Embedded Controller. This enables burst mode on the EC to allow the host to issue several commands back-to-back. While in this mode, writes to mapped multi-byte data are locked out to ensure data consistency.
    AcpiBurstEnable = 0x0082,
    /// ACPI Burst Disable Embedded Controller. This disables burst mode on the EC and stops preventing EC writes to mapped multi-byte data.
    AcpiBurstDisable = 0x0083,
    /// ACPI Query Embedded Controller. This clears the lowest-order bit in the currently pending host events, and sets the result code to the 1-based index of the bit (event 0x00000001 = 1, event 0x80000000 = 32), or 0 if no event was pending.
    AcpiQueryEvent = 0x0084,
    /// These all use ec_response_host_event_mask.
    HostEventGetB = 0x0087,
    /// These all use ec_response_host_event_mask.
    HostEventGetSmiMask = 0x0088,
    /// These all use ec_response_host_event_mask.
    HostEventGetSciMask = 0x0089,
    /// These all use ec_params_host_event_mask.
    HostEventSetSmiMask = 0x008A,
    /// These all use ec_params_host_event_mask.
    HostEventSetSciMask = 0x008B,
    /// These all use ec_params_host_event_mask.
    HostEventClear = 0x008C,
    /// These all use ec_response_host_event_mask.
    HostEventGetWakeMask = 0x008D,
    /// These all use ec_params_host_event_mask.
    HostEventSetWakeMask = 0x008E,
    /// These all use ec_params_host_event_mask.
    HostEventClearB = 0x008F,
    /// Enable/disable LCD backlight.
    SwitchEnableBklight = 0x0090,
    /// Enable/disable WLAN/Bluetooth.
    SwitchEnableWireless = 0x0091,
    /// Set GPIO output value.
    GpioSet = 0x0092,
    /// Get GPIO value.
    GpioGet = 0x0093,
    /// Read I2C bus. CAUTION: Deprecated, not supported in EC builds >= 8398.0.0. Use EC_CMD_I2C_PASSTHRU instead.
    I2cRead = 0x0094,
    /// Write I2C bus. CAUTION: Deprecated, not supported in EC builds >= 8398.0.0. Use EC_CMD_I2C_PASSTHRU instead.
    I2cWrite = 0x0095,
    /// Force charge state machine to stop charging the battery or force it to discharge the battery.
    ChargeControl = 0x0096,
    /// Snapshot console output buffer for use by EC_CMD_CONSOLE_READ.
    ConsoleSnapshot = 0x0097,
    /// Read data from the saved snapshot. Response is null-terminated string. Empty string, if there is no more remaining output.
    ConsoleRead = 0x0098,
    /// Cut off battery power immediately or after the host has shut down.
    BatteryCutOff = 0x0099,
    /// Switch USB mux or return to automatic switching.
    UsbMux = 0x009A,
    /// Switch on/off a LDO.
    LdoSet = 0x009B,
    /// Get LDO state.
    LdoGet = 0x009C,
    /// Get power info. Note: v0 of this command is deprecated.
    PowerInfo = 0x009D,
    /// I2C passthru command.
    I2cPassthru = 0x009E,
    /// Power button hang detect.
    HangDetect = 0x009F,
    /// This is the single catch-all host command to exchange data regarding the charge state machine (v2 and up).
    ChargeState = 0x00A0,
    /// Set maximum battery charging current.
    ChargeCurrentLimit = 0x00A1,
    /// Set maximum external voltage / current.
    ExternalPowerLimit = 0x00A2,
    /// Set maximum voltage & current of a dedicated charge port.
    OverrideDedicatedChargerLimit = 0x00A3,
    /// Unified host event programming interface - Should be used by newer versions of BIOS/OS to program host events and masks.
    HostEvent = 0x00A4,
    /// Set the delay before going into hibernation.
    HibernationDelay = 0x00A8,
    /// Inform the EC when entering a sleep state.
    HostSleepEvent = 0x00A9,
    /// Device events.
    DeviceEvent = 0x00AA,
    /// Get / Set 16-bit smart battery registers.
    SbReadWord = 0x00B0,
    /// Get / Set 16-bit smart battery registers.
    SbWriteWord = 0x00B1,
    /// Get / Set string smart battery parameters formatted as SMBUS "block".
    SbReadBlock = 0x00B2,
    /// Get / Set string smart battery parameters formatted as SMBUS "block".
    SbWriteBlock = 0x00B3,
    /// Battery vendor parameters. Get or set vendor-specific parameters in the battery. On a set operation, the response contains the actual value set, which may be rounded or clipped from the requested value.
    BatteryVendorParam = 0x00B4,
    /// Smart Battery Firmware Update Commands.
    SbFwUpdate = 0x00B5,
    /// Entering Verified Boot Mode Command. Default mode is VBOOT_MODE_NORMAL if EC did not receive this command. Valid Modes are: normal, developer, and recovery. EC no longer needs to know what mode vboot has entered, so this command is deprecated.
    EnteringMode = 0x00B6,
    /// I2C passthru protection command: Protects I2C tunnels against access on certain addresses (board-specific).
    I2cPassthruProtect = 0x00B7,
    /// CEC message from the AP to be written on the CEC bus.
    CecWriteMsg = 0x00B8,
    /// Set various CEC parameters.
    CecSet = 0x00BA,
    /// Read various CEC parameters.
    CecGet = 0x00BB,
    /// Commands for audio codec.
    EcCodec = 0x00BC,
    /// Commands for DMIC on audio codec.
    EcCodecDmic = 0x00BD,
    /// Commands for I2S RX on audio codec.
    EcCodecI2sRx = 0x00BE,
    /// Commands for WoV on audio codec.
    EcCodecWov = 0x00BF,
    /// Commands for PoE PSE controller.
    Pse = 0x00C0,
    /// Reboot NOW. This command will work even when the EC LPC interface is busy, because the reboot command is processed at interrupt level. Note that when the EC reboots, the host will reboot too, so there is no response to this command. Use EC_CMD_REBOOT_EC to reboot the EC more politely.
    Reboot = 0x00D1,
    /// TODO(crosbug.com/p/23747): This is a confusing name, since it doesn't necessarily reboot the EC. Rename to "image" or something similar?
    RebootEc = 0x00D2,
    /// Get information on last EC panic. Returns variable-length platform-dependent panic information. See panic.h for details.
    GetPanicInfo = 0x00D3,
    /// Resend last response (not supported on LPC). Returns EC_RES_UNAVAILABLE if there is no response available.
    ResendResponse = 0x00DB,
    /// This header byte on a command indicates version 0. Any header byte less than this means that we are talking to an old EC which doesn't support versioning. In that case, we assume version 0.
    Version0 = 0x00DC,
    /// EC to PD MCU exchange status command.
    PdExchangeStatus = 0x0100,
    /// Set USB type-C port role and muxes. Deprecated in favor of TYPEC_STATUS and TYPEC_CONTROL commands.
    UsbPdControl = 0x0101,
    UsbPdPorts = 0x0102,
    UsbPdPowerInfo = 0x0103,
    /// AP to PD MCU host event status command, cleared on read.
    PdHostEventStatus = 0x0104,
    /// This command will return the number of USB PD charge port + the number of dedicated port present. EC_CMD_USB_PD_PORTS does NOT include the dedicated ports.
    ChargePortCount = 0x0105,
    /// Write USB-PD device FW.
    UsbPdFwUpdate = 0x0110,
    /// Write USB-PD Accessory RW_HASH table entry.
    UsbPdRwHashEntry = 0x0111,
    /// Read USB-PD Accessory info.
    UsbPdDevInfo = 0x0112,
    /// Read USB-PD Device discovery info.
    UsbPdDiscovery = 0x0113,
    /// Override default charge behavior.
    PdChargePortOverride = 0x0114,
    /// Read (and delete) one entry of PD event log.
    PdGetLogEntry = 0x0115,
    /// Get/Set USB-PD Alternate mode info.
    UsbPdGetAmode = 0x0116,
    UsbPdSetAmode = 0x0117,
    /// Ask the PD MCU to record a log of a requested type.
    PdWriteLogEntry = 0x0118,
    /// Control USB-PD chip.
    PdControl = 0x0119,
    /// Get info about USB-C SS muxes.
    UsbPdMuxInfo = 0x011A,
    PdChipInfo = 0x011B,
    /// Run RW signature verification and get status.
    RwsigCheckStatus = 0x011C,
    /// For controlling RWSIG task.
    RwsigAction = 0x011D,
    /// Run verification on a slot.
    EfsVerify = 0x011E,
    /// Retrieve info from Cros Board Info store. Response is based on the data type. Integers return a uint32. Strings return a string, using the response size to determine how big it is.
    GetCrosBoardInfo = 0x011F,
    /// Write info into Cros Board Info on EEPROM. Write fails if the board has hardware write-protect enabled.
    SetCrosBoardInfo = 0x0120,
    /// Information about resets of the AP by the EC and the EC's own uptime.
    GetUptimeInfo = 0x0121,
    /// Add entropy to the device secret (stored in the rollback region). Depending on the chip, the operation may take a long time (e.g. to erase flash), so the commands are asynchronous.
    AddEntropy = 0x0122,
    /// Perform a single read of a given ADC channel.
    AdcRead = 0x0123,
    /// Read back rollback info.
    RollbackInfo = 0x0124,
    /// Issue AP reset.
    ApReset = 0x0125,
    /// Locate peripheral chips.
    LocateChip = 0x0126,
    /// Reboot AP on G3. This command is used for validation purpose, where the AP needs to be returned back to S0 state from G3 state without using the servo to trigger wake events.
    RebootApOnG3 = 0x0127,
    /// Get PD port capabilities. Returns the following static capabilities of the given port: power role, try-power role, and data role.
    GetPdPortCaps = 0x0128,
    /// Button press simulation. This command is used to simulate a button press. NOTE: This is only available on unlocked devices for testing purposes only.
    Button = 0x0129,
    /// "Get the Keyboard Config". An EC implementing this command is expected to be vivaldi capable, i.e. can send action codes for the top row keys.
    GetKeybdConfig = 0x012A,
    /// Configure smart discharge.
    SmartDischarge = 0x012B,
    /// Get basic info of voltage regulator for given index. Returns the regulator name and supported voltage list in mV.
    RegulatorGetInfo = 0x012C,
    /// Configure the regulator as enabled / disabled.
    RegulatorEnable = 0x012D,
    /// Query if the regulator is enabled. Returns 1 if the regulator is enabled, 0 if not.
    RegulatorIsEnabled = 0x012E,
    /// Set voltage for the voltage regulator within the range specified. The driver should select the voltage in range closest to min_mv.
    RegulatorSetVoltage = 0x012F,
    /// Get the currently configured voltage for the voltage regulator. Note that this might be called before the regulator is enabled, and this should return the configured output voltage if the regulator is enabled.
    RegulatorGetVoltage = 0x0130,
    /// Gather all discovery information for the given port and partner type.
    TypecDiscovery = 0x0131,
    /// USB Type-C commands for AP-controlled device policy.
    TypecControl = 0x0132,
    /// Gather all status information for a port.
    TypecStatus = 0x0133,
    /// Reserve a range of host commands for the CR51 firmware.
    Cr51Base = 0x0300,
    /// Reserve a range of host commands for the CR51 firmware.
    Cr51Last = 0x03FF,
    /// Fingerprint SPI sensor passthru command: prototyping ONLY.
    FpPassthru = 0x0400,
    /// Configure the Fingerprint MCU behavior.
    FpMode = 0x0402,
    /// Retrieve Fingerprint sensor information.
    FpInfo = 0x0403,
    /// Get the last captured finger frame or a template content.
    FpFrame = 0x0404,
    /// Load a template into the MCU.
    FpTemplate = 0x0405,
    /// Clear the current fingerprint user context and set a new one.
    FpContext = 0x0406,
    FpStats = 0x0407,
    FpSeed = 0x0408,
    FpEncStatus = 0x0409,
    FpReadMatchSecret = 0x040A,
    /// Perform touchpad self test.
    TpSelfTest = 0x0500,
    /// Get number of frame types, and the size of each type.
    TpFrameInfo = 0x0501,
    /// Create a snapshot of current frame readings.
    TpFrameSnapshot = 0x0502,
    /// Read the frame.
    TpFrameGet = 0x0503,
    /// Get battery static information, i.e. information that never changes, or very infrequently.
    BatteryGetStatic = 0x0600,
    /// Get battery dynamic information, i.e. information that is likely to change every time it is read.
    BatteryGetDynamic = 0x0601,
    /// Control charger chip. Used to control charger chip on the slave.
    ChargerControl = 0x0602,
    /// Reserve a range of host commands for board-specific, experimental, or special purpose features.
    BoardSpecificBase = 0x3E00,
    /// Reserve a range of host commands for board-specific, experimental, or special purpose features.
    BoardSpecificLast = 0x3FFF,
}

#[repr(C)]
pub struct CrosEcCommandV2 {
    pub version: u32 = 0,
    pub command: u32,
    pub outsize: u32,
    pub insize: u32 = 0,
    pub result: u32 = 0,
    pub data: [u8; 0] = [],
}

#[repr(C)]
pub struct FullWriteV2Command<T> {
    pub header: CrosEcCommandV2,
    pub payload: T,
}

const EC_MEMMAP_SIZE: usize = 255;

#[repr(C)]
pub struct CrosEcReadmemV2 {
    pub offset: u32,
    pub bytes: u32,
    pub buffer: [u8; EC_MEMMAP_SIZE],
}

const CROS_EC_MAGIC: u8 = 0xEC;
const CROS_EC_DEV_IOCXCMD: c_int = 0;
const CROS_EC_DEV_IOCRDMEM_V2: c_int = 1;

ioctl_readwrite!(
    cros_ec_cmd,
    CROS_EC_MAGIC,
    CROS_EC_DEV_IOCXCMD,
    CrosEcCommandV2
);

ioctl_readwrite!(
    cros_ec_readmem,
    CROS_EC_MAGIC,
    CROS_EC_DEV_IOCRDMEM_V2,
    CrosEcReadmemV2
);

/// # Safety
///
/// The caller must ensure that `payload` is a valid pointer to a `CrosEcCommandV2` struct.
pub unsafe fn fire(payload: *mut CrosEcCommandV2) -> Result<Option<NonZero<c_int>>, Box<dyn std::error::Error + Send + Sync>> {
    let fd = CROS_EC_FILE.as_ref().map_err(|e| e.to_string())?.as_raw_fd();
    let result = unsafe { cros_ec_cmd(fd, payload) }?;
    if result < 0 {
        Err(Box::new(nix::Error::from_raw(result)))
    } else {
        Ok(NonZero::<c_int>::new(result))
    }
}
