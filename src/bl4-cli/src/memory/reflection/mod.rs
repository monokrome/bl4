//! UE5 Reflection Data Structures
//!
//! Types and functions for UE5 reflection system:
//! - UObject metadata (UObjectInfo)
//! - Property types (EPropertyType)
//! - Struct/Property/Enum info structures
//! - UClass discovery and metaclass analysis

mod types;
mod uclass;

pub use types::{
    EPropertyType, EnumInfo, PropertyInfo, StructInfo, UClassMetaclassInfo, UObjectInfo,
    UObjectOffsets,
};
pub use uclass::{discover_uclass_metaclass_exhaustive, find_all_uclasses};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_property_type_from_name() {
        assert_eq!(
            EPropertyType::from_name("IntProperty"),
            EPropertyType::IntProperty
        );
        assert_eq!(
            EPropertyType::from_name("BoolProperty"),
            EPropertyType::BoolProperty
        );
        assert_eq!(
            EPropertyType::from_name("StructProperty"),
            EPropertyType::StructProperty
        );
        assert_eq!(
            EPropertyType::from_name("ArrayProperty"),
            EPropertyType::ArrayProperty
        );
        assert_eq!(
            EPropertyType::from_name("MapProperty"),
            EPropertyType::MapProperty
        );
        assert_eq!(
            EPropertyType::from_name("UnknownType"),
            EPropertyType::Unknown
        );
    }

    #[test]
    fn test_property_type_aliases() {
        // ClassProperty maps to ObjectProperty
        assert_eq!(
            EPropertyType::from_name("ClassProperty"),
            EPropertyType::ObjectProperty
        );
        // SoftClassProperty maps to SoftObjectProperty
        assert_eq!(
            EPropertyType::from_name("SoftClassProperty"),
            EPropertyType::SoftObjectProperty
        );
        // MulticastInlineDelegateProperty maps to MulticastDelegateProperty
        assert_eq!(
            EPropertyType::from_name("MulticastInlineDelegateProperty"),
            EPropertyType::MulticastDelegateProperty
        );
    }

    #[test]
    fn test_property_type_to_usmap_id() {
        assert_eq!(EPropertyType::ByteProperty.to_usmap_id(), 0);
        assert_eq!(EPropertyType::BoolProperty.to_usmap_id(), 1);
        assert_eq!(EPropertyType::IntProperty.to_usmap_id(), 2);
        assert_eq!(EPropertyType::StructProperty.to_usmap_id(), 9);
        assert_eq!(EPropertyType::MapProperty.to_usmap_id(), 24);
        assert_eq!(EPropertyType::Unknown.to_usmap_id(), 255);
    }

    #[test]
    fn test_property_type_roundtrip() {
        let types = [
            "ByteProperty",
            "BoolProperty",
            "IntProperty",
            "FloatProperty",
            "ObjectProperty",
            "NameProperty",
            "StructProperty",
            "ArrayProperty",
        ];
        for name in types {
            let prop_type = EPropertyType::from_name(name);
            assert_ne!(prop_type, EPropertyType::Unknown, "Failed for {}", name);
            assert_ne!(prop_type.to_usmap_id(), 255, "Failed usmap id for {}", name);
        }
    }
}
