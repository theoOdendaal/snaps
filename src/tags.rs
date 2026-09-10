
use crate::error::Error;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SnapshotTag {
    Untagged,
    Hourly,
    Daily,
    Weekly,
    Adhoc(char),
}

impl Default for SnapshotTag {
    fn default() -> Self {
        Self::Untagged
    }
}

impl std::fmt::Display for SnapshotTag {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Untagged => write!(f, "u"),
            Self::Hourly=> write!(f, "h"),
            Self::Daily => write!(f, "d"),
            Self::Weekly=> write!(f, "w"),
            Self::Adhoc(c)=> write!(f, "a{}", c),
        
        }
    }
}

impl<'a> SnapshotTag {
    pub fn get_tag_mask(&self) -> String {
        let mask = match self {
            Self::Untagged => "u----",
            Self::Hourly=> "-h---",
            Self::Daily=> "--d--",
            Self::Weekly => "---w-",
            Self::Adhoc(_)=> "----a",
        };
        mask.into()
    }
}



impl<'a> TryFrom<&'a str> for SnapshotTag {
    type Error = Error;

    fn try_from(value: &'a str) -> Result<Self, Self::Error> {
        
        if value.is_empty() {
            return Err(Error::EmptyTag);
        }

        let mut chars = value.chars();

        match (chars.next(), chars.next()) {
            (Some('u'), None) => Ok(Self::Untagged),
            (Some('h'), None) => Ok(Self::Hourly),
            (Some('d'), None) => Ok(Self::Daily),
            (Some('w'), None) => Ok(Self::Weekly),
            (Some('a'), Some(c)) if chars.next().is_none() && c.is_alphanumeric() => {
                Ok(Self::Adhoc(c))
            },

            _ => Err(Error::UnknownTag(value.into()))

        }
    }

}

/*
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_tags() {
        assert_eq!(SnapshotTag::try_from("h"), Ok(SnapshotTag::Hourly));
        assert_eq!(SnapshotTag::try_from("d"), Ok(SnapshotTag::Daily));
        assert_eq!(SnapshotTag::try_from("w"), Ok(SnapshotTag::Weekly));
        assert_eq!(SnapshotTag::try_from("a1"), Ok(SnapshotTag::Adhoc('1')));
        assert_eq!(SnapshotTag::try_from("aa"), Ok(SnapshotTag::Adhoc('a')));
    }

    #[test]
    fn test_invalid_tags() {
        assert_eq!(SnapshotTag::try_from("a42"), Err(Error::UnknownTag("a42")));
        assert_eq!(SnapshotTag::try_from(""), Err(Error::EmptyTag));
        assert_eq!(SnapshotTag::try_from("hourly"), Err(Error::UnknownTag("hourly")));
        assert_eq!(SnapshotTag::try_from("a"), Err(Error::UnknownTag("a")));
        assert_eq!(SnapshotTag::try_from("a "), Err(Error::UnknownTag("a ")));
    }
}*/
