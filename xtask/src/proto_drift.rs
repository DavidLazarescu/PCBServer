//! `cargo xtask proto-drift`: compare the vendored KiCad IPC definitions in
//! `crates/konnect-ipc/proto/` with KiCad's `api/proto/` at one tag.
//!
//! The vendored tree is a byte-for-byte copy of one KiCad tag, recorded in
//! `crates/konnect-ipc/proto/README.md`. This task reports how a given tag
//! differs from it and, with `--update`, replaces the tree with that tag, so
//! a resync starts from one command. Deciding what the new fields mean for
//! the client stays a person's job.
//!
//! It reads KiCad's GitLab through `curl` and `git` (network required), or a
//! local KiCad checkout with `--from`. It is never part of the test suite.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;

const PROJECT: &str = "kicad%2Fcode%2Fkicad";
const REPOSITORY: &str = "https://gitlab.com/kicad/code/kicad";
const UPSTREAM_DIR: &str = "api/proto";
/// GitLab's largest page. A full page could mean a truncated listing.
const PAGE: usize = 100;

pub const USAGE: &str = "cargo xtask proto-drift <kicad-tag> [--from <kicad-checkout>] [--update]";

type Tree = BTreeMap<String, Vec<u8>>;

/// How the vendored tree differs from upstream. Paths are relative to the
/// proto root, with `/` separators.
#[derive(Debug, Default, PartialEq)]
struct Drift {
    changed: Vec<String>,
    /// Upstream has it, the vendored tree does not.
    added: Vec<String>,
    /// The vendored tree has it, upstream does not.
    removed: Vec<String>,
}

impl Drift {
    fn is_empty(&self) -> bool {
        self.changed.is_empty() && self.added.is_empty() && self.removed.is_empty()
    }
}

fn compare(vendored: &Tree, upstream: &Tree) -> Drift {
    let mut drift = Drift::default();
    for (path, bytes) in upstream {
        match vendored.get(path) {
            None => drift.added.push(path.clone()),
            Some(ours) if ours != bytes => drift.changed.push(path.clone()),
            Some(_) => {}
        }
    }
    drift.removed = vendored
        .keys()
        .filter(|path| !upstream.contains_key(*path))
        .cloned()
        .collect();
    drift
}

/// The proto paths `build.rs` hands to prost, relative to the proto root.
fn compiled_paths(build_rs: &str) -> BTreeSet<String> {
    build_rs
        .split('"')
        .filter_map(|literal| literal.strip_prefix("proto/"))
        .filter(|path| path.ends_with(".proto"))
        .map(str::to_string)
        .collect()
}

/// Lines present in `new` and absent from `old`, as a count of each side.
/// A multiset difference, not a diff: enough to size a change; read the
/// files themselves (or `git diff` after `--update`) for the content.
fn line_delta(old: &[u8], new: &[u8]) -> (usize, usize) {
    fn count(bytes: &[u8]) -> BTreeMap<&[u8], usize> {
        let mut lines = BTreeMap::new();
        for line in bytes.split(|b| *b == b'\n') {
            *lines.entry(line).or_default() += 1;
        }
        lines
    }
    fn surplus(a: &BTreeMap<&[u8], usize>, b: &BTreeMap<&[u8], usize>) -> usize {
        a.iter()
            .map(|(line, n)| n.saturating_sub(*b.get(line).unwrap_or(&0)))
            .sum()
    }
    let (old, new) = (count(old), count(new));
    (surplus(&new, &old), surplus(&old, &new))
}

/// `Since:` annotations upstream that the vendored file does not have yet.
fn new_since_tags(old: &[u8], new: &[u8]) -> BTreeSet<String> {
    let tags = |bytes: &[u8]| -> BTreeSet<String> {
        String::from_utf8_lossy(bytes)
            .lines()
            .filter_map(|line| line.split_once("Since:"))
            .map(|(_, version)| version.trim().to_string())
            .collect()
    };
    let known = tags(old);
    tags(new).difference(&known).cloned().collect()
}

fn read_tree(root: &Path) -> Result<Tree, String> {
    let mut tree = Tree::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries =
            std::fs::read_dir(&dir).map_err(|e| format!("cannot read {}: {e}", dir.display()))?;
        for entry in entries {
            let path = entry.map_err(|e| e.to_string())?.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|ext| ext == "proto") {
                let relative = path
                    .strip_prefix(root)
                    .expect("walked from root")
                    .components()
                    .map(|c| c.as_os_str().to_string_lossy().into_owned())
                    .collect::<Vec<_>>()
                    .join("/");
                let bytes = std::fs::read(&path)
                    .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
                tree.insert(relative, bytes);
            }
        }
    }
    Ok(tree)
}

