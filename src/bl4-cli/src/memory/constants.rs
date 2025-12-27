//! BL4 UE5.5 Memory Layout Constants
//!
//! SDK offsets and structure layouts for Borderlands 4's Unreal Engine 5.5.

// Allow dead code for SDK documentation constants - these are reference values
// for UE5 structure layouts that may be used in future implementations.
#![allow(dead_code)]

/// Windows PE image base address for BL4 executable
pub const PE_IMAGE_BASE: usize = 0x140000000;

/// PE header offset (e_lfanew) within DOS header
pub const PE_HEADER_OFFSET_LOCATION: usize = 0x3C;

/// Maximum valid PE header offset
pub const PE_HEADER_MAX_OFFSET: usize = 0x1000;

// -- UObject Layout (VERIFIED from SDK dump) --
// Source: BL4 SDK dump with Denuvo unpacking
//
// class UObject {
//   uint64_t vTable;        // +0x00 (8 bytes)
//   int32_t Flags;          // +0x08 (4 bytes)
//   int32_t InternalIndex;  // +0x0C (4 bytes)
//   class UClass *Class;    // +0x10 (8 bytes) - ClassPrivate
//   class FName Name;       // +0x18 (8 bytes) - NamePrivate (ComparisonIndex + Number)
//   class UObject *Outer;   // +0x20 (8 bytes) - OuterPrivate
// }; // Size: 0x28
//
// This matches standard UE5 layout (not custom like previously suspected).

/// VTable pointer offset in UObjectBase
pub const UOBJECT_VTABLE_OFFSET: usize = 0x00;

/// Object flags offset in UObjectBase
pub const UOBJECT_FLAGS_OFFSET: usize = 0x08;

/// Internal index offset in UObjectBase
pub const UOBJECT_INTERNAL_INDEX_OFFSET: usize = 0x0C;

/// ClassPrivate (UClass*) offset in UObjectBase
/// VERIFIED: Standard UE5 layout at +0x10
pub const UOBJECT_CLASS_OFFSET: usize = 0x10;

/// NamePrivate (FName) offset in UObjectBase
/// The FName ComparisonIndex is the first 4 bytes, Number is next 4 bytes
pub const UOBJECT_NAME_OFFSET: usize = 0x18;

/// OuterPrivate (UObject*) offset in UObjectBase
pub const UOBJECT_OUTER_OFFSET: usize = 0x20;

/// Minimum bytes to read for UObject header
pub const UOBJECT_HEADER_SIZE: usize = 0x28;

// -- SDK Data Pointers (offsets from PE_IMAGE_BASE) --
// These are offsets, add to PE_IMAGE_BASE (0x140000000) to get virtual address
// Updated for latest patch (Nov 2025)

/// GUObjectArray offset from image base (VA = 0x1513878f0)
pub const GOBJECTS_OFFSET: usize = 0x113878f0;

/// GNames (FNamePool) offset from image base (VA = 0x1512a1c80)
pub const GNAMES_OFFSET: usize = 0x112a1c80;

/// GWorld offset from image base (VA = 0x151532cb8)
pub const GWORLD_OFFSET: usize = 0x11532cb8;

/// ProcessEvent function offset from image base
pub const PROCESS_EVENT_OFFSET: usize = 0x14f7010;

/// ProcessEvent vtable index
pub const PROCESS_EVENT_VTABLE_INDEX: usize = 0x49;

// -- UField Layout --
// class UField : public UObject {
//   class UField *Next;  // +0x28
// }; // Size: 0x30

/// UField::Next offset
pub const UFIELD_NEXT_OFFSET: usize = 0x28;

/// UField size
pub const UFIELD_SIZE: usize = 0x30;

// -- UStruct Layout --
// class UStruct : public UField {
//   char pad_0030[16];      // +0x30
//   class UStruct *Super;   // +0x40
//   class UField *Children; // +0x48
//   char pad_0050[8];       // +0x50
//   int32_t Size;           // +0x58
//   int16_t MinAlignment;   // +0x5C
//   char pad_005E[82];      // +0x5E
// }; // Size: 0xB0

/// UStruct::Super offset
pub const USTRUCT_SUPER_OFFSET: usize = 0x40;

/// UStruct::Children offset (UField* linked list for UFunctions, not properties!)
pub const USTRUCT_CHILDREN_OFFSET: usize = 0x48;

/// UStruct::ChildProperties offset (FField* linked list for FProperty - UE5 only)
pub const USTRUCT_CHILDPROPERTIES_OFFSET: usize = 0x50;

/// UStruct::Size offset
pub const USTRUCT_SIZE_OFFSET: usize = 0x58;

/// UStruct::MinAlignment offset
pub const USTRUCT_MINALIGNMENT_OFFSET: usize = 0x5C;

