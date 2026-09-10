use std::fs::OpenOptions;
use std::io::Write;

use crate::error::Error;

const METADATA_FILE: &str = "/var/snaps/snapshots/arch-theo/.snapshot-meta";

#[repr(u8)]
#[derive(Debug, Default, PartialEq, Clone, Copy)]
pub enum RetentionTag {
    #[default]
    Untagged,
    Hourly,
    Daily,
    Weekly,
    Monthly,
    Adhoc,
}

impl RetentionTag {
    pub const ALL: [RetentionTag;6] = [
        Self::Untagged,
        Self::Hourly,
        Self::Daily,
        Self::Weekly,
        Self::Monthly,
        Self::Adhoc
    ];

    pub const fn as_char(self) -> char {
        match self {
            Self::Untagged => 'u',
            Self::Hourly => 'h',
            Self::Daily => 'd',
            Self::Weekly => 'w',
            Self::Monthly => 'm',
            Self::Adhoc => 'a',
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Untagged => "untagged",
            Self::Hourly => "hourly",
            Self::Daily => "daily",
            Self::Weekly => "weekly",
            Self::Monthly => "monthly",
            Self::Adhoc => "adhoc",
        }
    }

    pub fn get_tags_mask(tags: &[RetentionTag]) -> String {
        Self::ALL.iter().map(|tag| if tags.contains(tag) { tag.as_char() } else {'-'} ).collect()
    }


}

impl std::fmt::Display for RetentionTag {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
       f.write_str(self.as_str()) 
    }
}

impl TryFrom<&str> for RetentionTag {
    type Error = Error;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
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
            _ => Err(Error::UnknownTag(value.into())),
        }
    }
}

#[derive(Debug)]
pub struct SnapshotMetaData {
    timestamp: u64,
    tags: Vec<RetentionTag>,
}

impl std::fmt::Display for SnapshotMetaData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let tags = self
            .tags
            .iter()
            .map(|f| f.to_string())
            .collect::<Vec<_>>()
            .join(",");
        write!(f, "{}:{};", self.timestamp, tags)
    }
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

    pub fn append_to_metadata_file(&self) -> Result<(), Error> {
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(METADATA_FILE)?;
        writeln!(file, "{}", self)?;

        Ok(())
    }

    pub fn overwrite_metadata_file(snapshots: &[SnapshotMetaData]) -> Result<(), Error> {
        let mut file = std::fs::File::create(METADATA_FILE)?;
        for line in snapshots {
            writeln!(file, "{}", line)?;
        }
        Ok(())
    }
}

pub fn read_metadata_file_to_string() -> Result<String, Error> {
    Ok(std::fs::read_to_string(METADATA_FILE)?)
}

pub fn parse_metadata(file_content: &str) -> Result<Vec<SnapshotMetaData>, Error> {
    let mut snapshots = Vec::new();

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
            Some(tags) => tags
                .split(",")
                .map(RetentionTag::try_from)
                .collect::<Result<_, Error>>()?,
            None => vec![],
        };

        snapshots.push(SnapshotMetaData { timestamp, tags });
    }

    snapshots.sort_unstable_by_key(|a| std::cmp::Reverse(a.timestamp));

    Ok(snapshots)
}
