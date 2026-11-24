use super::detector::{revalidate, StorageAdapter, StorageDevice};
use crate::utils::errors::{Error, Result};
// No formatting path can bypass fresh identity validation and an exact typed acknowledgement.
pub fn format(
    adapter: &dyn StorageAdapter,
    expected: &StorageDevice,
    confirmation: &str,
) -> Result<()> {
    expected.safe()?;
    if confirmation != format!("ERASE {}", expected.id) {
        return Err(Error::Unsafe(
            "Type ERASE followed by the exact device identifier".into(),
        ));
    }
    let fresh = revalidate(adapter, expected)?;
    adapter.format(&fresh)
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    struct Mock {
        device: StorageDevice,
        called: Cell<bool>,
    }
    impl StorageAdapter for Mock {
        fn scan(&self) -> Result<Vec<StorageDevice>> {
            Ok(vec![self.device.clone()])
        }
        fn format(&self, _: &StorageDevice) -> Result<()> {
            self.called.set(true);
            Ok(())
        }
        fn eject(&self, _: &StorageDevice) -> Result<()> {
            Ok(())
        }
    }
    fn mock() -> Mock {
        Mock {
            device: StorageDevice {
                id: "sd1".into(),
                name: "mock".into(),
                mount: Some("/test".into()),
                capacity: 10,
                available: 10,
                filesystem: "vfat".into(),
                removable: true,
                system: false,
                identity: "serial1".into(),
            },
            called: Cell::new(false),
        }
    }
    #[test]
    fn requires_confirmation() {
        let m = mock();
        assert!(format(&m, &m.device, "").is_err());
        assert!(!m.called.get());
        format(&m, &m.device, "ERASE sd1").unwrap();
        assert!(m.called.get());
    }
    #[test]
    fn revalidates_identity() {
        let m = mock();
        let mut stale = m.device.clone();
        stale.identity = "old".into();
        assert!(format(&m, &stale, "ERASE sd1").is_err());
        assert!(!m.called.get());
    }
}
