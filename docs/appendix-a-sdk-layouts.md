# Appendix A: SDK Class Layouts

This appendix provides detailed memory layouts for BL4's core classes, extracted from SDK analysis. These are essential for memory hacking, cheat development, or deep reverse engineering.

---

## Core UE5 Types

### FName (8 bytes)

```cpp
struct FName {
    int32_t ComparisonIndex;  // 0x00 - Index into GNames pool
    int32_t Number;           // 0x04 - Instance number (e.g., "Actor_5")
};
```

### FVector (24 bytes)

```cpp
struct FVector {
    double X;  // 0x00
    double Y;  // 0x08
    double Z;  // 0x10
};
```

!!! warning
    UE5 uses `double` (8 bytes) for vectors, not `float` like UE4. This is a common source of offset calculation errors.

### FRotator (24 bytes)

```cpp
struct FRotator {
    double Pitch;  // 0x00
    double Yaw;    // 0x08
    double Roll;   // 0x10
};
```

### FQuat (32 bytes)

```cpp
struct FQuat {
    double X;  // 0x00
    double Y;  // 0x08
    double Z;  // 0x10
    double W;  // 0x18
};
```

### FTransform (96 bytes)

```cpp
struct FTransform {
    FQuat Rotation;       // 0x00 (32 bytes)
    FVector Translation;  // 0x20 (24 bytes)
    char pad[8];          // 0x38
    FVector Scale3D;      // 0x40 (24 bytes)
    char pad[8];          // 0x58
};  // Total: 0x60
```

### TArray (16 bytes)

```cpp
template<typename T>
struct TArray {
    T* Data;          // 0x00 - Pointer to element array
    int32_t Count;    // 0x08 - Current element count
    int32_t Max;      // 0x0C - Allocated capacity
};
```

### FString (16 bytes)

```cpp
struct FString {
    wchar_t* Data;    // 0x00 - Wide string pointer
    int32_t Count;    // 0x08 - String length
    int32_t Max;      // 0x0C - Buffer capacity
};
```

---

## UObject Hierarchy

### UObject (40 bytes)

The base class for all Unreal objects.

```cpp
class UObject {
    void* VTable;             // 0x00 - Virtual function table
    int32_t ObjectFlags;      // 0x08 - RF_* flags
    int32_t InternalIndex;    // 0x0C - Index in GUObjectArray
    UClass* ClassPrivate;     // 0x10 - Pointer to this object's class
    FName NamePrivate;        // 0x18 - Object's name
    UObject* OuterPrivate;    // 0x20 - Parent/container object
};  // Total: 0x28
```

| Offset | Size | Field | Purpose |
|--------|------|-------|---------|
| 0x00 | 8 | VTable | Points to virtual function table |
| 0x08 | 4 | ObjectFlags | Object state flags |
| 0x0C | 4 | InternalIndex | Position in GUObjectArray |
| 0x10 | 8 | ClassPrivate | Pointer to UClass |
| 0x18 | 8 | NamePrivate | FName (index + number) |
| 0x20 | 8 | OuterPrivate | Package/container pointer |

### UField (48 bytes)

```cpp
class UField : public UObject {
    UField* Next;  // 0x28 - Next field in chain
};
```

### UStruct (176 bytes)

```cpp
class UStruct : public UField {
    char pad[16];              // 0x30
    UStruct* SuperStruct;      // 0x40 - Parent class/struct
    UField* Children;          // 0x48 - First child field (legacy)
    FField* ChildProperties;   // 0x50 - Property linked list (UE5)
    int32_t PropertiesSize;    // 0x58 - Total size of properties
    int16_t MinAlignment;      // 0x5C
    char pad[82];              // 0x5E
};  // Total: 0xB0
```

### UClass (512 bytes)

```cpp
class UClass : public UStruct {
    char pad[96];              // 0xB0
    UObject* ClassDefaultObject;  // 0x110 - CDO pointer
    char pad[232];             // 0x118
};  // Total: 0x200
```

---

## Actor Hierarchy

### AActor (912 bytes)

```cpp
class AActor : public UObject {
    char pad[416];                    // 0x28
    USceneComponent* RootComponent;   // 0x1C8
    char pad[448];                    // 0x1D0
};  // Total: 0x390
```

### APawn (1040 bytes)

```cpp
class APawn : public AActor {
    char pad[32];                // 0x390
    APlayerState* PlayerState;   // 0x3B0
    char pad[8];                 // 0x3B8
    AController* Controller;     // 0x3C0
    char pad[72];                // 0x3C8
};  // Total: 0x410
```

