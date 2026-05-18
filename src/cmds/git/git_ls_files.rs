//! Git ls-files filter - truncates large file listings for repos with many tracked files

use anyhow::Result;

pub fn filter_git_ls_files(input: &str) -> Result<String> {
    let lines: Vec<&str> = input.lines().collect();
    let line_count = lines.len();

    // For small repos, pass through unchanged
    if line_count <= 1000 {
        return Ok(input.to_string());
    }

    // For large repos, show first 50 files + count of remaining
    let shown = 50;
    let mut output_lines: Vec<String> = lines
        .iter()
        .take(shown)
        .map(|s| s.to_string())
        .collect();

    let remaining = line_count - shown;
    output_lines.push(format!("[{} more files]", remaining));
    output_lines.push(format!("Total: {} files", line_count));

    Ok(output_lines.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_git_ls_files_empty() {
        let result = filter_git_ls_files("");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "");
    }

    #[test]
    fn test_git_ls_files_small_no_truncation() {
        let small_list = (0..50)
            .map(|i| format!("file_{}.txt", i))
            .collect::<Vec<_>>()
            .join("\n");

        let result = filter_git_ls_files(&small_list);
        assert!(result.is_ok());
        let output = result.unwrap();
        // Should be identical (no truncation)
        assert_eq!(output, small_list);
    }

    #[test]
    fn test_git_ls_files_large_truncated() {
        let large_list = (0..2000)
            .map(|i| format!("file_{}.txt", i))
            .collect::<Vec<_>>()
            .join("\n");

        let result = filter_git_ls_files(&large_list);
        assert!(result.is_ok());
        let output = result.unwrap();
        // Should be truncated
        assert!(output.len() < large_list.len() / 2);
        assert!(output.contains("[1950 more files]"));
        assert!(output.contains("Total: 2000 files"));
    }

    #[test]
    fn test_git_ls_files_large_token_savings() {
        let large_list = (0..3000)
            .map(|i| format!("src/module_{}/file_{}.rs", i / 10, i))
            .collect::<Vec<_>>()
            .join("\n");

        let result = filter_git_ls_files(&large_list);
        assert!(result.is_ok());
        let output = result.unwrap();
        // Basic assertion that output is much smaller
        assert!(output.len() < large_list.len() / 2);
    }

    #[test]
    fn test_git_ls_files_exactly_1000() {
        let list_1000 = (0..1000)
            .map(|i| format!("file_{}.txt", i))
            .collect::<Vec<_>>()
            .join("\n");

        let result = filter_git_ls_files(&list_1000);
        assert!(result.is_ok());
        let output = result.unwrap();
        // At exactly 1000, should not truncate
        assert_eq!(output, list_1000);
    }

    #[test]
    fn test_git_ls_files_1001() {
        let list_1001 = (0..1001)
            .map(|i| format!("file_{}.txt", i))
            .collect::<Vec<_>>()
            .join("\n");

        let result = filter_git_ls_files(&list_1001);
        assert!(result.is_ok());
        let output = result.unwrap();
        // At 1001, should truncate
        assert!(output.len() < list_1001.len() / 2);
    }
}
