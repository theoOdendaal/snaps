use snaps::{
    commands::{
        checkhealth::handle_checkhealth, list::handle_list, remove::handle_remove,
        take::handle_take,
    },
    error::Error,
};

// FIXME: Symbolic links should also be established in the snapshot.

// TODO: Create logic that will show the high level folder
// which will be included in the snapshot.

// TODO: How to identify corrupt snapshot?
// Copy to tmp and then mv to snapshot_dir?

#[derive(Debug)]
enum Command {
    Take {
        tag: Option<snaps::meta::RetentionTag>,
    },
    List {
        size: bool,
        indexes: Option<Vec<usize>>,
        acronyms: Option<Vec<snaps::meta::RetentionTag>>,
    },
    Restore {
        id: usize,
    },
    Remove {
        indexes: Option<Vec<usize>>,
        acronyms: Option<Vec<snaps::meta::RetentionTag>>,
        force: bool,
    },
    CheckHealth { dump: bool },
    Help,
    Version,
}

fn print_help() {
    let help_text = "\
\x1b[1mUsage:\x1b[0m snaps [COMMAND] [OPTIONS]

\x1b[1mCommands:\x1b[0m
  take                  Take a new snapshot
  list                  Display existing snapshots
  restore <ID>          Restore a specific snapshot
  rm                    Permanently remove one or more existing snapshot
  checkhealth           Sense check configurations

\x1b[1mOptions (for take):\x1b[0m
  --tag <TAG>         Assign specified tag to snapshot [u, h, d, w, m, a]

\x1b[1mOptions (for list):\x1b[0m
  -s, --size            Display allocated size for each snapshot
  -i <ITEMS>            Select snapshot(s) using using indexes (e.x. 1 or 1,3,4 or 1-21)
  -a <ACRONYMS>         Select snapshot(s) using tag acronyms [u, h, d, w, m, a]
  --remove              Remove selected snapshots, equivalent to using snap rm [OPTIONS]
  --force               Force action without explicit confirmation
                        \x1b[2m*(Note: Short flags can be combined, e.g., -sa, -si)\x1b[0m

\x1b[1mOptions (for rm):\x1b[0m
  -i <ITEMS>            Select snapshot(s) using using indexes (e.x. 1 or 1,3,4 or 1-21)
  -a <ACRONYMS>         Remove snapshot(s) selected using tag acronyms [u, h, d, w, m, a]
  --force               Force action without explicit confirmation

\x1b[1mOptions (for checkhealth):\x1b[0m
  --dump                Dump the existing snapshot directory into the metadata file

\x1b[1mGeneral Options:\x1b[0m
  -h, --help            Print help information
  -v, --version         Print version information
";
    print!("{}", help_text);
}

fn parse_acronyms(values: Option<&String>) -> Result<Vec<snaps::meta::RetentionTag>, Error> {
    values
        .map(|acrs| acrs.split(',').map(|a| a.try_into()).collect())
        .unwrap_or_else(|| Ok(Vec::new()))
}

fn parse_items(values: Option<&String>) -> Result<Vec<usize>, Error> {
    let Some(its) = values else {
        return Ok(Vec::new());
    };

    its.split(',')
        .map(|a| {
            let parts: Vec<&str> = a.split('-').collect();
            if parts.len() == 2 {
                let start = parts[0].parse::<usize>()?;
                let end = parts[1].parse::<usize>()?;
                assert!(start < end);
                Ok((start..=end).collect::<Vec<usize>>())
            } else {
                Ok(vec![a.parse::<usize>()?])
            }
        })
        .collect::<Result<Vec<Vec<usize>>, Error>>()
        .map(|nested| nested.into_iter().flatten().collect())
}

fn parse_id(value: Option<&String>) -> Result<usize, Error> {
    match value {
        Some(value) => Ok(value.parse()?),
        None => Err(Error::FromStrError("Missing id value".into())),
    }
}

fn parse_argument(value: std::env::Args) -> Result<Command, Error> {
    let args: Vec<String> = value.collect();

    let mut command: Option<&String> = None;
    let mut size = false;
    let mut take_tag = None;
    let mut indexes: Option<&String> = None;
    let mut acronyms: Option<&String> = None;
    let mut remove = false;
    let mut force = false;
    let mut dump = false;

    //let mut id: Option<&String> = None;

    // First argument is always the bin
    let mut iter = args.iter().skip(1).peekable();

    while let Some(arg) = iter.next() {
        if arg.starts_with('-') && !arg.starts_with("--") {
            for c in arg.chars().skip(1) {
                match c {
                    'h' => return Ok(Command::Help),
                    'v' => return Ok(Command::Version),
                    's' => size = true,
                    'i' => indexes = iter.next(),
                    'a' => acronyms = iter.next(),
                    _ => unimplemented!("Invalid short argument: {}", c),
                }
            }
        } else if arg.starts_with("--") {
            match arg.as_str() {
                "--tag" => take_tag = iter.next(),
                "--remove" => remove = true,
                "--force" => force = true,
                "--help" => return Ok(Command::Help),
                "--version" => return Ok(Command::Version),
                "--dump" => dump = true,
                _ => unimplemented!("Invalid long argument: {}", arg),
            }
        } else {
            command = Some(arg);
        }
    }

    let Some(cmd) = command else {
        return Ok(Command::Help);
    };

    match cmd.as_str() {
        "take" => {
            let tag = take_tag.and_then(|t| snaps::meta::RetentionTag::try_from(t.as_str()).ok());
            Ok(Command::Take { tag })
        }

        "list" if !remove => {
            let indexes = parse_items(indexes)?;
            let acronyms = parse_acronyms(acronyms)?;
            Ok(Command::List {
                size,
                indexes: Some(indexes),
                acronyms: Some(acronyms),
            })
        }

        //"restore" => Ok(Command::Restore { id: () },
        "rm" | "list" if remove => {
            let indexes = parse_items(indexes)?;
            let acronyms = parse_acronyms(acronyms)?;
            Ok(Command::Remove {
                indexes: Some(indexes),
                acronyms: Some(acronyms),
                force,
            })
        }

        "checkhealth" => Ok(Command::CheckHealth { dump }),

        _ => unimplemented!("Unknown command: {}", cmd),
    }
}

fn main() -> Result<(), Error> {
    
    let start = std::time::Instant::now();

    let command = parse_argument(std::env::args())?;

    match command {
        Command::Help => print_help(),

        Command::Version => println!("{} {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION")),

        Command::Take { tag } => {
            handle_take(tag)?;
        }

        Command::List {
            size,
            indexes,
            acronyms,
        } => {
            handle_list(size, indexes, acronyms)?;
        }

        Command::Restore { id: _ } => {
            unimplemented!()
        }

        Command::Remove {
            indexes,
            acronyms,
            force,
        } => {
            handle_remove(indexes, acronyms, force)?;
        }

        Command::CheckHealth { dump }=> {
            handle_checkhealth(dump)?;
        }
    }
    
    let duration = start.elapsed();
    println!("Execution time: {:?}", duration);


    Ok(())
}
