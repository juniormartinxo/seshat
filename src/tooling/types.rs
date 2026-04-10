use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolCommand {
    pub name: String,
    pub command: Vec<String>,
    pub check_type: String,
    pub blocking: bool,
    pub pass_files: bool,
    pub extensions: Option<Vec<String>>,
    pub fix_command: Option<Vec<String>>,
    pub auto_fix: bool,
}

impl ToolCommand {
    pub(super) fn new(name: &str, command: &[&str], check_type: &str) -> Self {
        Self {
            name: name.to_string(),
            command: command.iter().map(|value| (*value).to_string()).collect(),
            check_type: check_type.to_string(),
            blocking: true,
            pass_files: false,
            extensions: None,
            fix_command: None,
            auto_fix: false,
        }
    }

    pub(super) fn with_files(mut self) -> Self {
        self.pass_files = true;
        self
    }

    pub(super) fn with_fix(mut self, command: &[&str]) -> Self {
        self.fix_command = Some(command.iter().map(|value| (*value).to_string()).collect());
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolResult {
    pub tool: String,
    pub check_type: String,
    pub success: bool,
    pub output: String,
    pub blocking: bool,
    pub skipped: bool,
    pub skip_reason: String,
}

impl ToolResult {
    pub(super) fn skipped(tool: &ToolCommand, reason: impl Into<String>) -> Self {
        Self {
            tool: tool.name.clone(),
            check_type: tool.check_type.clone(),
            success: true,
            output: String::new(),
            blocking: tool.blocking,
            skipped: true,
            skip_reason: reason.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolOutputBlock {
    pub text: String,
    pub status: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolingConfig {
    pub project_type: String,
    pub tools: BTreeMap<String, ToolCommand>,
}

impl ToolingConfig {
    pub fn get_tools_for_check(&self, check_type: &str) -> Vec<ToolCommand> {
        if check_type == "full" {
            return self.tools.values().cloned().collect();
        }
        self.tools
            .values()
            .filter(|tool| tool.check_type == check_type)
            .cloned()
            .collect()
    }
}
