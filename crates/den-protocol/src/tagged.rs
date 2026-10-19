//! String-only discriminators for extensible unions (Spec 02 §3, §7.2).
//!
//! Serde buffers internally tagged enums with an untagged fallback, and its buffered variant
//! identifier accepts integer indices. Check only the tag as the map streams through; keep the
//! derived decoder for variant fields, duplicate detection and unknown-string fallbacks.

use serde::Deserializer;
use serde::de::{DeserializeSeed, MapAccess, Visitor, value::StringDeserializer};

pub(crate) struct StringKind<D>(pub D);

impl<'de, D: Deserializer<'de>> Deserializer<'de> for StringKind<D> {
    type Error = D::Error;

    fn deserialize_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        self.0.deserialize_map(KindVisitor(visitor))
    }

    serde::forward_to_deserialize_any! {
        bool i8 i16 i32 i64 u8 u16 u32 u64 f32 f64 char str string bytes byte_buf option
        unit unit_struct newtype_struct seq tuple tuple_struct map struct enum identifier ignored_any
    }
}

struct KindVisitor<V>(V);

impl<'de, V: Visitor<'de>> Visitor<'de> for KindVisitor<V> {
    type Value = V::Value;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        self.0.expecting(formatter)
    }

    fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<Self::Value, A::Error> {
        self.0.visit_map(KindMap {
            map,
            is_kind: false,
        })
    }
}

struct KindMap<A> {
    map: A,
    is_kind: bool,
}

impl<'de, A: MapAccess<'de>> MapAccess<'de> for KindMap<A> {
    type Error = A::Error;

    fn next_key_seed<K: DeserializeSeed<'de>>(
        &mut self,
        seed: K,
    ) -> Result<Option<K::Value>, Self::Error> {
        let Some(key) = self.map.next_key::<String>()? else {
            return Ok(None);
        };
        self.is_kind = key == "kind";
        seed.deserialize(StringDeserializer::<Self::Error>::new(key))
            .map(Some)
    }

    fn next_value_seed<V: DeserializeSeed<'de>>(
        &mut self,
        seed: V,
    ) -> Result<V::Value, Self::Error> {
        if self.is_kind {
            let tag = self.map.next_value::<String>()?;
            seed.deserialize(StringDeserializer::<Self::Error>::new(tag))
        } else {
            self.map.next_value_seed(seed)
        }
    }

    fn size_hint(&self) -> Option<usize> {
        self.map.size_hint()
    }
}

// Keep the remote derive private so `Node::deserialize` / `Fragment::deserialize` cannot bypass
// the check. Reuse the variant declaration instead of maintaining a second list of wire fields.
macro_rules! string_kind {
    ($remote:literal; $(#[$attr:meta])* pub enum $name:ident { $($variants:tt)* }) => {
        $(#[$attr])*
        pub enum $name { $($variants)* }

        impl<'de> serde::Deserialize<'de> for $name {
            fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                $(#[$attr])*
                #[derive(serde::Deserialize)]
                #[serde(remote = $remote)]
                #[allow(dead_code)]
                enum Wire { $($variants)* }

                Wire::deserialize($crate::tagged::StringKind(deserializer))
            }
        }
    };
}

pub(crate) use string_kind;
