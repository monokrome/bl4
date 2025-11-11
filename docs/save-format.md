# Save File Format

This document describes the structure of .sav files, including encryption, compression, and the complete YAML data schema.

## File Structure

```
.sav file → AES-256-ECB decrypt → PKCS7 unpad → zlib decompress → YAML data
```

### Encryption

- **Algorithm**: AES-256-ECB
- **Key derivation**: BASE_KEY XOR'd with Steam ID (little-endian u64)
  - BASE_KEY: First 8 bytes are XOR'd with Steam ID bytes
  - Remaining 24 bytes unchanged
- **Padding**: PKCS7 to 16-byte blocks
- **No IV**: ECB mode (block cipher, each 16-byte block encrypted independently)

### Compression & Integrity

After decryption and unpadding, the data structure is:

```
[zlib compressed YAML] [adler32 checksum (4 bytes LE)] [uncompressed length (4 bytes LE)]
```

- **Compression**: zlib (DEFLATE, level 9)
- **Checksum**: adler32 of uncompressed YAML data (little-endian)
- **Length**: Size of uncompressed YAML (little-endian)

The game validates both the adler32 checksum and length. Missing or incorrect footer causes "Data Corrupted" errors.

## YAML Schema

### Top-Level Structure

```yaml
state:                      # Main character state
onlinecharacterprefs:       # Online/multiplayer preferences
pips:                       # UI notification markers
stats:                      # Player statistics and tutorials
gbx_discovery_pc:           # PC-specific discovery data (Fog of Discovery)
gbx_discovery_pg:           # Platform-generic discovery data
activities:                 # Activities tracking
missions:                   # Mission/quest progress
oak.ui.progression_data:    # UI progression preferences
save_game_header:           # Save file metadata
progression:                # Character progression tracking
globals:                    # Global flags/state
timed_facts:                # Time-based game facts
```

## state

The primary character state section containing all gameplay data.

### Character Identity

```yaml
state:
  char_guid: string          # Character GUID (32 hex chars, VALIDATED)
  class: string              # Character class (e.g., "Char_DarkSiren")
  char_name: string          # Display name
  player_difficulty: string  # Difficulty level ("Easy", "Normal", "Hard", etc.)
```

**Note**: `char_guid` appears to be a unique character identifier generated when the character is created. It can be safely randomized without causing issues.

### Experience

Array of experience entries, one per progression type:

```yaml
experience:
  - type: string    # "Character" or "Specialization"
    level: int      # Current level
    points: int     # XP points toward next level
```

### Inventory

```yaml
inventory:
  items:
    backpack:
      slot_N:
        serial: string  # Item serial (base85-like encoding, starts with @Ug)
        flags: int      # Item flags (1 = equipped, etc.)
  equipped_inventory:
    equipped:
      slot_N:
        - serial: string
          flags: int
  active_slot: int  # Currently active weapon slot
```

**Item Serials**: Encoded in a custom base85-like format starting with `@Ug`. The 4th character indicates item type:
- `r` - Weapon
- `e` - Equipment type E
- `d` - Equipment type D
- `w` - Weapon special
- `u` - Utility
- `f` - Consumable
- `!` - Special

