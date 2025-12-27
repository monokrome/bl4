//! NCS field type parsing for decompressed content

/// NCS field type suffix
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Type {
    /// `|map` - Nested map/object
    Map,
    /// `|leaf:` - String leaf value
    Leaf,
    /// `|leaf:typename` - Typed leaf
    TypedLeaf(String),
    /// `|empty` - Boolean/empty flag
    Empty,
}

impl Type {
    /// Parse field type from suffix string
    pub fn parse(suffix: &str) -> Option<Self> {
        match suffix {
            "map" => Some(Self::Map),
            "empty" => Some(Self::Empty),
            "leaf:" => Some(Self::Leaf),
            s if s.starts_with("leaf:") => Some(Self::TypedLeaf(s[5..].to_string())),
            _ => None,
        }
    }
}

/// Parsed field with name and type
#[derive(Debug, Clone)]
pub struct Field {
    pub name: String,
    pub field_type: Type,
}

impl Field {
    /// Parse a field from "name|type" notation
    pub fn parse(s: &str) -> Option<Self> {
        let (name, suffix) = s.split_once('|')?;
        let field_type = Type::parse(suffix)?;
        Some(Self {
            name: name.to_string(),
            field_type,
        })
    }
}

/// Known NCS field names from reverse engineering
pub mod known {
    pub const GBX_SECTIONS: &str = "gbx_sections|map";
    pub const CHILDREN: &str = "children|map";
    pub const DEPENDENCIES: &str = "dependencies|map";
    pub const SECTIONS: &str = "sections|map";
    pub const CONFIGS: &str = "configs|map";
    pub const ATTRIBUTES: &str = "attributes|map";
    pub const DAMAGE_SOURCE_LEAF: &str = "damagesource|leaf:damagesource";
    pub const DAMAGE_SOURCE_MAP: &str = "damagesource|map";
    pub const DAMAGE_TAGS: &str = "damagetags|leaf:";
    pub const TAGS_LEAF: &str = "tags|leaf:";
    pub const TAGS_MAP: &str = "tags|map";
    pub const STATS: &str = "stats|leaf:";
    pub const WEAPON_FIRE: &str = "weaponfire|map";
    pub const EFFECT_PARAMETERS: &str = "effectparameters|map";
    pub const PIPS: &str = "pips|map";
    pub const WHEEL_SETUPS: &str = "wheelsetups|map";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_field_parse_map() {
        let field = Field::parse("children|map").unwrap();
        assert_eq!(field.name, "children");
        assert_eq!(field.field_type, Type::Map);
    }

    #[test]
    fn test_field_parse_typed_leaf() {
        let field = Field::parse("damagesource|leaf:damagesource").unwrap();
        assert_eq!(field.name, "damagesource");
        assert_eq!(field.field_type, Type::TypedLeaf("damagesource".to_string()));
    }

    #[test]
    fn test_field_parse_leaf() {
        let field = Field::parse("stats|leaf:").unwrap();
        assert_eq!(field.name, "stats");
        assert_eq!(field.field_type, Type::Leaf);
    }

    #[test]
    fn test_field_parse_empty() {
        let field = Field::parse("flag|empty").unwrap();
        assert_eq!(field.name, "flag");
        assert_eq!(field.field_type, Type::Empty);
    }

    #[test]
    fn test_field_parse_invalid() {
        assert!(Field::parse("no_separator").is_none());
        assert!(Field::parse("unknown|badtype").is_none());
    }
}
