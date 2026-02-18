// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use std::{
    sync::{
        atomic::{AtomicBool, AtomicU8, Ordering},
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

    // 7-byte HID reports (propolis::hw::usb::usbdev::vnc_tablet::REPORT_SIZE)
    let output = vm.run_shell_command("od -tx1 -w7 -N14 /dev/hidraw0").await?;
    waiting_outer.store(false, Ordering::Relaxed);

    assert!(
        output.contains("0000000 01"),
        "primary mouse button press (01) not found in raw HID event dump:\n{output}"
    );
}

// tests that migration succeeds while the xHC and USB tablet's interrupt
// endpoint are in use.
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

    const VM0_HIDRAW_READ: u8 = 0;
    const VM1_HIDRAW_READ: u8 = 1;
    const DONE: u8 = 2;
    let waiting_outer = Arc::new(AtomicU8::new(VM0_HIDRAW_READ));
    let waiting_0 = waiting_outer.clone();
    let waiting_1 = waiting_outer.clone();

    let mut vnc_client = vm0.vnc_client()?;
    std::thread::spawn(move || {
        // continually generate HID reports until /dev/hidraw0 is opened and read
        while waiting_0.load(Ordering::Relaxed) == VM0_HIDRAW_READ {
            std::thread::sleep(Duration::from_secs(1));
            vnc_client.send_pointer_event(0x01u8, 234, 567).unwrap();
            vnc_client.send_pointer_event(0x01u8, 234, 568).unwrap();
        }
        vnc_client.disconnect().unwrap();
    });

    // bg: keep hidraw0 open so USB endpoint stays active during migration
    vm0.run_shell_command("od -tx1 -w7 /dev/hidraw0 &> /tmp/hid.txt &").await?;

    let mut retries = 0;
    while retries < 10 {
        if vm0.run_shell_command("wc -l < /tmp/hid.txt").await? == "0" {
            std::thread::sleep(Duration::from_secs(1));
            retries += 1;
        } else {
            break;
        }
    }
    assert_ne!(retries, 10);

    waiting_outer.store(VM1_HIDRAW_READ, Ordering::Relaxed);

    vm1.migrate_from(&vm0, Uuid::new_v4(), MigrationTimeout::default()).await?;

    let mut vnc_client = vm1.vnc_client()?;
    std::thread::spawn(move || {
        // send slightly different events to vm1 while hidraw0 is still open
        // (different mouse button, so we can tell which events were sent
        // before/after migration in the HID report dump)
        while waiting_1.load(Ordering::Relaxed) == VM1_HIDRAW_READ {
            std::thread::sleep(Duration::from_secs(1));
            vnc_client.send_pointer_event(0x02u8, 234, 567).unwrap();
            vnc_client.send_pointer_event(0x02u8, 234, 568).unwrap();
        }
        vnc_client.disconnect().unwrap();
    });

    // retries = 0;
    // while retries < 10 {
    //     if u32::from_str_radix(
    //         &vm1.run_shell_command("wc -l < /tmp/hid.txt").await?,
    //         10,
    //     )
    //     .unwrap()
    //         < 3
    //     {
    std::thread::sleep(Duration::from_secs(5));
    //         retries += 1;
    //     } else {
    //         break;
    //     }
    // }
    // assert_ne!(retries, 10);
    waiting_outer.store(DONE, Ordering::Relaxed);

    // kill hidraw0 read
    vm1.run_shell_command("pkill od").await?;

    // check contents of stdout file
    let output = vm1.run_shell_command("cat /tmp/hid.txt").await?;
    assert_eq!("", output); // TODO
}
