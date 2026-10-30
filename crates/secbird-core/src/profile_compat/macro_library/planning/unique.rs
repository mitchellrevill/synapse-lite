//! Check every JSON key without constructing a second semantic document.
use crate::{Error, Result};
use serde::de::{self, DeserializeSeed, MapAccess, SeqAccess, Visitor};
use serde_json::Value;
use std::{collections::BTreeSet, fmt};

pub(super) struct Limits {
    pub bytes: usize,
    pub nodes: usize,
    pub depth: usize,
}
pub(super) const BINDING: Limits = Limits {
    bytes: 4096,
    nodes: 256,
    depth: 8,
};
pub(super) const DEFINITION: Limits = Limits {
    bytes: 1024 * 1024,
    nodes: 32768,
    depth: 64,
};
struct Budget {
    remaining: usize,
    depth: usize,
}
struct Seed<'a> {
    budget: &'a mut Budget,
    depth: usize,
}
impl<'de> DeserializeSeed<'de> for Seed<'_> {
    type Value = ();
    fn deserialize<D: de::Deserializer<'de>>(self, d: D) -> std::result::Result<(), D::Error> {
        if self.depth > self.budget.depth || self.budget.remaining == 0 {
            return Err(de::Error::custom("Macro JSON exceeds its node/depth bound"));
        }
        self.budget.remaining -= 1;
        d.deserialize_any(self)
    }
}
impl<'de> Visitor<'de> for Seed<'_> {
    type Value = ();
    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("bounded JSON with unique keys")
    }
    fn visit_unit<E: de::Error>(self) -> std::result::Result<(), E> {
        Ok(())
    }
    fn visit_bool<E: de::Error>(self, _: bool) -> std::result::Result<(), E> {
        Ok(())
    }
    fn visit_i64<E: de::Error>(self, _: i64) -> std::result::Result<(), E> {
        Ok(())
    }
    fn visit_u64<E: de::Error>(self, _: u64) -> std::result::Result<(), E> {
        Ok(())
    }
    fn visit_i128<E: de::Error>(self, _: i128) -> std::result::Result<(), E> {
        Ok(())
    }
    fn visit_u128<E: de::Error>(self, _: u128) -> std::result::Result<(), E> {
        Ok(())
    }
    fn visit_f64<E: de::Error>(self, _: f64) -> std::result::Result<(), E> {
        Ok(())
    }
    fn visit_str<E: de::Error>(self, _: &str) -> std::result::Result<(), E> {
        Ok(())
    }
    fn visit_string<E: de::Error>(self, _: String) -> std::result::Result<(), E> {
        Ok(())
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut access: A) -> std::result::Result<(), A::Error> {
        while access
            .next_element_seed(Seed {
                budget: self.budget,
                depth: self.depth + 1,
            })?
            .is_some()
        {}
        Ok(())
    }
    fn visit_map<A: MapAccess<'de>>(self, mut access: A) -> std::result::Result<(), A::Error> {
        let mut seen = BTreeSet::new();
        while let Some(key) = access.next_key::<String>()? {
            if self.budget.remaining == 0 {
                return Err(de::Error::custom("Macro JSON exceeds its node bound"));
            }
            self.budget.remaining -= 1;
            if !seen.insert(key) {
                return Err(de::Error::custom("Duplicate macro JSON key"));
            }
            access.next_value_seed(Seed {
                budget: self.budget,
                depth: self.depth + 1,
            })?;
        }
        Ok(())
    }
}

pub(super) fn parse(text: &str, limits: Limits) -> Result<Value> {
    if text.len() > limits.bytes {
        return Err(Error::InvalidArg(
            "Macro JSON exceeds its byte bound".into(),
        ));
    }
    let mut budget = Budget {
        remaining: limits.nodes,
        depth: limits.depth,
    };
    let mut parser = serde_json::Deserializer::from_str(text);
    Seed {
        budget: &mut budget,
        depth: 1,
    }
    .deserialize(&mut parser)
    .and_then(|()| parser.end())
    .map_err(|e| Error::InvalidArg(e.to_string()))?;
    // Ordinary Value parsing preserves arbitrary-precision number semantics for
    // the existing source hash implementation; the streaming pass keeps no tree.
    serde_json::from_str(text).map_err(|e| Error::InvalidArg(e.to_string()))
}
