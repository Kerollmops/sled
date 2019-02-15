use super::*;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct Versions {
    pending: Option<(u64, Option<IVec>)>,
    versions: Vec<(u64, Option<IVec>)>,
}

impl PartialEq for Versions {
    fn eq(&self, other: &Versions) -> bool {
        self.pending == other.pending
            && self.versions == other.versions
    }
}

impl Versions {
    pub(crate) fn apply(&mut self, frag: &Frag, _config: &Config) {
        match frag {
            Frag::PendingVersion(vsn, val) => {
                assert!(self.last_visible_lsn() < *vsn);
                self.pending = Some((*vsn, val.clone()));
            }
            Frag::CommitVersion(vsn) => {
                assert!(self.last_visible_lsn() < *vsn);
                if let Some((pending_vsn, val)) = self.pending.take()
                {
                    assert_eq!(pending_vsn, *vsn);
                    self.versions.push((pending_vsn, val));
                } else {
                    panic!("CommitVersion received on Frag without that version pending");
                }
            }
            Frag::PushVersion(vsn, val) => {
                assert!(self.last_visible_lsn() < *vsn);
                assert!(self.pending.is_none());
                self.versions.push((*vsn, val.clone()));
            }
            Frag::MergeVersion(vsn, val) => {
                assert!(self.last_visible_lsn() < *vsn);
                assert!(self.pending.is_none());
            }
            other => panic!(
                "Versions::apply called on unexpected frag: {:?}",
                other
            ),
        }
    }

    // returns the currently visible version at the given timestamp
    pub(crate) fn visible(&self, ts: u64) -> (u64, Option<IVec>) {
        if let Some((ref vts, ref val)) = self.pending {
            if *vts == ts {
                return (*vts, val.clone());
            }
        }

        for (ref vts, ref val) in self.versions.iter().rev() {
            if *vts <= ts {
                return (*vts, val.clone());
            }
        }

        (0, None)
    }

    fn last_visible_lsn(&self) -> u64 {
        self.versions
            .iter()
            .rev()
            .nth(0)
            .map(|(vsn, _)| *vsn)
            .unwrap_or(0)
    }
}
