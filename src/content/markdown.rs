use std::{
    fs, io,
    path::{Path, PathBuf},
};

pub fn markdown_files(
    directory: &Path,
    missing_directory_is_empty: bool,
) -> io::Result<Vec<PathBuf>> {
    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(source) if missing_directory_is_empty && source.kind() == io::ErrorKind::NotFound => {
            return Ok(Vec::new())
        }
        Err(source) => return Err(source),
    };

    let mut paths = entries
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .is_some_and(|ext| ext == "mdx" || ext == "md")
        })
        .collect::<Vec<_>>();

    paths.sort();
    Ok(paths)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs::{create_dir_all, remove_dir_all, write},
        sync::atomic::{AtomicU64, Ordering},
        time::{SystemTime, UNIX_EPOCH},
    };

    static TEST_DIR_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn test_dir() -> PathBuf {
        let id = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("test clock is valid")
            .as_nanos();
        let counter = TEST_DIR_COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "markdown-files-test-{}-{}-{}",
            std::process::id(),
            id,
            counter
        ));
        create_dir_all(&dir).expect("test dir can be created");
        dir
    }

    #[test]
    fn lists_markdown_files_in_sorted_order() {
        let dir = test_dir();
        write(dir.join("b.md"), "b").expect("file can be written");
        write(dir.join("a.mdx"), "a").expect("file can be written");
        write(dir.join("ignored.txt"), "ignored").expect("file can be written");

        let paths = markdown_files(&dir, false).expect("markdown files load");

        assert_eq!(paths, vec![dir.join("a.mdx"), dir.join("b.md")]);

        remove_dir_all(dir).expect("test dir can be removed");
    }

    #[test]
    fn missing_directory_can_be_empty() {
        let dir = test_dir();
        let missing = dir.join("missing");

        let paths = markdown_files(&missing, true).expect("missing directory loads");

        assert!(paths.is_empty());

        remove_dir_all(dir).expect("test dir can be removed");
    }
}
