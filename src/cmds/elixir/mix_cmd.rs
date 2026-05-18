//! Mix general filter - handles mix compile, deps.get, format, linter.all, etc.

use crate::core::stream::exec_capture;
use crate::core::utils::resolved_command;
use anyhow::Context;
use lazy_static::lazy_static;
use regex::Regex;

lazy_static! {
    // Match progress dots and spinner characters (at end of line)
    static ref PROGRESS_CHARS_RE: Regex =
        Regex::new(r"\.{2,}|[\|/\-\\]{2,}").unwrap();

    // Match dependency download lines to be grouped
    static ref DEP_DOWNLOAD_RE: Regex =
        Regex::new(r"^[a-z0-9_\-]+\s+v[\d\.]+").unwrap();

    // Match error/warning keywords (preserve)
    static ref ERROR_WARNING_RE: Regex =
        Regex::new(r"(?i)(error|warning|failed|cannot|fatal)").unwrap();

    // Match summary/completion lines (preserve)
    static ref SUMMARY_RE: Regex =
        Regex::new(r"^(Finished|Completed|Generated|done|Built)").unwrap();

    // Match "Resolving" and other setup notices
    static ref SETUP_NOTICE_RE: Regex =
        Regex::new(r"(?i)^(Resolving|Loading|Starting|Preparing|Checking)").unwrap();
}

#[derive(Clone, Debug)]
pub struct MixArgs {
    pub args: Vec<String>,
}

impl MixArgs {
    pub fn to_cmd_args(&self) -> Vec<String> {
        self.args.clone()
    }
}

pub fn run(args: MixArgs) -> std::result::Result<i32, anyhow::Error> {
    let timer = crate::core::tracking::TimedExecution::start();

    let mut cmd = resolved_command("mix");
    for arg in &args.to_cmd_args() {
        cmd.arg(arg);
    }

    let result = exec_capture(&mut cmd).context("Failed to execute mix")?;

    let filtered = filter_mix(&result.stdout).unwrap_or_else(|e| {
        eprintln!("rtk: filter warning: {}", e);
        result.stdout.clone()
    });

    timer.track("mix", "rtk mix", &result.stdout, &filtered);
    print!("{}", filtered);

    Ok(result.exit_code)
}

fn filter_mix(input: &str) -> std::result::Result<String, anyhow::Error> {
    let mut output_lines = Vec::new();
    let mut blank_count = 0;
    let mut dep_buffer = Vec::new();

    for line in input.lines() {
        let trimmed = line.trim();

        // Track blank lines for condensing
        if trimmed.is_empty() {
            blank_count += 1;
            if blank_count <= 1 {
                output_lines.push(String::new());
            }
            continue;
        }

        blank_count = 0;

        // Skip lines that are mostly progress dots or spinner chars (cleanup noise)
        if PROGRESS_CHARS_RE.is_match(trimmed) && trimmed.len() < 10 {
            continue;
        }

        // Flush dependency buffer if we hit a non-dependency line
        if !DEP_DOWNLOAD_RE.is_match(trimmed) && !dep_buffer.is_empty() {
            flush_dep_buffer(&mut output_lines, &mut dep_buffer);
        }

        // Collect dependency download lines
        if DEP_DOWNLOAD_RE.is_match(trimmed) {
            dep_buffer.push(line.to_string());
            continue;
        }

        // Preserve errors, warnings, and summaries
        if ERROR_WARNING_RE.is_match(line) || SUMMARY_RE.is_match(trimmed) {
            output_lines.push(line.to_string());
            continue;
        }

        // Skip some redundant setup notices (but keep errors)
        if SETUP_NOTICE_RE.is_match(trimmed) && !ERROR_WARNING_RE.is_match(line) {
            // Keep the first occurrence of each setup notice
            let notice_type = trimmed.split_whitespace().next().unwrap_or("");
            let already_seen = output_lines.iter().any(|l| l.starts_with(notice_type));
            if !already_seen {
                output_lines.push(line.to_string());
            }
            continue;
        }

        // Keep everything else
        output_lines.push(line.to_string());
    }

    // Flush any remaining dependencies
    if !dep_buffer.is_empty() {
        flush_dep_buffer(&mut output_lines, &mut dep_buffer);
    }

    Ok(output_lines.join("\n"))
}

fn flush_dep_buffer(output_lines: &mut Vec<String>, dep_buffer: &mut Vec<String>) {
    if dep_buffer.len() <= 3 {
        output_lines.append(dep_buffer);
    } else {
        output_lines.push(dep_buffer[0].clone());
        output_lines.push(format!("[{} more dependencies]", dep_buffer.len() - 2));
        output_lines.push(dep_buffer[dep_buffer.len() - 1].clone());
        dep_buffer.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mix_empty_input() {
        let result = filter_mix("");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "");
    }

    #[test]
    fn test_mix_single_line() {
        let input = "Finished in 0.5 seconds";
        let result = filter_mix(input);
        assert!(result.is_ok());
        assert!(result.unwrap().contains("Finished"));
    }

    #[test]
    fn test_mix_removes_progress_dots() {
        let input = "...\n....\nDone";
        let result = filter_mix(input);
        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(!output.contains("..."));
        assert!(output.contains("Done"));
    }

    #[test]
    fn test_mix_preserves_errors() {
        let input = "error: something went wrong\nDone";
        let result = filter_mix(input);
        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(output.contains("error"));
    }
}
