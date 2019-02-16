use super::*;

#[derive(Clone, Eq, PartialEq, Debug, Serialize, Deserialize)]
pub(crate) struct Versions {
    pending: Option<Version>,
    versions: Vec<Version>,
}

#[derive(Clone, Eq, PartialEq, Debug, Serialize, Deserialize)]
enum Version {
    Del(u64),
    Set(u64, IVec),
    Merge(u64, IVec),
}

impl Version {
    fn ts(&self) -> u64 {
        match *self {
            Version::Del(ts)
            | Version::Set(ts, _)
            | Version::Merge(ts, _) => ts,
        }
    }
}

impl Versions {
    pub(crate) fn apply(&mut self, frag: &Frag, _config: &Config) {
        if let Frag::VersionCommit(ts) = frag {
            assert!(self.last_visible_lsn() < *ts);
            assert!(self.pending.is_some());
        } else {
            assert!(self.pending.is_none());
        }

        match frag {
            Frag::VersionSet(ts, val) => {
                assert!(self.last_visible_lsn() < *ts);
                self.versions.push(Version::Set(*ts, val.clone()));
            }
            Frag::VersionMerge(ts, val) => {
                assert!(self.last_visible_lsn() < *ts);
                self.versions.push(Version::Merge(*ts, val.clone()));
            }
            Frag::VersionDel(ts) => {
                assert!(self.last_visible_lsn() < *ts);
                self.versions.push(Version::Del(*ts));
            }
            Frag::VersionPendingSet(ts, val) => {
                assert!(self.last_visible_lsn() < *ts);
                self.pending = Some(Version::Set(*ts, val.clone()));
            }
            Frag::VersionPendingMerge(ts, val) => {
                assert!(self.last_visible_lsn() < *ts);
                self.pending = Some(Version::Merge(*ts, val.clone()));
            }
            Frag::VersionPendingDel(ts) => {
                assert!(self.last_visible_lsn() < *ts);
                self.pending = Some(Version::Del(*ts));
            }
            Frag::VersionCommit(ts) => {
                assert!(self.last_visible_lsn() < *ts);
                if let Some(pending_vsn) = self.pending.take() {
                    assert_eq!(pending_vsn.ts(), *ts);
                    self.versions.push(pending_vsn);
                } else {
                    panic!("VersionCommit received on Frag without that version pending");
                }
            }
            other => panic!(
                "Versions::apply called on unexpected frag: {:?}",
                other
            ),
        }
    }

    // returns the currently visible version at the given timestamp
    pub(crate) fn visible(&self, ts: u64) -> Version {
        if let Some(pending_vsn) = self.pending {
            if pending_vsn.ts() == ts {
                return pending_vsn.clone();
            }
        }

        for vsn in self.versions.iter().rev() {
            if vsn.ts() <= ts {
                return vsn.clone();
            }
        }

        Version::Del(0)
    }

    fn last_visible_lsn(&self) -> u64 {
        self.versions.last().map(|vsn| vsn.ts()).unwrap_or(0)
    }
}
