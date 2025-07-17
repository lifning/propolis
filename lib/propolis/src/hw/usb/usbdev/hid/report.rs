// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use bitstruct::bitstruct;

use super::HIDReportType;

bitstruct! {
    #[derive(Clone, Copy, Debug, Default)]
    struct ItemPrefix(pub u8) {
        size: ItemSize = 0..2;
        tag: ItemTag = 2..8;
    }
}

#[repr(u8)]
#[derive(strum::FromRepr, Copy, Clone, Debug)]
enum ItemTag {
    // -- main --
    /// Data provided by one or more physical controls
    Input = 0b1000_00,
    /// Data sent to the device, such as LED states
    Output = 0b1001_00,
    /// Configuration information that can be sent to the device
    Feature = 0b1011_00,
    Collection = 0b1010_00,
    EndCollection = 0b1100_00,

    // -- global --
    /// "Unsigned integer specifying the current Usage Page. Since a usage are
    /// 32 bit values, Usage Page items can be used to conserve space in a
    /// report descriptor by setting the high order 16 bits of a subsequent
    /// usages. Any usage that follows which is defines 16 bits or less is
    /// interpreted as a Usage ID and concatenated with the Usage Page to form
    /// a 32 bit Usage." - HID 1.11 sect 6.2.2.7
    UsagePage = 0b0000_01,
    LogicalMinimum = 0b0001_01,
    LogicalMaximum = 0b0010_01,
    PhysicalMinimum = 0b0011_01,
    PhysicalMaximum = 0b0100_01,
    UnitExponent = 0b0101_01,
    Unit = 0b0110_01,

    /// Size of the report fields in bits
    ReportSize = 0b0111_01,
    ReportID = 0b1000_01,
    ReportCount = 0b1001_01,
    Push = 0b1010_01,
    Pop = 0b1011_01,

    // -- local --
    Usage = 0b0000_10,
    UsageMinimum = 0b0001_10,
    UsageMaximum = 0b0010_10,
    DesignatorIndex = 0b0011_10,
    DesignatorMinimum = 0b0100_10,
    DesignatorMaximum = 0b0101_10,
    StringIndex = 0b0111_10,
    StringMinimum = 0b1000_10,
    StringMaximum = 0b1001_10,
    Delimiter = 0b1010_10,

    // -- long --
    LongItem = 0b1111_11,
}

impl ItemTag {
    pub fn zero_byte(&self) -> ItemPrefix {
        ItemPrefix(0).with_size(ItemSize::_0).with_tag(*self)
    }
    pub fn one_byte(&self) -> ItemPrefix {
        ItemPrefix(0).with_size(ItemSize::_1).with_tag(*self)
    }
    pub fn two_byte(&self) -> ItemPrefix {
        ItemPrefix(0).with_size(ItemSize::_2).with_tag(*self)
    }
    pub fn four_byte(&self) -> ItemPrefix {
        ItemPrefix(0).with_size(ItemSize::_4).with_tag(*self)
    }
}

impl Into<u8> for ItemTag {
    fn into(self) -> u8 {
        self as u8
    }
}
impl From<u8> for ItemTag {
    fn from(value: u8) -> Self {
        Self::from_repr(value).expect(
            "ItemTag must only be convered from 6-bit field in ItemPrefix",
        )
    }
}

#[derive(Copy, Clone, Debug)]
enum ItemSize {
    _0,
    _1,
    _2, // 'long' items are considered this: bDataSize and bLongItemTag
    _4,
}
impl ItemSize {
    pub fn value(&self) -> usize {
        match self {
            ItemSize::_0 => 0,
            ItemSize::_1 => 1,
            ItemSize::_2 => 2,
            ItemSize::_4 => 4,
        }
    }
}
// representation for bitstruct
impl Into<u8> for ItemSize {
    fn into(self) -> u8 {
        match self {
            ItemSize::_0 => 0,
            ItemSize::_1 => 1,
            ItemSize::_2 => 2,
            ItemSize::_4 => 3, // gotcha!
        }
    }
}
impl From<u8> for ItemSize {
    fn from(value: u8) -> Self {
        match value {
            0 => ItemSize::_0,
            1 => ItemSize::_1,
            2 => ItemSize::_2,
            3 => ItemSize::_4,
            _ => panic!("ItemSize must only be convered from 2-bit field in ItemPrefix, got {value}")
        }
    }
}

