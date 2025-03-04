//! Provides the main utilities to display USB types within this crate - primarily used by `cyme` binary.
//!
//! TODO: There is some repeat code that could probably be made into functions/generics
use clap::ValueEnum;
use colored::*;
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use std::cmp;
use std::collections::HashMap;
use rand::{distributions::Alphanumeric, seq::IteratorRandom, Rng};

use crate::colour;
use crate::icon;
use crate::system_profiler;
use crate::system_profiler::{USBBus, USBDevice};
use crate::usb::{ConfigAttributes, Direction, USBConfiguration, USBEndpoint, USBInterface};

const MAX_VERBOSITY: u8 = 4;
const ICON_HEADING: &'static str = "I";

/// Info that can be printed about a [`USBDevice`]
#[non_exhaustive]
#[derive(Debug, ValueEnum, Eq, PartialEq, Clone, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DeviceBlocks {
    /// Number of bus device is attached
    BusNumber,
    /// Bus issued device number
    DeviceNumber,
    /// Position of device in parent branch
    BranchPosition,
    /// Linux style port path
    PortPath,
    /// Linux udev reported syspath
    SysPath,
    /// Linux udev reported driver loaded for device
    Driver,
    /// Icon based on VID/PID
    Icon,
    /// Unique vendor identifier - purchased from USB IF
    VendorId,
    /// Vendor unique product identifier
    ProductId,
    /// The device name as reported in descriptor or using usb_ids if None
    Name,
    /// The device manufacturer as provided in descriptor or using usb_ids if None
    Manufacturer,
    /// The device product name as reported by usb_ids vidpid lookup
    ProductName,
    /// The device vendor name as reported by usb_ids vid lookup
    VendorName,
    /// Device serial string as reported by descriptor
    Serial,
    /// Advertised device capable speed
    Speed,
    /// Position along all branches back to trunk device
    TreePositions,
    /// macOS system_profiler only - actually bus current in mA not power!
    BusPower,
    /// macOS system_profiler only - actually bus current used in mA not power!
    BusPowerUsed,
    /// macOS system_profiler only - actually bus current used in mA not power!
    ExtraCurrentUsed,
    /// The device version
    BcdDevice,
    /// The supported USB version
    BcdUsb,
    /// Class of interface provided by USB IF - only available when using libusb
    ClassCode,
    /// Sub-class of interface provided by USB IF - only available when using libusb
    SubClass,
    /// Prototol code for interface provided by USB IF - only available when using libusb
    Protocol,
}

/// Info that can be printed about a [`USBBus`]
#[non_exhaustive]
#[derive(Debug, ValueEnum, Eq, PartialEq, Hash, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BusBlocks {
    /// System bus number identifier
    BusNumber,
    /// Icon based on VID/PID
    Icon,
    /// Bus name from descriptor or usb_ids
    Name,
    /// Host Controller on macOS, vendor put here when using libusb
    HostController,
    /// Understood to be vendor ID - it is when using libusb
    PciVendor,
    /// Understood to be product ID - it is when using libusb
    PciDevice,
    /// Revsision of hardware
    PciRevision,
    /// syspath style port path to bus, applicable to Linux only
    PortPath,
}

/// Info that can be printed about a [`USBConfiguration`]
#[non_exhaustive]
#[derive(Debug, ValueEnum, Eq, PartialEq, Hash, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ConfigurationBlocks {
    /// Name from string descriptor
    Name,
    /// Number of config, bConfigurationValue; value to set to enable to configuration
    Number,
    /// Interfaces available for this configuruation
    NumInterfaces,
    /// Attributes of configuration, bmAttributes
    Attributes,
    /// Icon representation of bmAttributes
    IconAttributes,
    /// Maximum current consumption in mA
    MaxPower,
}

/// Info that can be printed about a [`USBInterface`]
#[non_exhaustive]
#[derive(Debug, ValueEnum, Eq, PartialEq, Hash, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum InterfaceBlocks {
    /// Name from string descriptor
    Name,
    /// Interface number
    Number,
    /// Interface port path, applicable to Linux
    PortPath,
    /// Class of interface provided by USB IF
    ClassCode,
    /// Sub-class of interface provided by USB IF
    SubClass,
    /// Prototol code for interface provided by USB IF
    Protocol,
    /// Interfaces can have the same number but an alternate settings defined here
    AltSetting,
    /// Driver obtained from udev on Linux only
    Driver,
    /// syspath obtained from udev on Linux only
    SysPath,
    /// An interface can have many endpoints
    NumEndpoints,
    /// Icon based on ClassCode/SubCode/Protocol
    Icon,
}

/// Info that can be printed about a [`USBEndpoint`]
#[non_exhaustive]
#[derive(Debug, ValueEnum, Eq, PartialEq, Hash, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EndpointBlocks {
    /// Endpoint number on interface
    Number,
    /// Direction of data into endpoint
    Direction,
    /// Type of data transfer endpoint accepts
    TransferType,
    /// Synchronisation type (Iso mode)
    SyncType,
    /// Usage type (Iso mode)
    UsageType,
    /// Maximum packet size in bytes endpoint can send/recieve
    MaxPacketSize,
    /// Interval for polling endpoint data transfers. Value in frame counts. Ignored for Bulk & Control Endpoints. Isochronous must equal 1 and field may range from 1 to 255 for interrupt endpoints.
    Interval,
}

/// Intended to be `impl` by a xxxBlocks `enum`
pub trait Block<B, T> {
    /// List of default blocks to use for printing T with optional `verbose` for maximum verbosity
    fn default_blocks(verbose: bool) -> Vec<Self>
    where
        Self: Sized;

    /// Creates a HashMap of B keys to usize of longest value for that key in the `d` Vec; values can then be padded to match this
    fn generate_padding(d: &Vec<&T>) -> HashMap<B, usize>;

    /// Colour the block String
    fn colour(&self, s: &String, ct: &colour::ColourTheme) -> ColoredString;

    /// Creates the heading for the block value, for use with the heading flag
    fn heading(&self, pad: &HashMap<B, usize>) -> String;

    /// Returns whether the value intended for the block is a String type
    fn value_is_string(&self) -> bool;

    /// Formats the value associated with the block into a display String
    fn format_value(
        &self,
        d: &T,
        pad: &HashMap<B, usize>,
        settings: &PrintSettings,
    ) -> Option<String>;

    /// Formats u16 values like VID as base16 or base10 depending on decimal setting
    fn format_base_u16(v: u16, settings: &PrintSettings) -> String {
        if settings.decimal {
            format!("{:6}", v)
        } else {
            format!("0x{:04x}", v)
        }
    }

    /// Formats u8 values like codes as base16 or base10 depending on decimal setting
    fn format_base_u8(v: u8, settings: &PrintSettings) -> String {
        if settings.decimal {
            format!("{:3}", v)
        } else {
            format!("0x{:02x}", v)
        }
    }
}

impl DeviceBlocks {
    /// Default `DeviceBlocks` for tree printing are different to list, get them here
    pub fn default_device_tree_blocks() -> Vec<DeviceBlocks> {
        vec![
            DeviceBlocks::Icon,
            DeviceBlocks::DeviceNumber,
            DeviceBlocks::VendorId,
            DeviceBlocks::ProductId,
            DeviceBlocks::Name,
            DeviceBlocks::Serial,
        ]
    }
}

impl Block<DeviceBlocks, USBDevice> for DeviceBlocks {
    fn default_blocks(verbose: bool) -> Vec<DeviceBlocks> {
        if verbose {
            vec![
                DeviceBlocks::BusNumber,
                DeviceBlocks::DeviceNumber,
                DeviceBlocks::TreePositions,
                DeviceBlocks::PortPath,
                DeviceBlocks::Icon,
                DeviceBlocks::VendorId,
                DeviceBlocks::ProductId,
                DeviceBlocks::BcdDevice,
                DeviceBlocks::BcdUsb,
                DeviceBlocks::ClassCode,
                DeviceBlocks::SubClass,
                DeviceBlocks::Protocol,
                DeviceBlocks::Name,
                DeviceBlocks::Manufacturer,
                DeviceBlocks::Serial,
                DeviceBlocks::Driver,
                DeviceBlocks::Speed,
            ]
        } else {
            vec![
                DeviceBlocks::BusNumber,
                DeviceBlocks::DeviceNumber,
                DeviceBlocks::Icon,
                DeviceBlocks::VendorId,
                DeviceBlocks::ProductId,
                DeviceBlocks::Name,
                DeviceBlocks::Serial,
                DeviceBlocks::Speed,
            ]
        }
    }

