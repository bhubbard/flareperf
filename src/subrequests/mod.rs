use colored::Colorize;
use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_parser::Parser;
use oxc_span::SourceType;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubrequestIssue {
    pub rule_id: String,
    pub title: String,
    pub description: String,
    pub line: usize,
    pub severity: String,
    pub recommendation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubrequestReport {
    pub file_path: PathBuf,
    pub total_subrequest_callsites: usize,
    pub detected_fanouts: usize,
    pub issues: Vec<SubrequestIssue>,
}

impl SubrequestReport {
    pub fn is_passing(&self) -> bool {
        self.issues.is_empty()
    }
}

pub fn analyze_subrequests(path: &Path, content: &str) -> Result<SubrequestReport, String> {
    let allocator = Allocator::default();
    let source_type = SourceType::from_path(path).unwrap_or_else(|_| SourceType::ts());
    let parser_ret = Parser::new(&allocator, content, source_type).parse();

    if !parser_ret.diagnostics.is_empty() {
        return Err(format!("AST parsing error in {}: {:?}", path.display(), parser_ret.diagnostics[0]));
    }

    let program = parser_ret.program;
    let mut issues = Vec::new();
    let mut total_calls = 0usize;
    let mut fanouts = 0usize;

    for stmt in &program.body {
        walk_statement(stmt, content, false, &mut total_calls, &mut fanouts, &mut issues);
    }

    Ok(SubrequestReport {
        file_path: path.to_path_buf(),
        total_subrequest_callsites: total_calls,
        detected_fanouts: fanouts,
        issues,
    })
}

fn walk_statement(
    stmt: &Statement,
    content: &str,
    in_loop: bool,
    total_calls: &mut usize,
    fanouts: &mut usize,
    issues: &mut Vec<SubrequestIssue>,
) {
    match stmt {
        Statement::ForStatement(s) => {
            walk_statement(&s.body, content, true, total_calls, fanouts, issues);
        }
        Statement::ForInStatement(s) => {
            walk_statement(&s.body, content, true, total_calls, fanouts, issues);
        }
        Statement::ForOfStatement(s) => {
            walk_statement(&s.body, content, true, total_calls, fanouts, issues);
        }
        Statement::WhileStatement(s) => {
            walk_statement(&s.body, content, true, total_calls, fanouts, issues);
        }
        Statement::DoWhileStatement(s) => {
            walk_statement(&s.body, content, true, total_calls, fanouts, issues);
        }
        Statement::BlockStatement(s) => {
            for inner in &s.body {
                walk_statement(inner, content, in_loop, total_calls, fanouts, issues);
            }
        }
        Statement::IfStatement(s) => {
            walk_statement(&s.consequent, content, in_loop, total_calls, fanouts, issues);
            if let Some(alt) = &s.alternate {
                walk_statement(alt, content, in_loop, total_calls, fanouts, issues);
            }
        }
        Statement::ExpressionStatement(s) => {
            walk_expression(&s.expression, content, in_loop, total_calls, fanouts, issues);
        }
        Statement::VariableDeclaration(s) => {
            for decl in &s.declarations {
                if let Some(init) = &decl.init {
                    walk_expression(init, content, in_loop, total_calls, fanouts, issues);
                }
            }
        }
        Statement::ExportDefaultDeclaration(s) => {
            match &s.declaration {
                ExportDefaultDeclarationKind::FunctionDeclaration(func) => {
                    if let Some(body) = &func.body {
                        for inner in &body.statements {
                            walk_statement(inner, content, in_loop, total_calls, fanouts, issues);
                        }
                    }
                }
                decl => {
                    if let Some(expr) = decl.as_expression() {
                        walk_expression(expr, content, in_loop, total_calls, fanouts, issues);
                    }
                }
            }
        }
        Statement::FunctionDeclaration(func) => {
            if let Some(body) = &func.body {
                for inner in &body.statements {
                    walk_statement(inner, content, in_loop, total_calls, fanouts, issues);
                }
            }
        }
        _ => {}
    }
}

fn walk_expression(
    expr: &Expression,
    content: &str,
    in_loop: bool,
    total_calls: &mut usize,
    fanouts: &mut usize,
    issues: &mut Vec<SubrequestIssue>,
) {
    match expr {
        Expression::CallExpression(call) => {
            let is_subreq = is_subrequest_call(&call.callee);
            if is_subreq {
                *total_calls += 1;
                if in_loop {
                    *fanouts += 1;
                    let line = get_line_number(content, call.span.start);
                    issues.push(SubrequestIssue {
                        rule_id: "SUB-001".to_string(),
                        title: "Subrequest Invoked Inside Loop".to_string(),
                        description: "A fetch() or edge storage call is invoked inside a loop, risking Cloudflare's 50 subrequest limit per invocation (Error 1042).".to_string(),
                        line,
                        severity: "CRITICAL".to_string(),
                        recommendation: "Batch requests using env.DB.batch(), KV bulk get, or limit concurrency using a chunking queue (p-limit).".to_string(),
                    });
                }
            }

            // Check for Promise.all(items.map(fetch))
            if is_promise_all(&call.callee) && !call.arguments.is_empty()
                && let Some(Argument::ArrayExpression(arr)) = call.arguments.first()
                && arr.elements.len() > 50
            {
                *fanouts += 1;
                let line = get_line_number(content, call.span.start);
                issues.push(SubrequestIssue {
                    rule_id: "SUB-002".to_string(),
                    title: "Promise.all Array Exceeds 50 Subrequests".to_string(),
                    description: format!("Promise.all() explicitly declares {} concurrent requests, exceeding the 50 subrequest cap.", arr.elements.len()),
                    line,
                    severity: "CRITICAL".to_string(),
                    recommendation: "Chunk requests into batches of <= 10 or queue background tasks with Cloudflare Queues.".to_string(),
                });
            }

            walk_expression(&call.callee, content, in_loop, total_calls, fanouts, issues);
            for arg in &call.arguments {
                if let Some(e) = arg.as_expression() {
                    walk_expression(e, content, in_loop, total_calls, fanouts, issues);
                }
            }
        }
        Expression::AwaitExpression(await_expr) => {
            walk_expression(&await_expr.argument, content, in_loop, total_calls, fanouts, issues);
        }
        Expression::ObjectExpression(obj) => {
            for prop in &obj.properties {
                match prop {
                    ObjectPropertyKind::ObjectProperty(p) => {
                        walk_expression(&p.value, content, in_loop, total_calls, fanouts, issues);
                    }
                    ObjectPropertyKind::SpreadProperty(s) => {
                        walk_expression(&s.argument, content, in_loop, total_calls, fanouts, issues);
                    }
                }
            }
        }
        Expression::FunctionExpression(f) => {
            if let Some(body) = &f.body {
                for inner in &body.statements {
                    walk_statement(inner, content, in_loop, total_calls, fanouts, issues);
                }
            }
        }
        Expression::ArrowFunctionExpression(f) => {
            if let ArrowFunctionBody::FunctionBody(body) = &f.body {
                for inner in &body.statements {
                    walk_statement(inner, content, in_loop, total_calls, fanouts, issues);
                }
            } else if let Some(e) = f.body.as_expression() {
                walk_expression(e, content, in_loop, total_calls, fanouts, issues);
            }
        }
        Expression::StaticMemberExpression(m) => {
            walk_expression(&m.object, content, in_loop, total_calls, fanouts, issues);
        }
        _ => {}
    }
}

fn is_subrequest_call(expr: &Expression) -> bool {
    match expr {
        Expression::Identifier(ident) => ident.name == "fetch",
        Expression::StaticMemberExpression(member) => {
            let prop = &member.property.name;
            prop == "fetch" || prop == "get" || prop == "put" || prop == "delete" || prop == "all" || prop == "run"
        }
        _ => false,
    }
}

fn is_promise_all(expr: &Expression) -> bool {
    if let Expression::StaticMemberExpression(member) = expr
        && let Expression::Identifier(ident) = &member.object
    {
        return ident.name == "Promise" && member.property.name == "all";
    }
    false
}

fn get_line_number(content: &str, byte_offset: u32) -> usize {
    let offset = byte_offset as usize;
    if offset >= content.len() {
        return content.lines().count().max(1);
    }
    content[..offset].chars().filter(|&c| c == '\n').count() + 1
}

pub fn render_terminal_subrequests(report: &SubrequestReport) -> String {
    let mut out = String::new();
    out.push_str(&format!("{}: {}\n", "File".bold(), report.file_path.display().to_string().cyan()));
    out.push_str(&format!("Total Subrequest Callsites: {} | Fanouts in Loops: {}\n", report.total_subrequest_callsites, report.detected_fanouts));

    if report.issues.is_empty() {
        out.push_str(&format!("{} All subrequest patterns within safe 50-call limits!\n", "✓".green().bold()));
    } else {
        out.push_str(&format!("{} Found {} subrequest fanout risks:\n\n", "✗".red().bold(), report.issues.len()));
        for (idx, issue) in report.issues.iter().enumerate() {
            out.push_str(&format!("{}. [{}] {} (Line {}) - {}\n", idx + 1, issue.severity.red().bold(), issue.rule_id.cyan(), issue.line, issue.title.bold()));
            out.push_str(&format!("   Description: {}\n", issue.description));
            out.push_str(&format!("   Remediation: {}\n\n", issue.recommendation.dimmed()));
        }
    }

    out
}