fn run_tool(program: &str, args: &[&str]) -> Result<Vec<u8>, String> {
    let output = Command::new(program)
        .args(args)
        .output()
        .map_err(|e| format!("cannot run {program}: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "{program} {} failed ({}): {}",
            args.join(" "),
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(output.stdout)
}

fn fetch(url: &str) -> Result<Vec<u8>, String> {
    run_tool(
        "curl",
        &["--silent", "--show-error", "--fail", "--location", url],
    )
}

fn fetch_upstream(tag: &str) -> Result<Tree, String> {
    let listing_url = format!(
        "https://gitlab.com/api/v4/projects/{PROJECT}/repository/tree\
         ?path={UPSTREAM_DIR}&ref={tag}&recursive=true&per_page={PAGE}"
    );
    let listing: serde_json::Value = serde_json::from_slice(&fetch(&listing_url)?)
        .map_err(|e| format!("GitLab returned an unreadable tree listing: {e}"))?;
    let entries = listing
        .as_array()
        .ok_or_else(|| format!("GitLab did not list {UPSTREAM_DIR} at '{tag}': {listing}"))?;
    if entries.len() >= PAGE {
        return Err(format!(
            "the listing filled a whole page of {PAGE}; it may be truncated, so nothing \
             is compared. Use --from with a KiCad checkout."
        ));
    }
    let prefix = format!("{UPSTREAM_DIR}/");
    let mut tree = Tree::new();
    for entry in entries {
        if entry["type"] != "blob" {
            continue;
        }
        let path = entry["path"]
            .as_str()
            .ok_or("a tree entry without a path")?;
        let Some(relative) = path.strip_prefix(&prefix) else {
            continue;
        };
        if !relative.ends_with(".proto") {
            continue;
        }
        let bytes = fetch(&format!("{REPOSITORY}/-/raw/{tag}/{path}"))?;
        tree.insert(relative.to_string(), bytes);
    }
    if tree.is_empty() {
        return Err(format!("no .proto files under {UPSTREAM_DIR} at '{tag}'"));
    }
    Ok(tree)
}

/// The commit a tag points at, peeled through an annotated tag object.
fn tag_commit(tag: &str) -> Result<String, String> {
    let refs = format!("refs/tags/{tag}");
    let peeled = format!("{refs}^{{}}");
    let out = run_tool(
        "git",
        &["ls-remote", &format!("{REPOSITORY}.git"), &refs, &peeled],
    )?;
    let out = String::from_utf8_lossy(&out);
    let find = |name: &str| {
        out.lines()
            .filter_map(|line| line.split_once('\t'))
            .find(|(_, r)| *r == name)
            .map(|(hash, _)| hash.to_string())
    };
    find(&peeled)
        .or_else(|| find(&refs))
        .ok_or_else(|| format!("KiCad has no tag '{tag}'"))
}

fn write_tree(root: &Path, upstream: &Tree, drift: &Drift) -> Result<(), String> {
    for path in drift.changed.iter().chain(&drift.added) {
        let target = root.join(path);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        std::fs::write(&target, &upstream[path])
            .map_err(|e| format!("cannot write {}: {e}", target.display()))?;
    }
    for path in &drift.removed {
        let target = root.join(path);
        std::fs::remove_file(&target)
            .map_err(|e| format!("cannot remove {}: {e}", target.display()))?;
    }
    Ok(())
}

/// Returns the process exit code: 0 when the trees agree (or `--update`
/// replaced them), 1 when they differ, 2 when the comparison could not run.
pub fn run(repo_root: &Path, args: &[String]) -> i32 {
    let mut tag = None;
    let mut from: Option<PathBuf> = None;
    let mut update = false;
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--update" => update = true,
            "--from" => match args.next() {
                Some(dir) => from = Some(PathBuf::from(dir)),
                None => return usage("--from needs a directory"),
            },
            flag if flag.starts_with('-') => return usage(&format!("unknown option {flag}")),
            value if tag.is_none() => tag = Some(value.to_string()),
            extra => return usage(&format!("unexpected argument {extra}")),
        }
    }
    let Some(tag) = tag else {
        return usage("a KiCad tag is required, e.g. 10.0.8");
    };

    let proto_root = repo_root.join("crates/konnect-ipc/proto");
    let result = (|| -> Result<i32, String> {
        let vendored = read_tree(&proto_root)?;
        let (upstream, source) = match &from {
            Some(checkout) => (
                read_tree(&checkout.join(UPSTREAM_DIR))?,
                format!("{} (checkout, tag not verified)", checkout.display()),
            ),
            None => (
                fetch_upstream(&tag)?,
                format!("KiCad {tag}, commit {}", tag_commit(&tag)?),
            ),
        };
        let build_rs = std::fs::read_to_string(repo_root.join("crates/konnect-ipc/build.rs"))
            .map_err(|e| format!("cannot read build.rs: {e}"))?;
        let compiled = compiled_paths(&build_rs);
        let drift = compare(&vendored, &upstream);

        println!("upstream: {source}");
        println!(
            "vendored: {} files, upstream: {} files",
            vendored.len(),
            upstream.len()
        );
        for path in &drift.changed {
            let (added, removed) = line_delta(&vendored[path], &upstream[path]);
            let since = new_since_tags(&vendored[path], &upstream[path]);
            let since = if since.is_empty() {
                String::new()
            } else {
                format!(
                    ", new Since: {}",
                    since.into_iter().collect::<Vec<_>>().join(", ")
                )
            };
            println!("  changed  {path} (+{added} -{removed} lines{since})");
        }
        for path in &drift.added {
            println!("  new      {path}");
        }
        for path in &drift.removed {
            println!("  gone     {path}");
        }
        for path in upstream.keys().filter(|path| !compiled.contains(*path)) {
            println!("  note: build.rs does not compile {path}");
        }
        for path in compiled.iter().filter(|path| !upstream.contains_key(*path)) {
            println!("  warning: build.rs compiles {path}, which upstream does not have");
        }

        if drift.is_empty() {
            println!("no drift: the vendored tree matches byte for byte");
            return Ok(0);
        }
        if !update {
            println!("drift found; rerun with --update to replace the vendored tree");
            return Ok(1);
        }
        write_tree(&proto_root, &upstream, &drift)?;
        println!(
            "updated {} file(s). Next: build.rs for new files, the provenance table in \
             crates/konnect-ipc/proto/README.md, and every new field in builders.rs, \
             client.rs and types.rs.",
            drift.changed.len() + drift.added.len() + drift.removed.len()
        );
        Ok(0)
    })();
    match result {
        Ok(code) => code,
        Err(error) => {
            eprintln!("proto-drift: {error}");
            2
        }
    }
}

