// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

pub mod control;
pub mod interrupt;

pub mod migrate {
    use serde::{Deserialize, Serialize};

    use super::{
        control::migrate::ControlEndpointV1,
        interrupt::migrate::InterruptInEndpointV1,
    };

    #[derive(Serialize, Deserialize)]
    pub enum EndpointV1 {
        Control(ControlEndpointV1),
        InterruptIn(InterruptInEndpointV1),
    }
}

#[cfg(test)]
mod test {
    use std::sync::Arc;

    use crate::{hw::pci, vmm::PhysMap};

    pub(crate) fn test_pci_state() -> Arc<pci::DeviceState> {
        let mut pci_state = pci::Builder::new(pci::Ident::default())
            .add_cap_msix(pci::BarN::BAR0, 1)
            .finish();
        let mut phys_map = PhysMap::new_test(16 * 1024);
        phys_map.add_test_mem("guest-ram".to_string(), 0, 16 * 1024).unwrap();
        pci_state.acc_mem = phys_map.finalize();
        Arc::new(pci_state)
    }
}
