//! Variáveis de módulo: nomes candidatos, escopo local e as refs `LE_VAR`/`ESCREVE_VAR`
//! (TASK-10, D2 e D3).

use super::{Ctx, RefKind, push_ref};
use std::collections::HashSet;
use tree_sitter::Node;

fn assigned_name(ctx: &Ctx, stmt: Node) -> Option<String> {
    if !matches!(stmt.kind(), "assignment" | "augmented_assignment") {
        return None;
    }
    let left = stmt.child_by_field_name("left")?;
    (left.kind() == "identifier").then(|| ctx.text(left).to_string())
}

/// Nomes candidatos a variável de módulo: atribuídos em instruções do módulo (fora de
/// funções e classes) ou importados.
pub(super) fn collect_var_names(ctx: &mut Ctx, root: Node) {
    let mut names: Vec<String> = ctx
        .facts
        .imports
        .iter()
        .map(|i| i.local_name.clone())
        .collect();
    fn walk(ctx: &Ctx, node: Node, names: &mut Vec<String>) {
        let mut cursor = node.walk();
        for child in node.named_children(&mut cursor) {
            match child.kind() {
                "function_definition" | "class_definition" | "decorated_definition" | "lambda" => {}
                "expression_statement" => {
                    if let Some(name) = child.named_child(0).and_then(|a| assigned_name(ctx, a)) {
                        names.push(name);
                    }
                }
                _ => walk(ctx, child, names),
            }
        }
    }
    walk(ctx, root, &mut names);
    ctx.var_names.extend(names);
}

/// Nomes simples que um alvo liga (parâmetro, atribuição, `for`), sem entrar em `obj.attr` nem
/// em `v[i]`.
fn bound_identifiers(ctx: &Ctx, node: Node, out: &mut HashSet<String>) {
    if node.kind() == "identifier" {
        out.insert(ctx.text(node).to_string());
        return;
    }
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if child.kind() != "attribute" && child.kind() != "subscript" {
            bound_identifiers(ctx, child, out);
        }
    }
}

/// Nomes de uma função: os próprios (parâmetros, atribuídos sem `global`, `nonlocal`) ocultam a
/// variável de módulo; os declarados `global` a escrevem.
struct FunctionNames {
    locals: HashSet<String>,
    globals: HashSet<String>,
}

fn function_names(ctx: &Ctx, def: Node, body: Node) -> FunctionNames {
    fn walk(ctx: &Ctx, node: Node, names: &mut FunctionNames) {
        let mut cursor = node.walk();
        for child in node.named_children(&mut cursor) {
            match child.kind() {
                "global_statement" => {
                    let mut c = child.walk();
                    for id in child.named_children(&mut c) {
                        names.globals.insert(ctx.text(id).to_string());
                    }
                }
                // `nonlocal` aponta para uma função envolvente, nunca para o módulo.
                "nonlocal_statement" => {
                    let mut c = child.walk();
                    for id in child.named_children(&mut c) {
                        names.locals.insert(ctx.text(id).to_string());
                    }
                }
                "assignment"
                | "augmented_assignment"
                | "for_statement"
                | "with_item"
                | "named_expression"
                | "as_pattern" => {
                    for field in ["left", "name", "alias"] {
                        if let Some(t) = child.child_by_field_name(field) {
                            bound_identifiers(ctx, t, &mut names.locals);
                        }
                    }
                }
                "parameters" | "lambda_parameters" => {
                    bound_identifiers(ctx, child, &mut names.locals)
                }
                _ => {}
            }
            walk(ctx, child, names);
        }
    }
    let mut names = FunctionNames {
        locals: HashSet::new(),
        globals: HashSet::new(),
    };
    if let Some(params) = def.child_by_field_name("parameters") {
        bound_identifiers(ctx, params, &mut names.locals);
    }
    walk(ctx, body, &mut names);
    for g in &names.globals {
        names.locals.remove(g);
    }
    names
}

/// Onde a varredura está: o símbolo de origem das refs e o que ele pode escrever.
struct Walk<'a> {
    src: &'a str,
    locals: &'a HashSet<String>,
    /// Nomes que a função declarou `global`; vazio fora de função.
    globals: &'a HashSet<String>,
    in_function: bool,
    /// Atribuição a nome simples vale como escrita (módulo; função só sob `global`).
    writes: bool,
}

pub(super) fn function_var_effects(ctx: &mut Ctx, def: Node, body: Node, src: &str) {
    let names = function_names(ctx, def, body);
    let w = Walk {
        src,
        locals: &names.locals,
        globals: &names.globals,
        in_function: true,
        writes: true,
    };
    visit_children(ctx, body, &w);
}

/// Instruções do módulo, e, a partir delas, corpos de classe, decoradores, bases e argumentos
/// padrão: escritas e leituras de nomes simples. O corpo de uma função é visto por
/// `function_var_effects`.
pub(super) fn module_var_effects(ctx: &mut Ctx, root: Node, module: &str) {
    let none = HashSet::new();
    let w = Walk {
        src: module,
        locals: &none,
        globals: &none,
        in_function: false,
        writes: true,
    };
    visit_children(ctx, root, &w);
}

fn visit_children(ctx: &mut Ctx, node: Node, w: &Walk) {
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        visit(ctx, child, w);
    }
}

