//! Drop rate and location lookup for legendary items
//!
//! Provides a database of boss → legendary item mappings with drop probabilities.

mod db;
mod extract;
mod types;

pub use db::DropsDb;
pub use extract::{
    extract_drops_from_itempool, extract_drops_from_itempoollist, generate_drops_manifest,
};
pub use types::{
    BossNameMapping, DropEntry, DropLocation, DropProbabilities, DropSource, DropsManifest,
    WorldDropPool,
};

#[cfg(test)]
mod tests {
    use super::*;

    fn test_manifest() -> DropsManifest {
        DropsManifest {
            version: 1,
            probabilities: DropProbabilities::default(),
            drops: vec![
                DropEntry {
                    source: "MeatheadRider_Jockey".to_string(),
                    source_display: Some("Saddleback".to_string()),
                    source_type: DropSource::Boss,
                    manufacturer: "JAK".to_string(),
                    gear_type: "SG".to_string(),
                    item_name: "Hellwalker".to_string(),
                    item_id: "JAK_SG.comp_05_legendary_Hellwalker".to_string(),
                    pool: "itempool_jak_sg_05_legendary_Hellwalker_shiny".to_string(),
                    drop_tier: "Primary".to_string(),
                    drop_chance: 0.20,
                },
                DropEntry {
                    source: "Timekeeper_Guardian".to_string(),
                    source_display: Some("Guardian Timekeeper".to_string()),
                    source_type: DropSource::Boss,
                    manufacturer: "MAL".to_string(),
                    gear_type: "SM".to_string(),
                    item_name: "PlasmaCoil".to_string(),
                    item_id: "MAL_SM.comp_05_legendary_PlasmaCoil".to_string(),
                    pool: "itempool_mal_sm_05_legendary_PlasmaCoil_shiny".to_string(),
                    drop_tier: "Primary".to_string(),
                    drop_chance: 0.20,
                },
            ],
        }
    }

    #[test]
    fn test_find_by_name_exact() {
        let db = DropsDb::from_manifest(test_manifest());
        let results = db.find_by_name("Hellwalker");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].source, "MeatheadRider_Jockey");
        assert_eq!(results[0].chance, 0.20);
    }

    #[test]
    fn test_find_by_name_case_insensitive() {
        let db = DropsDb::from_manifest(test_manifest());
        let results = db.find_by_name("hellwalker");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].source, "MeatheadRider_Jockey");
    }

    #[test]
    fn test_find_by_name_partial() {
        let db = DropsDb::from_manifest(test_manifest());
        let results = db.find_by_name("plasma");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].source, "Timekeeper_Guardian");
    }

    #[test]
    fn test_find_by_boss() {
        let db = DropsDb::from_manifest(test_manifest());
        let results = db.find_by_boss("Timekeeper_Guardian");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].item_name, "PlasmaCoil");
    }

    #[test]
    fn test_sorted_by_chance() {
        let mut manifest = test_manifest();
        manifest.drops.push(DropEntry {
            source: "AnotherBoss".to_string(),
            source_display: Some("Another Boss".to_string()),
            source_type: DropSource::Boss,
            manufacturer: "JAK".to_string(),
            gear_type: "SG".to_string(),
            item_name: "Hellwalker".to_string(),
            item_id: "JAK_SG.comp_05_legendary_Hellwalker".to_string(),
            pool: "itempool_jak_sg_05_legendary_Hellwalker_shiny".to_string(),
            drop_tier: "Secondary".to_string(),
            drop_chance: 0.08,
        });

        let db = DropsDb::from_manifest(manifest);
        let results = db.find_by_name("Hellwalker");

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].source, "MeatheadRider_Jockey");
        assert_eq!(results[0].chance, 0.20);
        assert_eq!(results[1].source, "AnotherBoss");
        assert_eq!(results[1].chance, 0.08);
    }
}
