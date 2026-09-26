use std::cmp::Ordering;

pub(super) fn compare_versions(left: &str, right: &str) -> Option<Ordering> {
    let left = ParsedVersion::parse(left)?;
    let right = ParsedVersion::parse(right)?;
    Some(left.cmp(&right))
}

#[derive(Eq, PartialEq)]
struct ParsedVersion<'a> {
    core: [u64; 4],
    pre_release: Option<Vec<&'a str>>,
}

impl<'a> ParsedVersion<'a> {
    fn parse(value: &'a str) -> Option<Self> {
        let (value, build) = value
            .split_once('+')
            .map_or((value, None), |(value, build)| (value, Some(build)));
        if build.is_some_and(|build| !valid_version_identifiers(build, false)) {
            return None;
        }
        let (core, pre_release) = value
            .split_once('-')
            .map_or((value, None), |(core, pre)| (core, Some(pre)));
        let parts = core.split('.').collect::<Vec<_>>();
        if !(2..=4).contains(&parts.len()) || pre_release == Some("") {
            return None;
        }
        let mut numbers = [0; 4];
        for (index, part) in parts.into_iter().enumerate() {
            if part.is_empty() || (part.len() > 1 && part.starts_with('0')) {
                return None;
            }
            numbers[index] = part.parse().ok()?;
        }
        let pre_release = pre_release
            .filter(|value| valid_version_identifiers(value, true))
            .map(|value| value.split('.').collect::<Vec<_>>());
        if value.contains('-') && pre_release.is_none() {
            return None;
        }
        Some(Self {
            core: numbers,
            pre_release,
        })
    }
}

fn valid_version_identifiers(value: &str, reject_numeric_leading_zero: bool) -> bool {
    value.split('.').all(|part| {
        !part.is_empty()
            && part
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
            && (!reject_numeric_leading_zero
                || !part.bytes().all(|byte| byte.is_ascii_digit())
                || part.len() == 1
                || !part.starts_with('0'))
    })
}

impl Ord for ParsedVersion<'_> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.core
            .cmp(&other.core)
            .then_with(|| match (&self.pre_release, &other.pre_release) {
                (None, None) => Ordering::Equal,
                (None, Some(_)) => Ordering::Greater,
                (Some(_), None) => Ordering::Less,
                (Some(left), Some(right)) => compare_pre_release(left, right),
            })
    }
}

impl PartialOrd for ParsedVersion<'_> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

fn compare_pre_release(left: &[&str], right: &[&str]) -> Ordering {
    for (left, right) in left.iter().zip(right) {
        let ordering = match (left.parse::<u64>(), right.parse::<u64>()) {
            (Ok(left), Ok(right)) => left.cmp(&right),
            (Ok(_), Err(_)) => Ordering::Less,
            (Err(_), Ok(_)) => Ordering::Greater,
            (Err(_), Err(_)) => left.cmp(right),
        };
        if ordering != Ordering::Equal {
            return ordering;
        }
    }
    left.len().cmp(&right.len())
}
