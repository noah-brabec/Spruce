use std::fs;
use std::io::Write;


use crate::name_analysis::*;
use crate::typecheck::Environment;


pub fn gen_prog(out: &mut fs::File, prog: &Prog, env: &Environment) {
    let js_helpers = fs::read_to_string("src/helper.js").expect("cannot read js helpers file");
    write!(out, "{}", js_helpers).expect("failed to write helpers");

    for t in &prog.types {
        write!(out, "{}", gen_type(prog, env, t)).expect("failed to write line");
    }

    for def in &prog.definitions {
        let (stmt_str, _) = gen_stmt(prog, env, def, 0);
        write!(out, "{}", stmt_str).expect("failed to write line");
    }

    for func in &prog.functions {
        write!(out, "{}", gen_func(prog, env, func, 0)).expect("failed to write line");
    }

    write!(out, "\nconsole.log(main())").expect("failed to write line");
}

fn gen_type(prog: &Prog, env: &Environment, t: &TypeNode) -> String {
    let mut output = format!("const {} = {{\n", t.val.name);

    for opt in &t.val.options {
        output = append_line(&output, format!("{}: '{}',\n", opt.val.name.to_ascii_uppercase(), opt.val.name), 1);
    }

    output = format!("{}}}\n", output);

    output
}

fn gen_func(prog: &Prog, env: &Environment, func_node: &FuncNode, indent: usize) -> String {
    let func = &func_node.val;
    let mut output = append_line(&String::from(""), format!("function {}(", gen_sym(&prog.symbol_table, &func.name)), indent);

    let body_indent = indent + 1;

    func.args.first().as_ref().map(|arg| {
        output = format!("{}{}", output, gen_sym(&prog.symbol_table, arg));
    });
    for arg in func.args.iter().skip(1) {
        output = format!("{}, {}", output, gen_sym(&prog.symbol_table, arg));
    };

    output = format!("{}){{\n", output);
    let (body_str, body_val) = gen_body(prog, env, &func.body, body_indent);
    output = format!("{}{}", output, body_str);

    body_val.map(|val| {
        output = append_line(&output, format!("return {};\n", val), body_indent);
    });

    append_line(&output, String::from("}\n"), indent)
}

fn gen_body(prog: &Prog, env: &Environment, body_node: &BodyNode, indent: usize) -> (String, Option<String>) {
    let body = &body_node.val;
    let mut output = String::from("");

    for stmt in &body.stmts {
        let (stmt_str, _) = gen_stmt(prog, env, stmt, indent);
        output = append_line(&output, stmt_str, indent);
    }

    body.expr.as_ref().map(|expr| {
        output = append_line(&output, format!("var _body_val = {};\n", gen_expr(prog, &expr)), indent);
    });

    let val_handle = match (&body.expr, body.stmts.last()) {
        (Some(expr), _) => Some(String::from("_body_val")),
        (_, Some(stmt)) => {
            let (_, stmt_val) = gen_stmt(prog, env, stmt, indent);
            stmt_val
        }
        (_, _) => None
    };

    (output, val_handle)
}

fn gen_stmt(prog: &Prog, env: &Environment, stmt: &StmtNode, indent: usize) -> (String, Option<String>) {
    match &stmt.val {
        Stmt::Assign(tgt, expr) => {
            let (tgt_str, tgt_var) = gen_target(prog, tgt);
            (format!("{} = {};\n", tgt_str, gen_expr(prog, expr)), Some(tgt_var))
        }
        Stmt::Case(case) => gen_case(prog, env, &case, indent),
        Stmt::FnCall(fn_id, args) => {
            let mut output = format!("var _fn_val = {}(", gen_sym(&prog.symbol_table, fn_id).to_owned());

            args.first().as_ref().map(|arg| {
                output = format!("{}{}", output, gen_expr(prog, arg));
            });
            for arg in args.iter().skip(1) {
                output = format!("{}, {}", output, gen_expr(prog, arg));
            };

            (format!("{});\n", output), Some(String::from("_fn_val")))
        }
    }
}

fn gen_case(prog: &Prog, env: &Environment, case_node: &CaseNode, indent: usize) -> (String, Option<String>) {
    let case = &case_node.val;
    // first indents are already added by stmt. This should be fixed later
    let mut output = format!("var _case_expr{} = {};\n", case.id, gen_expr(prog, &case.expr));
    output = append_line(&output, format!("var _case_val{};\n", case.id), indent);
    output = append_line(&output, format!("switch(_case_expr{}[0]){{\n", case.id), indent);

    for opt in &case.options {
        output = format!("{}{}", output, gen_case_option(prog, env, &opt, case.id, indent + 1));
    }

    output = append_line(&output, String::from("}\n"), indent);
    (output, Some(format!("_case_val{}", case.id)))
}

