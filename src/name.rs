use crate::error::Error;

#[derive(Debug, Clone, Copy)]
pub struct SnapshotName {
    timestamp: u64,
    tag: Option<crate::tags::SnapshotTag>,
}

impl std::fmt::Display for SnapshotName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{}_{}", self.timestamp, self.tag.unwrap_or_default())
    }
}

impl<'a> SnapshotName {
   
    pub fn new(timestamp: u64, tag: Option<crate::tags::SnapshotTag>) -> Self {
        Self {
            timestamp,
            tag
        }
    }

    pub fn timestamp(&self) -> u64 {
        self.timestamp
    }

    pub fn tag(&self) -> Option<crate::tags::SnapshotTag> {
        self.tag
    }

    pub fn from_str(s: &'a str) -> Result<Self, Error> {
        
        let (ts_str, tag) = match s.rsplit_once("_") {
            Some((ts_str, tag_str)) => {
                let tag = crate::tags::SnapshotTag::try_from(tag_str)?;
                (ts_str, Some(tag))
            },
            None => (s, None)
        };

        let timestamp = ts_str.parse::<u64>()?;

        Ok(Self { timestamp, tag })

    }
}
