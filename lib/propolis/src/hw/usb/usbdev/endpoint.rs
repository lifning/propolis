// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

pub mod control;
pub mod interrupt;

pub mod migrate {
    use serde::{Deserialize, Serialize};

    #[derive(Serialize, Deserialize)]
    pub enum EndpointV1 {
        Control {
            current_setup: Option<u64>,
            payload: Option<Vec<u8>>,
            bytes_transferred: usize,
        },
        InterruptIn {},
    }
}
