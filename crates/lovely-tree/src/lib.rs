pub mod types;

pub use types::{ElementUuid, NodeId};

#[cfg(test)]
mod tests {
    use super::types::*;

    #[test]
    fn element_uuid_roundtrips_through_string() {
        let u = ElementUuid::new_v4();
        let s = u.to_string();
        let parsed: ElementUuid = s.parse().unwrap();
        assert_eq!(u, parsed);
    }

    #[test]
    fn nil_is_nil() {
        assert_eq!(ElementUuid::nil().to_string(), uuid::Uuid::nil().to_string());
    }
}