    fn generate_padding(d: &Vec<&system_profiler::USBDevice>) -> HashMap<Self, usize> {
        HashMap::from([
            (
                DeviceBlocks::Name,
                cmp::max(
                    DeviceBlocks::Name.heading(&Default::default()).len(),
                    d.iter().map(|d| d.name.len()).max().unwrap_or(0),
                ),
            ),
            (
                DeviceBlocks::Serial,
                cmp::max(
                    DeviceBlocks::Serial.heading(&Default::default()).len(),
                    d.iter()
                        .map(|d| d.serial_num.as_ref().unwrap_or(&String::new()).len())
                        .max()
                        .unwrap_or(0),
                ),
            ),
            (
                DeviceBlocks::Manufacturer,
                cmp::max(
                    DeviceBlocks::Manufacturer
                        .heading(&Default::default())
                        .len(),
                    d.iter()
                        .map(|d| d.manufacturer.as_ref().unwrap_or(&String::new()).len())
                        .max()
                        .unwrap_or(0),
                ),
            ),
            (
                DeviceBlocks::TreePositions,
                cmp::max(
                    DeviceBlocks::TreePositions
                        .heading(&Default::default())
                        .len(),
                    d.iter()
                        .map(|d| d.location_id.tree_positions.len() * 2)
                        .max()
                        .unwrap_or(0),
                ),
            ),
            (
                DeviceBlocks::PortPath,
                cmp::max(
                    DeviceBlocks::PortPath.heading(&Default::default()).len(),
                    d.iter().map(|d| d.port_path().len()).max().unwrap_or(0),
                ),
            ),
            (
                DeviceBlocks::SysPath,
                cmp::max(
                    DeviceBlocks::SysPath.heading(&Default::default()).len(),
                    d.iter()
                        .map(|d| {
                            d.extra
                                .as_ref()
                                .map_or(0, |e| e.syspath.as_ref().unwrap_or(&String::new()).len())
                        })
                        .max()
                        .unwrap_or(0),
                ),
            ),
            (
                DeviceBlocks::Driver,
                cmp::max(
                    DeviceBlocks::Driver.heading(&Default::default()).len(),
                    d.iter()
                        .map(|d| {
                            d.extra
                                .as_ref()
                                .map_or(0, |e| e.driver.as_ref().unwrap_or(&String::new()).len())
                        })
                        .max()
                        .unwrap_or(0),
                ),
            ),
            (
                DeviceBlocks::ProductName,
                cmp::max(
                    DeviceBlocks::ProductName.heading(&Default::default()).len(),
                    d.iter()
                        .map(|d| {
                            d.extra.as_ref().map_or(0, |e| {
                                e.product_name.as_ref().unwrap_or(&String::new()).len()
                            })
                        })
                        .max()
                        .unwrap_or(0),
                ),
            ),
            (
                DeviceBlocks::VendorName,
                cmp::max(
                    DeviceBlocks::VendorName.heading(&Default::default()).len(),
                    d.iter()
                        .map(|d| {
                            d.extra
                                .as_ref()
                                .map_or(0, |e| e.vendor.as_ref().unwrap_or(&String::new()).len())
                        })
                        .max()
                        .unwrap_or(0),
                ),
            ),
            (
                DeviceBlocks::ClassCode,
                cmp::max(
                    DeviceBlocks::ClassCode.heading(&Default::default()).len(),
                    d.iter()
                        .map(|d| {
                            d.class
                                .as_ref()
                                .map_or(String::new(), |c| c.to_string())
                                .len()
                        })
                        .max()
                        .unwrap_or(0),
                ),
            ),
        ])
    }

    fn value_is_string(&self) -> bool {
        match self {
            DeviceBlocks::Name
            | DeviceBlocks::Serial
            | DeviceBlocks::PortPath
            | DeviceBlocks::Manufacturer => true,
            _ => false,
        }
    }

    fn format_value(
        &self,
        d: &USBDevice,
        pad: &HashMap<Self, usize>,
        settings: &PrintSettings,
    ) -> Option<String> {
        match self {
            DeviceBlocks::BusNumber => Some(format!("{:3}", d.location_id.bus)),
            DeviceBlocks::DeviceNumber => Some(format!("{:3}", d.location_id.number)),
            DeviceBlocks::BranchPosition => Some(format!("{:3}", d.get_branch_position())),
            DeviceBlocks::PortPath => Some(format!(
                "{:pad$}",
                d.port_path(),
                pad = pad.get(self).unwrap_or(&0)
            )),
            DeviceBlocks::SysPath => Some(match d.extra.as_ref() {
                Some(e) => format!(
                    "{:pad$}",
                    e.syspath.as_ref().unwrap_or(&format!(
                        "{:pad$}",
                        "-",
                        pad = pad.get(self).unwrap_or(&0)
                    )),
                    pad = pad.get(self).unwrap_or(&0)
                ),
                None => format!("{:pad$}", "-", pad = pad.get(self).unwrap_or(&0)),
            }),
            DeviceBlocks::Driver => Some(match d.extra.as_ref() {
                Some(e) => format!(
                    "{:pad$}",
                    e.driver.as_ref().unwrap_or(&format!(
                        "{:pad$}",
                        "-",
                        pad = pad.get(self).unwrap_or(&0)
                    )),
                    pad = pad.get(self).unwrap_or(&0)
                ),
                None => format!("{:pad$}", "-", pad = pad.get(self).unwrap_or(&0)),
            }),
            DeviceBlocks::ProductName => Some(match d.extra.as_ref() {
                Some(e) => format!(
                    "{:pad$}",
                    e.product_name.as_ref().unwrap_or(&format!(
                        "{:pad$}",
                        "-",
                        pad = pad.get(self).unwrap_or(&0)
                    )),
                    pad = pad.get(self).unwrap_or(&0)
                ),
                None => format!("{:pad$}", "-", pad = pad.get(self).unwrap_or(&0)),
            }),
            DeviceBlocks::VendorName => Some(match d.extra.as_ref() {
                Some(e) => format!(
                    "{:pad$}",
                    e.vendor.as_ref().unwrap_or(&format!(
                        "{:pad$}",
                        "-",
                        pad = pad.get(self).unwrap_or(&0)
                    )),
                    pad = pad.get(self).unwrap_or(&0)
                ),
                None => format!("{:pad$}", "-", pad = pad.get(self).unwrap_or(&0)),
            }),
            DeviceBlocks::Icon => settings
                .icons
                .as_ref()
                .map_or(None, |i| Some(i.get_device_icon(d))),
            DeviceBlocks::VendorId => Some(match d.vendor_id {
                Some(v) => Self::format_base_u16(v, settings),
                None => format!("{:>6}", "-"),
            }),
            DeviceBlocks::ProductId => Some(match d.product_id {
                Some(v) => Self::format_base_u16(v, settings),
                None => format!("{:>6}", "-"),
            }),
            DeviceBlocks::Name => Some(format!(
                "{:pad$}",
                d.name,
                pad = pad.get(self).unwrap_or(&0)
            )),
            DeviceBlocks::Manufacturer => Some(match d.manufacturer.as_ref() {
                Some(v) => format!("{:pad$}", v, pad = pad.get(self).unwrap_or(&0)),
                None => format!("{:pad$}", "-", pad = pad.get(self).unwrap_or(&0)),
            }),
            DeviceBlocks::Serial => Some(match d.serial_num.as_ref() {
                Some(v) => format!("{:pad$}", v, pad = pad.get(self).unwrap_or(&0)),
                None => format!("{:pad$}", "-", pad = pad.get(self).unwrap_or(&0)),
            }),
            DeviceBlocks::Speed => Some(match d.device_speed.as_ref() {
                Some(v) => format!("{:>10}", v.to_string()),
                None => format!("{:>10}", "-"),
            }),
            DeviceBlocks::TreePositions => Some(format!(
                "{:pad$}",
                format!("{:}", d.location_id.tree_positions.iter().format("-")),
                pad = pad.get(self).unwrap_or(&0)
            )),
            DeviceBlocks::BusPower => Some(match d.bus_power {
                Some(v) => format!("{:3} mA", v),
                None => format!("{:>6}", "-"),
            }),
            DeviceBlocks::BusPowerUsed => Some(match d.bus_power_used {
                Some(v) => format!("{:3} mA", v),
                None => format!("{:>6}", "-"),
            }),
            DeviceBlocks::ExtraCurrentUsed => Some(match d.extra_current_used {
                Some(v) => format!("{:3} mA", v),
                None => format!("{:>6}", "-"),
            }),
            DeviceBlocks::BcdDevice => Some(match d.bcd_device {
                Some(v) => format!("{:5}", v.to_string()),
                None => format!("{:>5}", "-"),
            }),
            DeviceBlocks::BcdUsb => Some(match d.bcd_usb {
                Some(v) => format!("{:5}", v.to_string()),
                None => format!("{:>5}", "-"),
            }),
            DeviceBlocks::ClassCode => Some(match d.class.as_ref() {
                Some(v) => format!("{:pad$}", v.to_string(), pad = pad.get(self).unwrap_or(&0)),
                None => format!("{:pad$}", "-", pad = pad.get(self).unwrap_or(&0)),
            }),
            DeviceBlocks::SubClass => Some(match d.sub_class.as_ref() {
                Some(v) => Self::format_base_u8(*v, settings),
                None => format!("{:>4}", "-"),
            }),
            DeviceBlocks::Protocol => Some(match d.protocol.as_ref() {
                Some(v) => Self::format_base_u8(*v, settings),
                None => format!("{:>4}", "-"),
            }),
            // _ => None,
        }
    }