bitstruct! {
    /// HID 1.11 sect 6.2.2.5
    // TODO: better names for these fields, or enums
    #[derive(Clone, Copy, Debug, Default)]
    struct InputOutputFeatureItem(pub u32) {
        /// 0 if Data
        /// 1 if Constant (read-only to host)
        constant: bool = 0;
        /// 0 if Array,
        /// 1 if Variable
        variable: bool = 1;
        /// 0 if Absolute (e.g. tablet),
        /// 1 if Relative to previous reports (e.g. mouse).
        relative: bool = 2;
        /// True iff data 'rolls over' when reaching the extremes of its range
        /// (e.g. a dial resetting to 0 at 360 degrees)
        wrap: bool = 3;
        /// True iff data is processed in some way and no longer represents a
        /// linear relationship to what was measured.
        non_linear: bool = 4;
        /// True iff the control has a resting state (e.g. non-toggle buttons,
        /// self-centering joysticks) to which it returns when the user is not
        /// interacting with it.
        no_preferred: bool = 5;
        /// True iff the control has a state in which it won't send meaningful
        /// data, such as controls that require the user to physically interact
        /// with the control. (A null state is reported by sending a value
        /// outside of the control's logical min..max range)
        null_state: bool = 6;
        /// For Input items, this bit is RESERVED.
        volatile: bool = 7;
        /// 0 if Bitfield,
        /// 1 if Buffered Bytes (the control emits a fixed-size stream of bytes).
        buffered_bytes: bool = 8;
        reserved: u32 = 9..32;
    }
}

/// HID Usage Tables 1.6
#[repr(u32)]
pub enum UsagePage {
    GenericDesktopControls = 0x01,
    SimulationControls = 0x02,
    VRControls = 0x03,
    SportControls = 0x04,
    GameControls = 0x05,
    GenericDeviceControls = 0x06,
    KeyCodes = 0x07,
    LED = 0x08,
    Button = 0x09,
    Ordinal = 0x0A,
    TelephonyDevice = 0x0B,
    Consumer = 0x0C,
    Digitizers = 0x0D,
    Haptics = 0x0E,
    PhysicalInput = 0x0F,
    Unicode = 0x10,
    SoC = 0x11,
    EyeAndHeadTrackers = 0x12,
    AuxiliaryDisplay = 0x14,
    Sensors = 0x20,
    MedicalInstrument = 0x40,
    BrailleDisplay = 0x41,
    LightingAndIllumination = 0x59,
    Monitor = 0x80,
    MonitorEnumerated = 0x81,
    Power = 0x84,
    BatterySystem = 0x85,
    BarcodeScanner = 0x8C,
    Scales = 0x8D,
    MagneticStripeReader = 0x8E,
    CameraControl = 0x90,
    Arcade = 0x91,
    GamingDevice = 0x92,
    /// Requires two-byte size in ItemPrefix!
    FIDOAlliance = 0xF1D0,
}

/// HID Usage Tables 1.6 sect 4
#[repr(u32)]
pub enum GenericDesktopUsage {
    Undefined = 0x00,
    Pointer = 0x01,
    Mouse = 0x02,
    Reserved = 0x03,
    Joystick = 0x04,
    Gamepad = 0x05,
    Keyboard = 0x06,
    Keypad = 0x07,
    MultiAxisController = 0x08,
    /// Note: Does *not* contain touchscreen data; rather, intended for
    /// buttons, wheels, and simple indicators.
    TabletPCSystemControls = 0x09,
    WaterCoolingDevice = 0x0A,
    ComputerChassisDevice = 0x0B,
    WirelessRadioControls = 0x0C,
    PortableDeviceControl = 0x0D,
    SystemMultiAxisController = 0x0E,
    SpatialController = 0x0F,
    AssistiveControl = 0x10,
    DeviceDock = 0x11,
    DockableDevice = 0x12,
    CallStateManagementControl = 0x13,
    // 0x14-0x2F reserved
    X = 0x30,
    Y = 0x31,
    Z = 0x32,
    Rx = 0x33,
    Ry = 0x34,
    Rz = 0x35,
    Slider = 0x36,
    Dial = 0x37,
    Wheel = 0x38,
    HatSwitch = 0x39,
    CountedBuffer = 0x3A,
    ByteCount = 0x3B,
    MotionWakeup = 0x3C,
    Start = 0x3D,
    Select = 0x3E,
    // [TODO more ...]
}

/// HID Usage Tables 1.6 sect 16
#[repr(u32)]
pub enum DigitizerUsage {
    Undefined = 0x00,
    Digitizer = 0x01,
    Pen = 0x02,
    LightPen = 0x03,
    TouchScreen = 0x04,
    TouchPad = 0x05,
    Whiteboard = 0x06,
    CoordinateMeasuringMachine = 0x07,
    ThreeDimensionalDigitizer = 0x08,
    StereoPlotter = 0x09,
    ArticulatedArm = 0x0A,
    Armature = 0x0B,
    MultiplePointDigitizer = 0x0C,
    FreeSpaceWand = 0x0D,
    DeviceConfiguration = 0x0E,
    CapacitiveHeatMapDigitizer = 0x0F,
    Stylus = 0x20,
    Puck = 0x21,
    Finger = 0x22,
    Devicesettings = 0x23,
    CharacterGesture = 0x24,
    TipPressure = 0x30,
    BarrelPressure = 0x31,
    InRange = 0x32,
    Touch = 0x33,
    Untouch = 0x34,
    Tap = 0x35,
    Quality = 0x36,
    DataValid = 0x37,
    TransducerIndex = 0x38,
    TabletFunctionKeys = 0x39,
    ProgramChangeKeys = 0x3A,
    BatteryStrength = 0x3B,
    Invert = 0x3C,
    XTilt = 0x3D,
    YTilt = 0x3E,
    Azimuth = 0x3F,
    Altitude = 0x40,
    // [TODO more...]
}

