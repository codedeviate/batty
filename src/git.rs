use git2::{DiffOptions, Repository};
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineChange {
    Added,
    Modified,
    RemovedAbove,
}

/// Map of 1-indexed line numbers to change kind for a single file, comparing
/// the working-tree version against HEAD. Returns empty map if not in a repo
/// or the file is unchanged / untracked.
pub fn diff_for_file(path: &Path) -> HashMap<usize, LineChange> {
    let mut out = HashMap::new();
    let Ok(repo) = Repository::discover(path) else { return out };
    let Some(workdir) = repo.workdir().and_then(|w| w.canonicalize().ok()) else {
        return out;
    };
    let Some(rel) = path
        .canonicalize()
        .ok()
        .and_then(|p| p.strip_prefix(&workdir).ok().map(|q| q.to_path_buf()))
    else {
        return out;
    };

    let head_tree = match repo.head().and_then(|h| h.peel_to_tree()) {
        Ok(t) => t,
        Err(_) => return out,
    };

    let mut opts = DiffOptions::new();
    opts.pathspec(&rel);
    opts.context_lines(0);

    if let Ok(diff) = repo.diff_tree_to_workdir_with_index(Some(&head_tree), Some(&mut opts)) {
        let _ = diff.foreach(
            &mut |_, _| true,
            None,
            None,
            Some(&mut |_, _, line| {
                let new_lineno = line.new_lineno().map(|n| n as usize);
                let old_lineno = line.old_lineno().map(|n| n as usize);
                match line.origin() {
                    '+' => {
                        if let Some(n) = new_lineno {
                            match out.get(&n) {
                                Some(LineChange::RemovedAbove) => {
                                    out.insert(n, LineChange::Modified);
                                }
                                Some(_) => {} // already Added/Modified
                                None => {
                                    out.insert(n, LineChange::Added);
                                }
                            }
                        }
                    }
                    '-' => {
                        if let Some(n) = old_lineno {
                            match out.get(&n) {
                                Some(LineChange::Added) => {
                                    out.insert(n, LineChange::Modified);
                                }
                                Some(_) => {} // already Modified/RemovedAbove
                                None => {
                                    out.insert(n, LineChange::RemovedAbove);
                                }
                            }
                        }
                    }
                    _ => {}
                }
                true
            }),
        );
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::process::Command;

    fn run_git(p: &Path, args: &[&str]) {
        Command::new("git").args(args).current_dir(p).output().unwrap();
    }

    fn init_repo(p: &Path) {
        run_git(p, &["init", "-q"]);
        run_git(p, &["config", "user.email", "t@e.x"]);
        run_git(p, &["config", "user.name", "t"]);
    }

    #[test]
    fn empty_for_non_repo() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("a.txt");
        fs::write(&f, "hello\n").unwrap();
        assert!(diff_for_file(&f).is_empty());
    }

    #[test]
    fn detects_added_line() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path();
        init_repo(p);
        let f = p.join("a.txt");
        fs::write(&f, "line1\n").unwrap();
        run_git(p, &["add", "a.txt"]);
        run_git(p, &["commit", "-q", "-m", "init"]);
        fs::write(&f, "line1\nline2\n").unwrap();
        let map = diff_for_file(&f);
        assert_eq!(map.get(&2), Some(&LineChange::Added));
    }

    #[test]
    fn detects_modified_line() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path();
        init_repo(p);
        let f = p.join("a.txt");
        fs::write(&f, "alpha\n").unwrap();
        run_git(p, &["add", "a.txt"]);
        run_git(p, &["commit", "-q", "-m", "init"]);
        fs::write(&f, "ALPHA\n").unwrap();
        let map = diff_for_file(&f);
        assert_eq!(map.get(&1), Some(&LineChange::Modified), "got: {:?}", map);
    }

    #[test]
    fn detects_deleted_line() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path();
        init_repo(p);
        let f = p.join("a.txt");
        fs::write(&f, "line1\nline2\n").unwrap();
        run_git(p, &["add", "a.txt"]);
        run_git(p, &["commit", "-q", "-m", "init"]);
        fs::write(&f, "line1\n").unwrap();
        let map = diff_for_file(&f);
        // Deletion of line 2: gets reported as RemovedAbove keyed at old line number 2.
        assert_eq!(map.get(&2), Some(&LineChange::RemovedAbove), "got: {:?}", map);
    }

    #[test]
    fn untracked_file_returns_empty() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path();
        init_repo(p);
        // Make initial commit so HEAD exists
        let init = p.join("README");
        fs::write(&init, "x\n").unwrap();
        run_git(p, &["add", "README"]);
        run_git(p, &["commit", "-q", "-m", "init"]);
        // Now create an untracked file
        let f = p.join("untracked.txt");
        fs::write(&f, "new content\n").unwrap();
        assert!(diff_for_file(&f).is_empty(), "untracked file should produce no markers");
    }
}