    fn colour(&self, s: &String, ct: &colour::ColourTheme) -> ColoredString {
        match self {
            DeviceBlocks::BcdUsb | DeviceBlocks::BcdDevice | DeviceBlocks::DeviceNumber => {
                ct.number.map_or(s.normal(), |c| s.color(c))
            }
            DeviceBlocks::BusNumber
            | DeviceBlocks::BranchPosition
            | DeviceBlocks::TreePositions => ct.location.map_or(s.normal(), |c| s.color(c)),
            DeviceBlocks::Icon => ct.icon.map_or(s.normal(), |c| s.color(c)),
            DeviceBlocks::PortPath | DeviceBlocks::SysPath => {
                ct.path.map_or(s.normal(), |c| s.color(c))
            }
            DeviceBlocks::VendorId => ct.vid.map_or(s.normal(), |c| s.color(c)),
            DeviceBlocks::ProductId => ct.pid.map_or(s.normal(), |c| s.color(c)),
            DeviceBlocks::Name | DeviceBlocks::ProductName => {
                ct.name.map_or(s.normal(), |c| s.color(c))
            }
            DeviceBlocks::Serial => ct.serial.map_or(s.normal(), |c| s.color(c)),
            DeviceBlocks::Manufacturer | DeviceBlocks::VendorName => {
                ct.manufacturer.map_or(s.normal(), |c| s.color(c))
            }
            DeviceBlocks::Driver => ct.driver.map_or(s.normal(), |c| s.color(c)),
            DeviceBlocks::Speed => ct.speed.map_or(s.normal(), |c| s.color(c)),
            DeviceBlocks::BusPower
            | DeviceBlocks::BusPowerUsed
            | DeviceBlocks::ExtraCurrentUsed => ct.power.map_or(s.normal(), |c| s.color(c)),
            DeviceBlocks::ClassCode => ct.class_code.map_or(s.normal(), |c| s.color(c)),
            DeviceBlocks::SubClass => ct.sub_code.map_or(s.normal(), |c| s.color(c)),
            DeviceBlocks::Protocol => ct.protocol.map_or(s.normal(), |c| s.color(c)),
            // _ => s.normal(),
        }
    }

    fn heading(&self, pad: &HashMap<Self, usize>) -> String {
        match self {
            DeviceBlocks::BusNumber => "Bus".into(),
            DeviceBlocks::DeviceNumber => " # ".into(),
            DeviceBlocks::BranchPosition => "Prt".into(),
            DeviceBlocks::PortPath => {
                format!("{:^pad$}", "PPath", pad = pad.get(self).unwrap_or(&0))
            }
            DeviceBlocks::SysPath => {
                format!("{:^pad$}", "SPath", pad = pad.get(self).unwrap_or(&0))
            }
            DeviceBlocks::Driver => {
                format!("{:^pad$}", "Driver", pad = pad.get(self).unwrap_or(&0))
            }
            DeviceBlocks::VendorId => format!("{:^6}", "VID"),
            DeviceBlocks::ProductId => format!("{:^6}", "PID"),
            DeviceBlocks::Name => format!("{:^pad$}", "Name", pad = pad.get(self).unwrap_or(&0)),
            DeviceBlocks::Manufacturer => {
                format!(
                    "{:^pad$}",
                    "Manufacturer",
                    pad = pad.get(self).unwrap_or(&0)
                )
            }
            DeviceBlocks::ProductName => {
                format!("{:^pad$}", "PName", pad = pad.get(self).unwrap_or(&0))
            }
            DeviceBlocks::VendorName => {
                format!("{:^pad$}", "VName", pad = pad.get(self).unwrap_or(&0))
            }
            DeviceBlocks::Serial => {
                format!("{:^pad$}", "Serial", pad = pad.get(self).unwrap_or(&0))
            }
            DeviceBlocks::Speed => format!("{:^10}", "Speed"),
            DeviceBlocks::TreePositions => {
                format!("{:^pad$}", "TPos", pad = pad.get(self).unwrap_or(&0))
            }
            // will be 000 mA = 6
            DeviceBlocks::BusPower => "PBus".into(),
            DeviceBlocks::BusPowerUsed => "PUsd".into(),
            DeviceBlocks::ExtraCurrentUsed => "PExr".into(),
            // 00.00 = 5
            DeviceBlocks::BcdDevice => "Dev V".into(),
            DeviceBlocks::BcdUsb => "USB V".into(),
            DeviceBlocks::ClassCode => {
                format!("{:^pad$}", "Class", pad = pad.get(self).unwrap_or(&0))
            }
            DeviceBlocks::SubClass => "SubC".into(),
            DeviceBlocks::Protocol => "Pcol".into(),
            DeviceBlocks::Icon => ICON_HEADING.into(),
            // _ => "",
        }
    }
}

impl Block<BusBlocks, USBBus> for BusBlocks {
    fn default_blocks(verbose: bool) -> Vec<BusBlocks> {
        if verbose {
            vec![
                BusBlocks::Icon,
                BusBlocks::PortPath,
                BusBlocks::Name,
                BusBlocks::HostController,
                BusBlocks::PciVendor,
                BusBlocks::PciDevice,
                BusBlocks::PciRevision,
            ]
        } else {
            vec![BusBlocks::Name, BusBlocks::HostController]
        }
    }

    fn generate_padding(d: &Vec<&system_profiler::USBBus>) -> HashMap<Self, usize> {
        HashMap::from([
            (
                BusBlocks::Name,
                cmp::max(
                    BusBlocks::Name.heading(&Default::default()).len(),
                    d.iter().map(|d| d.name.len()).max().unwrap_or(0),
                ),
            ),
            (
                BusBlocks::HostController,
                cmp::max(
                    BusBlocks::HostController.heading(&Default::default()).len(),
                    d.iter().map(|d| d.host_controller.len()).max().unwrap_or(0),
                ),
            ),
            (
                BusBlocks::PortPath,
                cmp::max(
                    BusBlocks::PortPath.heading(&Default::default()).len(),
                    d.iter().map(|d| d.path().len()).max().unwrap_or(0),
                ),
            ),
        ])
    }

    fn value_is_string(&self) -> bool {
        match self {
            BusBlocks::Name | BusBlocks::HostController => true,
            _ => false,
        }
    }

    fn colour(&self, s: &String, ct: &colour::ColourTheme) -> ColoredString {
        match self {
            BusBlocks::BusNumber => ct.location.map_or(s.normal(), |c| s.color(c)),
            BusBlocks::PciVendor => ct.vid.map_or(s.normal(), |c| s.color(c)),
            BusBlocks::PciDevice => ct.pid.map_or(s.normal(), |c| s.color(c)),
            BusBlocks::Name => ct.name.map_or(s.normal(), |c| s.color(c)),
            BusBlocks::HostController => ct.serial.map_or(s.normal(), |c| s.color(c)),
            BusBlocks::PciRevision => ct.number.map_or(s.normal(), |c| s.color(c)),
            BusBlocks::Icon => ct.icon.map_or(s.normal(), |c| s.color(c)),
            BusBlocks::PortPath => ct.path.map_or(s.normal(), |c| s.color(c)),
            // _ => s.normal(),
        }
    }

    fn format_value(
        &self,
        bus: &system_profiler::USBBus,
        pad: &HashMap<Self, usize>,
        settings: &PrintSettings,
    ) -> Option<String> {
        match self {
            BusBlocks::BusNumber => Some(format!("{:3}", bus.get_bus_number())),
            BusBlocks::Icon => settings
                .icons
                .as_ref()
                .map_or(None, |i| Some(i.get_bus_icon(bus))),
            BusBlocks::PciVendor => Some(match bus.pci_vendor {
                Some(v) => Self::format_base_u16(v, settings),
                None => format!("{:>6}", "-"),
            }),
            BusBlocks::PciDevice => Some(match bus.pci_device {
                Some(v) => Self::format_base_u16(v, settings),
                None => format!("{:>6}", "-"),
            }),
            BusBlocks::PciRevision => Some(match bus.pci_revision {
                Some(v) => Self::format_base_u16(v, settings),
                None => format!("{:>6}", "-"),
            }),
            BusBlocks::Name => Some(format!(
                "{:pad$}",
                bus.name,
                pad = pad.get(self).unwrap_or(&0)
            )),
            BusBlocks::HostController => Some(format!(
                "{:pad$}",
                bus.host_controller,
                pad = pad.get(self).unwrap_or(&0)
            )),
            BusBlocks::PortPath => Some(format!(
                "{:pad$}",
                bus.path(),
                pad = pad.get(self).unwrap_or(&0)
            )),
            // _ => None,
        }
    }

    fn heading(&self, pad: &HashMap<Self, usize>) -> String {
        match self {
            BusBlocks::BusNumber => "Bus".into(),
            BusBlocks::PortPath => "PortPath".into(),
            BusBlocks::PciDevice => " PID ".into(),
            BusBlocks::PciVendor => " VID ".into(),
            BusBlocks::PciRevision => " Rev ".into(),
            BusBlocks::Name => format!("{:^pad$}", "Name", pad = pad.get(self).unwrap_or(&0)),
            BusBlocks::HostController => {
                format!(
                    "{:^pad$}",
                    "Host Controller",
                    pad = pad.get(self).unwrap_or(&0)
                )
            }
            BusBlocks::Icon => ICON_HEADING.into(),
            // _ => "",
        }
    }
}

