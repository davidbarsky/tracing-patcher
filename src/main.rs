use chrono::{DateTime, Utc};
use git2::{BranchType, Repository};
use std::collections::BTreeSet;

fn main() -> Result<(), anyhow::Error> {
    let repo = Repository::open("/Users/dbarsky/Developer/tracing")?;

    let main = repo.find_branch("master", BranchType::Local)?;
    let main = main
        .into_reference()
        .target()
        .expect("Unable to unwrap main Oid");
    let backport = repo.find_branch("v0.1.x", BranchType::Local)?;
    let backport = backport
        .into_reference()
        .target()
        .expect("Unable to unwrap backport Oid");

    let until: DateTime<Utc> = DateTime::parse_from_rfc2822("18 Feb 2023 23:00:00 GMT")?.into();

    let main_commits = commits_on_branch(&repo, main, Some(&until))?;
    let backport_commits = commits_on_branch(&repo, backport, Some(&until))?;

    let difference = main_commits.difference(&backport_commits);
    for diff in difference {
        println!("{}", diff);
    }

    Ok(())
}

fn commits_on_branch(
    repo: &Repository,
    oid: git2::Oid,
    until: Option<&DateTime<Utc>>,
) -> Result<BTreeSet<String>, anyhow::Error> {
    let mut walk = repo.revwalk()?;
    walk.set_sorting(git2::Sort::TIME)?;
    walk.push(oid)?;
    let mut backport_commits = BTreeSet::new();
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

        let time_since_epoch = commit.time().seconds();
        let dt: DateTime<Utc> =
            DateTime::<Utc>::from_timestamp(time_since_epoch, 0).expect("unable to convert");

        if let Some(until) = until {
            if dt >= *until {
                backport_commits.insert(next);
            }
        }
    }
    Ok(backport_commits)
}