fn gen_case_option(prog: &Prog, env: &Environment, option_node: &CaseOptionNode, case_id: CaseID, indent: usize) -> String {
    let option = &option_node.val;
    let mut output = append_line(&String::from(""), format!("case {}:\n", gen_pattern(prog, &option.pattern)), indent);

    let body_indent = indent + 1;

    for (i, arg) in option.pattern.val.args.iter().enumerate() {
        output = append_line(&output, format!("var {} = _case_expr{}[{}];\n", gen_sym(&prog.symbol_table, arg), case_id, i + 1), body_indent);
    }

    match &option.body.val {
        CaseBody::Body(body) => {
            let (body, val_handle) = gen_body(prog, env, body, body_indent);
            output = format!("{}{}", output, body);
            val_handle.map(|handle| {
                output = append_line(&output, format!("_case_val{} = {};\n", case_id, handle), body_indent);
            });
        }
        CaseBody::Expr(expr) => {
            output = append_line(&output, format!("_case_val{} = {};\n", case_id, gen_expr(prog, expr)), body_indent);
        }
    }

    append_line(&output, String::from("break;\n"), body_indent)
}

fn gen_pattern(prog: &Prog, pattern: &CasePatternNode) -> String {
    gen_adtval(&prog.type_table, &pattern.val.base)
}

fn gen_target(prog: &Prog, tgt: &TargetNode) -> (String, String) {
    match tgt.val {
        Target::Mutable(sym) | Target::Var(sym) => {
            let var_name = gen_sym(&prog.symbol_table, &sym);
            (format!("var {}", var_name), var_name)
        }
        Target::Update(sym) => {
            let var_name = gen_sym(&prog.symbol_table, &sym);
            (var_name.clone(), var_name)
        }
    }
}

fn gen_expr(prog: &Prog, expr: &ExprNode) -> String {
    match &expr.val {
        Expr::Add(left, right) => format!("({} + {})", gen_expr(prog, left), gen_expr(prog, right)),
        Expr::Subt(left, right) => format!("({} - {})", gen_expr(prog, left), gen_expr(prog, right)),
        Expr::Mult(left, right) => format!("({} * {})", gen_expr(prog, left), gen_expr(prog, right)),
        Expr::Div(left, right) => format!("(~~({} / {}))", gen_expr(prog, left), gen_expr(prog, right)),
        Expr::Pow(left, right) => unimplemented!(),
        Expr::Mod(left, right) => format!("({} % {})", gen_expr(prog, left), gen_expr(prog, right)),
        Expr::Eq(left, right) => format!("_to_bool({} == {})", gen_expr(prog, left), gen_expr(prog, right)),
        Expr::NotEq(left, right) => format!("_to_bool({} != {})", gen_expr(prog, left), gen_expr(prog, right)),
        Expr::LtEq(left, right) => format!("_to_bool({} <= {})", gen_expr(prog, left), gen_expr(prog, right)),
        Expr::GtEq(left, right) => format!("_to_bool({} >= {})", gen_expr(prog, left), gen_expr(prog, right)),
        Expr::Lt(left, right) => format!("_to_bool({} < {})", gen_expr(prog, left), gen_expr(prog, right)),
        Expr::Gt(left, right) => format!("_to_bool({} > {})", gen_expr(prog, left), gen_expr(prog, right)),
        Expr::Lit(l) => format!("{}", l),
        Expr::Id(id) => gen_sym(&prog.symbol_table, id),
        Expr::FnCall(fn_id, args) => {
            let mut output = format!("{}(", gen_sym(&prog.symbol_table, fn_id).to_owned());

            args.first().as_ref().map(|arg| {
                output = format!("{}{}", output, gen_expr(prog, arg));
            });
            for arg in args.iter().skip(1) {
                output = format!("{}, {}", output, gen_expr(prog, arg));
            };

            format!("{})", output)
        }
        Expr::ADTVal(base, args) => {
            let mut output = format!("[{}", gen_adtval(&prog.type_table, base));

            for arg in args {
                output = format!("{}, {}", output, gen_expr(prog, &arg));
            }

            format!("{}]", output)
        }
    }
}

/// Currently just uses the symbol's name. This may need a more sophisticated
/// scheme later on if we want to implement variable shadowing
fn gen_sym(table: &SymbolTable, id: &SymbolID) -> String {
    let sym = table.lookup_id(id).expect("Symbol not found");
    String::from(format!("{}", sym.name))
}

fn gen_adtval(types: &TypeTableExt, id: &ADTValID) -> String {
    let val = types.values.get(id).expect("Symbol not found");
    let adt = types.types.get(&val.data_type).expect("Dangling type id");
    String::from(format!("{}.{}", adt.name, val.name.to_ascii_uppercase()))
}

fn append_line(output: &String, line: String, indent: usize) -> String {
    format!("{}{}{}", output, "\t".repeat(indent), line)
}