/// UStruct total size
pub const USTRUCT_SIZE: usize = 0xB0;

// -- UClass Layout --
// class UClass : public UStruct {
//   char pad_00B0[96];              // +0xB0
//   class UObject *DefaultObject;  // +0x110
//   char pad_0118[232];             // +0x118
// }; // Size: 0x200

/// UClass::DefaultObject offset
pub const UCLASS_DEFAULT_OBJECT_OFFSET: usize = 0x110;

/// UClass total size
pub const UCLASS_SIZE: usize = 0x200;

// -- FField Layout (base class for FProperty) --
// UE5 changed from UProperty to FProperty (no longer UObject-derived)

/// FField::ClassPrivate offset - pointer to FFieldClass
pub const FFIELD_CLASS_OFFSET: usize = 0x00;

/// FField::Owner offset - FFieldVariant (UObject* or FField*)
pub const FFIELD_OWNER_OFFSET: usize = 0x08;

/// FField::Next offset - pointer to next FField in linked list
pub const FFIELD_NEXT_OFFSET: usize = 0x18;

/// FField::NamePrivate offset - FName
pub const FFIELD_NAME_OFFSET: usize = 0x20;

/// FField::FlagsPrivate offset - EObjectFlags
pub const FFIELD_FLAGS_OFFSET: usize = 0x28;

// -- FProperty Layout (extends FField) --

/// FProperty::ArrayDim offset
pub const FPROPERTY_ARRAYDIM_OFFSET: usize = 0x30;

/// FProperty::ElementSize offset
pub const FPROPERTY_ELEMENTSIZE_OFFSET: usize = 0x34;

/// FProperty::PropertyFlags offset (EPropertyFlags, 8 bytes)
pub const FPROPERTY_PROPERTYFLAGS_OFFSET: usize = 0x38;

/// FProperty::RepIndex offset
pub const FPROPERTY_REPINDEX_OFFSET: usize = 0x40;

/// FProperty::Offset_Internal offset - offset within struct
pub const FPROPERTY_OFFSET_OFFSET: usize = 0x4C;

/// FProperty total size (base, without type-specific data)
pub const FPROPERTY_BASE_SIZE: usize = 0x78;

// -- FFieldClass Layout --

/// FFieldClass::Name offset - FName identifying the property type
pub const FFIELDCLASS_NAME_OFFSET: usize = 0x00;

/// FFieldClass::Id offset - unique ID
pub const FFIELDCLASS_ID_OFFSET: usize = 0x08;

/// FFieldClass::CastFlags offset
pub const FFIELDCLASS_CASTFLAGS_OFFSET: usize = 0x10;

/// FFieldClass::ClassFlags offset
pub const FFIELDCLASS_CLASSFLAGS_OFFSET: usize = 0x18;

/// FFieldClass::SuperClass offset - parent FFieldClass*
pub const FFIELDCLASS_SUPERCLASS_OFFSET: usize = 0x20;

// -- Component Offsets --

/// ComponentToWorld offset in USceneComponent
pub const COMPONENT_TO_WORLD_OFFSET: usize = 0x240;

/// Bones TArray offset in USkinnedMeshComponent
pub const BONES_OFFSET: usize = 0x6A8;

/// Bones2 TArray offset in USkinnedMeshComponent
pub const BONES2_OFFSET: usize = 0x6B8;

// -- Actor Offsets --

/// RootComponent offset in AActor
pub const ACTOR_ROOT_COMPONENT_OFFSET: usize = 0x1C8;

// -- Controller Offsets --

/// PlayerState offset in AController
pub const CONTROLLER_PLAYER_STATE_OFFSET: usize = 0x398;

/// Pawn offset in AController
pub const CONTROLLER_PAWN_OFFSET: usize = 0x3D0;

/// Character offset in AController
pub const CONTROLLER_CHARACTER_OFFSET: usize = 0x3E0;

// -- PlayerController Offsets --

/// AcknowledgedPawn offset in APlayerController
pub const PLAYERCONTROLLER_ACKNOWLEDGED_PAWN_OFFSET: usize = 0x438;

/// PlayerCameraManager offset in APlayerController
pub const PLAYERCONTROLLER_CAMERA_MANAGER_OFFSET: usize = 0x448;

/// CheatManager offset in APlayerController
pub const PLAYERCONTROLLER_CHEAT_MANAGER_OFFSET: usize = 0x4F8;

/// CheatClass offset in APlayerController
pub const PLAYERCONTROLLER_CHEAT_CLASS_OFFSET: usize = 0x500;

// -- World Offsets --

/// PersistentLevel offset in UWorld
pub const WORLD_PERSISTENT_LEVEL_OFFSET: usize = 0x30;

