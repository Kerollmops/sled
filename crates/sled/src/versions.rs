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
    pub(crate) fn apply(&mut self, frag: &Frag) {
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

    // returns the currently visible version at the given timestamp,
    // possibly using a merge operator to consolidate multiple
    pub(crate) fn visible(
        &self,
        ts: u64,
        config: &Config,
    ) -> (u64, Option<IVec>) {
        let mut to_merge = vec![];

        if let Some(pending_vsn) = self.pending {
            if pending_vsn.ts() == ts {
                match pending_vsn {
                    Version::Del(ts) => return (ts, None),
                    Version::Set(ts, val) => {
                        return (ts, Some(val.clone()));
                    }
                    Version::Merge(ts, val) => {
                        to_merge.push(val);
                    }
                }
            }
        }

        for vsn in self.versions.iter().rev() {
            if vsn.ts() <= ts {
                match vsn {
                    Version::Del(ts) => {
                        if to_merge.is_empty() {
                            return (*ts, None);
                        } else {
                            break;
                        }
                    }
                    Version::Set(ts, val) => {
                        if to_merge.is_empty() {
                            return (*ts, Some(val.clone()));
                        } else {
                            to_merge.push(*val);
                            break;
                        }
                    }
                    Version::Merge(ts, val) => to_merge.push(*val),
                }
            }
        }

        if to_merge.is_empty() {
            return (0, None);
        }

        let merge_fn_ptr = config
            .merge_operator
            .expect("must have a merge operator set");

        unsafe {
            let merge_fn: MergeOperator =
                std::mem::transmute(merge_fn_ptr);
            let new =
                merge_fn(&*decoded_k, Some(&records[idx].1), &val);
        }
    }

    fn last_visible_lsn(&self) -> u64 {
        self.versions.last().map(|vsn| vsn.ts()).unwrap_or(0)
    }
}
