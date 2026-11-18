use std::fs;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crates dir")
        .parent()
        .expect("repo root")
        .to_path_buf()
}

fn collect_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if name == "node_modules" || name == "target" || name == ".git" || name == "dist" {
                continue;
            }
            collect_files(&path, out);
        } else {
            out.push(path);
        }
    }
}

#[test]
fn repository_contains_no_real_credential_prefixes() {
    let root = repo_root();
    let forbidden = vec![
        ["AK", "IA"].concat(),
        ["gh", "p_"].concat(),
        ["github_", "pat_"].concat(),
        ["xo", "xb-"].concat(),
        ["-----BEGIN ", "RSA PRIVATE KEY-----"].concat(),
        ["-----BEGIN ", "PRIVATE KEY-----"].concat(),
    ];

    let mut files = Vec::new();
    for folder in ["fixtures", "crates", "apps", "packages"] {
        collect_files(&root.join(folder), &mut files);
    }

    for file in files {
        if let Ok(text) = fs::read_to_string(&file) {
            for marker in &forbidden {
                assert!(
                    !text.contains(marker),
                    "forbidden credential marker '{}' found in '{}'",
                    marker,
                    file.display()
                );
            }
        }
    }
}