/// GameState offset in UWorld
pub const WORLD_GAME_STATE_OFFSET: usize = 0x178;

/// Levels TArray offset in UWorld
pub const WORLD_LEVELS_OFFSET: usize = 0x190;

/// OwningGameInstance offset in UWorld
pub const WORLD_GAME_INSTANCE_OFFSET: usize = 0x1F0;

// -- ULevel Offsets --

/// Actors TArray offset in ULevel
pub const LEVEL_ACTORS_OFFSET: usize = 0xA0;

// -- Character Offsets --

/// Mesh offset in ACharacter
pub const CHARACTER_MESH_OFFSET: usize = 0x428;

/// CharacterMovement offset in ACharacter
pub const CHARACTER_MOVEMENT_OFFSET: usize = 0x430;

// -- Pawn Offsets --

/// PlayerState offset in APawn
pub const PAWN_PLAYER_STATE_OFFSET: usize = 0x3B0;

/// Controller offset in APawn
pub const PAWN_CONTROLLER_OFFSET: usize = 0x3C0;

// -- PlayerState Offsets --

/// PawnPrivate offset in APlayerState
pub const PLAYERSTATE_PAWN_OFFSET: usize = 0x408;

/// PlayerNamePrivate offset in APlayerState
pub const PLAYERSTATE_NAME_OFFSET: usize = 0x428;

// -- Oak Character Offsets (BL4 specific) --

/// DamageState offset in AOakCharacter
pub const OAK_CHARACTER_DAMAGE_STATE_OFFSET: usize = 0x4038;

/// HealthState offset in AOakCharacter
pub const OAK_CHARACTER_HEALTH_STATE_OFFSET: usize = 0x4640;

/// HealthCondition offset in AOakCharacter
pub const OAK_CHARACTER_HEALTH_CONDITION_OFFSET: usize = 0x5B98;

/// ActiveWeapons offset in AOakCharacter
pub const OAK_CHARACTER_ACTIVE_WEAPONS_OFFSET: usize = 0x5F50;

/// DownState offset in AOakCharacter
pub const OAK_CHARACTER_DOWN_STATE_OFFSET: usize = 0x6F40;

/// AmmoRegenerate offset in AOakCharacter
pub const OAK_CHARACTER_AMMO_REGEN_OFFSET: usize = 0x95E8;

// -- Oak PlayerController Offsets --

/// OakCharacter offset in AOakPlayerController
pub const OAK_PLAYERCONTROLLER_CHARACTER_OFFSET: usize = 0x0F20;

/// PersonalVehicleState offset in AOakPlayerController
pub const OAK_PLAYERCONTROLLER_VEHICLE_STATE_OFFSET: usize = 0x3880;

// -- Currency Manager Offsets --

/// Currencies TArray offset in UGbxCurrencyManager
pub const CURRENCY_MANAGER_CURRENCIES_OFFSET: usize = 0x30;

// -- FName Encoding --

/// FNamePool header address discovered from BL4 dump
pub const FNAMEPOOL_HEADER_ADDR: usize = 0x1513b0c80;

/// Block index shift for FName ComparisonIndex
pub const FNAME_BLOCK_SHIFT: u32 = 16;

/// Block offset mask for FName ComparisonIndex
pub const FNAME_OFFSET_MASK: u32 = 0xFFFF;

/// FName "Class" index (block 0, byte offset 1176 = 0x498, index = 1176/2 = 588)
pub const FNAME_CLASS_INDEX: u32 = 588;

/// Known FName indices for core UE types (block 0)
pub const FNAME_SCRIPTSTRUCT_INDEX: u32 = 92;
pub const FNAME_FUNCTION_INDEX: u32 = 93;
pub const FNAME_PACKAGE_INDEX: u32 = 126;
pub const FNAME_OBJECT_INDEX: u32 = 86;

// -- GUObjectArray --

/// Chunk size for GUObjectArray (objects per chunk)
pub const GUOBJECTARRAY_CHUNK_SIZE: usize = 65536;

// -- UClass Metaclass --
// With the VERIFIED SDK layout (ClassPrivate at +0x10, NamePrivate at +0x18),
// we can now properly search for UClass metaclass using the correct offsets.
// The previous anomaly was due to searching at wrong offsets (+0x08, +0x18).
//
// UClass metaclass characteristics:
// - ClassPrivate (+0x10) points to ITSELF (self-referential)
// - NamePrivate (+0x18) has FName index 588 ("Class")
// - vtable[0] points to code section
//
// TODO: Re-run metaclass scan with correct SDK offsets to find true UClass.

/// UClass metaclass address - PLACEHOLDER (needs re-scan with correct offsets)
/// This was found with wrong offsets; re-scan needed.
pub const UCLASS_METACLASS_ADDR: usize = 0x1514d3ed0;