/// HID 1.11 sect 6.2.2.6
#[repr(u8)]
#[derive(strum::FromRepr, Copy, Clone, Debug)]
pub enum Collection {
    Physical = 0,    // group of axes
    Application = 1, // mouse, keyboard
    Logical = 2,     // interrelated data
    Report = 3,
    NamedArray = 4,
    UsageSwitch = 5,
    UsageModifier = 6,
}

#[derive(Debug)]
pub enum Part {
    // TODO: fancier than u32 for item value
    Item(ItemPrefix, u32),
    Collection(Collection, Vec<Part>),
}

impl Part {
    fn serialize(&self) -> Box<dyn Iterator<Item = u8> + '_> {
        match self {
            Part::Item(prefix, item) => {
                if let ItemTag::LongItem = prefix.tag() {
                    todo!()
                } else {
                    Box::new(
                        [prefix.0].into_iter().chain(
                            item.to_le_bytes()
                                .into_iter()
                                .take(prefix.size().value()),
                        ),
                    )
                }
            }
            Part::Collection(collection, parts) => Box::new(
                [ItemTag::Collection.one_byte().0, *collection as u8]
                    .into_iter()
                    .chain(parts.iter().flat_map(|part| part.serialize()))
                    .chain([ItemTag::EndCollection.zero_byte().0]),
            ),
        }
    }
}

#[derive(Debug)]
pub struct ReportDescriptor {
    pub report_type: HIDReportType,
    pub parts: Vec<Part>,
}

impl ReportDescriptor {
    pub fn length(&self) -> u16 {
        self.serialize().count() as u16
    }
    pub fn serialize(&self) -> impl Iterator<Item = u8> + '_ {
        self.parts.iter().flat_map(|part| part.serialize())
    }
}

// TODO: generate this
pub fn tablet_report_descriptor() -> ReportDescriptor {
    ReportDescriptor {
        report_type: HIDReportType::Input,
        parts: vec![
            Part::Item(
                ItemTag::UsagePage.one_byte(),
                UsagePage::GenericDesktopControls as u32,
            ),
            Part::Item(
                ItemTag::Usage.one_byte(),
                GenericDesktopUsage::Mouse as u32,
            ),
            Part::Collection(
                Collection::Application,
                vec![
                    Part::Item(ItemTag::ReportID.one_byte(), 1),
                    Part::Item(
                        ItemTag::Usage.one_byte(),
                        GenericDesktopUsage::Pointer as u32,
                    ),
                    Part::Collection(
                        Collection::Physical,
                        vec![
                            Part::Item(
                                ItemTag::UsagePage.one_byte(),
                                UsagePage::Button as u32,
                            ),
                            // VNC mouse button reports are in the form of a one-byte
                            // bitfield, with bit 0 representing 'disabled', so 1..=7
                            Part::Item(ItemTag::UsageMinimum.one_byte(), 1),
                            Part::Item(ItemTag::UsageMaximum.one_byte(), 7),
                            Part::Item(ItemTag::LogicalMinimum.one_byte(), 0),
                            Part::Item(ItemTag::LogicalMaximum.one_byte(), 1),
                            // 7 buttons to report...
                            Part::Item(ItemTag::ReportCount.one_byte(), 7),
                            // 1 bit each
                            Part::Item(ItemTag::ReportSize.one_byte(), 1),
                            Part::Item(
                                ItemTag::Input.one_byte(),
                                InputOutputFeatureItem(0)
                                    .with_constant(false) // data
                                    .with_variable(false) // array
                                    .with_relative(false) // absolute
                                    .with_buffered_bytes(false) // bitfield
                                    .0,
                            ),
                        ],
                    ),
                ],
            ),
            Part::Item(
                ItemTag::UsagePage.one_byte(),
                UsagePage::Digitizers as u32,
            ),
            Part::Item(
                ItemTag::Usage.one_byte(),
                DigitizerUsage::Digitizer as u32,
            ),
            Part::Collection(
                Collection::Application,
                vec![
                    // TODO report id 2, 3
                ],
            ),
        ],
    }
}
