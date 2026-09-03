use colored::Colorize;
use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_parser::Parser;
use oxc_span::SourceType;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IsolateIssue {
    pub rule_id: String,
    pub title: String,
    pub description: String,
    pub line: usize,
    pub severity: IsolateSeverity,
    pub recommendation: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IsolateSeverity {
    Critical,
    High,
    Medium,
    Low,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IsolateReport {
    pub file_path: PathBuf,
    pub total_statements: usize,
    pub top_level_sync_complexity: u32,
    pub estimated_cold_start_ms: f64,
    pub issues: Vec<IsolateIssue>,
}

impl IsolateReport {
    pub fn is_passing(&self, max_complexity: u32) -> bool {
        self.top_level_sync_complexity <= max_complexity
            && !self
                .issues
                .iter()
                .any(|i| i.severity == IsolateSeverity::Critical)
    }
}

pub fn analyze_isolate_script(path: &Path, content: &str) -> Result<IsolateReport, String> {
    let allocator = Allocator::default();
    let source_type = SourceType::from_path(path).unwrap_or_else(|_| SourceType::ts());
    let parser_ret = Parser::new(&allocator, content, source_type).parse();

    if !parser_ret.diagnostics.is_empty() {
        return Err(format!(
            "AST parsing error in {}: {:?}",
            path.display(),
            parser_ret.diagnostics[0]
        ));
    }

    let program = parser_ret.program;
    let mut issues = Vec::new();
    let mut complexity = 0u32;

    for stmt in &program.body {
        match stmt {
            Statement::ExportDefaultDeclaration(_) => {
                // Export default handler is evaluated per request, safe from cold-start penalty
                continue;
            }
            Statement::FunctionDeclaration(_) => {
                // Function definitions are parsed but not executed synchronously at cold-start
                continue;
            }
            Statement::ForStatement(for_stmt) => {
                complexity += 25;
                let line = get_line_number(content, for_stmt.span.start);
                issues.push(IsolateIssue {
                    rule_id: "ISO-001".to_string(),
                    title: "Top-Level Synchronous Loop in Global Scope".to_string(),
                    description: "Top-level 'for' loop executed during Worker isolate initialization blocks startup thread.".to_string(),
                    line,
                    severity: IsolateSeverity::High,
                    recommendation: "Move loop execution inside the request handler or cache results in KV/D1/R2.".to_string(),
                });
            }
            Statement::WhileStatement(while_stmt) => {
                complexity += 30;
                let line = get_line_number(content, while_stmt.span.start);
                issues.push(IsolateIssue {
                    rule_id: "ISO-002".to_string(),
                    title: "Top-Level While Loop in Global Scope".to_string(),
                    description: "Top-level 'while' loop executed at module load time can trigger Cloudflare Error 1101 (Worker CPU time exceeded during startup).".to_string(),
                    line,
                    severity: IsolateSeverity::Critical,
                    recommendation: "Defer while loop execution into an async request handler or scheduled cron trigger.".to_string(),
                });
            }
            Statement::VariableDeclaration(var_decl) => {
                complexity += 1;
                for declarator in &var_decl.declarations {
                    if let Some(init) = &declarator.init {
                        check_expression(init, content, &mut complexity, &mut issues);
                    }
                }
            }
            Statement::ExpressionStatement(expr_stmt) => {
                complexity += 2;
                check_expression(&expr_stmt.expression, content, &mut complexity, &mut issues);
            }
            _ => {
                complexity += 1;
            }
        }
    }

    let estimated_cold_start_ms = 5.0 + (complexity as f64 * 1.5);

    Ok(IsolateReport {
        file_path: path.to_path_buf(),
        total_statements: program.body.len(),
        top_level_sync_complexity: complexity,
        estimated_cold_start_ms,
        issues,
    })
}

fn check_expression(
    expr: &Expression,
    content: &str,
    complexity: &mut u32,
    issues: &mut Vec<IsolateIssue>,
) {
    match expr {
        Expression::CallExpression(call) => {
            *complexity += 5;
            let line = get_line_number(content, call.span.start);

            // Check for JSON.parse at top-level
            if let Expression::StaticMemberExpression(member) = &call.callee
                && let Expression::Identifier(ident) = &member.object
                && ident.name == "JSON"
                && member.property.name == "parse"
            {
                *complexity += 15;
                issues.push(IsolateIssue {
                            rule_id: "ISO-003".to_string(),
                            title: "Top-Level Synchronous JSON.parse()".to_string(),
                            description: "Synchronous JSON parsing at top-level scope adds cold-start parse latency on every isolate boot.".to_string(),
                            line,
                            severity: IsolateSeverity::Medium,
                            recommendation: "Import static JSON files directly or parse lazily on first request.".to_string(),
                        });
            }
        }
        Expression::RegExpLiteral(_) => {
            *complexity += 2;
        }
        Expression::ArrayExpression(arr) if arr.elements.len() > 100 => {
            *complexity += 20;
            let line = get_line_number(content, arr.span.start);
            issues.push(IsolateIssue {
                    rule_id: "ISO-004".to_string(),
                    title: "Large Static Array Instantiation at Module Scope".to_string(),
                    description: format!("Large static array with {} elements instantiated at top-level increases V8 heap footprint on cold-start.", arr.elements.len()),
                    line,
                    severity: IsolateSeverity::Low,
                    recommendation: "Stream large datasets from Cloudflare R2 or query from D1/KV on demand.".to_string(),
                });
        }
        _ => {}
    }
}

fn get_line_number(content: &str, byte_offset: u32) -> usize {
    let offset = byte_offset as usize;
    if offset >= content.len() {
        return content.lines().count().max(1);
    }
    content[..offset].chars().filter(|&c| c == '\n').count() + 1
}

pub fn render_terminal_isolate(report: &IsolateReport) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "{}: {}\n",
        "File".bold(),
        report.file_path.display().to_string().cyan()
    ));
    out.push_str(&format!(
        "{}: {} (Est. Cold Start: ~{:.1}ms)\n",
        "Isolate Complexity".bold(),
        report.top_level_sync_complexity,
        report.estimated_cold_start_ms
    ));

    if report.issues.is_empty() {
        out.push_str(&format!(
            "{} No cold-start isolate bottlenecks detected!\n",
            "✓".green().bold()
        ));
    } else {
        out.push_str(&format!(
            "{} Found {} isolate initialization warnings:\n\n",
            "⚠".yellow().bold(),
            report.issues.len()
        ));
        for (idx, issue) in report.issues.iter().enumerate() {
            let sev_str = match issue.severity {
                IsolateSeverity::Critical => "[CRITICAL]".red().bold(),
                IsolateSeverity::High => "[HIGH]".yellow().bold(),
                IsolateSeverity::Medium => "[MEDIUM]".cyan(),
                IsolateSeverity::Low => "[LOW]".white(),
            };
            out.push_str(&format!(
                "{}. {} {} (Line {}) - {}\n",
                idx + 1,
                sev_str,
                issue.rule_id.cyan(),
                issue.line,
                issue.title.bold()
            ));
            out.push_str(&format!("   Description: {}\n", issue.description));
            out.push_str(&format!(
                "   Remediation: {}\n\n",
                issue.recommendation.dimmed()
            ));
        }
    }

    out
}
