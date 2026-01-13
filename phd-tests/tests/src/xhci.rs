// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use std::sync::Arc;

use phd_testcase::*;
use propolis_client::instance_spec::{
    PciPath, SpecKey, UsbDevice, UsbDeviceType, XhciController,
};

#[phd_testcase]
async fn usb_device_enumerates(ctx: &Framework) {
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
            usb_device_type: UsbDeviceType::HidTablet,
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
async fn usb_tablet_vnc_pointer_events(ctx: &Framework) {
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
            usb_device_type: UsbDeviceType::HidTablet,
        });

    let spec = config.vm_spec(ctx).await?;

    let mut vm = ctx.spawn_vm_with_spec(spec, None).await?;
    if !vm.guest_os_kind().is_linux() {
        phd_skip!("USB/VNC test uses evtest to enumerate devices and receive HID events");
    }

    vm.launch().await?;
    vm.wait_to_boot().await?;

    let device_node = vm.run_shell_command("echo '' | evtest 2>&1 | grep 'Propolis HID Tablet' | head -1 | cut -d: -f1").await?;
    assert!(device_node.starts_with("/dev/input/event"));

    let vm = Arc::new(vm);
    let task_vm = vm.to_owned();

    let task = tokio::spawn(async move {
        task_vm
        .run_shell_command(
            &format!("evtest {device_node} | grep --line-buffered '^Event:.*(BTN_LEFT)' | head -1")
        ).await
    });
    let mut vnc_client = vm.vnc_client()?;
    vnc_client.send_pointer_event(0x01u8, 234, 567)?;
    vnc_client.disconnect()?;

    let output = task.await??;
    assert!(output.contains("BTN_LEFT"));
}
