use std::fmt;
use std::hash::Hash;

use chrono::{DateTime, NaiveDateTime, Utc};
use clap::Parser;
use git2::Repository;
use regex::Regex;
use rustc_hash::FxHashSet;

#[derive(Debug, Default, Ord)]
struct Commit {
    message: String,
    pull_request: i64,
    date: DateTime<Utc>,
}

impl PartialEq for Commit {
    fn eq(&self, other: &Self) -> bool {
        self.pull_request == other.pull_request
    }
}

impl Eq for Commit {}

impl Hash for Commit {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.pull_request.hash(state);
    }
}

impl PartialOrd for Commit {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.pull_request.partial_cmp(&other.pull_request)
    }
}

impl fmt::Display for Commit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({})", self.message, self.date)
    }
}

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path corresponding to the location of `tracing`'s repo.
    path: std::path::PathBuf,
    /// Date, in a year-month-day hour-minute-second format.
    since: String,
}

fn main() -> Result<(), anyhow::Error> {
    let args = Args::parse();

    let repo = Repository::open(args.path)?;
    let since = NaiveDateTime::parse_from_str(&args.since, "%F %X")?.and_utc();

    let main = repo.find_branch("master", git2::BranchType::Local)?;
    let main = main
        .into_reference()
        .target()
        .expect("Unable to unwrap main Oid");
    let backport = repo.find_branch("v0.1.x", git2::BranchType::Local)?;
    let backport = backport
        .into_reference()
        .target()
        .expect("Unable to unwrap backport Oid");

    let main_commits = commits_on_branch(&repo, main, Some(&since))?;
    let backport_commits = commits_on_branch(&repo, backport, Some(&since))?;

    let mut difference = main_commits
        .difference(&backport_commits)
        .collect::<Vec<&Commit>>();

    difference.sort_by(|a, b| a.date.cmp(&b.date));

    for diff in difference {
        println!("{}", diff)
    }

    Ok(())
}

fn commits_on_branch(
    repo: &Repository,
    oid: git2::Oid,
    until: Option<&DateTime<Utc>>,
) -> Result<FxHashSet<Commit>, anyhow::Error> {
    let mut walk = repo.revwalk()?;
    walk.set_sorting(git2::Sort::TIME)?;
    walk.push(oid)?;
    let mut backport_commits: FxHashSet<Commit> = FxHashSet::default();

    // attemping to match on a parenthesized (#2700)
    let re = Regex::new(r"(?:\(#)(\d{4}|\d{3}|\d{2}|\d{1})(?:\))")?;

    for commit in walk {
        let oid = commit?;
        let commit = repo.find_commit(oid)?;
        let message = commit
            .message()
            .map(|message| message.to_owned().clone())
            .unwrap();

        let next = message
            .lines()
            .next()
            .expect("Unable to get first line")
            .to_owned();

        let mut pull_request = 0;
        if let Some(cap) = re.captures(&next) {
            let num = &cap[1];
            pull_request = num.parse()?;
        }

        let time_since_epoch = commit.time().seconds();
        let dt: DateTime<Utc> =
            DateTime::<Utc>::from_timestamp(time_since_epoch, 0).expect("unable to convert");

        let commit = Commit {
            message: next,
            pull_request,
            date: dt,
        };

        if let Some(until) = until {
            if dt >= *until {
                backport_commits.insert(commit);
            }
        }
    }
    Ok(backport_commits)
}
