// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};

use phd_testcase::{phd_framework::test_vm::MigrationTimeout, *};
use propolis_client::instance_spec::{
    PciPath, SpecKey, UsbDevice, UsbDeviceType, XhciController,
};
use uuid::Uuid;

#[phd_testcase]
async fn usb_device_enumerates(ctx: &TestCtx) {
    let mut config = ctx.vm_config_builder("xhci_usb_device_enumerates_test");
    let xhc_name: SpecKey = "xhc0".into();
    const PCI_DEV: u8 = 3;
    const USB_PORT: u8 = 2;
    config
        .xhci_controller(
            xhc_name.to_owned(),
            XhciController { pci_path: PciPath::new(0, PCI_DEV, 0)? },
        )
        .usb_device(UsbDevice {
            xhc_device: xhc_name,
            root_hub_port_num: USB_PORT,
            usb_device_type: UsbDeviceType::Null,
        });

    let spec = config.vm_spec(ctx).await?;

    let mut vm = ctx.spawn_vm_with_spec(spec, None).await?;
    if !vm.guest_os_kind().is_linux() {
        phd_skip!("xHCI/USB test uses sysfs to enumerate devices");
    }

    vm.launch().await?;
    vm.wait_to_boot().await?;

    let output = vm
        .run_shell_command(
            &format!("cat /sys/devices/pci0000:00/0000:00:{PCI_DEV:02}.0/usb1/1-{USB_PORT}/idVendor"),
        )
        .await?;
    // it's a device provided by Oxide (registered USB vendor ID 0x38c6)
    assert_eq!(output, "38c6");
}

#[phd_testcase]
async fn usb_tablet_vnc_pointer_events(ctx: &TestCtx) {
    let mut config =
        ctx.vm_config_builder("xhci_usb_tablet_vnc_pointer_events_test");
    let xhc_name: SpecKey = "xhc0".into();
    const PCI_DEV: u8 = 3;
    const USB_PORT: u8 = 2;
    config
        .xhci_controller(
            xhc_name.to_owned(),
            XhciController { pci_path: PciPath::new(0, PCI_DEV, 0)? },
        )
        .usb_device(UsbDevice {
            xhc_device: xhc_name,
            root_hub_port_num: USB_PORT,
            usb_device_type: UsbDeviceType::HidTablet,
        });

    let spec = config.vm_spec(ctx).await?;

    let mut vm = ctx.spawn_vm_with_spec(spec, None).await?;
    if !vm.guest_os_kind().is_linux() {
        phd_skip!("USB/VNC test uses /dev/hidraw0 to receive raw HID events");
    }

    vm.launch().await?;
    vm.wait_to_boot().await?;

    let waiting = Arc::new(AtomicBool::new(true));
    let waiting_outer = waiting.clone();
    let mut vnc_client = vm.vnc_client()?;
    std::thread::spawn(move || {
        // continually generate HID reports until /dev/hidraw0 is opened and read
        while waiting.load(Ordering::Relaxed) {
            std::thread::sleep(Duration::from_secs(1));
            vnc_client.send_pointer_event(0x01u8, 234, 567).unwrap();
            vnc_client.send_pointer_event(0x01u8, 234, 568).unwrap();
        }
        vnc_client.disconnect().unwrap();
    });

    let output = vm
        .run_shell_command(
            // 7-byte HID reports (propolis::hw::usb::usbdev::vnc_tablet::REPORT_SIZE)
            "od -tx1 -w7 -N14 /dev/hidraw0",
        )
        .await?;
    waiting_outer.store(false, Ordering::Relaxed);

    assert!(
        output.contains("0000000 01"),
        "primary mouse button press (01) not found in raw HID event dump:\n{output}"
    );
}

#[phd_testcase]
async fn usb_tablet_migration(ctx: &TestCtx) {
    let mut config = ctx.vm_config_builder("usb_tablet_migration");
    let xhc_name: SpecKey = "xhc0".into();
    const PCI_DEV: u8 = 3;
    const USB_PORT: u8 = 2;
    config
        .xhci_controller(
            xhc_name.to_owned(),
            XhciController { pci_path: PciPath::new(0, PCI_DEV, 0)? },
        )
        .usb_device(UsbDevice {
            xhc_device: xhc_name,
            root_hub_port_num: USB_PORT,
            usb_device_type: UsbDeviceType::HidTablet,
        });

    let spec = config.vm_spec(ctx).await?;

    let mut vm0 = ctx.spawn_vm_with_spec(spec, None).await?;
    let mut vm1 = ctx
        .spawn_successor_vm("usb_tablet_migration_successor", &vm0, None)
        .await?;

    vm0.launch().await?;
    vm0.wait_to_boot().await?;

    // TODO: start running hidraw read with i/o redirected, send click

    vm1.migrate_from(&vm0, Uuid::new_v4(), MigrationTimeout::default()).await?;

    // TODO: send different click, kill hidraw read, check contents of stdout file
    todo!();
}
