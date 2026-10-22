//! Trust-on-first-use. A device's key is pinned the first time we see it. If its
//! alias then shows up with a different key, that key is "suspect" until the user decides.
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum TrustLevel {
    New,
    Known,
    Verified,
    Suspect,
    Blocked,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Observation {
    Seen,
    New,
    IdentityChanged { old_fingerprint: String },
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
struct Record {
    alias: String,
    verified: bool,
    blocked: bool,
    auto_accept: bool,
}

pub struct TrustStore {
    path: Option<PathBuf>,
    records: HashMap<String, Record>,
    suspects: HashSet<String>,
}

impl TrustStore {
    pub fn load(path: PathBuf) -> Self {
        let records = std::fs::read(&path)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        Self {
            path: Some(path),
            records,
            suspects: HashSet::new(),
        }
    }

    pub fn in_memory() -> Self {
        Self {
            path: None,
            records: HashMap::new(),
            suspects: HashSet::new(),
        }
    }

    // ponytail: alias is the identity key for TOFU; two real devices with the same
    // name will trip it once each until the user accepts both keys.
    pub fn observe(&mut self, fp: &str, alias: &str) -> Observation {
        if let Some(r) = self.records.get_mut(fp) {
            if r.alias != alias {
                r.alias = alias.to_string();
                self.save();
            }
            return Observation::Seen;
        }
        if self.suspects.contains(fp) {
            return Observation::Seen;
        }
        if let Some(old) = self
            .records
            .iter()
            .find(|(_, r)| r.alias == alias && !r.blocked)
            .map(|(k, _)| k.clone())
        {
            self.suspects.insert(fp.to_string());
            return Observation::IdentityChanged {
                old_fingerprint: old,
            };
        }
        self.records.insert(
            fp.to_string(),
            Record {
                alias: alias.to_string(),
                ..Default::default()
            },
        );
        self.save();
        Observation::New
    }

    pub fn level(&self, fp: &str) -> TrustLevel {
        match self.records.get(fp) {
            Some(r) if r.blocked => TrustLevel::Blocked,
            _ if self.suspects.contains(fp) => TrustLevel::Suspect,
            Some(r) if r.verified => TrustLevel::Verified,
            Some(_) => TrustLevel::Known,
            None => TrustLevel::New,
        }
    }

    /// What `observe` would conclude, without pinning anything. Used for
    /// claims we could not prove (the peer may be lying about its key).
    pub fn peek(&self, fp: &str, alias: &str) -> TrustLevel {
        match self.level(fp) {
            TrustLevel::New
                if self
                    .records
                    .values()
                    .any(|r| r.alias == alias && !r.blocked) =>
            {
                TrustLevel::Suspect
            }
            level => level,
        }
    }

    pub fn auto_accept(&self, fp: &str) -> bool {
        self.records
            .get(fp)
            .is_some_and(|r| r.auto_accept && !r.blocked)
    }

    /// "It's a reinstall": trust this key. The old key stays pinned too.
    pub fn accept_new_key(&mut self, fp: &str, alias: &str) {
        self.suspects.remove(fp);
        self.records
            .entry(fp.to_string())
            .or_insert_with(|| Record {
                alias: alias.to_string(),
                ..Default::default()
            });
        self.save();
    }

    pub fn set_verified(&mut self, fp: &str, verified: bool) {
        if let Some(r) = self.records.get_mut(fp) {
            r.verified = verified;
            self.save();
        }
    }

    pub fn set_auto_accept(&mut self, fp: &str, on: bool) {
        if let Some(r) = self.records.get_mut(fp) {
            r.auto_accept = on;
            self.save();
        }
    }

    pub fn block(&mut self, fp: &str, alias: &str) {
        self.suspects.remove(fp);
        let r = self
            .records
            .entry(fp.to_string())
            .or_insert_with(|| Record {
                alias: alias.to_string(),
                ..Default::default()
            });
        r.blocked = true;
        self.save();
    }

    fn save(&self) {
        let Some(path) = &self.path else { return };
        match serde_json::to_vec_pretty(&self.records) {
            Ok(bytes) => {
                if let Err(e) = std::fs::write(path, bytes) {
                    tracing::warn!("saving trust store: {e}");
                }
            }
            Err(e) => tracing::warn!("encoding trust store: {e}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_sight_pins_then_known() {
        let mut t = TrustStore::in_memory();
        assert_eq!(t.level("AA"), TrustLevel::New);
        assert_eq!(t.observe("AA", "bob"), Observation::New);
        assert_eq!(t.observe("AA", "bob"), Observation::Seen);
        assert_eq!(t.level("AA"), TrustLevel::Known);
    }

    #[test]
    fn same_alias_new_key_is_suspect_until_accepted() {
        let mut t = TrustStore::in_memory();
        t.observe("AA", "bob");
        assert_eq!(
            t.observe("BB", "bob"),
            Observation::IdentityChanged {
                old_fingerprint: "AA".into()
            }
        );
        assert_eq!(t.level("BB"), TrustLevel::Suspect);
        // no repeated event for the same suspect
        assert_eq!(t.observe("BB", "bob"), Observation::Seen);
        t.accept_new_key("BB", "bob");
        assert_eq!(t.level("BB"), TrustLevel::Known);
        assert_eq!(t.level("AA"), TrustLevel::Known);
    }

    #[test]
    fn verified_blocked_auto_accept() {
        let mut t = TrustStore::in_memory();
        t.observe("AA", "bob");
        t.set_verified("AA", true);
        assert_eq!(t.level("AA"), TrustLevel::Verified);
        assert!(!t.auto_accept("AA"));
        t.set_auto_accept("AA", true);
        assert!(t.auto_accept("AA"));
        t.block("CC", "mallory");
        assert_eq!(t.level("CC"), TrustLevel::Blocked);
        assert_eq!(t.observe("CC", "mallory"), Observation::Seen);
    }

    #[test]
    fn peek_predicts_without_pinning() {
        let mut t = TrustStore::in_memory();
        t.observe("AA", "bob");
        assert_eq!(t.peek("BB", "bob"), TrustLevel::Suspect);
        assert_eq!(t.peek("CC", "carol"), TrustLevel::New);
        assert_eq!(t.peek("AA", "x"), TrustLevel::Known);
        assert_eq!(t.level("BB"), TrustLevel::New);
        assert_eq!(t.observe("CC", "carol"), Observation::New);
    }

    #[test]
    fn persists_to_disk() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("trust.json");
        {
            let mut t = TrustStore::load(path.clone());
            t.observe("AA", "bob");
            t.set_verified("AA", true);
        }
        let t = TrustStore::load(path);
        assert_eq!(t.level("AA"), TrustLevel::Verified);
    }

    #[test]
    fn corrupt_file_starts_empty() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("trust.json");
        std::fs::write(&path, "{not json").unwrap();
        assert_eq!(TrustStore::load(path).level("AA"), TrustLevel::New);
    }
}
