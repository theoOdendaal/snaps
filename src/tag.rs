// This crate should still be used to independently reconstruct the metadata file
// when its corrupted.

/*use crate::error::Error;

// FIXME: Weekly should only start after last daily. Otherwise there is too much overlap between
// daily and weekly.

const HOUR_IN_SECONDS: u64 = 60 * 60;
const DAY_IN_SECONDS: u64 = HOUR_IN_SECONDS * 24;
const WEEK_IN_SECONDS: u64 = DAY_IN_SECONDS * 7;

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum Tags {
    Untagged,
    Today,
    Yesterday,
    Daily,
    Weekly,
}

impl From<&Tags> for u8 {
    fn from(value: &Tags) -> Self {
        match value {
            Tags::Untagged => 0,
            Tags::Today => FLAG_TODAY,
            Tags::Yesterday => FLAG_YESTERDAY,
            Tags::Daily => FLAG_DAILY,
            Tags::Weekly => FLAG_WEEKLY,
        }
    }
}

impl TryFrom<&str> for Tags {
    type Error = Error;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "u" | "U" => Ok(Tags::Untagged),
            "t" | "T" => Ok(Tags::Today),
            "y" | "Y" => Ok(Tags::Yesterday),
            "d" | "D" => Ok(Tags::Daily),
            "w" | "W" => Ok(Tags::Weekly),
            _ => Err(Error::FromStrError(format!(
                "Unable to parse '{}' to 'Tag'",
                value
            ))),
        }
    }
}

// FIXME:Weekly snaps should remain static, and be based on a pre-defined start.
// If I just do NOW / WEEK_AS_SECONDS, it will be rolling?

const FLAG_TODAY: u8 = 0b0000_0001;
const FLAG_YESTERDAY: u8 = 0b0000_0010;
const FLAG_DAILY: u8 = 0b0000_0100;
const FLAG_WEEKLY: u8 = 0b0000_1000;

// Aggrgates a collection of tags into a
// single bitmask that can be evaluated
// using matches_any.
pub fn tag_collection_into_bitmask(tags: &[Tags]) -> u8 {
    let mut bitmask = 0u8;

    for t in tags {
        bitmask |= u8::from(t);
    }
    bitmask
}

pub fn matches_any(mask: u8, query_mask: u8) -> bool {
    (mask & query_mask) != 0
}

fn has_no_tag(mask: u8) -> bool {
    mask == 0
}

fn has_today_tag(mask: u8) -> bool {
    (mask & FLAG_TODAY) != 0
}

fn has_yesterday_tag(mask: u8) -> bool {
    (mask & FLAG_YESTERDAY) != 0
}

fn has_daily_tag(mask: u8) -> bool {
    (mask & FLAG_DAILY) != 0
}

fn has_weekly_tag(mask: u8) -> bool {
    (mask & FLAG_WEEKLY) != 0
}

pub fn get_tag_map(mask: u8) -> (bool, bool, bool, bool, bool) {
    (
        has_no_tag(mask),
        has_today_tag(mask),
        has_yesterday_tag(mask),
        has_daily_tag(mask),
        has_weekly_tag(mask),
    )
}

pub fn generate_tags(snapshot_timestamps: &[u64]) -> Result<Vec<(u64, u8)>, Error> {
    // Define retention policy.
    const TODAY_MAX: usize = 5;
    const YESTERDAY_MAX: usize = 5;
    const DAILY_MAX: usize = 7;
    const WEEKLY_MAX: usize = 3;

    // Define policy counters.
    let mut today_count: usize = 0;
    let mut yesterday_count: usize = 0;
    let mut daily_count: usize = 0;
    let mut weekly_count: usize = 0;

    // Sort existing snapshots descending.
    let mut sorted = snapshot_timestamps.to_vec();
    sorted.sort_unstable_by(|a, b| b.cmp(a));
    let snapshot_count = sorted.len();

    let now_secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs();

    // Create baseline values.
    let today_start = (now_secs / DAY_IN_SECONDS) * DAY_IN_SECONDS;
    let yesterday_start = today_start - DAY_IN_SECONDS;

    let mut last_daily = u64::MAX;
    let mut last_weekly = u64::MAX;

    let mut results = Vec::with_capacity(snapshot_count);
    for s in sorted {
        let mut tag = 0u8;

        // Bucket into tag category.
        // If > today, then it can only have the
        // today tag. Nesting is used to ensure
        // to ensure iteration does not traverse
        // all conditions.
        if s > today_start {
            if today_count < TODAY_MAX {
                today_count += 1;
                tag |= FLAG_TODAY;
            }
        } else {
            if s > yesterday_start && yesterday_count < YESTERDAY_MAX {
                yesterday_count += 1;
                tag |= FLAG_YESTERDAY;
            }

            let current_daily = s / DAY_IN_SECONDS;
            if current_daily != last_daily && daily_count < DAILY_MAX {
                last_daily = current_daily;
                daily_count += 1;
                tag |= FLAG_DAILY;
            }

            let current_weekly = s / WEEK_IN_SECONDS;
            if current_weekly != last_weekly && weekly_count < WEEKLY_MAX {
                last_weekly = current_weekly;
                weekly_count += 1;
                tag |= FLAG_WEEKLY;
            }
        }

        results.push((s, tag));
    }

    Ok(results)
}

#[cfg(test)]
mod tag_tests {
    use super::*;

    #[test]
    fn test_empty_snapshots() {
        let snaps: [u64; 0] = [];
        let tags = generate_tags(&snaps).unwrap();

        let today_snaps: Vec<_> = tags
            .iter()
            .filter_map(|(a, b)| if has_today_tag(*b) { Some(a) } else { None })
            .collect();
        let yesterday_snaps: Vec<_> = tags
            .iter()
            .filter_map(|(a, b)| if has_yesterday_tag(*b) { Some(a) } else { None })
            .collect();
        let daily_snaps: Vec<_> = tags
            .iter()
            .filter_map(|(a, b)| if has_daily_tag(*b) { Some(a) } else { None })
            .collect();
        let weekly_snaps: Vec<_> = tags
            .iter()
            .filter_map(|(a, b)| if has_weekly_tag(*b) { Some(a) } else { None })
            .collect();

        assert!(today_snaps.is_empty());
        assert!(yesterday_snaps.is_empty());
        assert!(daily_snaps.is_empty());
        assert!(weekly_snaps.is_empty());
    }

    #[test]
    fn test_snaps() {
        if let Ok(system_time) = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
        {
            let system_time = system_time.as_secs();

            let snaps = [
                system_time - WEEK_IN_SECONDS,
                system_time - (DAY_IN_SECONDS + 2 * HOUR_IN_SECONDS),
                system_time - (DAY_IN_SECONDS + HOUR_IN_SECONDS),
                system_time - DAY_IN_SECONDS,
                system_time,
            ];

            let tags = generate_tags(&snaps).unwrap();

            let today_snaps: Vec<_> = tags
                .iter()
                .filter_map(|(a, b)| if has_today_tag(*b) { Some(a) } else { None })
                .collect();
            let yesterday_snaps: Vec<_> = tags
                .iter()
                .filter_map(|(a, b)| if has_yesterday_tag(*b) { Some(a) } else { None })
                .collect();
            let daily_snaps: Vec<_> = tags
                .iter()
                .filter_map(|(a, b)| if has_daily_tag(*b) { Some(a) } else { None })
                .collect();
            let weekly_snaps: Vec<_> = tags
                .iter()
                .filter_map(|(a, b)| if has_weekly_tag(*b) { Some(a) } else { None })
                .collect();

            assert_eq!(today_snaps, [&snaps[4]]);
            assert_eq!(yesterday_snaps, [&snaps[3], &snaps[2], &snaps[1]]);
            assert_eq!(daily_snaps, [&snaps[3], &snaps[0]]);
            assert_eq!(weekly_snaps, [&snaps[3], &snaps[0]]);
        }
    }
}*/
