use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::Write;

use crate::error::{Error, WithContext};

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

#[derive(Debug, PartialEq)]
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

impl SnapshotMetaData {
    pub fn new(timestamp: u64, tags: Vec<RetentionTag>) -> Result<Self, Error> {
        
        if tags.is_empty() {
            return Err(Error::EmptyTag("Unable to create snapshot metadata with empty tag".into()));
        }

        Ok(Self { timestamp, tags })
    }

    pub fn timestamp(&self) -> u64 {
        self.timestamp
    }

    pub fn tags(& self) -> &[RetentionTag] {
        &self.tags
    }

    pub fn contains_tag(&self, tag: &RetentionTag) -> bool {
        self.tags.contains(tag)
    }

    pub fn append_to_metadata_file(&self) -> Result<(), Error> {
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(METADATA_FILE)
            .with_context(METADATA_FILE)?;
        writeln!(file, "{}", self)?;

        Ok(())
    }

    pub fn overwrite_metadata_file(snapshots: &[SnapshotMetaData]) -> Result<(), Error> {

        let mut file = std::fs::File::create(METADATA_FILE)
            .with_context(METADATA_FILE)?;

        for line in snapshots {
            writeln!(file, "{}", line)?;
        }
        Ok(())
    }
}

pub fn read_metadata_file_to_string() -> Result<String, Error> {
    Ok(std::fs::read_to_string(METADATA_FILE)
        .with_context(METADATA_FILE)?)
}

struct MetadataFileIter<'a> {
    content: std::str::Split<'a, &'a str>,
}

// FIXME: Should I not make this TryFrom rather?
impl<'a> From<&'a str> for MetadataFileIter<'a> {
    fn from(value: &'a str) -> Self {
        Self { content: value.split(";")}
    }
}

impl<'a> Iterator for MetadataFileIter<'a> {
    type Item = Result<SnapshotMetaData, Error>;

    fn next(&mut self) -> Option<Self::Item> {

        if let Some(snapshot) = self.content.next() {
            let trimmed = snapshot.trim();

            if trimmed.is_empty() { return self.next(); }

            let (head, tail) = match trimmed.rsplit_once(":") {
                Some((head, tail)) => (head, Some(tail)),
                None => (snapshot, None),
            };
            
            let timestamp = match head.parse::<u64>() {
                Ok(t) => t,
                Err(e) => return Some(Err(e.into())),
            };

            let tags: Vec<RetentionTag> = match tail {
                Some(tags) => {
                    let parsed = tags
                    .split(",")
                    .map(RetentionTag::try_from)
                    .collect::<Result<_, Error>>();

                    match parsed {
                        Ok(t) => t,
                        Err(e) => return Some(Err(e)),
                    }

                },

                None => vec![],
            };

            Some(Ok(SnapshotMetaData { timestamp, tags }))

        } else {
            None
        }
        
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.content.size_hint()
    }
}


pub fn parse_metadata_as_ordered_vec(file_content: &str) -> Result<Vec<SnapshotMetaData>, Error> {
    
    let iter = MetadataFileIter::from(file_content);
   
    let mut snapshots  = iter.collect::<Result<Vec<SnapshotMetaData>, Error>>()?;

    snapshots.sort_unstable_by_key(|a| std::cmp::Reverse(a.timestamp));

    Ok(snapshots)

}

pub fn parse_metadata_as_hashmap(file_content: &str) -> Result<HashMap<u64, Vec<RetentionTag>>, Error> {
    
    let iter = MetadataFileIter::from(file_content);

    let mut snapshots = HashMap::new();

    for snapshot in iter {
        let snapshot = snapshot?;
        let timestamp = snapshot.timestamp();
        let tags = snapshot.tags().to_vec();

        snapshots.insert(timestamp, tags);
    }

    Ok(snapshots)
}

pub fn dump_snapshots() -> Result<(), Error> {

    let host_location = crate::location::get_host_location()?;
    let snapshots = crate::location::retrieve_snapshots_as_ordered_vec(&host_location)?;

    let metadata_file_content = read_metadata_file_to_string()?;
    let metadata = parse_metadata_as_hashmap(&metadata_file_content)?;

    let metadata  = snapshots.iter().map(|s| {
        let tags = metadata.get(s).unwrap_or(&vec![RetentionTag::Untagged]).to_vec();
        SnapshotMetaData::new(*s, tags)
    }).collect::<Result<Vec<SnapshotMetaData>, Error>>()?;
    
    crate::meta::SnapshotMetaData::overwrite_metadata_file(&metadata)?;

    Ok(())
}
