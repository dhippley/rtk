//! Xcodebuild filter - condenses Xcode build tool verbose output

use crate::core::stream::exec_capture;
use crate::core::utils::resolved_command;
use anyhow::Context;
use lazy_static::lazy_static;
use regex::Regex;

lazy_static! {
    // Match verbose compile lines (CompileSwift, CompileC, CompileAsset, Ld, etc.)
    static ref COMPILE_LINE_RE: Regex =
        Regex::new(r"^(Compile|CompileSwift|CompileC|CompileAsset|CompileMetalFile|Ld|ProcessInfoPlistFile|CodeSign|Link|ProcessProductPackaging|GenerateDSYMFile|CopyStringsFile|CreateBuildDirectory)").unwrap();

    // Match error and warning lines (preserve)
    static ref ERROR_WARNING_RE: Regex =
        Regex::new(r"(?i)(error:|warning:|failed|Build failed|fatal|FAILED)").unwrap();

    // Match build complete/result lines (preserve)
    static ref BUILD_RESULT_RE: Regex =
        Regex::new(r"(?i)^(Build complete|Finished|succeeded|failed|Build\s|Xcode)").unwrap();
}

#[derive(Clone, Debug)]
pub struct XcodebuildArgs {
    pub args: Vec<String>,
}

impl XcodebuildArgs {
    pub fn to_cmd_args(&self) -> Vec<String> {
        self.args.clone()
    }
}

pub fn run(args: XcodebuildArgs) -> std::result::Result<i32, anyhow::Error> {
    let timer = crate::core::tracking::TimedExecution::start();

    let mut cmd = resolved_command("xcodebuild");
    for arg in &args.to_cmd_args() {
        cmd.arg(arg);
    }

    let result = exec_capture(&mut cmd).context("Failed to execute xcodebuild")?;

    let filtered = filter_xcodebuild(&result.stdout).unwrap_or_else(|e| {
        eprintln!("rtk: filter warning: {}", e);
        result.stdout.clone()
    });

    timer.track("xcodebuild", "rtk xcodebuild", &result.stdout, &filtered);
    print!("{}", filtered);

    Ok(result.exit_code)
}

fn filter_xcodebuild(input: &str) -> std::result::Result<String, anyhow::Error> {
    let mut output_lines = Vec::new();
    let mut blank_count = 0;

    for line in input.lines() {
        // Track and condense blank lines
        if line.trim().is_empty() {
            blank_count += 1;
            if blank_count <= 1 {
                output_lines.push(String::new());
            }
            continue;
        }

        blank_count = 0;

        // Skip verbose compile lines
        if COMPILE_LINE_RE.is_match(line) {
            continue;
        }

        // Preserve error, warning, and build result lines
        if ERROR_WARNING_RE.is_match(line) || BUILD_RESULT_RE.is_match(line) {
            output_lines.push(line.to_string());
            continue;
        }

        // Keep other lines (environment info, etc.)
        output_lines.push(line.to_string());
    }

    Ok(output_lines.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_xcodebuild_empty_input() {
        let result = filter_xcodebuild("");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "");
    }

    #[test]
    fn test_xcodebuild_single_line() {
        let input = "Build complete!";
        let result = filter_xcodebuild(input);
        assert!(result.is_ok());
        assert!(result.unwrap().contains("Build complete"));
    }

    #[test]
    fn test_xcodebuild_removes_compile_lines() {
        let input = "CompileSwift /path/to/file.swift\nBuild complete!";
        let result = filter_xcodebuild(input);
        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(!output.contains("CompileSwift"));
        assert!(output.contains("Build complete"));
    }

    #[test]
    fn test_xcodebuild_preserves_errors() {
        let input = "CompileSwift /path/to/file.swift\nerror: syntax error\nBuild failed";
        let result = filter_xcodebuild(input);
        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(output.contains("error"));
        assert!(output.contains("Build failed"));
        assert!(!output.contains("CompileSwift"));
    }
}
