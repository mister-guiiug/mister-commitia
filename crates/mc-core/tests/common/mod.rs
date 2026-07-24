#![allow(dead_code)]

pub mod mockhttp;

use std::path::PathBuf;

use git2::{IndexAddOption, Oid, Repository, RepositoryInitOptions, Signature, Time};

pub fn skills_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../skills")
}

pub struct Fixture {
    pub dir: tempfile::TempDir,
    pub repo: Repository,
}

pub fn sig(t: i64) -> Signature<'static> {
    Signature::new("Test Author", "author@example.org", &Time::new(t, 0)).unwrap()
}

pub fn init_repo() -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let mut opts = RepositoryInitOptions::new();
    opts.initial_head("main");
    let repo = Repository::init_opts(dir.path(), &opts).unwrap();
    {
        let mut cfg = repo.config().unwrap();
        cfg.set_str("user.name", "Test Author").unwrap();
        cfg.set_str("user.email", "author@example.org").unwrap();
        cfg.set_str("commit.gpgsign", "false").unwrap();
    }
    Fixture { dir, repo }
}

pub fn commit(repo: &Repository, files: &[(&str, &str)], msg: &str, t: i64) -> Oid {
    let workdir = repo.workdir().unwrap();
    for (p, c) in files {
        let full = workdir.join(p);
        if let Some(parent) = full.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(full, c).unwrap();
    }
    let mut index = repo.index().unwrap();
    index
        .add_all(["*"].iter(), IndexAddOption::DEFAULT, None)
        .unwrap();
    index.write().unwrap();
    let tree_id = index.write_tree().unwrap();
    let tree = repo.find_tree(tree_id).unwrap();
    let s = sig(t);
    let parent_oid = repo.head().ok().and_then(|h| h.target());
    match parent_oid {
        Some(p) => {
            let parent = repo.find_commit(p).unwrap();
            repo.commit(Some("HEAD"), &s, &s, msg, &tree, &[&parent]).unwrap()
        }
        None => repo.commit(Some("HEAD"), &s, &s, msg, &tree, &[]).unwrap(),
    }
}

pub fn checkout_new_branch(repo: &Repository, name: &str) {
    let head = repo.head().unwrap().target().unwrap();
    let c = repo.find_commit(head).unwrap();
    repo.branch(name, &c, false).unwrap();
    repo.set_head(&format!("refs/heads/{name}")).unwrap();
    repo.checkout_head(Some(git2::build::CheckoutBuilder::new().force()))
        .unwrap();
}

/// Dépôt standard : `main` (2 commits) + `feature/checkout` (4 commits variés,
/// extraite de main, HEAD dessus). Retourne les 4 SHA de la feature.
pub fn feature_fixture() -> (Fixture, Vec<Oid>) {
    let f = init_repo();
    commit(&f.repo, &[("README.md", "# app\n")], "chore: init", 1_700_000_000);
    commit(
        &f.repo,
        &[("src/main.rs", "fn main() {}\n")],
        "feat(core): bootstrap application",
        1_700_000_100,
    );
    checkout_new_branch(&f.repo, "feature/checkout");
    let c1 = commit(
        &f.repo,
        &[("src/pay.rs", "pub fn pay() {}\n")],
        "feat(pay): add express payment flow",
        1_700_000_200,
    );
    let c2 = commit(
        &f.repo,
        &[("src/pay.rs", "pub fn pay() { /* v2 */ }\n")],
        "wip",
        1_700_000_300,
    );
    let c3 = commit(
        &f.repo,
        &[("src/pay.rs", "pub fn pay() { /* v3 */ }\n")],
        "fix stuff\n\n🤖 Generated with Claude Code\nCo-Authored-By: Claude <noreply@anthropic.com>",
        1_700_000_400,
    );
    let c4 = commit(
        &f.repo,
        &[("docs/pay.md", "# pay\n")],
        "update JIRA-123\n\nSigned-off-by: Jane Doe <jane@example.org>",
        1_700_000_500,
    );
    (f, vec![c1, c2, c3, c4])
}

pub fn core_with(f: &Fixture) -> (mc_core::Core, String) {
    std::env::set_var("MC_SECRETS_MODE", "memory");
    let core = mc_core::Core::in_memory(skills_dir()).unwrap();
    let repo = core
        .repo_declare(f.dir.path().to_str().unwrap())
        .unwrap();
    (core, repo.id)
}

/// Pousse `branch` vers un remote bare local + crée la réf remote-tracking.
pub fn add_remote_and_push(f: &Fixture, branch: &str) -> tempfile::TempDir {
    let bare_dir = tempfile::tempdir().unwrap();
    Repository::init_bare(bare_dir.path()).unwrap();
    f.repo
        .remote("origin", bare_dir.path().to_str().unwrap())
        .unwrap();
    let mut remote = f.repo.find_remote("origin").unwrap();
    remote
        .push(&[&format!("refs/heads/{branch}:refs/heads/{branch}")], None)
        .unwrap();
    remote
        .fetch(
            &[&format!("+refs/heads/{branch}:refs/remotes/origin/{branch}")],
            None,
            None,
        )
        .unwrap();
    bare_dir
}

pub fn op(seq: u32, operation: mc_core::model::Operation) -> mc_core::model::PlanOp {
    mc_core::model::PlanOp {
        seq,
        op: operation,
        origin: "test".into(),
        risk: mc_core::model::Risk::Low,
        approved_by: Some("test".into()),
        approved_at: None,
    }
}