impl Block<ConfigurationBlocks, USBConfiguration> for ConfigurationBlocks {
    fn default_blocks(verbose: bool) -> Vec<ConfigurationBlocks> {
        if verbose {
            vec![
                ConfigurationBlocks::Number,
                ConfigurationBlocks::IconAttributes,
                ConfigurationBlocks::Attributes,
                ConfigurationBlocks::NumInterfaces,
                ConfigurationBlocks::MaxPower,
                ConfigurationBlocks::Name,
            ]
        } else {
            vec![
                ConfigurationBlocks::Number,
                ConfigurationBlocks::IconAttributes,
                ConfigurationBlocks::MaxPower,
                ConfigurationBlocks::Name,
            ]
        }
    }

    fn generate_padding(d: &Vec<&USBConfiguration>) -> HashMap<Self, usize> {
        HashMap::from([
            (
                ConfigurationBlocks::Name,
                cmp::max(
                    ConfigurationBlocks::Name.heading(&Default::default()).len(),
                    d.iter().map(|d| d.name.len()).max().unwrap_or(0),
                ),
            ),
            (
                ConfigurationBlocks::Attributes,
                cmp::max(
                    ConfigurationBlocks::Attributes
                        .heading(&Default::default())
                        .len(),
                    d.iter()
                        .map(|d| d.attributes_string().len())
                        .max()
                        .unwrap_or(0),
                ),
            ),
        ])
    }

    fn value_is_string(&self) -> bool {
        match self {
            ConfigurationBlocks::Name | ConfigurationBlocks::Attributes => true,
            _ => false,
        }
    }

    fn colour(&self, s: &String, ct: &colour::ColourTheme) -> ColoredString {
        match self {
            ConfigurationBlocks::Number => ct.location.map_or(s.normal(), |c| s.color(c)),
            ConfigurationBlocks::NumInterfaces => ct.number.map_or(s.normal(), |c| s.color(c)),
            ConfigurationBlocks::MaxPower => ct.power.map_or(s.normal(), |c| s.color(c)),
            ConfigurationBlocks::Name => ct.name.map_or(s.normal(), |c| s.color(c)),
            ConfigurationBlocks::Attributes => ct.attributes.map_or(s.normal(), |c| s.color(c)),
            ConfigurationBlocks::IconAttributes => ct.icon.map_or(s.normal(), |c| s.color(c)),
            // _ => s.normal(),
        }
    }

    fn format_value(
        &self,
        config: &USBConfiguration,
        pad: &HashMap<Self, usize>,
        settings: &PrintSettings,
    ) -> Option<String> {
        match self {
            ConfigurationBlocks::Number => Some(format!("{:2}", config.number)),
            ConfigurationBlocks::NumInterfaces => Some(format!("{:2}", config.interfaces.len())),
            ConfigurationBlocks::Name => Some(format!(
                "{:pad$}",
                config.name,
                pad = pad.get(self).unwrap_or(&0)
            )),
            ConfigurationBlocks::MaxPower => Some(format!("{:3}", config.max_power)),
            ConfigurationBlocks::Attributes => Some(format!(
                "{:pad$}",
                config.attributes_string(),
                pad = pad.get(self).unwrap_or(&0)
            )),
            ConfigurationBlocks::IconAttributes => Some(format!(
                "{:pad$}",
                attributes_to_icons(&config.attributes, settings),
                pad = pad.get(self).unwrap_or(&3)
            )),
            // _ => None,
        }
    }

    fn heading(&self, pad: &HashMap<Self, usize>) -> String {
        match self {
            ConfigurationBlocks::Number => " #".into(),
            ConfigurationBlocks::NumInterfaces => "I#".into(),
            ConfigurationBlocks::MaxPower => "PMax".into(),
            ConfigurationBlocks::Name => {
                format!("{:^pad$}", "Name", pad = pad.get(self).unwrap_or(&0))
            }
            ConfigurationBlocks::Attributes => {
                format!("{:^pad$}", "Attributes", pad = pad.get(self).unwrap_or(&0))
            }
            ConfigurationBlocks::IconAttributes => {
                format!("{:^pad$}", ICON_HEADING, pad = pad.get(self).unwrap_or(&3))
            } // getting len of utf-8 icons is not pretty so resort to fixed 3
              // _ => "",
        }
    }
}

impl Block<InterfaceBlocks, USBInterface> for InterfaceBlocks {
    fn default_blocks(verbose: bool) -> Vec<InterfaceBlocks> {
        if verbose {
            vec![
                InterfaceBlocks::PortPath,
                InterfaceBlocks::Icon,
                InterfaceBlocks::AltSetting,
                InterfaceBlocks::ClassCode,
                InterfaceBlocks::SubClass,
                InterfaceBlocks::Protocol,
                InterfaceBlocks::Name,
                InterfaceBlocks::Driver,
                InterfaceBlocks::NumEndpoints,
            ]
        } else {
            vec![
                InterfaceBlocks::PortPath,
                InterfaceBlocks::Icon,
                InterfaceBlocks::AltSetting,
                InterfaceBlocks::ClassCode,
                InterfaceBlocks::SubClass,
                InterfaceBlocks::Protocol,
                InterfaceBlocks::Name,
            ]
        }
    }

    fn generate_padding(d: &Vec<&USBInterface>) -> HashMap<Self, usize> {
        HashMap::from([
            (
                InterfaceBlocks::Name,
                cmp::max(
                    InterfaceBlocks::Name.heading(&Default::default()).len(),
                    d.iter().map(|d| d.name.len()).max().unwrap_or(0),
                ),
            ),
            (
                InterfaceBlocks::ClassCode,
                cmp::max(
                    InterfaceBlocks::ClassCode
                        .heading(&Default::default())
                        .len(),
                    d.iter()
                        .map(|d| d.class.to_string().len())
                        .max()
                        .unwrap_or(0),
                ),
            ),
            (
                InterfaceBlocks::PortPath,
                cmp::max(
                    InterfaceBlocks::PortPath.heading(&Default::default()).len(),
                    d.iter().map(|d| d.path.len()).max().unwrap_or(0),
                ),
            ),
            (
                InterfaceBlocks::SysPath,
                cmp::max(
                    InterfaceBlocks::SysPath.heading(&Default::default()).len(),
                    d.iter()
                        .map(|d| d.syspath.as_ref().unwrap_or(&String::new()).len())
                        .max()
                        .unwrap_or(0),
                ),
            ),
            (
                InterfaceBlocks::Driver,
                cmp::max(
                    InterfaceBlocks::Driver.heading(&Default::default()).len(),
                    d.iter()
                        .map(|d| d.driver.as_ref().unwrap_or(&String::new()).len())
                        .max()
                        .unwrap_or(0),
                ),
            ),
        ])
    }

    fn value_is_string(&self) -> bool {
        match self {
            InterfaceBlocks::Name
            | InterfaceBlocks::PortPath
            | InterfaceBlocks::ClassCode
            | InterfaceBlocks::Driver
            | InterfaceBlocks::SysPath => true,
            _ => false,
        }
    }

    fn colour(&self, s: &String, ct: &colour::ColourTheme) -> ColoredString {
        match self {
            InterfaceBlocks::Number => ct.number.map_or(s.normal(), |c| s.color(c)),
            InterfaceBlocks::Name => ct.name.map_or(s.normal(), |c| s.color(c)),
            InterfaceBlocks::PortPath | InterfaceBlocks::SysPath => {
                ct.path.map_or(s.normal(), |c| s.color(c))
            }
            InterfaceBlocks::Icon => ct.icon.map_or(s.normal(), |c| s.color(c)),
            InterfaceBlocks::ClassCode => ct.class_code.map_or(s.normal(), |c| s.color(c)),
            InterfaceBlocks::SubClass => ct.sub_code.map_or(s.normal(), |c| s.color(c)),
            InterfaceBlocks::Protocol => ct.protocol.map_or(s.normal(), |c| s.color(c)),
            InterfaceBlocks::Driver => ct.driver.map_or(s.normal(), |c| s.color(c)),
            InterfaceBlocks::AltSetting | InterfaceBlocks::NumEndpoints => {
                ct.number.map_or(s.normal(), |c| s.color(c))
            }
            // _ => s.normal(),
        }
    }