See [Item Serials](#item-serials) for detailed format.

### Player Customization

```yaml
player_customization: {}  # Appearance and cosmetic settings
```

### Packages

Reward packages (preorder bonuses, etc.):

```yaml
packages:
  - time_received: int      # Unix timestamp
    game_stage: string      # Game stage when received ("-1" for immediate)
    reward_scale: float     # Scaling factor
    rewards_def: string     # Reward definition name (e.g., "RewardPackage_PreOrder")
    source: string          # Source identifier
    viewed: bool            # Has player seen this package
```

### Currencies

```yaml
currencies:
  cash: int           # In-game money
  eridium: int        # Premium currency
  golden_key: string  # Golden keys (value "shift" = Shift rewards)
```

### Ammo

```yaml
ammo:
  assaultrifle: int
  pistol: int
  shotgun: int
  smg: int
  sniper: int
  repairkit: int
```

### Location State

```yaml
checkpoint_name: string      # Current checkpoint (e.g., "Intro_P.Respawn")
world_region_name: string    # Current region (e.g., "grasslands_Prison")
personal_vehicle: string     # Vehicle type (e.g., "PV_Borg")
hover_drive: string          # Hover drive type or "None"
vehicle_weapon_slot: int     # Active vehicle weapon slot
```

### Lost Loot

```yaml
lostloot:
  cooldown: string  # Cooldown timer ("-1" = none)
  items: []         # List of lost items
```

### Other State Fields

```yaml
blackmarket_cooldown: int    # Blackmarket restock timer
challenge_objectives: {}     # Challenge progress
total_playtime: float        # Total seconds played
last_played_timestamp: int   # Unix timestamp
first_timestamp: mixed       # First play timestamp (may be null)
```

## onlinecharacterprefs

Online multiplayer preferences:

```yaml
onlinecharacterprefs:
  recentfriends: []  # Recent co-op players
```

## pips

UI notification markers (the "!" indicators):

```yaml
pips:
  pips_list: !tags
    - string  # Notification path (e.g., "Character.inventory.anynew")
```

The `!tags` YAML tag indicates this is a special list type.

## stats

Player statistics and tutorial progress:

```yaml
stats:
  tutorial:
    look: int  # Tutorial completion flags (bitfield)
```

## gbx_discovery_pc

PC-specific discovery data (Fog of Discovery system):

```yaml
gbx_discovery_pc:
  pins_state: {}     # Map pins
  saveid: int        # Save identifier
  fodsaveversion: int  # FOD format version (e.g., 2)
  foddatas:
    - levelname: string          # Level/map name (e.g., "Intro_P")
      foddimensionx: int         # FOD grid width (e.g., 128)
      foddimensiony: int         # FOD grid height (e.g., 128)
      compressiontype: string    # Compression ("Zlib")
      foddata: string            # Base64-encoded compressed FOD bitmap
  mapviewsavedata:
    hiddenfilters: []  # Hidden map filter categories
  metrics:
    lastworld: string           # Last visited world
    lastregion: string          # Last visited region
    hasseenworldlist: [string]  # Worlds player has visited
    hasseenregionlist: [string] # Regions player has visited
  non_hosted_dlblob: {}  # Non-hosted downloadable content blob
```

**FOD Data**: The `foddata` field contains a base64-encoded, zlib-compressed bitmap representing explored areas of the map. Each bit corresponds to a grid cell (foddimensionx × foddimensiony).

## gbx_discovery_pg

Platform-generic discovery data:

```yaml
gbx_discovery_pg:
  dlblob: {}  # Downloadable content blob
```

## activities

Activity tracking system:

```yaml
activities:
  allactivities: {}  # Activity progress data
```

## missions

Mission state and progress:

```yaml
missions:
  tracked_missions: [string]  # Currently tracked mission names
  local_sets:
    missionset_name:
      missions:
        mission_name:
          status: string  # Mission status
          # Additional mission-specific fields
```

**Mission Statuses**:
- `Kickoffing` - Mission starting/initializing
- `Active` - Mission in progress
- `Completed` - Mission finished

## oak.ui.progression_data

UI progression preferences:

```yaml
oak.ui.progression_data:
  ui_progression_data:
    active_tree_index: int                       # Active skill tree tab
    dont_ask_again_skills: bool                  # Skip skills tutorial
    dont_ask_again_respec_skills: bool           # Skip respec confirmation
    dont_ask_again_specializations: bool         # Skip specialization tutorial
    dont_ask_again_respec_specializations: bool  # Skip spec respec confirmation
    dont_ask_again_respec_node: bool             # Skip node respec confirmation
```

## save_game_header

Save file metadata:

```yaml
save_game_header:
  guid: string      # Save GUID (32 hex chars, VALIDATED)
  timestamp: int    # Unix timestamp of last save
```

**Note**: This GUID appears to be a save file identifier. It can be safely randomized without causing issues.

## progression

Character progression token pools:

```yaml
progression:
  progress_state_data:
    characterprogresspoints:
      id: string           # Token pool GUID (32 hex chars)
      reset_count: int     # Number of respecs
    specializationtokenpool:
      id: string           # Token pool GUID (32 hex chars)
      reset_count: int     # Number of respecs
```

## globals

Global game state (currently empty in early game):

```yaml
globals: {}
```

## timed_facts

Time-based game facts (currently empty in early game):

```yaml
timed_facts: {}
```

## Item Serials

Item serials use a custom base85-like encoding with bit-packed data.

### Format

```
@Ug[type][encoded_data]
```

Example: `@Uga`wSaA`L54ppc~ZK@8c7Ahy/90C`
- Prefix: `@Ug`
- Type: `a` (pistol in this case)
- Encoded data: ``a`wSaA`L54ppc~ZK@8c7Ahy/90C``

### Decoded Structure

After base85 decode and bit unpacking:

```
[Magic Header (7 bits)] [Item Type] [Token Stream]
```

**Magic Header**: `0010000` (binary) = 16 (decimal)

**Token Stream**: Variable-length tokens:
- `00` - Separator/Terminator
- `01` - Soft separator
- `100` - VarInt (variable-length integer)
- `110` - VarBit (variable-length bit field)
- `101` - Part structure (index + values)
- `111` - String (length + UTF-8 bytes)

### Example: Starting Pistol

Serial: `@Uga`wSaA`L54ppc~ZK@8c7Ahy/90C`

Decoded:
- Type: `a` (pistol)
- Bytes: 22 total
- Hex: `2110c0310e966c2f70aa1512b65570556854e4577fb0`
- Tokens:
  - VarInt(4) - Manufacturer ID
  - Separator - End of data

This is the simplest possible item - just a manufacturer ID (4) with no parts, level, or other attributes.

The encoded data contains:
- Item stats (damage, accuracy, etc.)
- Manufacturer ID
- Rarity level
- Item level
- Parts/components
- Additional item-specific data

See `crates/bl4/src/serial.rs` for decoding implementation.

## Data Types

### GUIDs

GUIDs appear in several places:
- `state.char_guid` - Character identifier (32 hex chars, validated)
- `save_game_header.guid` - Save identifier (32 hex chars, validated)
- `progression.progress_state_data.*.id` - Token pool identifiers (32 hex chars)

These appear to be UUIDv4-style but with non-standard variant bits. They can be safely randomized without causing save corruption.

### Timestamps

Unix timestamps (seconds since epoch):
- `state.last_played_timestamp`
- `state.packages[].time_received`
- `save_game_header.timestamp`

## Notes

### File Format

- The 8-byte footer (adler32 + length) is critical for game validation
- Empty objects `{}` and arrays `[]` should be preserved
- The YAML uses custom tags like `!tags` for special data types
- FOD data is double-encoded: base64 wrapping zlib compression
- Many fields use string representations of numbers (e.g., `game_stage: '-1'`)
- Boolean values are lowercase (`true`/`false`)

### GUIDs

GUIDs in the save file:
1. `state.char_guid` - Character identifier
2. `save_game_header.guid` - Save file identifier
3. `progression.progress_state_data.*.id` - Token pool identifiers

These GUIDs can be safely randomized (tested and verified). The game does not validate them against external data.

### Encryption Implementation

See `crates/bl4/src/crypto.rs` for the complete encryption/decryption implementation, including:
- `derive_key()` - Steam ID-based key derivation
- `decrypt_sav()` - Full decryption pipeline
- `encrypt_sav()` - Full encryption pipeline with footer

## profile.sav Structure

The `profile.sav` file contains account-wide data that applies to all characters. Unlike character saves, this file contains no gameplay state - only settings, cosmetics, and unlocks.

### Top-Level Structure

```yaml
inputprefs:          # Input/controller settings
ui:                  # UI preferences and settings
onlineprefs:         # Online/multiplayer preferences
domains:             # Unlocked cosmetics and items
echoprefs:           # Echo device preferences
ui_screen_data:      # Last visited menu screens
audioprefs:          # Audio/volume settings
oak.ui.news_data:    # News article tracking
save_game_header:    # Profile metadata
deep_freeze_pips:    # Archived notification pips
pips:                # Current notification pips
```

### inputprefs

Input and controller settings:

```yaml
inputprefs:
  # General input
  toggle_zoom: bool
  toggle_crouch: bool
  toggle_sprint: bool
  enable_dash: bool
  censor_content: bool
  display_damage_numbers: bool

  # Sensitivity
  look_sensitivity_horizontal: float      # 0.0 - 1.0
  look_sensitivity_vertical: float
  ads_sensitivity_horizontal: float       # Aim-down-sights sensitivity
  ads_sensitivity_vertical: float

  # Motion controls
  motion_controls_look_sensitivity_horizontal: float
  motion_controls_look_sensitivity_vertical: float
  motion_controls_ads_sensitivity_horizontal: float
  motion_controls_ads_sensitivity_vertical: float
  motion_controls_setting: int

  # JCMS (Joy-Con motion controls)
  jcms_look_sensitivity_horizontal: float
  jcms_look_sensitivity_vertical: float
  jcms_ads_sensitivity_horizontal: float
  jcms_ads_sensitivity_vertical: float
  jcms_enable_control: int

  # Aim assist
  controller_ads_snap: bool
  mouse_ads_snap: bool
  controller_aim_assist: bool
  controller_aim_recentering: bool

  # Camera
  camera_head_bob: float                  # 0.0 - 1.0

  # Controls
  mantle_with_forward: bool
  invert_look_x_axis_kbm: bool
  invert_look_y_axis_kbm: bool
  invert_move_x_axis_kbm: bool
  invert_move_y_axis_kbm: bool
  invert_look_x_axis_gamepad: bool
  invert_look_y_axis_gamepad: bool
  invert_move_x_axis_gamepad: bool
  invert_move_y_axis_gamepad: bool

  # Dead zones (gamepad)
  left_stick_radial_dead_zone_inner: float
  left_stick_radial_dead_zone_outer: float
  left_stick_axial_dead_zone_inner: float
  left_stick_axial_dead_zone_outer: float
  right_stick_radial_dead_zone_inner: float
  right_stick_radial_dead_zone_outer: float
  right_stick_axial_dead_zone_inner: float
  right_stick_axial_dead_zone_outer: float

  # Glide controls
  use_toggle_glide_kbm: bool
  use_toggle_glide_gamepad: bool

  # HUD
  crosshair_position: int                 # 0=center, 1=raised
  compass_vertical_indicator_config: int
  radar_enabled: int

  # Vehicle
  vehicle_fov: float                      # Field of view (degrees)

  # Map
  map_viewer_zoom_speed: float

  # Grapple
  grapple_pitch_correction: bool

  # Gamepad presets
  on_foot_gamepad_stick_preset: string
  is_southpaw_on_foot_gamepad_stick_preset_active: bool
  on_vehicle_gamepad_stick_preset: string
  is_southpaw_on_vehicle_gamepad_stick_preset_active: bool

  # Feedback
  rumble_enabled: bool
  camera_shake_intensity: float           # 0.0 - 1.0
  reactivetriggers_enabled: bool          # DualSense adaptive triggers

  # Key bindings
  binding_profiles:
    - profile_identifier: string          # e.g., "InputUserSettings.Profiles.Default"
      class_path: string                  # UE class path
      object_path: string                 # UE object path
      dirty_mappings:                     # Custom key bindings
        - mapping_name: string            # Action name
          current_key_name: string        # Key/button name
          hardware_device_input_class_name: string
          hardware_device_hardware_device_identifier: string
          hardware_device_primary_device_type: int
          hardware_device_supported_features_mask: int
          slot: int
          localized: bool
```

### ui

UI preferences and accessibility:

```yaml
ui:
  default: bool
  user_preferences:
    safe_area:
      horizontal_ratio: float             # Screen safe area margins
      vertical_ratio: float
    font_scaling:
      menu: int                           # Font size adjustment
      hud: int
    color_blind_mode: int                 # 0=off, 1-3=different modes
    high_contrast_mode:
      hud: bool
      crosshair: bool
  subtitles:
    size: float                           # Subtitle size multiplier
    speaker_color: [float, float, float, float]  # RGBA
    text_opacity: float
    text_color: [float, float, float, float]
    background_opacity: float
    background_color: [float, float, float, float]
```

### domains

Unlocked cosmetics and items (account-wide):

```yaml
domains:
  default_customization:
    character_class_customizations:
      class_name:                         # e.g., "Char_DarkSiren"
        character_head: string            # Head cosmetic
        character_skin: string            # Skin cosmetic
        emote_1: string                   # Emote slot 1
        emote_2: string
        emote_3: string
        emote_4: string
    echo_theme: string                    # Echo device theme
    echo_weapon_ornament_theme: string    # Weapon charm theme
  unlockables:
    unlockable_heads:
      entries: [string]                   # List of unlocked heads
    unlockable_skins:
      entries: [string]                   # List of unlocked skins
    unlockable_emotes:
      entries: [string]                   # List of unlocked emotes
    unlockable_echo_themes:
      entries: [string]                   # List of unlocked echo themes
    unlockable_room_decorations:
      entries: [string]                   # List of room decorations
    unlockable_weapon_ornaments:
      entries: [string]                   # List of weapon charms
    unlockable_weapon_skins:
      entries: [string]                   # List of weapon skins
    unlockable_vehicles:
      entries: [string]                   # List of vehicle skins
  vault_cards:
    activated_card: string                # Currently active vault card ("none" or card name)
```

**Cosmetic Naming**: Cosmetic entries use Unreal Engine asset paths like:
- `Unlockable_Heads.Head_DarkSiren_01`
- `Unlockable_Skins.Skin_Paladin_02`
- `Unlockable_Weapons.Mat14_Grunt`

### audioprefs

Audio settings:

```yaml
audioprefs:
  # Volume controls (0.0 - 1.0)
  volume_overall: float
  volume_sfx: float
  volume_music: float
  volume_dialog: float

  # Trim levels (relative adjustments)
  ui_trim: float
  player_weapons_trim: float
  explosions_trim: float
  outgoing_damage_trim: float
  incoming_damage_trim: float
  combat_voice_volume: float
  player_voice_volume: float
  player_efforts_trim: float
  player_callouts_trim: float
  player_idle_lines_trim: float
  menu_music_volume: float
  game_music_volume: float
  cinematic_music_volume: float
  boss_music_trim: float

  # Character-specific
  claptrap_volume: float                  # Claptrap voice volume

  # Settings
  playinbackground: bool                  # Play audio when unfocused
  mutehitnotifies: bool                   # Mute damage hit sounds
  audio_preset: int                       # Audio preset selection
  force_mono: bool                        # Force mono audio
  use_controller_speaker: bool            # Use DualSense speaker
  eq_mode: int                            # Equalizer mode
  audio_language: string                  # e.g., "English(US)"
```

### echoprefs

Echo device preferences:

```yaml
echoprefs:
  path_visibility_duration: int           # Waypoint marker duration (0-3)
```

### ui_screen_data

UI state tracking:

```yaml
ui_screen_data:
  last_visited_status_menu_id: string     # Last opened menu ID
```

### oak.ui.news_data

News and announcements:

```yaml
oak.ui.news_data:
  news_data:
    latest_popup_article: int             # Latest article ID shown
```

### save_game_header

Profile metadata:

```yaml
save_game_header:
  guid: string                            # Profile GUID (32 hex chars)
  timestamp: int                          # Unix timestamp
```

### pips & deep_freeze_pips

Notification markers (the "!" indicators in menus):

```yaml
pips:
  pips_list: !tags                        # Active notifications
    - string                              # e.g., "profile.DLC.Headhunter"

deep_freeze_pips:
  pips_list_deep_freeze: !tags            # Archived notifications
    - string
```

**Pip Paths**: Common pip indicators include:
- `profile.DLC.Headhunter` - New DLC content
- `profile.DLC.premium` - Premium content
- `profile.DLC.preorder` - Pre-order bonuses
- `profile.newgame.ultimatevaulthunter` - New game mode

## Profile vs Character Saves

| Aspect | profile.sav | character.sav |
|--------|-------------|---------------|
| **Scope** | Account-wide | Single character |
| **Contains** | Settings, cosmetics, unlocks | Gameplay state, inventory, progress |
| **Shared** | All characters | One character only |
| **Editable** | Only when game closed | Only when game closed |
| **Encryption** | Same as character saves | Same as profile |

**Important**: Profile.sav should never be edited while the game is running, as it's kept in memory and will overwrite disk changes on exit.

## Related Documentation

- [Game Data Structures](./game-data.md) - Manufacturer IDs, rarity values, item classes (TODO)
- [Memory Structure](./memory.md) - In-memory data structures
- [Structures](./structures.md) - Memory dump analysis
