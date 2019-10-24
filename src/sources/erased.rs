use super::*;
use std::fmt::Debug;
use std::hash::{Hash, Hasher};

pub trait ErasedDesktopBackgroundSource {
    fn name(&self) -> &str;
    fn original(&self, id: &OriginalKey) -> OriginalResult<&dyn Original>;
    fn reload(&mut self) -> Vec<OriginalChange<OriginalKey, Box<dyn Debug>>>;
}

#[derive(Clone)]
pub struct OriginalKey {
    value: serde_json::Value,
    comparer: Box<fn(&OriginalKey, &OriginalKey) -> KeyRelation>,
    hasher: Box<fn(&OriginalKey, &mut dyn Hasher)>,
}

impl CompareKey for OriginalKey { 
    fn compare(&self, other: &Self) -> KeyRelation { (self.comparer)(self, other) } 
}

impl Hash for OriginalKey { 
    fn hash<H: Hasher>(&self, hasher: &mut H) { (self.hasher)(self, hasher) } 
}

impl PartialEq for OriginalKey {
    fn eq(&self, other: &Self) -> bool { self.compare(other) == KeyRelation::SameOriginal }
}

impl Eq for OriginalKey { }

impl OriginalKey {
    fn new<'a, S: DesktopBackgroundSource<'a>>(key: S::Key) -> OriginalKey {
        OriginalKey {
            value: serde_json::to_value(key).expect("Could not serialize original key to JSON!"),
            comparer: Box::new(key_comparer::<S>),
            hasher: Box::new(key_hasher::<S>)
        }
    }

    fn try_deserialize<K: serde::de::DeserializeOwned>(&self) -> Option<K> {
        serde_json::from_value(self.value.clone()).ok()
    }
}

fn key_comparer<'a, S: DesktopBackgroundSource<'a>>(k1: &OriginalKey, k2: &OriginalKey) -> KeyRelation {
    match (serde_json::from_value::<S::Key>(k1.value.clone()), serde_json::from_value::<S::Key>(k2.value.clone())) {
        (Ok(k1), Ok(k2)) => k1.compare(&k2),
        _ => KeyRelation::Distinct,
    }
}

struct HashWrapper<'a>(&'a mut dyn Hasher);
impl<'a> Hasher for HashWrapper<'a> {
    fn write(&mut self, bytes: &[u8]) { self.0.write(bytes); }
    fn finish(&self) -> u64 { self.0.finish() }
}

fn key_hasher<'a, S: DesktopBackgroundSource<'a>>(key: &OriginalKey, hasher: &mut dyn Hasher) {
    let key = serde_json::from_value::<S::Key>(key.value.clone()).expect("Corrupt OriginalKey detected!");
    key.hash(&mut HashWrapper(hasher));
}

impl<S: for<'a> DesktopBackgroundSource<'a>> ErasedDesktopBackgroundSource for S {
    fn name(&self) -> &str { self.name() }

    fn original(&self, key: &OriginalKey) -> OriginalResult<&dyn Original> {
        key.try_deserialize()
            .map(|k| self.original(&k).map(|o| o as &dyn Original))
            .unwrap_or(OriginalResult::WrongSource)
    }

    fn reload(&mut self) -> Vec<OriginalChange<OriginalKey, Box<dyn Debug>>> {
        self.reload().into_iter().map(|c| OriginalChange {
            key: OriginalKey::new::<S>(c.key),
            kind: match c.kind {
                ChangeKind::New => ChangeKind::New,
                ChangeKind::Deleted => ChangeKind::Deleted,
                ChangeKind::Altered => ChangeKind::Altered,
                ChangeKind::Unavailable(e) => ChangeKind::Unavailable(Box::new(e) as Box<dyn Debug>),
            }
        }).collect()
    }
}