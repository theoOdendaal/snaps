// FIXME: This needs to be greatly improved.
// I feel like there are too many unnecessary
// file being included.

const IGNORED_ROOT_DIRECTORIES: &[&str] = &[
    "bin",
    //"boot",
    "dev",
    //"etc",
    //"home",
    "lib",
    "lib64",
    "lost+found",
    "media",
    "mnt",
    "opt",
    "proc",
    //"root",
    "run",
    "sbin",
    "srv",
    "sys",
    "tmp",
    //"usr",
    //"var"
];

const IGNORED_COMPONENTS: &[&str] = &["target", ".cache", ".cargo"];

const IGNORED_ROOT_ANCHORED_COMPONENTS: &[&[&str]] = &[
    &["var", env!("CARGO_PKG_NAME")],
    &["var", "lib", "pacman"],
    &["var", "tmp"],
    &["var", "cache"],
    &["var", "log"],
    &["var", "run"],
    &["var", "lock"],
    &["home", "theo", "projects"],
    &["home", "theo", ".MathWorks"],
    &["home", "theo", ".rustup"],
];

const TAIL_ANCHORED_COMPONENTS: &[&[&str]] = &[
    &[".local", "share", "nvim"],
    &[".local", "state", "nvim"],
    &[".config", "mozilla", "firefox"],
    &[".config", "chromium"],
];


fn root_level_exclusion(path: &std::path::Path) -> bool {
    if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
        return IGNORED_ROOT_DIRECTORIES.contains(&file_name);
    }
    false
}

// TODO: This can be heavily optimized.
// Why do I recheck the prefixes every time?
// At most it should be checked at the start?
pub fn should_ignore(path: &std::path::Path) -> bool {

    if let Some(parent) = path.parent()
        && parent == "/"
        && root_level_exclusion(path)
    {
        return true;
    }

    // Since the fs walk is recursive, I only need
    // to consider the last componenent of the
    // path passed. Previous ineligible components
    // would have already been excluded.
    if let Some(folder) = path.file_name()
        && IGNORED_COMPONENTS.iter().any(|&c| folder == c)
    {
        return true;
    }

    let comps: Vec<&str> = path
        .components()
        .filter_map(|c| match c {
            std::path::Component::Normal(s) => s.to_str(),
            _ => None,
        })
        .collect();

    for rule in IGNORED_ROOT_ANCHORED_COMPONENTS {
        if comps.starts_with(rule) {
            return true;
        }
    }

    for rule in TAIL_ANCHORED_COMPONENTS {
        if comps.ends_with(rule) {
            return true;
        }
    }

    false
}

#[cfg(test)]
mod filter_tests {
    use super::*;

    #[test]
    fn test_root_level_exclusions() {
        let proc_path = std::path::Path::new("/proc");
        assert!(should_ignore(proc_path));

        let sys_path = std::path::Path::new("/sys");
        assert!(should_ignore(sys_path));

        let etc_path = std::path::Path::new("/etc");
        assert!(!should_ignore(etc_path));
    }
}
