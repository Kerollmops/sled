use super::*;

#[derive(
    Default, Clone, Eq, PartialEq, Debug, Serialize, Deserialize,
)]
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
            assert!(self.highest_visible_timestamp() < *ts);
            assert!(self.pending.is_some());
        } else {
            assert!(self.pending.is_none());
        }

        match frag {
            Frag::VersionPendingSet(ts, val) => {
                assert!(self.highest_visible_timestamp() < *ts);
                self.pending = Some(Version::Set(*ts, val.clone()));
            }
            Frag::VersionPendingMerge(ts, val) => {
                assert!(self.highest_visible_timestamp() < *ts);
                self.pending = Some(Version::Merge(*ts, val.clone()));
            }
            Frag::VersionPendingDel(ts) => {
                assert!(self.highest_visible_timestamp() < *ts);
                self.pending = Some(Version::Del(*ts));
            }
            Frag::VersionCommit(ts) => {
                assert!(self.highest_visible_timestamp() < *ts);
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
        key: &[u8],
        ts: u64,
        config: &Config,
    ) -> (u64, Option<IVec>) {
        let mut to_merge = vec![];
        let mut ret_ts = 0;

        if let Some(pending_vsn) = self.pending {
            if pending_vsn.ts() == ts {
                match pending_vsn {
                    Version::Del(ts) => return (ts, None),
                    Version::Set(ts, val) => {
                        return (ts, Some(val.clone()));
                    }
                    Version::Merge(ts, val) => {
                        to_merge.push(val);
                        if ret_ts == 0 {
                            ret_ts = ts;
                        }
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
                            if ret_ts == 0 {
                                ret_ts = *ts;
                            }
                            break;
                        }
                    }
                    Version::Merge(ts, val) => to_merge.push(*val),
                }
            }
        }

        if to_merge.is_empty() {
            assert_eq!(ret_ts, 0);
            return (ret_ts, None);
        }

        let merge_fn_ptr = config
            .merge_operator
            .expect("must have a merge operator set");

        let merge_fn: MergeOperator =
            unsafe { std::mem::transmute(merge_fn_ptr) };

        let mut new = to_merge.pop().unwrap();

        if to_merge.is_empty() {
            let new = merge_fn(key, None, &new);
            return (ret_ts, new.map(|v| v.into()));
        }

        while let Some(merge) = to_merge.pop() {
            let new = merge_fn(key, Some(&merge), &new);
        }

        (ret_ts, Some(new))
    }

    pub(crate) fn highest_visible_timestamp(&self) -> u64 {
        self.versions.last().map(|vsn| vsn.ts()).unwrap_or(0)
    }

    pub(crate) fn has_pending(&self) -> bool {
        self.pending.is_some()
    }
}

pub(crate) fn pull_version(
    pages: &PageCache<BLinkMaterializer, Frag, Recovery>,
    pid: PageId,
    key: &[u8],
    ts: u64,
    config: &Config,
    guard: &Guard,
) -> Result<(u64, Option<IVec>), ()> {
    let (versions_frag, _ptr) =
        pages.get(pid, guard).map(|page_get| page_get.unwrap())?;

    let versions = versions_frag.unwrap_versions();

    Ok(versions.visible(key, ts, config))
}