    fn format_value(
        &self,
        interface: &USBInterface,
        pad: &HashMap<Self, usize>,
        settings: &PrintSettings,
    ) -> Option<String> {
        match self {
            InterfaceBlocks::Number => Some(format!("{:2}", interface.number)),
            InterfaceBlocks::Name => Some(format!(
                "{:pad$}",
                interface.name,
                pad = pad.get(self).unwrap_or(&0)
            )),
            InterfaceBlocks::NumEndpoints => Some(format!("{:2}", interface.endpoints.len())),
            InterfaceBlocks::PortPath => Some(format!(
                "{:pad$}",
                interface.path,
                pad = pad.get(self).unwrap_or(&0)
            )),
            InterfaceBlocks::SysPath => Some(match interface.syspath.as_ref() {
                Some(v) => format!("{:pad$}", v, pad = pad.get(self).unwrap_or(&0)),
                None => format!("{:pad$}", "-", pad = pad.get(self).unwrap_or(&0)),
            }),
            InterfaceBlocks::Driver => Some(match interface.driver.as_ref() {
                Some(v) => format!("{:pad$}", v, pad = pad.get(self).unwrap_or(&0)),
                None => format!("{:pad$}", "-", pad = pad.get(self).unwrap_or(&0)),
            }),
            InterfaceBlocks::ClassCode => Some(format!(
                "{:pad$}",
                interface.class.to_string(),
                pad = pad.get(self).unwrap_or(&0)
            )),
            InterfaceBlocks::SubClass => Some(Self::format_base_u8(interface.sub_class, settings)),
            InterfaceBlocks::Protocol => Some(Self::format_base_u8(interface.protocol, settings)),
            InterfaceBlocks::AltSetting => {
                Some(Self::format_base_u8(interface.alt_setting, settings))
            }
            InterfaceBlocks::Icon => settings.icons.as_ref().map_or(None, |i| {
                Some(i.get_classifier_icon(
                    &interface.class,
                    interface.sub_class,
                    interface.protocol,
                ))
            }),
            // _ => None,
        }
    }

    fn heading(&self, pad: &HashMap<Self, usize>) -> String {
        match self {
            InterfaceBlocks::Number => " #".into(),
            InterfaceBlocks::Name => format!("{:^pad$}", "Name", pad = pad.get(self).unwrap_or(&0)),
            InterfaceBlocks::NumEndpoints => "E#".into(),
            InterfaceBlocks::PortPath => {
                format!("{:^pad$}", "PortPath", pad = pad.get(self).unwrap_or(&0))
            }
            InterfaceBlocks::SysPath => {
                format!("{:^pad$}", "SysPath", pad = pad.get(self).unwrap_or(&0))
            }
            InterfaceBlocks::Driver => {
                format!("{:^pad$}", "Driver", pad = pad.get(self).unwrap_or(&0))
            }
            InterfaceBlocks::ClassCode => {
                format!("{:^pad$}", "Class", pad = pad.get(self).unwrap_or(&0))
            }
            InterfaceBlocks::SubClass => "SubC".into(),
            InterfaceBlocks::Protocol => "Pcol".into(),
            InterfaceBlocks::AltSetting => "Alt#".into(),
            InterfaceBlocks::Icon => ICON_HEADING.into(),
            // _ => "",
        }
    }
}

impl Block<EndpointBlocks, USBEndpoint> for EndpointBlocks {
    fn default_blocks(verbose: bool) -> Vec<EndpointBlocks> {
        if verbose {
            vec![
                EndpointBlocks::Number,
                EndpointBlocks::Direction,
                EndpointBlocks::TransferType,
                EndpointBlocks::SyncType,
                EndpointBlocks::UsageType,
                EndpointBlocks::Interval,
                EndpointBlocks::MaxPacketSize,
            ]
        } else {
            vec![
                EndpointBlocks::Number,
                EndpointBlocks::Direction,
                EndpointBlocks::TransferType,
                EndpointBlocks::SyncType,
                EndpointBlocks::UsageType,
                EndpointBlocks::MaxPacketSize,
            ]
        }
    }

    fn generate_padding(d: &Vec<&USBEndpoint>) -> HashMap<Self, usize> {
        HashMap::from([
            (
                EndpointBlocks::TransferType,
                cmp::max(
                    EndpointBlocks::TransferType
                        .heading(&Default::default())
                        .len(),
                    d.iter()
                        .map(|d| d.transfer_type.to_string().len())
                        .max()
                        .unwrap_or(0),
                ),
            ),
            (
                EndpointBlocks::SyncType,
                cmp::max(
                    EndpointBlocks::SyncType.heading(&Default::default()).len(),
                    d.iter()
                        .map(|d| d.sync_type.to_string().len())
                        .max()
                        .unwrap_or(0),
                ),
            ),
            (
                EndpointBlocks::UsageType,
                cmp::max(
                    EndpointBlocks::UsageType.heading(&Default::default()).len(),
                    d.iter()
                        .map(|d| d.usage_type.to_string().len())
                        .max()
                        .unwrap_or(0),
                ),
            ),
            (
                EndpointBlocks::Direction,
                cmp::max(
                    EndpointBlocks::Direction.heading(&Default::default()).len(),
                    d.iter()
                        .map(|d| d.address.direction.to_string().len())
                        .max()
                        .unwrap_or(0),
                ),
            ),
            (
                EndpointBlocks::MaxPacketSize,
                cmp::max(
                    EndpointBlocks::MaxPacketSize
                        .heading(&Default::default())
                        .len(),
                    d.iter()
                        .map(|d| d.max_packet_string().len())
                        .max()
                        .unwrap_or(0),
                ),
            ),
        ])
    }

    fn value_is_string(&self) -> bool {
        match self {
            EndpointBlocks::TransferType
            | EndpointBlocks::SyncType
            | EndpointBlocks::UsageType
            | EndpointBlocks::Direction => true,
            _ => false,
        }
    }

    fn colour(&self, s: &String, ct: &colour::ColourTheme) -> ColoredString {
        match self {
            EndpointBlocks::Number | EndpointBlocks::Interval | EndpointBlocks::MaxPacketSize => {
                ct.number.map_or(s.normal(), |c| s.color(c))
            }
            EndpointBlocks::Direction
            | EndpointBlocks::UsageType
            | EndpointBlocks::TransferType
            | EndpointBlocks::SyncType => ct.attributes.map_or(s.normal(), |c| s.color(c)),
        }
    }

    fn format_value(
        &self,
        end: &USBEndpoint,
        pad: &HashMap<Self, usize>,
        _settings: &PrintSettings,
    ) -> Option<String> {
        match self {
            EndpointBlocks::Number => Some(format!("{:2}", end.address.number)),
            EndpointBlocks::Interval => Some(format!("{:2}", end.interval)),
            EndpointBlocks::MaxPacketSize => Some(format!(
                "{:pad$}",
                end.max_packet_string(),
                pad = pad.get(self).unwrap_or(&0)
            )),
            EndpointBlocks::Direction => Some(format!(
                "{:pad$}",
                end.address.direction.to_string(),
                pad = pad.get(self).unwrap_or(&0)
            )),
            EndpointBlocks::TransferType => Some(format!(
                "{:pad$}",
                end.transfer_type.to_string(),
                pad = pad.get(self).unwrap_or(&0)
            )),
            EndpointBlocks::SyncType => Some(format!(
                "{:pad$}",
                end.sync_type.to_string(),
                pad = pad.get(self).unwrap_or(&0)
            )),
            EndpointBlocks::UsageType => Some(format!(
                "{:pad$}",
                end.usage_type.to_string(),
                pad = pad.get(self).unwrap_or(&0)
            )),
            // _ => None,
        }
    }

    fn heading(&self, pad: &HashMap<Self, usize>) -> String {
        match self {
            EndpointBlocks::Number => " #".into(),
            EndpointBlocks::Interval => "Iv".into(),
            EndpointBlocks::MaxPacketSize => {
                format!("{:^pad$}", "MaxPkB", pad = pad.get(self).unwrap_or(&0))
            }
            EndpointBlocks::Direction => {
                format!("{:^pad$}", "Dir", pad = pad.get(self).unwrap_or(&0))
            }
            EndpointBlocks::TransferType => {
                format!("{:^pad$}", "TransferT", pad = pad.get(self).unwrap_or(&0))
            }
            EndpointBlocks::SyncType => {
                format!("{:^pad$}", "SyncT", pad = pad.get(self).unwrap_or(&0))
            }
            EndpointBlocks::UsageType => {
                format!("{:^pad$}", "UsageT", pad = pad.get(self).unwrap_or(&0))
            }
            // _ => "",
        }
    }
}

/// Value to sort [`USBDevice`]
#[derive(Default, PartialEq, Eq, Debug, ValueEnum, Clone, Serialize, Deserialize)]
pub enum Sort {
    #[default]
    /// Sort by position in parent branch
    BranchPosition,
    /// Sort by bus device number
    DeviceNumber,
    /// No sorting; whatever order it was parsed
    NoSort,
}

impl Sort {
    /// The clone and sort the [`USBDevice`]s `d`
    pub fn sort_devices(
        &self,
        d: &Vec<system_profiler::USBDevice>,
    ) -> Vec<system_profiler::USBDevice> {
        let mut sorted = d.to_owned();
        match self {
            Sort::BranchPosition => sorted.sort_by_key(|d| d.get_branch_position()),
            Sort::DeviceNumber => sorted.sort_by_key(|d| d.location_id.number),
            _ => (),
        }

        sorted
    }

    /// The clone and sort the references to [`USBDevice`]s `d`
    pub fn sort_devices_ref<'a>(
        &self,
        d: &Vec<&'a system_profiler::USBDevice>,
    ) -> Vec<&'a system_profiler::USBDevice> {
        let mut sorted = d.to_owned();
        match self {
            Sort::BranchPosition => sorted.sort_by_key(|d| d.get_branch_position()),
            Sort::DeviceNumber => sorted.sort_by_key(|d| d.location_id.number),
            _ => (),
        }

        sorted
    }
}

/// Value to group [`USBDevice`]
#[derive(Default, Debug, ValueEnum, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Group {
    #[default]
    /// No grouping
    NoGroup,
    /// Group into buses with bus info as heading - like a flat tree
    Bus,
}

