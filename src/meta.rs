// Snapshot metadata is stored in /var/snaps/snapshots/arch-theo/.snapshot-meta

use crate::error::Error;

const METADATA_FILE: &str = "/var/snaps/snapshots/arch-theo/.snapshot-meta";

#[repr(u8)]
#[derive(Debug, PartialEq, Clone, Copy)]
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

impl RetentionTag {
    pub fn get_tags_mask(tags: &[RetentionTag]) -> String {
        format!("{}{}{}{}{}{}",
            if tags.contains(&RetentionTag::Untagged) { "u" } else {"-"},
            if tags.contains(&RetentionTag::Hourly) { "h" } else {"-"},
            if tags.contains(&RetentionTag::Daily) { "d" } else {"-"},
            if tags.contains(&RetentionTag::Weekly) { "w" } else {"-"},
            if tags.contains(&RetentionTag::Monthly) { "m" } else {"-"},
            if tags.contains(&RetentionTag::Adhoc) { "a" } else {"-"},
            )


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

impl TryFrom<char> for RetentionTag {
    type Error = Error;

    fn try_from(value: char) -> Result<Self, Self::Error> {
        match value {
            'u' => Ok(Self::Untagged),
            'h' => Ok(Self::Hourly),
            'd' => Ok(Self::Daily),
            'w' => Ok(Self::Weekly),
            'm' => Ok(Self::Monthly),
            'a' => Ok(Self::Adhoc),
            _ => Err(Error::UnknownTag(value.into()))
        }
    }
}

#[derive(Debug)]
pub struct SnapshotMetaData {
    timestamp: u64,
    tags: Vec<RetentionTag>,
}

impl<'a> SnapshotMetaData {
    pub fn new(timestamp: u64, tags: Vec<RetentionTag>) -> Self {
        Self { timestamp, tags }
    }

    pub fn tags(&'a self) -> &'a [RetentionTag] {
        &self.tags
    }

    pub fn contains_tag(&self, tag: &RetentionTag) -> bool {
        self.tags.contains(tag)
    }

    

}

pub fn read_metadata_file_to_string() -> Result<String, Error> {
    Ok(std::fs::read_to_string(METADATA_FILE)?)
}

pub fn parse_metadata<'a>(file_content: &'a str) -> Result<Vec<SnapshotMetaData>, Error> {

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

        let tags: Vec<RetentionTag> = match tail {
            Some(tags) => {

                tags.split(",").into_iter().map(|t| RetentionTag::try_from(t)).collect::<Result<_, Error>>()?
            },
            None => vec![],

        };

        snapshots.push(SnapshotMetaData {
            timestamp,
            tags,
        }); 
    }
    
    snapshots.sort_unstable_by(|a, b| b.timestamp.cmp(&a.timestamp));


    Ok(snapshots)
    
}