### ACharacter (1864 bytes)

```cpp
class ACharacter : public APawn {
    char pad[24];                              // 0x410
    USkeletalMeshComponent* Mesh;              // 0x428
    UCharacterMovementComponent* Movement;     // 0x430
    char pad[784];                             // 0x438
};  // Total: 0x748
```

---

## Controller Classes

### AController (1064 bytes)

```cpp
class AController : public AActor {
    char pad[8];                  // 0x390
    APlayerState* PlayerState;    // 0x398
    char pad[48];                 // 0x3A0
    APawn* Pawn;                  // 0x3D0
    char pad[8];                  // 0x3D8
    ACharacter* Character;        // 0x3E0
    char pad[64];                 // 0x3E8
};  // Total: 0x428
```

### APlayerController (2392+ bytes)

```cpp
class APlayerController : public AController {
    char pad[16];                           // 0x428
    APawn* AcknowledgedPawn;                // 0x438
    char pad[8];                            // 0x440
    APlayerCameraManager* CameraManager;    // 0x448
    char pad[168];                          // 0x450
    UCheatManager* CheatManager;            // 0x4F8
    UClass* CheatClass;                     // 0x500
    char pad[1104];                         // 0x508
};  // Total: 0x958+
```

---

## BL4/Oak Classes

### AGbxPlayerController (3496 bytes)

```cpp
class AGbxPlayerController : public APlayerController {
    char pad[176];                        // 0x958
    ACharacter* PrimaryCharacter;         // 0xA08
    char pad[536];                        // 0xA10
    bool bUseGbxCurrencyManager;          // 0xC28
    char pad[7];                          // 0xC29
    UGbxCurrencyManager* CurrencyManager; // 0xC30
    char pad[288];                        // 0xC38
    bool bUseRewardsManager;              // 0xD58
    char pad[7];                          // 0xD59
    UGbxRewardsManager* RewardsManager;   // 0xD60
    char pad[64];                         // 0xD68
};  // Total: 0xDA8
```

### AOakPlayerController (15424 bytes)

```cpp
class AOakPlayerController : public AGbxPlayerController {
    char pad[376];                    // 0xDA8
    AOakCharacter* OakCharacter;      // 0xF20
    char pad[8880];                   // 0xF28
    bool bIsCurrentlyTargeted;        // 0x31D8
    char pad[48];                     // 0x31D9
    bool bFullyAimingAtTarget;        // 0x3209
    // ... more fields
};  // Total: 0x3C40
```

### AGbxCharacter (15232 bytes)

```cpp
class AGbxCharacter : public ACharacter {
    char pad[13368];  // 0x748
};  // Total: 0x3B80
```

### AOakCharacter (38800 bytes)

The main player/enemy character class. This is one of the largest classes in the game.

```cpp
class AOakCharacter : public AGbxCharacter {
    char pad[1208];                           // 0x3B80
    FOakDamageState DamageState;              // 0x4038 (size: 0x608)
    FOakCharacterHealthState HealthState;     // 0x4640 (size: 0x1E8)
    char pad[4976];                           // 0x4828
    ECharacterHealthCondition HealthCondition; // 0x5B98
    char pad[951];                            // 0x5B99
    FOakActiveWeaponsState ActiveWeapons;     // 0x5F50 (size: 0x210)
    char pad[960];                            // 0x6160
    FDownState DownState;                     // 0x6F40 (size: 0x398)
    char pad[8];                              // 0x72D8
    AOakCharacter* ActorBeingRevived;         // 0x72E0
    // ... many more fields
    FGbxAttributeFloat AmmoRegenerate;        // 0x95E8
};  // Total: 0x9790
```

**Key offsets for AOakCharacter:**

| Offset | Size | Field | Description |
|--------|------|-------|-------------|
| 0x4038 | 0x608 | DamageState | Damage tracking state |
| 0x4640 | 0x1E8 | HealthState | Health/shield values |
| 0x5B98 | 1 | HealthCondition | Healthy/Injured/Dead enum |
| 0x5F50 | 0x210 | ActiveWeapons | Equipped weapon state |
| 0x6F40 | 0x398 | DownState | FFYL state |
| 0x72E0 | 8 | ActorBeingRevived | Revive target pointer |

---

## Inventory Classes

### AInventory (2224 bytes)

```cpp
class AInventory : public AActor {
    char pad[1312];  // 0x390
};  // Total: 0x8B0
```

### AWeapon (3400 bytes)