/// Charactor printing settings
// TODO use this as printing: Vec<display::Printing> with default [display::Printing::Utf8, display::Printing::Icons]
#[derive(Debug, ValueEnum, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Printing {
    /// Use utf-8 charactors
    Utf8,
    /// Use ascii charactors
    Ascii,
    /// Show icons
    Icons,
    /// Show no icons
    NoIcons,
}

/// Options for [`PrintSettings`] mask_serials
#[derive(Default, Debug, ValueEnum, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MaskSerial {
    #[default]
    /// Hide with '*' char
    Hide,
    /// Mask by randomising existing chars
    Scramble,
    /// Mask by replacing length with random chars
    Replace,
}

/// Passed to printing functions allows default args
#[derive(Debug, Default)]
pub struct PrintSettings {
    /// Don't pad in order to align blocks
    pub no_padding: bool,
    /// Print in decimal not base16
    pub decimal: bool,
    /// No tree printing
    pub tree: bool,
    /// Hide empty buses
    pub hide_buses: bool,
    /// Sort devices
    pub sort_devices: Sort,
    /// Sort buses by bus number
    pub sort_buses: bool,
    /// Group devices
    pub group_devices: Group,
    /// Print headings for blocks
    pub headings: bool,
    /// Level of verbosity
    pub verbosity: u8,
    /// Print more blocks by default
    pub more: bool,
    /// Print as json
    pub json: bool,
    /// Scramble serial numbers, useful if sharing sensitive device dumps
    pub mask_serials: Option<MaskSerial>,
    /// [`DeviceBlocks`] to use for printing
    pub device_blocks: Option<Vec<DeviceBlocks>>,
    /// [`BusBlocks`] to use for printing
    pub bus_blocks: Option<Vec<BusBlocks>>,
    /// [`ConfigurationBlocks`] to use for printing
    pub config_blocks: Option<Vec<ConfigurationBlocks>>,
    /// [`InterfaceBlocks`] to use for printing
    pub interface_blocks: Option<Vec<InterfaceBlocks>>,
    /// [`EndpointBlocks`] to use for printing
    pub endpoint_blocks: Option<Vec<EndpointBlocks>>,
    /// [`crate::icon::IconTheme`] to apply - None to not print any icons
    pub icons: Option<icon::IconTheme>,
    /// [`crate::colour::ColourTheme`] to apply - None to not colour
    pub colours: Option<colour::ColourTheme>,
}

/// Converts a HashSet of [`ConfigAttributes`] a String of nerd icons
fn attributes_to_icons(attributes: &Vec<ConfigAttributes>, settings: &PrintSettings) -> String {
    let mut icon_strs = Vec::new();
    if settings.icons.is_some() {
        for a in attributes {
            match a {
                ConfigAttributes::SelfPowered => icon_strs.push("\u{fba4}"), // ﮤ
                ConfigAttributes::RemoteWakeup => icon_strs.push("\u{f654}"), // 
            }
        }
    }
    icon_strs.join(" ")
}

/// Formats each [`Block`] value shown from a device `d`
pub fn render_value<B, T>(
    d: &T,
    blocks: &Vec<impl Block<B, T>>,
    pad: &HashMap<B, usize>,
    settings: &PrintSettings,
) -> Vec<String> {
    let mut ret = Vec::new();
    for b in blocks {
        if let Some(string) = b.format_value(d, pad, settings) {
            match &settings.colours {
                Some(c) => ret.push(format!("{}", b.colour(&string, &c))),
                None => ret.push(format!("{}", string)),
            }
        }
    }

    ret
}

/// Renders the headings for each [`Block`] being shown
pub fn render_heading<B, T>(
    blocks: &Vec<impl Block<B, T>>,
    pad: &HashMap<B, usize>,
) -> Vec<String> {
    let mut ret = Vec::new();

    for b in blocks {
        ret.push(b.heading(pad).to_string())
    }

    ret
}

/// Generates tree formating and values given `current_tree`, current `branch_length` and item `index` in branch
fn generate_tree_data(
    current_tree: &TreeData,
    branch_length: usize,
    index: usize,
    settings: &PrintSettings,
) -> TreeData {
    let mut pass_tree = current_tree.clone();

    // get prefix from icons if tree - maybe should cache these before build rather than lookup each time...
    if settings.tree {
        pass_tree.prefix = if pass_tree.depth > 0 {
            let edge_icon = if index + 1 != pass_tree.branch_length {
                icon::Icon::TreeLine
            } else {
                icon::Icon::TreeBlank
            };

            format!(
                "{}{}",
                pass_tree.prefix,
                settings
                    .icons
                    .as_ref()
                    .map_or(icon::get_ascii_tree_icon(&edge_icon), |i| i
                        .get_tree_icon(&edge_icon))
            )
        } else {
            format!("{}", pass_tree.prefix)
        };
    }

    pass_tree.depth += 1;
    pass_tree.branch_length = branch_length;
    pass_tree.trunk_index = index as u8;

    return pass_tree;
}

/// Print `devices` `USBDevice` references without looking down each device's devices!
pub fn print_flattened_devices(
    devices: &Vec<&system_profiler::USBDevice>,
    settings: &PrintSettings,
) {
    let db = settings
        .device_blocks
        .to_owned()
        .unwrap_or(DeviceBlocks::default_blocks(
            settings.verbosity >= MAX_VERBOSITY || settings.more,
        ));
    let pad = if !settings.no_padding {
        DeviceBlocks::generate_padding(devices)
    } else {
        HashMap::new()
    };
    log::trace!("Flattened devices padding {:?}", pad);

    let sorted = settings.sort_devices.sort_devices_ref(&devices);

    if settings.headings {
        let heading = render_heading(&db, &pad).join(" ");
        println!("{}", heading.bold().underline());
    }

    for (i, device) in sorted.into_iter().enumerate() {
        println!("{}", render_value(device, &db, &pad, settings).join(" "));
        // print the configurations
        if let Some(extra) = device.extra.as_ref() {
            if settings.verbosity >= 1 {
                let blocks = (
                    &settings.config_blocks.to_owned().unwrap_or(Block::<
                        ConfigurationBlocks,
                        USBConfiguration,
                    >::default_blocks(
                        settings.verbosity >= MAX_VERBOSITY || settings.more,
                    )),
                    &settings.interface_blocks.to_owned().unwrap_or(Block::<
                        InterfaceBlocks,
                        USBInterface,
                    >::default_blocks(
                        settings.verbosity >= MAX_VERBOSITY || settings.more,
                    )),
                    &settings.endpoint_blocks.to_owned().unwrap_or(Block::<
                        EndpointBlocks,
                        USBEndpoint,
                    >::default_blocks(
                        settings.verbosity >= MAX_VERBOSITY || settings.more,
                    )),
                );
                // pass branch length as number of configurations for this device plus devices still to print
                print_configurations(
                    &extra.configurations,
                    blocks,
                    settings,
                    &generate_tree_data(
                        &Default::default(),
                        extra.configurations.len() + device.devices.as_ref().map_or(0, |d| d.len()),
                        i,
                        settings,
                    ),
                );
            }
        } else if settings.verbosity >= 1 {
            log::warn!(
                "Unable to print verbose information for {} because libusb extra data is missing",
                device
            )
        }
    }
}

/// A way of printing a reference flattened `SPUSBDataType` rather than hard flatten
///
/// Prints each `&USBBus` and tuple pair `Vec<&USBDevice>`
pub fn print_bus_grouped(
    bus_devices: Vec<(&system_profiler::USBBus, Vec<&system_profiler::USBDevice>)>,
    settings: &PrintSettings,
) {
    let bb = settings.bus_blocks.to_owned().unwrap_or(
        Block::<BusBlocks, system_profiler::USBBus>::default_blocks(
            settings.verbosity >= MAX_VERBOSITY || settings.more,
        ),
    );
    let pad: HashMap<BusBlocks, usize> = if !settings.no_padding {
        BusBlocks::generate_padding(&bus_devices.iter().map(|bd| bd.0).collect())
    } else {
        HashMap::new()
    };

    for (bus, devices) in bus_devices {
        if settings.headings {
            let heading = render_heading(&bb, &pad).join(" ");
            println!("{}", heading.bold().underline());
        }
        println!("{}", render_value(bus, &bb, &pad, settings).join(" "));
        print_flattened_devices(&devices, settings);
        // new line for each group
        println!();
    }
}

/// Passed to print functions to support tree building
#[derive(Debug, Default, Clone)]
pub struct TreeData {
    /// Length of the branch sitting on
    branch_length: usize,
    /// Index within parent list of devices
    trunk_index: u8,
    /// Depth of tree being built - normally len() tree_positions but might not be if printing inner
    depth: usize,
    /// Prefix to apply, builds up as depth increases
    prefix: String,
}

