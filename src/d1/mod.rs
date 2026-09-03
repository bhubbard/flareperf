use colored::Colorize;
use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_parser::Parser;
use oxc_span::SourceType;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct D1Issue {
    pub rule_id: String,
    pub title: String,
    pub description: String,
    pub line: usize,
    pub recommendation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct D1Report {
    pub file_path: PathBuf,
    pub total_d1_queries: usize,
    pub n_plus_one_loops: usize,
    pub issues: Vec<D1Issue>,
}

impl D1Report {
    pub fn is_passing(&self) -> bool {
        self.issues.is_empty()
    }
}

pub fn analyze_d1_usage(path: &Path, content: &str) -> Result<D1Report, String> {
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
    let mut total_d1 = 0usize;
    let mut loops = 0usize;

    for stmt in &program.body {
        walk_stmt_d1(stmt, content, false, &mut total_d1, &mut loops, &mut issues);
    }

    Ok(D1Report {
        file_path: path.to_path_buf(),
        total_d1_queries: total_d1,
        n_plus_one_loops: loops,
        issues,
    })
}

fn walk_stmt_d1(
    stmt: &Statement,
    content: &str,
    in_loop: bool,
    total_d1: &mut usize,
    loops: &mut usize,
    issues: &mut Vec<D1Issue>,
) {
    match stmt {
        Statement::ForStatement(s) => walk_stmt_d1(&s.body, content, true, total_d1, loops, issues),
        Statement::ForInStatement(s) => {
            walk_stmt_d1(&s.body, content, true, total_d1, loops, issues)
        }
        Statement::ForOfStatement(s) => {
            walk_stmt_d1(&s.body, content, true, total_d1, loops, issues)
        }
        Statement::WhileStatement(s) => {
            walk_stmt_d1(&s.body, content, true, total_d1, loops, issues)
        }
        Statement::DoWhileStatement(s) => {
            walk_stmt_d1(&s.body, content, true, total_d1, loops, issues)
        }
        Statement::BlockStatement(s) => {
            for inner in &s.body {
                walk_stmt_d1(inner, content, in_loop, total_d1, loops, issues);
            }
        }
        Statement::IfStatement(s) => {
            walk_stmt_d1(&s.consequent, content, in_loop, total_d1, loops, issues);
            if let Some(alt) = &s.alternate {
                walk_stmt_d1(alt, content, in_loop, total_d1, loops, issues);
            }
        }
        Statement::ExpressionStatement(s) => {
            walk_expr_d1(&s.expression, content, in_loop, total_d1, loops, issues);
        }
        Statement::VariableDeclaration(s) => {
            for decl in &s.declarations {
                if let Some(init) = &decl.init {
                    walk_expr_d1(init, content, in_loop, total_d1, loops, issues);
                }
            }
        }
        Statement::FunctionDeclaration(f) => {
            if let Some(body) = &f.body {
                for inner in &body.statements {
                    walk_stmt_d1(inner, content, in_loop, total_d1, loops, issues);
                }
            }
        }
        Statement::ExportDefaultDeclaration(s) => match &s.declaration {
            ExportDefaultDeclarationKind::FunctionDeclaration(func) => {
                if let Some(body) = &func.body {
                    for inner in &body.statements {
                        walk_stmt_d1(inner, content, in_loop, total_d1, loops, issues);
                    }
                }
            }
            decl => {
                if let Some(expr) = decl.as_expression() {
                    walk_expr_d1(expr, content, in_loop, total_d1, loops, issues);
                }
            }
        },
        _ => {}
    }
}

fn walk_expr_d1(
    expr: &Expression,
    content: &str,
    in_loop: bool,
    total_d1: &mut usize,
    loops: &mut usize,
    issues: &mut Vec<D1Issue>,
) {
    match expr {
        Expression::CallExpression(call) => {
            if is_d1_call(&call.callee) {
                *total_d1 += 1;
                if in_loop {
                    *loops += 1;
                    let line = get_line_number(content, call.span.start);
                    issues.push(D1Issue {
                        rule_id: "D1-001".to_string(),
                        title: "D1 Query Inside Loop (N+1 Query Bottleneck)".to_string(),
                        description: "Executing individual D1 SQL queries in a loop incurs repeated SQLite serialization overhead at the edge.".to_string(),
                        line,
                        recommendation: "Use env.DB.batch([stmt1, stmt2, ...]) to execute multiple prepared statements in a single batch.".to_string(),
                    });
                }
            }
            walk_expr_d1(&call.callee, content, in_loop, total_d1, loops, issues);
            for arg in &call.arguments {
                if let Some(e) = arg.as_expression() {
                    walk_expr_d1(e, content, in_loop, total_d1, loops, issues);
                }
            }
        }
        Expression::AwaitExpression(aw) => {
            walk_expr_d1(&aw.argument, content, in_loop, total_d1, loops, issues);
        }
        Expression::ObjectExpression(obj) => {
            for prop in &obj.properties {
                match prop {
                    ObjectPropertyKind::ObjectProperty(p) => {
                        walk_expr_d1(&p.value, content, in_loop, total_d1, loops, issues);
                    }
                    ObjectPropertyKind::SpreadProperty(s) => {
                        walk_expr_d1(&s.argument, content, in_loop, total_d1, loops, issues);
                    }
                }
            }
        }
        Expression::FunctionExpression(f) => {
            if let Some(body) = &f.body {
                for inner in &body.statements {
                    walk_stmt_d1(inner, content, in_loop, total_d1, loops, issues);
                }
            }
        }
        Expression::ArrowFunctionExpression(f) => {
            if let ArrowFunctionBody::FunctionBody(body) = &f.body {
                for inner in &body.statements {
                    walk_stmt_d1(inner, content, in_loop, total_d1, loops, issues);
                }
            } else if let Some(e) = f.body.as_expression() {
                walk_expr_d1(e, content, in_loop, total_d1, loops, issues);
            }
        }
        Expression::StaticMemberExpression(m) => {
            walk_expr_d1(&m.object, content, in_loop, total_d1, loops, issues);
        }
        _ => {}
    }
}

fn is_d1_call(expr: &Expression) -> bool {
    if let Expression::StaticMemberExpression(member) = expr {
        let prop = &member.property.name;
        if prop == "prepare" || prop == "exec" || prop == "batch" {
            return true;
        }
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

pub fn render_terminal_d1(report: &D1Report) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "{}: {}\n",
        "File".bold(),
        report.file_path.display().to_string().cyan()
    ));
    out.push_str(&format!(
        "Total D1 Callsites: {} | N+1 Loops: {}\n",
        report.total_d1_queries, report.n_plus_one_loops
    ));

    if report.issues.is_empty() {
        out.push_str(&format!(
            "{} No D1 query batching bottlenecks detected!\n",
            "✓".green().bold()
        ));
    } else {
        out.push_str(&format!(
            "{} Found {} D1 performance optimization opportunities:\n\n",
            "⚠".yellow().bold(),
            report.issues.len()
        ));
        for (idx, issue) in report.issues.iter().enumerate() {
            out.push_str(&format!(
                "{}. [PERF] {} (Line {}) - {}\n",
                idx + 1,
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