fn usage(problem: &str) -> i32 {
    eprintln!("proto-drift: {problem}");
    eprintln!("usage: {USAGE}");
    2
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tree(entries: &[(&str, &str)]) -> Tree {
        entries
            .iter()
            .map(|(path, text)| (path.to_string(), text.as_bytes().to_vec()))
            .collect()
    }

    #[test]
    fn compare_names_changed_added_and_removed_files() {
        let vendored = tree(&[("a.proto", "x"), ("b.proto", "y"), ("old.proto", "z")]);
        let upstream = tree(&[("a.proto", "x"), ("b.proto", "y2"), ("new.proto", "w")]);
        assert_eq!(
            compare(&vendored, &upstream),
            Drift {
                changed: vec!["b.proto".into()],
                added: vec!["new.proto".into()],
                removed: vec!["old.proto".into()],
            }
        );
        assert!(compare(&vendored, &vendored).is_empty());
    }

    #[test]
    fn compiled_paths_reads_the_prost_list_from_build_rs() {
        let build_rs = r#"
            println!("cargo:rerun-if-changed=proto");
            let protos = &[
                "proto/common/envelope.proto",
                "proto/board/board.proto",
            ];
        "#;
        assert_eq!(
            compiled_paths(build_rs),
            ["board/board.proto", "common/envelope.proto"]
                .map(String::from)
                .into()
        );
    }

    #[test]
    fn line_delta_and_since_tags_size_a_change() {
        let old = b"message A\n{\n  int32 a = 1;\n}\n";
        let new = b"message A\n{\n  int32 a = 1;\n  // Since: 10.0.7\n  bool b = 2;\n}\n";
        assert_eq!(line_delta(old, new), (2, 0));
        assert_eq!(
            new_since_tags(old, new),
            BTreeSet::from(["10.0.7".to_string()])
        );
    }

    #[test]
    fn the_vendored_tree_is_what_build_rs_compiles() {
        // Every file prost is given must exist, or the build fails far from
        // the cause; this keeps the list honest without the network.
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        let vendored = read_tree(&root.join("crates/konnect-ipc/proto")).unwrap();
        let build_rs = std::fs::read_to_string(root.join("crates/konnect-ipc/build.rs")).unwrap();
        for path in compiled_paths(&build_rs) {
            assert!(
                vendored.contains_key(&path),
                "build.rs compiles missing {path}"
            );
        }
    }
}