/// All device [`USBEndpoint`]
pub fn print_endpoints(
    endpoints: &Vec<USBEndpoint>,
    blocks: &Vec<EndpointBlocks>,
    settings: &PrintSettings,
    tree: &TreeData,
) {
    let pad = if !settings.no_padding {
        EndpointBlocks::generate_padding(&endpoints.iter().map(|d| d).collect())
    } else {
        HashMap::new()
    };
    log::trace!("Print endpoints padding {:?}, tree {:?}", pad, tree);

    for (i, endpoint) in endpoints.iter().enumerate() {
        // get current prefix based on if last in tree and whether we are within the tree
        if settings.tree {
            let mut prefix = if tree.depth > 0 {
                let edge_icon = if i + 1 != tree.branch_length {
                    icon::Icon::TreeEdge
                } else {
                    icon::Icon::TreeCorner
                };
                let edge = settings
                    .icons
                    .as_ref()
                    .map_or(icon::get_ascii_tree_icon(&edge_icon), |i| {
                        i.get_tree_icon(&edge_icon)
                    });
                format!("{}{}", tree.prefix, edge)
            // zero depth
            } else {
                format!("{}", tree.prefix)
            };

            let mut terminator = settings.icons.as_ref().map_or(
                icon::get_ascii_tree_icon(&icon::Icon::Endpoint(endpoint.address.direction)),
                |i| i.get_tree_icon(&icon::Icon::Endpoint(endpoint.address.direction)),
            );

            // colour tree
            if let Some(ct) = settings.colours.as_ref() {
                prefix = ct
                    .tree
                    .map_or(prefix.normal(), |c| prefix.color(c))
                    .to_string();
                terminator = if endpoint.address.direction == Direction::In {
                    ct.tree_endpoint_in
                        .map_or(terminator.normal(), |c| terminator.color(c))
                        .to_string()
                } else {
                    ct.tree_endpoint_out
                        .map_or(terminator.normal(), |c| terminator.color(c))
                        .to_string()
                };
            }

            // maybe should just do once at start of bus
            if settings.headings && i == 0 {
                let heading = render_heading(&blocks, &pad).join(" ");
                println!("{}  {}", prefix, heading.bold().underline());
            }

            // render and print tree if doing it
            print!("{}{} ", prefix, terminator);
            println!(
                "{}",
                render_value(endpoint, blocks, &pad, settings).join(" ")
            );
        } else {
            if settings.headings && i == 0 {
                let heading = render_heading(blocks, &pad).join(" ");
                println!("{:spaces$}{}", "", heading.bold().underline(), spaces = 6);
            }

            println!(
                "{:spaces$}{}",
                "",
                render_value(endpoint, &blocks, &pad, settings).join(" "),
                spaces = 6
            );
        }
    }
}

/// All device [`USBInterface`]
pub fn print_interfaces(
    interfaces: &Vec<USBInterface>,
    blocks: (&Vec<InterfaceBlocks>, &Vec<EndpointBlocks>),
    settings: &PrintSettings,
    tree: &TreeData,
) {
    let pad = if !settings.no_padding {
        InterfaceBlocks::generate_padding(&interfaces.iter().map(|d| d).collect())
    } else {
        HashMap::new()
    };
    log::trace!("Print interfaces padding {:?}, tree {:?}", pad, tree);

    for (i, interface) in interfaces.iter().enumerate() {
        // get current prefix based on if last in tree and whether we are within the tree
        if settings.tree {
            let mut prefix = if tree.depth > 0 {
                let edge_icon = if i + 1 != tree.branch_length {
                    icon::Icon::TreeEdge
                } else {
                    icon::Icon::TreeCorner
                };
                let edge = settings
                    .icons
                    .as_ref()
                    .map_or(icon::get_ascii_tree_icon(&edge_icon), |i| {
                        i.get_tree_icon(&edge_icon)
                    });
                format!("{}{}", tree.prefix, edge)
            // zero depth
            } else {
                format!("{}", tree.prefix)
            };

            let mut terminator = settings.icons.as_ref().map_or(
                icon::get_ascii_tree_icon(&icon::Icon::TreeInterfaceTerminator),
                |i| i.get_tree_icon(&icon::Icon::TreeInterfaceTerminator),
            );

            // colour tree
            if let Some(ct) = settings.colours.as_ref() {
                prefix = ct
                    .tree
                    .map_or(prefix.normal(), |c| prefix.color(c))
                    .to_string();
                terminator = ct
                    .tree_interface_terminator
                    .map_or(terminator.normal(), |c| terminator.color(c))
                    .to_string();
            }

            // maybe should just do once at start of bus
            if settings.headings && i == 0 {
                let heading = render_heading(&blocks.0, &pad).join(" ");
                println!("{}  {}", prefix, heading.bold().underline());
            }

            // render and print tree if doing it
            print!("{}{} ", prefix, terminator);

            println!(
                "{}",
                render_value(interface, &blocks.0, &pad, settings).join(" ")
            );
        } else {
            if settings.headings && i == 0 {
                let heading = render_heading(&blocks.0, &pad).join(" ");
                println!("{:spaces$}{}", "", heading.bold().underline(), spaces = 4);
            }

            println!(
                "{:spaces$}{}",
                "",
                render_value(interface, &blocks.0, &pad, settings).join(" "),
                spaces = 4
            );
        }

        // print the endpoints
        if settings.verbosity >= 3 {
            print_endpoints(
                &interface.endpoints,
                &blocks.1,
                settings,
                &generate_tree_data(tree, interface.endpoints.len(), i, settings),
            );
        }
    }
}

/// All device [`USBConfiguration`]
pub fn print_configurations(
    configs: &Vec<USBConfiguration>,
    blocks: (
        &Vec<ConfigurationBlocks>,
        &Vec<InterfaceBlocks>,
        &Vec<EndpointBlocks>,
    ),
    settings: &PrintSettings,
    tree: &TreeData,
) {
    let pad = if !settings.no_padding {
        ConfigurationBlocks::generate_padding(&configs.iter().map(|d| d).collect())
    } else {
        HashMap::new()
    };
    log::trace!("Print configs padding {:?}, tree {:?}", pad, tree);

    for (i, config) in configs.iter().enumerate() {
        // get current prefix based on if last in tree and whether we are within the tree
        if settings.tree {
            let mut prefix = if tree.depth > 0 {
                let edge_icon = if i + 1 != tree.branch_length {
                    icon::Icon::TreeEdge
                } else {
                    icon::Icon::TreeCorner
                };
                let edge = settings
                    .icons
                    .as_ref()
                    .map_or(icon::get_ascii_tree_icon(&edge_icon), |i| {
                        i.get_tree_icon(&edge_icon)
                    });
                format!("{}{}", tree.prefix, edge)
            // zero depth
            } else {
                format!("{}", tree.prefix)
            };

            let mut terminator = settings.icons.as_ref().map_or(
                icon::get_ascii_tree_icon(&icon::Icon::TreeConfigurationTerminator),
                |i| i.get_tree_icon(&icon::Icon::TreeConfigurationTerminator),
            );

            // colour tree
            if let Some(ct) = settings.colours.as_ref() {
                prefix = ct
                    .tree
                    .map_or(prefix.normal(), |c| prefix.color(c))
                    .to_string();
                terminator = ct
                    .tree_configuration_terminator
                    .map_or(terminator.normal(), |c| terminator.color(c))
                    .to_string();
            }

            // maybe should just do once at start of bus
            if settings.headings && i == 0 {
                let heading = render_heading(blocks.0, &pad).join(" ");
                println!("{}  {}", prefix, heading.bold().underline());
            }

            // render and print tree if doing it
            print!("{}{} ", prefix, terminator);

            println!(
                "{}",
                render_value(config, blocks.0, &pad, settings).join(" ")
            );
        } else {
            if settings.headings && i == 0 {
                let heading = render_heading(blocks.0, &pad).join(" ");
                println!("{:spaces$}{}", "", heading.bold().underline(), spaces = 2);
            }

            println!(
                "{:spaces$}{}",
                "",
                render_value(config, blocks.0, &pad, settings).join(" "),
                spaces = 2
            );
        }

        // print the interfaces
        if settings.verbosity >= 2 {
            print_interfaces(
                &config.interfaces,
                (&blocks.1, &blocks.2),
                settings,
                &generate_tree_data(tree, config.interfaces.len(), i, settings),
            );
        }
    }
}