fn read(ctx: &mut Ctx, node: Node, w: &Walk) {
    let name = ctx.text(node).to_string();
    let parent = node.parent();
    let is_member = parent.is_some_and(|p| {
        p.kind() == "attribute" && p.child_by_field_name("attribute") == Some(node)
    });
    let is_label = parent.is_some_and(|p| {
        p.kind() == "keyword_argument" && p.child_by_field_name("name") == Some(node)
    });
    if !is_member && !is_label && ctx.var_names.contains(&name) && !w.locals.contains(&name) {
        push_ref(ctx, w.src, RefKind::LeVar, name, node);
    }
}

/// Alvo que liga nomes (desempacotamento, `for`, `with ... as`): o nome ligado não é leitura;
/// o que está dentro de `obj.attr` ou `v[i]` continua sendo lido.
fn bind_target(ctx: &mut Ctx, node: Node, w: &Walk) {
    match node.kind() {
        "identifier" => {}
        "pattern_list" | "tuple_pattern" | "list_pattern" | "list_splat_pattern"
        | "as_pattern_target" => {
            let mut cursor = node.walk();
            for child in node.named_children(&mut cursor) {
                bind_target(ctx, child, w);
            }
        }
        _ => visit(ctx, node, w),
    }
}

fn visit(ctx: &mut Ctx, node: Node, w: &Walk) {
    match node.kind() {
        "identifier" => read(ctx, node, w),
        "global_statement" | "nonlocal_statement" => {}
        // Alvos de `for` e de `with ... as` ligam o nome (D3 não os conta como escrita).
        "for_statement" => {
            let left = node.child_by_field_name("left");
            visit_binding(ctx, node, w, |c| Some(c) == left);
        }
        "as_pattern" => visit_binding(ctx, node, w, |c| c.kind() == "as_pattern_target"),
        "list_comprehension"
        | "set_comprehension"
        | "dictionary_comprehension"
        | "generator_expression" => visit_comprehension(ctx, node, w),
        "assignment" | "augmented_assignment" => visit_assignment(ctx, node, w),
        "class_definition" if !w.in_function => visit_class(ctx, node, w),
        "function_definition" if !w.in_function => visit_defaults(ctx, node, w),
        _ => visit_children(ctx, node, w),
    }
}

/// Visita os filhos de `node`; o que `is_target` aponta liga nomes e não é lido.
fn visit_binding(ctx: &mut Ctx, node: Node, w: &Walk, is_target: impl Fn(Node) -> bool) {
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if is_target(child) {
            bind_target(ctx, child, w);
        } else {
            visit(ctx, child, w);
        }
    }
}

/// Compreensão tem escopo próprio: os alvos dos `for` ocultam o nome dentro dela. Só o iterável
/// da primeira cláusula é lido fora desse escopo; os seguintes veem os alvos anteriores.
fn visit_comprehension(ctx: &mut Ctx, node: Node, w: &Walk) {
    let mut shadow = w.locals.clone();
    let mut cursor = node.walk();
    for clause in node.named_children(&mut cursor) {
        if clause.kind() == "for_in_clause"
            && let Some(left) = clause.child_by_field_name("left")
        {
            bound_identifiers(ctx, left, &mut shadow);
        }
    }
    let inner = Walk {
        locals: &shadow,
        ..*w
    };
    let mut first_clause = true;
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if child.kind() != "for_in_clause" {
            visit(ctx, child, &inner);
            continue;
        }
        if let Some(right) = child.child_by_field_name("right") {
            let scope = if first_clause { w } else { &inner };
            visit(ctx, right, scope);
        }
        first_clause = false;
    }
}

fn visit_assignment(ctx: &mut Ctx, node: Node, w: &Walk) {
    if let Some(left) = node.child_by_field_name("left") {
        if left.kind() == "identifier" {
            let name = ctx.text(left).to_string();
            let writes = w.writes
                && ctx.var_names.contains(&name)
                && (!w.in_function || w.globals.contains(&name));
            if writes {
                push_ref(ctx, w.src, RefKind::EscreveVar, name, left);
            }
        } else {
            bind_target(ctx, left, w);
        }
    }
    if let Some(right) = node.child_by_field_name("right") {
        visit(ctx, right, w);
    }
}

/// Fora de função, a classe é um escopo próprio: o corpo lê com a classe como origem e as
/// atribuições dele definem atributos, não escrevem variáveis de módulo.
fn visit_class(ctx: &mut Ctx, node: Node, w: &Walk) {
    let name = node
        .child_by_field_name("name")
        .map(|n| ctx.text(n).to_string())
        .unwrap_or_default();
    if let Some(bases) = node.child_by_field_name("superclasses") {
        visit_children(ctx, bases, w);
    }
    if let Some(body) = node.child_by_field_name("body") {
        let qname = format!("{}.{name}", w.src);
        let inner = Walk {
            src: &qname,
            writes: false,
            ..*w
        };
        visit_children(ctx, body, &inner);
    }
}

/// Fora de função só roda na definição o valor padrão dos parâmetros; o corpo tem a própria
/// varredura.
fn visit_defaults(ctx: &mut Ctx, node: Node, w: &Walk) {
    if let Some(params) = node.child_by_field_name("parameters") {
        let mut cursor = params.walk();
        for p in params.named_children(&mut cursor) {
            if let Some(value) = p.child_by_field_name("value") {
                visit(ctx, value, w);
            }
        }
    }
}
