//! Inject the 4 anomalous VarInt[2]=4 items into 5.sav for in-game testing

use std::fs;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let save_path = std::env::args().nth(1).expect("Usage: inject_items <path_to_5.sav> <steam_id>");
    let steam_id = std::env::args().nth(2).expect("Usage: inject_items <path_to_5.sav> <steam_id>");

    let serials = [
        "@Uga`wSFdiD{FRD<H8j0%E9n>vUPSiOB",
        "@Ugw$Yw5hh0gkD?i5ey2^>_}zwoVYM&2d@j4mVR`;*PRq;pw/rfp7Immb4H@o3Gsyf-n=W`*PXCI-a(S8kg~`6=w7h(O%h$z3b?Of46si>J7U~>o2nPT",
        "@Ugw$Yw5hh0gokbODQHN^OkfE}un5a(OL7hUCLft~0Lk-~o",
        "@UgxFw!5oS&X{Y4e(P>-64+SD9WA5<<>In+G_",
    ];

    // Read and decrypt
    let encrypted = fs::read(&save_path)?;
    let yaml_data = bl4::decrypt_sav(&encrypted, &steam_id)?;
    let mut save = bl4::SaveFile::from_yaml(&yaml_data)?;

    // Build the backpack map directly with serde_yaml
    let backpack_yaml = build_backpack_yaml(&serials);
    save.set_raw("state.inventory.items.backpack", &backpack_yaml)?;
    println!("Set backpack with {} items", serials.len());

    for (i, serial) in serials.iter().enumerate() {
        println!("  Slot {}: {}...", i, &serial[..34.min(serial.len())]);
    }

    // Encrypt and write
    let modified_yaml = save.to_yaml()?;
    let encrypted = bl4::encrypt_sav(&modified_yaml, &steam_id)?;
    fs::write(&save_path, &encrypted)?;

    println!("Saved to {}", save_path);
    Ok(())
}

fn build_backpack_yaml(serials: &[&str]) -> String {
    let mut yaml = String::new();
    for (i, serial) in serials.iter().enumerate() {
        // StateFlags::backpack() = 513 (bit 0 + bit 9)
        yaml.push_str(&format!(
            "slot_{}:\n  serial: '{}'\n  flags: 0\n  state_flags: 513\n",
            i, serial
        ));
    }
    yaml
}
