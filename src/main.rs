use anyhow::{anyhow, Result};
use gix::state::InProgress;
use gix::{
    sec::{self, trust::DefaultForLevel},
    Repository, ThreadSafeRepository,
};
use log::debug;
use std::process::Command;
use std::{path::PathBuf, process::exit};

pub struct Repo {
    pub repo: ThreadSafeRepository,

    /// If `current_dir` is a git repository or is contained within one,
    /// this is the current branch name of that repo.
    pub branch: Option<String>,

    /// If `current_dir` is a git repository or is contained within one,
    /// this is the path to the root of that repo.
    pub workdir: Option<PathBuf>,

    /// The path of the repository's `.git` directory.
    pub path: PathBuf,

    /// State
    pub state: Option<InProgress>,
    // /// Remote repository
    // pub remote: Option<Remote>,
}

const MODIFY_STATUS: &[&str] = &["M", "A", "T", "D", "R", "C", "U"];

fn main() {
    env_logger::init();

    let path = PathBuf::from(".");

    let s = match get_output(path) {
        Ok(output) => output,
        Err(e) => {
            debug!("{e}");
            exit(1);
        }
    };

    let cmd = Command::new("git")
        .arg("status")
        .arg("--porcelain")
        .output();

    let mut status = 0;

    if let Ok(cmd) = cmd {
        if cmd.status.success() {
            let out = String::from_utf8_lossy(&cmd.stdout);
            let mut out = out
                .trim()
                .split('\n')
                .filter_map(|x| x.rsplit_once(' '))
                .map(|x| x.0);

            match out.next() {
                None => {
                    println!("{s}");
                    status = 5;
                }
                Some(x)
                    if MODIFY_STATUS.contains(&x)
                        || MODIFY_STATUS.contains(&&x[..1])
                        || MODIFY_STATUS.contains(&&x[1..2]) =>
                {
                    println!("{s}");
                    status = 6;
                }
                Some("??") => {
                    println!("{s}");
                    status = 7;
                }
                _ => {
                    println!("{s}");
                }
            }
        } else {
            println!("{s}");
            status = 8;
        }
    }

    exit(status)
}

fn get_output(path: PathBuf) -> Result<String> {
    // custom open options
    let repo = get_repo(path)?;

    let git_repo = repo.repo.to_thread_local();

    let display_name = repo
        .branch
        .or_else(|| Some(git_repo.head_id().ok()?.to_hex_with_len(7).to_string()));

    let display_name = display_name.ok_or_else(|| anyhow!("can not get branch/hash name"))?;

    let s = if let Some(state) = repo.state {
        match state {
            InProgress::ApplyMailbox => format!("Mailbox progress {display_name}"),
            InProgress::ApplyMailboxRebase => {
                format!("Mailbox rebase progress {display_name}")
            }
            InProgress::Bisect => format!("Bisect progress {display_name}"),
            InProgress::CherryPick => format!("Cherry pick progress {display_name}"),
            InProgress::CherryPickSequence => {
                format!("Cherry pick sequence progress {display_name}")
            }
            InProgress::Merge => format!("Merge progress {display_name}"),
            InProgress::Rebase => format!("Rebase progress {display_name}"),
            InProgress::RebaseInteractive => {
                format!("Rebasing {display_name}")
            }
            InProgress::Revert => format!("Revert progress {display_name}"),
            InProgress::RevertSequence => format!("Revert Sequence progress {display_name}"),
        }
    } else {
        display_name
    };

    Ok(s)
}

fn get_repo(path: PathBuf) -> Result<Repo> {
    let mut git_open_opts_map = sec::trust::Mapping::<gix::open::Options>::default();

    let config = gix::open::permissions::Config {
        git_binary: false,
        system: false,
        git: false,
        user: false,
        env: true,
        includes: true,
    };
    git_open_opts_map.reduced = git_open_opts_map
        .reduced
        .permissions(gix::open::Permissions {
            config,
            ..gix::open::Permissions::default_for_level(sec::Trust::Reduced)
        });
    git_open_opts_map.full = git_open_opts_map.full.permissions(gix::open::Permissions {
        config,
        ..gix::open::Permissions::default_for_level(sec::Trust::Full)
    });
    let shared_repo = match ThreadSafeRepository::discover_with_environment_overrides_opts(
        path,
        Default::default(),
        git_open_opts_map,
    ) {
        Ok(repo) => repo,
        Err(e) => {
            return Err(anyhow!("Failed to find git repo: {e}").context(e));
        }
    };
    let repository = shared_repo.to_thread_local();
    let branch = get_current_branch(&repository);
    // let remote = get_remote_repository_info(&repository, branch.as_deref());
    let path = repository.path().to_path_buf();

    // let fs_monitor_value_is_true = repository;

    let repo = Repo {
        repo: shared_repo,
        branch,
        workdir: repository.work_dir().map(PathBuf::from),
        path,
        state: repository.state(),
        // remote,
    };

    Ok(repo)
}

fn get_current_branch(repository: &Repository) -> Option<String> {
    let name = repository.head_name().ok()??;
    let shorthand = name.shorten();

    Some(shorthand.to_string())
}
