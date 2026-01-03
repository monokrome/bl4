//! UE5 Reflection Type Definitions
//!
//! Core types for UE5 reflection system:
//! - UObject metadata
//! - Property types
//! - Struct/Property/Enum info structures

#![allow(dead_code)]

use super::super::constants::*;

/// Basic UObject information
#[allow(dead_code)]
pub struct UObjectInfo {
    pub address: usize,
    pub class_ptr: usize,
    pub name_index: u32,
    pub name: String,
    pub class_name: String,
}

/// Property type enumeration for usmap
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EPropertyType {
    ByteProperty,
    BoolProperty,
    IntProperty,
    FloatProperty,
    ObjectProperty,
    NameProperty,
    DelegateProperty,
    DoubleProperty,
    ArrayProperty,
    StructProperty,
    StrProperty,
    TextProperty,
    InterfaceProperty,
    MulticastDelegateProperty,
    WeakObjectProperty,
    LazyObjectProperty,
    AssetObjectProperty,
    SoftObjectProperty,
    UInt64Property,
    UInt32Property,
    UInt16Property,
    Int64Property,
    Int16Property,
    Int8Property,
    MapProperty,
    SetProperty,
    EnumProperty,
    FieldPathProperty,
    OptionalProperty,
    Unknown,
}

impl EPropertyType {
    pub fn from_name(name: &str) -> Self {
        match name {
            "ByteProperty" => Self::ByteProperty,
            "BoolProperty" => Self::BoolProperty,
            "IntProperty" => Self::IntProperty,
            "FloatProperty" => Self::FloatProperty,
            "ObjectProperty" => Self::ObjectProperty,
            "NameProperty" => Self::NameProperty,
            "DelegateProperty" => Self::DelegateProperty,
            "DoubleProperty" => Self::DoubleProperty,
            "ArrayProperty" => Self::ArrayProperty,
            "StructProperty" => Self::StructProperty,
            "StrProperty" => Self::StrProperty,
            "TextProperty" => Self::TextProperty,
            "InterfaceProperty" => Self::InterfaceProperty,
            "MulticastDelegateProperty"
            | "MulticastInlineDelegateProperty"
            | "MulticastSparseDelegateProperty" => Self::MulticastDelegateProperty,
            "WeakObjectProperty" => Self::WeakObjectProperty,
            "LazyObjectProperty" => Self::LazyObjectProperty,
            "AssetObjectProperty" => Self::AssetObjectProperty,
            "SoftObjectProperty" => Self::SoftObjectProperty,
            "UInt64Property" => Self::UInt64Property,
            "UInt32Property" => Self::UInt32Property,
            "UInt16Property" => Self::UInt16Property,
            "Int64Property" => Self::Int64Property,
            "Int16Property" => Self::Int16Property,
            "Int8Property" => Self::Int8Property,
            "MapProperty" => Self::MapProperty,
            "SetProperty" => Self::SetProperty,
            "EnumProperty" => Self::EnumProperty,
            "FieldPathProperty" => Self::FieldPathProperty,
            "OptionalProperty" => Self::OptionalProperty,
            "ClassProperty" => Self::ObjectProperty, // ClassProperty is a subtype of ObjectProperty
            "SoftClassProperty" => Self::SoftObjectProperty,
            _ => Self::Unknown,
        }
    }

    /// Get the usmap type ID
    pub fn to_usmap_id(&self) -> u8 {
        match self {
            Self::ByteProperty => 0,
            Self::BoolProperty => 1,
            Self::IntProperty => 2,
            Self::FloatProperty => 3,
            Self::ObjectProperty => 4,
            Self::NameProperty => 5,
            Self::DelegateProperty => 6,
            Self::DoubleProperty => 7,
            Self::ArrayProperty => 8,
            Self::StructProperty => 9,
            Self::StrProperty => 10,
            Self::TextProperty => 11,
            Self::InterfaceProperty => 12,
            Self::MulticastDelegateProperty => 13,
            Self::WeakObjectProperty => 14,
            Self::LazyObjectProperty => 15,
            Self::AssetObjectProperty => 16,
            Self::SoftObjectProperty => 17,
            Self::UInt64Property => 18,
            Self::UInt32Property => 19,
            Self::UInt16Property => 20,
            Self::Int64Property => 21,
            Self::Int16Property => 22,
            Self::Int8Property => 23,
            Self::MapProperty => 24,
            Self::SetProperty => 25,
            Self::EnumProperty => 26,
            Self::FieldPathProperty => 27,
            Self::OptionalProperty => 28,
            Self::Unknown => 255,
        }
    }
}

/// Property information extracted from FProperty
#[derive(Debug, Clone)]
pub struct PropertyInfo {
    /// Property name
    pub name: String,
    /// Property type (e.g., "IntProperty", "StructProperty")
    pub property_type: EPropertyType,
    /// Property type name string
    pub type_name: String,
    /// Array dimension (1 for regular, >1 for fixed arrays)
    pub array_dim: i32,
    /// Element size in bytes
    pub element_size: i32,
    /// Property flags (EPropertyFlags)
    pub property_flags: u64,
    /// Offset within struct
    pub offset: i32,
    /// For StructProperty: the struct type name
    pub struct_type: Option<String>,
    /// For EnumProperty: the enum type name
    pub enum_type: Option<String>,
    /// For ArrayProperty/SetProperty/MapProperty: inner property type
    pub inner_type: Option<Box<PropertyInfo>>,
    /// For MapProperty: value property type
    pub value_type: Option<Box<PropertyInfo>>,
}

/// UStruct/UClass with extracted properties
#[derive(Debug, Clone)]
pub struct StructInfo {
    /// Address of the UStruct in memory
    pub address: usize,
    /// Name of the struct/class
    pub name: String,
    /// Super class/struct name (if any)
    pub super_name: Option<String>,
    /// Properties in this struct
    pub properties: Vec<PropertyInfo>,
    /// Size of the struct in bytes
    pub struct_size: i32,
    /// Whether this is a UClass (vs UScriptStruct)
    pub is_class: bool,
}

/// Enum information
#[derive(Debug, Clone)]
pub struct EnumInfo {
    /// Address of the UEnum in memory
    pub address: usize,
    /// Name of the enum
    pub name: String,
    /// Enum values (name, value)
    pub values: Vec<(String, i64)>,
}

/// UE5 UObject offsets
/// These vary by engine version but are consistent within a build
pub struct UObjectOffsets {
    /// Offset of ClassPrivate (UClass*) in UObject
    pub class_offset: usize,
    /// Offset of NamePrivate (FName) in UObject
    pub name_offset: usize,
    /// Offset of OuterPrivate (UObject*) in UObject
    pub outer_offset: usize,
}

impl Default for UObjectOffsets {
    fn default() -> Self {
        // Uses constants defined at top of file - see UOBJECT_* for documentation
        Self {
            class_offset: UOBJECT_CLASS_OFFSET,
            name_offset: UOBJECT_NAME_OFFSET,
            outer_offset: UOBJECT_OUTER_OFFSET,
        }
    }
}

/// Result of UClass metaclass discovery
#[derive(Debug, Clone)]
pub struct UClassMetaclassInfo {
    /// Address of the UClass metaclass
    pub address: usize,
    /// Vtable address
    pub vtable: usize,
    /// Offset where ClassPrivate was found
    pub class_offset: usize,
    /// Offset where NamePrivate was found
    pub name_offset: usize,
    /// FName index
    pub fname_index: u32,
    /// Resolved name
    pub name: String,
}