```cpp
class AWeapon : public AInventory {
    char pad[912];                           // 0x8B0
    FDamageModifierData DamageModifierData;  // 0xC40 (size: 0x6C)
    char pad[12];                            // 0xCAC
    FGbxAttributeFloat ZoomTimeScale;        // 0xCB8
    char pad[132];                           // 0xCC4
};  // Total: 0xD48
```

---

## Currency System

### FSToken (12 bytes)

```cpp
struct FSToken {
    int32_t Hash;  // 0x00 - Token hash
    FName Name;    // 0x04 - Token name
};
```

### FGbxCurrency (24 bytes)

```cpp
struct FGbxCurrency {
    FSToken Token;   // 0x00
    char pad[4];     // 0x0C
    uint64_t Amount; // 0x10 - Currency amount
};
```

### UGbxCurrencyManager (64 bytes)

```cpp
class UGbxCurrencyManager : public UObject {
    char pad[8];                      // 0x28
    TArray<FGbxCurrency> Currencies;  // 0x30
};
```

**Currency indices:**

| Index | Currency |
|-------|----------|
| 0 | Cash |
| 1 | Eridium |
| 2 | Gold Keys |
| 3 | Unknown |

---

## Attribute Types

### FGbxAttributeFloat (12 bytes)

```cpp
struct FGbxAttributeFloat {
    char pad[4];       // 0x00
    float Value;       // 0x04 - Current value
    float BaseValue;   // 0x08 - Base value before modifiers
};
```

### FGbxAttributeInteger (12 bytes)

```cpp
struct FGbxAttributeInteger {
    char pad[4];        // 0x00
    int32_t Value;      // 0x04
    int32_t BaseValue;  // 0x08
};
```

---

## Enums

### ECharacterHealthCondition

```cpp
enum class ECharacterHealthCondition : int8_t {
    Healthy = 0,
    Injured = 1,
    Dead = 2
};
```

### EMovementMode

```cpp
enum class EMovementMode : int8_t {
    MOVE_None = 0,
    MOVE_Walking = 1,
    MOVE_NavWalking = 2,
    MOVE_Falling = 3,
    MOVE_Swimming = 4,
    MOVE_Flying = 5,
    MOVE_Custom = 6,
    MOVE_MAX = 7
};
```

---

## Global Pointers

These offsets are from the PE image base (0x140000000):

| Global | Offset | Virtual Address | Description |
|--------|--------|-----------------|-------------|
| GUObjectArray | 0x113878f0 | 0x1513878f0 | All UObjects |
| GNames | 0x112a1c80 | 0x1512a1c80 | FName string pool |
| GWorld | 0x11532cb8 | 0x151532cb8 | Current world |
| ProcessEvent | 0x14f7010 | 0x144f7010 | Event dispatcher |

!!! note
    These offsets are from the November 2025 patch. Earlier versions had offsets 0x1000 lower.

---

## Pattern Signatures

For finding globals via code scanning:

```
GNames:   48 8D 0D ? ? ? ? E8 ? ? ? ? C6 05 ? ? ? ? ? 8B 05
GObjects: 48 8B 15 ? ? ? ? C1 E8 ? 48 8D 0C 49 C1 E1 ? 48 03
GWorld:   48 8B 05 ? ? ? ? 48 89 44 24 ? 48 8D 54 24 ? 4C 8D
```

The `?` bytes are wildcards. Calculate target address from RIP-relative offset in the instruction.

---

## Mesh & Visibility

| Component Offset | Field | Description |
|------------------|-------|-------------|
| Mesh + 0x39C | LastSubmitTime | Last frame submitted |
| Mesh + 0x3A0 | LastRenderTimeOnScreen | Last visible frame |

**Visibility check**: If `LastSubmitTime > LastRenderTimeOnScreen`, the mesh was occluded (behind a wall).

---

## FProperty Layout

For parsing reflection data:

```cpp
// FField base (shared by all field types)
struct FField {
    void* VTable;           // 0x00
    UStruct* Owner;         // 0x08
    FField* Next;           // 0x10
    FName NamePrivate;      // 0x18
    uint32_t FlagsPrivate;  // 0x20
};

// FProperty extends FField
struct FProperty : FField {
    int32_t ArrayDim;       // 0x28 - Array size (1 for non-arrays)
    int32_t ElementSize;    // 0x2C - Size of one element
    uint64_t PropertyFlags; // 0x30 - CPF_* flags
    uint16_t RepIndex;      // 0x38
    char pad[2];            // 0x3A
    int32_t Offset_Internal;// 0x3C - Byte offset in struct
    // Type-specific data at 0x40+
};
```

---

*These layouts are from SDK dumps as of November 2025. Offsets may change with game patches.*
