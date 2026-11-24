use crate::utils::errors::{Error, Result};
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProgrammerDevice {
    pub id: String,
    pub serial: Option<String>,
    pub description: String,
    pub accessible: bool,
    pub identification: String,
}
pub fn compatible_interface(vid: u16, pid: u16) -> bool {
    vid == 0x0403 && pid == 0x6010
}
pub fn scan() -> Result<Vec<ProgrammerDevice>> {
    let devices = rusb::devices().map_err(|e| Error::Hardware(e.to_string()))?;
    let mut out = Vec::new();
    for device in devices.iter() {
        let Ok(desc) = device.device_descriptor() else {
            continue;
        };
        if !compatible_interface(desc.vendor_id(), desc.product_id()) {
            continue;
        }
        let handle = device.open();
        let serial = handle
            .as_ref()
            .ok()
            .and_then(|h| h.read_serial_number_string_ascii(&desc).ok());
        let product = handle
            .as_ref()
            .ok()
            .and_then(|h| h.read_product_string_ascii(&desc).ok())
            .unwrap_or_else(|| "FT2232-compatible programmer".into());
        out.push(ProgrammerDevice {
            id: format!("{}:{}", device.bus_number(), device.address()),
            serial,
            description: product,
            accessible: handle.is_ok(),
            identification: "Compatible USB programmer; board not positively identified".into(),
        });
    }
    Ok(out)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn no_generic_serial_identification() {
        assert!(compatible_interface(0x0403, 0x6010));
        assert!(!compatible_interface(0x0403, 0x6001));
        assert!(!compatible_interface(0x1a86, 0x7523));
    }
}
