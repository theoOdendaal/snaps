// Snapshot metadata is stored in /var/snaps/snapshots/arch-theo/.snapshot-meta

use crate::error::Error;

const METADATA_FILE: &str = "/var/snaps/snapshots/arch-theo/.snapshot-meta";

#[repr(u8)]
pub enum RetentionTag {
    Untagged,
    Hourly,
    Daily,
    Weekly,
    Monthly,
    Adhoc,
}

impl<'a> std::fmt::Display for RetentionTag {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Untagged => write!(f, "untagged"),
            Self::Hourly => write!(f, "hourly"),
            Self::Daily => write!(f, "daily"),
            Self::Weekly => write!(f, "weekly"),
            Self::Monthly => write!(f, "monthly"),
            Self::Adhoc => write!(f, "adhoc"),
        }
    }
}

impl<'a> TryFrom<&'a str> for RetentionTag {
    type Error = Error;

    fn try_from(value: &'a str) -> Result<Self, Self::Error> {
        match value {
            "untagged" => Ok(Self::Untagged),
            "hourly" => Ok(Self::Hourly),
            "daily" => Ok(Self::Daily),
            "weekly" => Ok(Self::Weekly),
            "monthly" => Ok(Self::Monthly),
            "adhoc" => Ok(Self::Adhoc),
            _ => Err(Error::UnknownTag(value.into())),
        }
    }
}

pub struct SnapshotMetaData<'a> {
    timestamp: u64,
    tags: &'a [RetentionTag],
}

impl<'a> SnapshotMetaData<'a> {
    pub fn new(timestamp: u64, tags: &'a [RetentionTag]) -> Self {
        Self { timestamp, tags }
    }

}

fn read_metadata_file_to_string() -> Result<String, Error> {
    Ok(std::fs::read_to_string(METADATA_FILE)?)
}

fn parse_metadata<'a>(file_content: &'a str) -> Result<Vec<SnapshotMetaData>, Error> {

    let mut snapshots = Vec::new();

    // Snpashots are separated using ";"
    for snapshot in file_content.split(";") {
        let snapshot = snapshot.trim();
        
        if snapshot.is_empty() {
            continue;
        }
        
        let (head, tail) = match snapshot.rsplit_once(":") {
            Some((head, tail)) => (head, Some(tail)),
            None => (snapshot, None),
        };

        let timestamp = head.parse::<u64>()?;

        let tags = match tail {
            Some(tags) => {

                tags.split(",").into_iter().map(|t| RetentionTag::try_from).collect()
            },
            None => vec![],

        };

        snapshots.push(SnapshotMetaData {
            timestamp,
            tags,
        }); 
    }
    Ok(snapshots)
    
}


fn parse_retention_tags<'a>(s: String) -> Result<Vec<RetentionTag>, Error> {
    
    s.split(",").map(|t| RetentionTag::try_from).collect()
    

}