/// Recursively print `devices`; will call for each `USBDevice` devices if `Some`
///
/// Will draw tree if `settings.tree`, otherwise it will be flat
pub fn print_devices(
    devices: &Vec<system_profiler::USBDevice>,
    db: &Vec<DeviceBlocks>,
    settings: &PrintSettings,
    tree: &TreeData,
) {
    let pad = if !settings.no_padding {
        DeviceBlocks::generate_padding(&devices.iter().map(|d| d).collect())
    } else {
        HashMap::new()
    };
    log::trace!("Print devices padding {:?}, tree {:?}", pad, tree);

    // sort so that can be ascending along branch
    let sorted = settings.sort_devices.sort_devices(&devices);

    for (i, device) in sorted.iter().enumerate() {
        // get current prefix based on if last in tree and whether we are within the tree
        if settings.tree {
            let mut prefix = if tree.depth > 0 {
                let edge_icon = if i + 1 != tree.branch_length {
                    icon::Icon::TreeEdge
                } else {
                    icon::Icon::TreeCorner
                };
                let edge = settings
                    .icons
                    .as_ref()
                    .map_or(icon::get_ascii_tree_icon(&edge_icon), |i| {
                        i.get_tree_icon(&edge_icon)
                    });
                format!("{}{}", tree.prefix, edge)
            // zero depth
            } else {
                format!("{}", tree.prefix)
            };

            let mut terminator = settings.icons.as_ref().map_or(
                icon::get_ascii_tree_icon(&icon::Icon::TreeDeviceTerminator),
                |i| i.get_tree_icon(&icon::Icon::TreeDeviceTerminator),
            );

            // colour tree
            if let Some(ct) = settings.colours.as_ref() {
                prefix = ct
                    .tree
                    .map_or(prefix.normal(), |c| prefix.color(c))
                    .to_string();
                terminator = ct
                    .tree_bus_terminator
                    .map_or(terminator.normal(), |c| terminator.color(c))
                    .to_string();
            }

            // maybe should just do once at start of bus
            if settings.headings && i == 0 {
                let heading = render_heading(db, &pad).join(" ");
                println!("{}  {}", prefix, heading.bold().underline());
            }

            // render and print tree if doing it
            print!("{}{} ", prefix, terminator);
        } else {
            if settings.headings && i == 0 {
                let heading = render_heading(db, &pad).join(" ");
                println!("{}", heading.bold().underline());
            }
        }

        // print the device
        println!("{}", render_value(device, db, &pad, settings).join(" "));

        // print the configurations
        if let Some(extra) = device.extra.as_ref() {
            if settings.verbosity >= 1 {
                let blocks = (
                    &settings.config_blocks.to_owned().unwrap_or(Block::<
                        ConfigurationBlocks,
                        USBConfiguration,
                    >::default_blocks(
                        settings.verbosity >= MAX_VERBOSITY || settings.more,
                    )),
                    &settings.interface_blocks.to_owned().unwrap_or(Block::<
                        InterfaceBlocks,
                        USBInterface,
                    >::default_blocks(
                        settings.verbosity >= MAX_VERBOSITY || settings.more,
                    )),
                    &settings.endpoint_blocks.to_owned().unwrap_or(Block::<
                        EndpointBlocks,
                        USBEndpoint,
                    >::default_blocks(
                        settings.verbosity >= MAX_VERBOSITY || settings.more,
                    )),
                );
                // pass branch length as number of configurations for this device plus devices still to print
                print_configurations(
                    &extra.configurations,
                    blocks,
                    settings,
                    &generate_tree_data(
                        &tree,
                        extra.configurations.len() + device.devices.as_ref().map_or(0, |d| d.len()),
                        i,
                        settings,
                    ),
                );
            }
        } else if settings.verbosity >= 1 {
            log::warn!(
                "Unable to print verbose information for {} because libusb extra data is missing",
                device
            )
        }

        match device.devices.as_ref() {
            Some(d) => {
                // and then walk down devices printing them too
                print_devices(
                    &d,
                    db,
                    settings,
                    &generate_tree_data(&tree, d.len(), i, settings),
                );
            }
            None => (),
        }
    }
}

/// Print SPUSBDataType
pub fn print_sp_usb(sp_usb: &system_profiler::SPUSBDataType, settings: &PrintSettings) {
    let bb = settings.bus_blocks.to_owned().unwrap_or(
        Block::<BusBlocks, system_profiler::USBBus>::default_blocks(
            settings.verbosity >= MAX_VERBOSITY || settings.more,
        ),
    );
    let db = settings.device_blocks.to_owned().unwrap_or(
        if settings.verbosity >= MAX_VERBOSITY || settings.more {
            DeviceBlocks::default_blocks(true)
        } else {
            if settings.tree {
                DeviceBlocks::default_device_tree_blocks()
            } else {
                DeviceBlocks::default_blocks(false)
            }
        },
    );

    let base_tree = TreeData {
        ..Default::default()
    };

    let pad: HashMap<BusBlocks, usize> = if !settings.no_padding {
        BusBlocks::generate_padding(&sp_usb.buses.iter().map(|b| b).collect())
    } else {
        HashMap::new()
    };

    log::trace!(
        "print SPUSBDataType settings, {:?}, padding {:?}, tree {:?}",
        settings,
        pad,
        base_tree
    );

    for (i, bus) in sp_usb.buses.iter().enumerate() {
        if settings.tree {
            let mut prefix = base_tree.prefix.to_owned();
            let mut start = settings
                .icons
                .as_ref()
                .map_or(icon::get_ascii_tree_icon(&icon::Icon::TreeBusStart), |i| {
                    i.get_tree_icon(&icon::Icon::TreeBusStart)
                });

            // colour tree
            if let Some(ct) = settings.colours.as_ref() {
                prefix = ct
                    .tree
                    .map_or(prefix.normal(), |c| prefix.color(c))
                    .to_string();
                start = ct
                    .tree_bus_start
                    .map_or(start.normal(), |c| start.color(c))
                    .to_string();
            }

            if settings.headings {
                let heading = render_heading(&bb, &pad).join(" ");
                // 2 spaces for bus start icon and space to info
                println!("{:>spaces$}{}", "", heading.bold().underline(), spaces = 2);
            }

            print!("{}{} ", prefix, start);
        } else {
            if settings.headings {
                let heading = render_heading(&bb, &pad).join(" ");
                // 2 spaces for bus start icon and space to info
                println!("{}", heading.bold().underline());
            }
        }
        println!("{}", render_value(bus, &bb, &pad, settings).join(" "));

        match bus.devices.as_ref() {
            Some(d) => {
                // and then walk down devices printing them too
                print_devices(
                    &d,
                    &db,
                    settings,
                    &generate_tree_data(&base_tree, d.len(), i, settings),
                );
            }
            None => (),
        }

        // separate bus groups with line
        println!();
    }
}

/// Mask the `device` serial if it has one using the [`MaskSerial`] method and recursively if `recursive`
pub fn mask_serial(device: &mut system_profiler::USBDevice, hide: &MaskSerial, recursive: bool) {
    if let Some(serial) = device.serial_num.as_mut() {
        *serial = match hide {
            MaskSerial::Hide => serial.chars().map(|_| '*').collect::<String>(),
            MaskSerial::Scramble =>
                serial.chars().map(|_| serial.chars().choose(&mut rand::thread_rng()).unwrap_or('*')).collect::<String>(),
            MaskSerial::Replace =>
                rand::thread_rng()
                    .sample_iter(Alphanumeric)
                    .take(serial.chars().count())
                    .map(char::from)
                    .collect::<String>().to_uppercase(),
        };
    }

    if recursive {
        device.devices.as_mut().map_or((), |dd| dd.iter_mut().for_each(|d| mask_serial(d, hide, recursive)));
    }
}

/// Main cyme bin prepare for printing function - changes mutable `sp_usb` with requested `filter` and sort in `settings`
pub fn prepare(
    sp_usb: &mut system_profiler::SPUSBDataType,
    filter: Option<system_profiler::USBFilter>,
    settings: &PrintSettings,
) {
    // if not printing tree, hard flatten now before filtering as filter will retain non-matching parents with matching devices in tree
    // but only do it if there is a filter, grouping by bus (which uses tree print without tree...) or json
    // flattening now will also mean hubs will be removed when listing if `hide_hubs` because they will appear empty
    if !settings.tree && (filter.is_some() || settings.group_devices == Group::Bus || settings.json)
    {
        sp_usb.flatten();
    }

    // do the filter if present; will keep parents of matched devices even if they do not match
    filter
        .as_ref()
        .map_or((), |f| f.retain_buses(&mut sp_usb.buses));

    // hide any empty buses and hubs now we've filtered
    if settings.hide_buses {
        sp_usb.buses.retain(|b| b.has_devices());
        // may still be empty hubs if the hub had an empty hub!
        if let Some(f) = filter.as_ref() {
            if f.exclude_empty_hub {
                sp_usb.buses.retain(|b| !b.has_empty_hubs());
            }
        }
    }

    // sort the buses if asked
    if settings.sort_buses {
        sp_usb.buses.sort_by_key(|d| d.get_bus_number());
    }

    // hide serials Recursively
    if let Some(hide) = settings.mask_serials.as_ref() {
        for bus in &mut sp_usb.buses {
            bus.devices.as_mut().map_or((), |devices| {
                for mut device in devices {
                    mask_serial(&mut device, hide, true);
                }
            });
        }
    }

    log::trace!("sp_usb data post filter and bus sort\n\r{:#}", sp_usb);
}

/// Main cyme bin print function
pub fn print(sp_usb: &system_profiler::SPUSBDataType, settings: &PrintSettings) {
    log::debug!("Printing with {:?}", settings);

    if settings.tree || settings.group_devices == Group::Bus {
        if settings.json {
            println!("{}", serde_json::to_string_pretty(&sp_usb).unwrap());
        } else {
            print_sp_usb(sp_usb, settings);
        }
    } else {
        match settings.group_devices {
            // completely flatten the bus and only print devices
            _ => {
                // get a list of all devices
                let devs = sp_usb.flatten_devices();

                if settings.json {
                    println!("{}", serde_json::to_string_pretty(&devs).unwrap());
                } else {
                    print_flattened_devices(&devs, settings);
                }
            }
        }
    }
}
