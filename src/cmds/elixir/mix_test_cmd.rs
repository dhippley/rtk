//! Mix test filter - condenses mix test output to show only failures and summary

use crate::core::stream::exec_capture;
use crate::core::utils::resolved_command;
use anyhow::Context;
use lazy_static::lazy_static;
use regex::Regex;

lazy_static! {
    // Match test result lines with timing
    static ref TEST_RESULT_RE: Regex =
        Regex::new(r"^\s+✓\s+.+?\s+\(\d+ms\)").unwrap();

    // Match passing test lines (verbose format)
    static ref PASSING_TEST_RE: Regex =
        Regex::new(r"^\s{2,}\d+\)\s+test.*?ok").unwrap();

    // Match failure markers - look for numbered test lines that might be failures
    static ref TEST_NUMBER_RE: Regex =
        Regex::new(r"^\s*\d+\)\s+").unwrap();

    // Match error lines
    static ref ERROR_RE: Regex =
        Regex::new(r"(?i)(error:|failed)").unwrap();

    // Match summary lines (to preserve)
    static ref SUMMARY_RE: Regex =
        Regex::new(r"^(\d+\s+passed|\d+\s+failed|\d+\s+skipped|Finished)").unwrap();

    // Match warning lines
    static ref WARNING_RE: Regex =
        Regex::new(r"(?i)warning").unwrap();
}

#[derive(Clone, Debug)]
pub struct MixTestArgs {
    pub args: Vec<String>,
}

impl MixTestArgs {
    pub fn to_cmd_args(&self) -> Vec<String> {
        let mut cmd = vec!["test".to_string()];
        cmd.extend(self.args.clone());
        cmd
    }
}

pub fn run(args: MixTestArgs) -> std::result::Result<i32, anyhow::Error> {
    let timer = crate::core::tracking::TimedExecution::start();

    let mut cmd = resolved_command("mix");
    for arg in &args.to_cmd_args() {
        cmd.arg(arg);
    }

    let result = exec_capture(&mut cmd).context("Failed to execute mix test")?;

    let filtered = filter_mix_test(&result.stdout).unwrap_or_else(|e| {
        eprintln!("rtk: filter warning: {}", e);
        result.stdout.clone()
    });

    timer.track("mix test", "rtk mix test", &result.stdout, &filtered);
    print!("{}", filtered);

    Ok(result.exit_code)
}

fn filter_mix_test(input: &str) -> std::result::Result<String, anyhow::Error> {
    let mut output_lines = Vec::new();
    let mut in_failure_block = false;
    let mut blank_count = 0;

    for line in input.lines() {
        // Track blank lines for condensing
        if line.trim().is_empty() {
            blank_count += 1;
            if blank_count <= 1 {
                output_lines.push(String::new());
            }
            continue;
        }

        blank_count = 0;

        // Check if this is a numbered test line (could be start of failure info)
        if TEST_NUMBER_RE.is_match(line) {
            in_failure_block = true;
            output_lines.push(line.to_string());
            continue;
        }

        // If in a failure block and we see an error line, keep it and all following lines until next test
        if in_failure_block {
            // Exit failure block if we see another numbered test or empty line followed by summary
            if line.trim().is_empty() || (TEST_NUMBER_RE.is_match(line) && !line.contains("1)")) {
                in_failure_block = false;
                continue;
            }
            output_lines.push(line.to_string());
            continue;
        }

        // Skip individual passing test lines
        if TEST_RESULT_RE.is_match(line) || PASSING_TEST_RE.is_match(line) {
            continue;
        }

        // Preserve summary, warning, and error lines
        if SUMMARY_RE.is_match(line) || WARNING_RE.is_match(line) || ERROR_RE.is_match(line) {
            output_lines.push(line.to_string());
        }
    }

    Ok(output_lines.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mix_test_empty_input() {
        let result = filter_mix_test("");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "");
    }

    #[test]
    fn test_mix_test_single_line() {
        let input = "1 passed";
        let result = filter_mix_test(input);
        assert!(result.is_ok());
        assert!(result.unwrap().contains("passed"));
    }

    #[test]
    fn test_mix_test_preserves_failures() {
        let input = "1) test_failure\nerror: failure reason\n2 passed";
        let result = filter_mix_test(input);
        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(output.contains("test_failure"));
        assert!(output.contains("error"));
    }
}
