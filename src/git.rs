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
    let workdir = match repo.workdir() {
        Some(w) => w,
        None => return out,
    };
    let rel = match path.canonicalize().ok().and_then(|p| p.strip_prefix(workdir).ok().map(|q| q.to_path_buf())) {
        Some(r) => r,
        None => return out,
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
                            out.entry(n).or_insert(LineChange::Added);
                        }
                    }
                    '-' => {
                        if let Some(n) = old_lineno {
                            if let Some(slot) = out.get_mut(&n) {
                                *slot = LineChange::Modified;
                            } else {
                                out.insert(n, LineChange::RemovedAbove);
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

    #[test]
    fn empty_for_non_repo() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("a.txt");
        fs::write(&f, "hello\n").unwrap();
        assert!(diff_for_file(&f).is_empty());
    }

    #[test]
    fn detects_added_line_in_repo() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path();
        let run = |args: &[&str]| {
            Command::new("git").args(args).current_dir(p).output().unwrap()
        };
        run(&["init", "-q"]);
        run(&["config", "user.email", "t@e.x"]);
        run(&["config", "user.name", "t"]);
        let f = p.join("a.txt");
        fs::write(&f, "line1\n").unwrap();
        run(&["add", "a.txt"]);
        run(&["commit", "-q", "-m", "init"]);
        fs::write(&f, "line1\nline2\n").unwrap();
        let map = diff_for_file(&f);
        assert_eq!(map.get(&2), Some(&LineChange::Added));
    }
}
