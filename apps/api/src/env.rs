use std::path::{Path, PathBuf};

pub fn load_env() {
    let root = workspace_root();
    load_pair(&root);
}

pub fn workspace_root() -> PathBuf {
    let start = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    find_workspace_root(&start).unwrap_or(start)
}

fn load_pair(root: &Path) {
    let local = root.join(".env.local");
    let base = root.join(".env");
    let _ = dotenvy::from_path(local);
    let _ = dotenvy::from_path(base);
}

fn find_workspace_root(start: &Path) -> Option<PathBuf> {
    for candidate in start.ancestors() {
        if candidate.join("pnpm-workspace.yaml").is_file()
            && candidate.join("apps/api/Cargo.toml").is_file()
        {
            return Some(candidate.to_path_buf());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_repo_root_from_api_subdir() {
        let root = unique_tmp("repo-root");
        std::fs::create_dir_all(root.join("apps/api")).unwrap();
        std::fs::write(root.join("pnpm-workspace.yaml"), "packages: []\n").unwrap();
        std::fs::write(root.join("apps/api/Cargo.toml"), "[package]\nname='x'\n").unwrap();

        let found = find_workspace_root(&root.join("apps/api")).unwrap();
        assert_eq!(found, root);
    }

    #[test]
    fn local_env_wins_and_base_fills() {
        let key_override = unique_key("OVERRIDE");
        let key_fill = unique_key("FILL");
        std::env::remove_var(&key_override);
        std::env::remove_var(&key_fill);

        let root = unique_tmp("env-load");
        std::fs::write(root.join(".env.local"), format!("{key_override}=local\n")).unwrap();
        std::fs::write(
            root.join(".env"),
            format!("{key_override}=base\n{key_fill}=base\n"),
        )
        .unwrap();

        load_pair(&root);

        assert_eq!(std::env::var(&key_override).unwrap(), "local");
        assert_eq!(std::env::var(&key_fill).unwrap(), "base");

        std::env::remove_var(&key_override);
        std::env::remove_var(&key_fill);
    }

    fn unique_tmp(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "aegis-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn unique_key(tag: &str) -> String {
        format!(
            "AEGIS_ENV_TEST_{tag}_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        )
    }
}
