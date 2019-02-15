use std::sync::{
    atomic::{AtomicUsize, Ordering::SeqCst},
    Arc,
};

use super::*;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct Versions {
    #[serde(with = "ser")]
    rts: Arc<AtomicUsize>,
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

    pub(crate) fn bump_rts(&self, rts: u64) {
        let mut current = self.rts.load(SeqCst);
        while current < rts as usize {
            let ret = self.rts.compare_and_swap(
                current,
                rts as usize,
                SeqCst,
            );
            if ret == current {
                // cas successful
                break;
            }
            current = ret;
        }
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

pub(crate) mod ser {
    use std::sync::{
        atomic::{AtomicUsize, Ordering::SeqCst},
        Arc,
    };

    use serde::de::{Deserializer, Visitor};
    use serde::ser::Serializer;

    pub(crate) fn serialize<S>(
        data: &Arc<AtomicUsize>,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u64(data.load(SeqCst) as u64)
    }

    struct VersionsVisitor;

    impl<'de> Visitor<'de> for VersionsVisitor {
        type Value = Arc<AtomicUsize>;

        fn expecting(
            &self,
            formatter: &mut std::fmt::Formatter,
        ) -> std::fmt::Result {
            formatter.write_str("a borrowed byte array")
        }

        #[inline]
        fn visit_u64<E>(self, v: u64) -> Result<Arc<AtomicUsize>, E> {
            Ok(Arc::new(AtomicUsize::new(v as usize)))
        }
    }

    pub(crate) fn deserialize<'de, D>(
        deserializer: D,
    ) -> Result<Arc<AtomicUsize>, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_u64(VersionsVisitor)
    }
}