/// vtable address of this self-referential object (for verification)
pub const UCLASS_METACLASS_VTABLE: usize = 0x14fd8a240;

// -- Pointer Validation --

/// Minimum valid pointer address (Windows user mode)
pub const MIN_VALID_POINTER: usize = 0x10000;

/// Maximum valid pointer address
pub const MAX_VALID_POINTER: usize = 0x800000000000;

/// Minimum vtable address (in executable range)
pub const MIN_VTABLE_ADDR: usize = 0x140000000;

/// Maximum vtable address (in executable data sections)
pub const MAX_VTABLE_ADDR: usize = 0x175000000;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pe_image_base_is_standard_x64() {
        // Windows x64 default image base
        assert_eq!(PE_IMAGE_BASE, 0x140000000);
    }

    #[test]
    fn test_uobject_layout_offsets() {
        // Verify UObject structure layout is consistent
        assert_eq!(UOBJECT_VTABLE_OFFSET, 0x00);
        assert_eq!(UOBJECT_FLAGS_OFFSET, 0x08);
        assert_eq!(UOBJECT_INTERNAL_INDEX_OFFSET, 0x0C);
        assert_eq!(UOBJECT_CLASS_OFFSET, 0x10);
        assert_eq!(UOBJECT_NAME_OFFSET, 0x18);
        assert_eq!(UOBJECT_OUTER_OFFSET, 0x20);
        assert_eq!(UOBJECT_HEADER_SIZE, 0x28);
    }

    #[test]
    fn test_ustruct_layout_offsets() {
        // UStruct extends UField, so offsets start at 0x30
        assert!(USTRUCT_SUPER_OFFSET > UFIELD_SIZE);
        assert_eq!(USTRUCT_SUPER_OFFSET, 0x40);
        assert_eq!(USTRUCT_CHILDREN_OFFSET, 0x48);
        assert_eq!(USTRUCT_CHILDPROPERTIES_OFFSET, 0x50);
        assert_eq!(USTRUCT_SIZE_OFFSET, 0x58);
    }

    #[test]
    fn test_fproperty_layout_offsets() {
        // FProperty extends FField
        assert!(FPROPERTY_ARRAYDIM_OFFSET > FFIELD_FLAGS_OFFSET);
        assert_eq!(FPROPERTY_ARRAYDIM_OFFSET, 0x30);
        assert_eq!(FPROPERTY_OFFSET_OFFSET, 0x4C);
    }

    #[test]
    fn test_sdk_offsets_are_reasonable() {
        // SDK offsets should be positive and less than 2GB
        assert!(GOBJECTS_OFFSET > 0);
        assert!(GOBJECTS_OFFSET < 0x80000000);
        assert!(GNAMES_OFFSET > 0);
        assert!(GNAMES_OFFSET < 0x80000000);
    }

    #[test]
    fn test_pointer_validation_ranges() {
        // Min pointer should exclude NULL and low addresses
        assert!(MIN_VALID_POINTER > 0);
        assert_eq!(MIN_VALID_POINTER, 0x10000);

        // Max pointer should be in 48-bit address space
        assert!(MAX_VALID_POINTER <= 0x800000000000);

        // vtable range should be within executable area
        assert!(MIN_VTABLE_ADDR >= PE_IMAGE_BASE);
        assert!(MAX_VTABLE_ADDR > MIN_VTABLE_ADDR);
    }

    #[test]
    fn test_fname_constants() {
        // FName block shift determines entries per block (2^16 = 65536)
        assert_eq!(FNAME_BLOCK_SHIFT, 16);
        assert_eq!(FNAME_OFFSET_MASK, 0xFFFF);

        // Known FName indices should be small positive values
        assert!(FNAME_CLASS_INDEX > 0);
        assert!(FNAME_CLASS_INDEX < 1000);
        assert!(FNAME_OBJECT_INDEX > 0);
        assert!(FNAME_OBJECT_INDEX < 1000);
    }

    #[test]
    fn test_guobjectarray_chunk_size() {
        // Chunk size should be 64K (65536 = 2^16)
        assert_eq!(GUOBJECTARRAY_CHUNK_SIZE, 65536);
        assert_eq!(GUOBJECTARRAY_CHUNK_SIZE, 1 << FNAME_BLOCK_SHIFT);
    }

    #[test]
    fn test_uclass_size_includes_default_object() {
        // UClass::DefaultObject should be at a valid offset
        assert!(UCLASS_DEFAULT_OBJECT_OFFSET > USTRUCT_SIZE);
        assert!(UCLASS_DEFAULT_OBJECT_OFFSET < UCLASS_SIZE);
    }
}
