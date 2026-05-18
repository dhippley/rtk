//! Mix run filter - handles mix run script execution output

use crate::core::stream::exec_capture;
use crate::core::utils::resolved_command;
use anyhow::Context;
use lazy_static::lazy_static;
use regex::Regex;

lazy_static! {
    // Match mix setup/compilation output to filter
    static ref MIX_SETUP_RE: Regex =
        Regex::new(r"(?i)^(Resolving|Compiling|Generated|Loading|Starting)").unwrap();

    // Match error lines (preserve)
    static ref ERROR_RE: Regex =
        Regex::new(r"(?i)(error|Error|failed|Failed|fatal)").unwrap();
}

#[derive(Clone, Debug)]
pub struct MixRunArgs {
    pub args: Vec<String>,
}

impl MixRunArgs {
    pub fn to_cmd_args(&self) -> Vec<String> {
        let mut cmd = vec!["run".to_string()];
        cmd.extend(self.args.clone());
        cmd
    }
}

pub fn run(args: MixRunArgs) -> std::result::Result<i32, anyhow::Error> {
    let timer = crate::core::tracking::TimedExecution::start();

    let mut cmd = resolved_command("mix");
    for arg in &args.to_cmd_args() {
        cmd.arg(arg);
    }

    let result = exec_capture(&mut cmd).context("Failed to execute mix run")?;

    let filtered = filter_mix_run(&result.stdout).unwrap_or_else(|e| {
        eprintln!("rtk: filter warning: {}", e);
        result.stdout.clone()
    });

    timer.track("mix run", "rtk mix run", &result.stdout, &filtered);
    print!("{}", filtered);

    Ok(result.exit_code)
}

fn filter_mix_run(input: &str) -> std::result::Result<String, anyhow::Error> {
    let mut output_lines = Vec::new();
    let mut in_setup = true;
    let mut blank_count = 0;

    for line in input.lines() {
        let trimmed = line.trim();

        // Track blank lines
        if trimmed.is_empty() {
            blank_count += 1;
            if blank_count <= 1 {
                output_lines.push(String::new());
            }
            continue;
        }

        blank_count = 0;

        // Check if we're still in setup phase
        if in_setup && MIX_SETUP_RE.is_match(trimmed) {
            continue;
        } else if in_setup {
            in_setup = false;
        }

        // Preserve error lines
        if ERROR_RE.is_match(line) {
            output_lines.push(line.to_string());
            continue;
        }

        // After setup, preserve everything (it's script output)
        if !in_setup {
            output_lines.push(line.to_string());
        }
    }

    Ok(output_lines.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mix_run_empty_input() {
        let result = filter_mix_run("");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "");
    }

    #[test]
    fn test_mix_run_single_line() {
        let input = "Hello from Elixir!";
        let result = filter_mix_run(input);
        assert!(result.is_ok());
        assert!(result.unwrap().contains("Hello"));
    }

    #[test]
    fn test_mix_run_filters_setup() {
        let input = "Resolving dependencies\nCompiling\nHello from script";
        let result = filter_mix_run(input);
        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(!output.contains("Resolving"));
        assert!(!output.contains("Compiling"));
        assert!(output.contains("Hello"));
    }
}
