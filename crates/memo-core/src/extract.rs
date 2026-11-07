//! Extração estática de Python (spec §5).

use tree_sitter::{Node, Parser};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolKind {
    Module,
    Class,
    Function,
    Method,
    Attribute,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolFact {
    pub qname: String,
    pub kind: SymbolKind,
    pub line_start: usize,
    pub line_end: usize,
    pub sig_hash: String,
    pub body_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReceiveFact {
    pub src_qname: String,
    /// Nome do parâmetro.
    pub attr: String,
    /// `None` quando o parâmetro não tem anotação (spec §5.3).
    pub dst_text: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FileFacts {
    pub symbols: Vec<SymbolFact>,
    pub receives: Vec<ReceiveFact>,
}

#[derive(Debug)]
pub struct ExtractError(pub String);

struct Ctx<'a> {
    src: &'a [u8],
    facts: FileFacts,
}

impl Ctx<'_> {
    fn text(&self, node: Node) -> &str {
        node.utf8_text(self.src).unwrap_or("")
    }
}

fn hash_of(bytes: &str) -> String {
    blake3::hash(bytes.as_bytes()).to_hex().to_string()
}

/// Lê um arquivo Python (`module` é o módulo pontuado) e devolve fatos.
pub fn extract_python(module: &str, source: &str) -> Result<FileFacts, ExtractError> {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_python::LANGUAGE.into())
        .map_err(|e| ExtractError(e.to_string()))?;
    let tree = parser
        .parse(source, None)
        .ok_or_else(|| ExtractError("parse failed".into()))?;
    let root = tree.root_node();
    if root.has_error() {
        return Err(ExtractError("syntax error".into()));
    }
    let mut ctx = Ctx {
        src: source.as_bytes(),
        facts: FileFacts::default(),
    };
    ctx.facts.symbols.push(SymbolFact {
        qname: module.to_string(),
        kind: SymbolKind::Module,
        line_start: root.start_position().row + 1,
        line_end: root.end_position().row + 1,
        sig_hash: hash_of(""),
        body_hash: hash_of(&serialize(&ctx, root)),
    });
    visit_block(&mut ctx, root, module, false);
    Ok(ctx.facts)
}

fn visit_block(ctx: &mut Ctx, block: Node, prefix: &str, in_class: bool) {
    let mut cursor = block.walk();
    for child in block.named_children(&mut cursor) {
        let def = if child.kind() == "decorated_definition" {
            child.child_by_field_name("definition")
        } else {
            Some(child)
        };
        match def.map(|d| (d.kind(), d)) {
            Some(("class_definition", d)) => visit_class(ctx, d, child, prefix),
            Some(("function_definition", d)) => visit_function(ctx, d, child, prefix, in_class),
            _ => {
                if in_class {
                    class_body_attribute(ctx, child, prefix);
                }
            }
        }
    }
}

fn name_of(ctx: &Ctx, def: Node) -> String {
    def.child_by_field_name("name")
        .map(|n| ctx.text(n).to_string())
        .unwrap_or_default()
}

fn decorators_text(ctx: &Ctx, outer: Node) -> String {
    let mut cursor = outer.walk();
    outer
        .named_children(&mut cursor)
        .filter(|n| n.kind() == "decorator")
        .map(|n| ctx.text(n).split_whitespace().collect::<String>())
        .collect::<Vec<_>>()
        .join(",")
}

fn visit_class(ctx: &mut Ctx, def: Node, outer: Node, prefix: &str) {
    let qname = format!("{prefix}.{}", name_of(ctx, def));
    let sig = format!(
        "class|{}|{}",
        decorators_text(ctx, outer),
        def.child_by_field_name("superclasses")
            .map(|n| ctx.text(n).split_whitespace().collect::<String>())
            .unwrap_or_default()
    );
    let body = def.child_by_field_name("body");
    let body_hash = body.map(|b| serialize(ctx, b)).unwrap_or_default();
    ctx.facts.symbols.push(SymbolFact {
        qname: qname.clone(),
        kind: SymbolKind::Class,
        line_start: outer.start_position().row + 1,
        line_end: outer.end_position().row + 1,
        sig_hash: hash_of(&sig),
        body_hash: hash_of(&body_hash),
    });
    if let Some(b) = body {
        visit_block(ctx, b, &qname, true);
    }
}

fn visit_function(ctx: &mut Ctx, def: Node, outer: Node, prefix: &str, in_class: bool) {
    let qname = format!("{prefix}.{}", name_of(ctx, def));
    let is_async = ctx.text(def).starts_with("async");
    let ret = def
        .child_by_field_name("return_type")
        .map(|n| ctx.text(n).split_whitespace().collect::<String>())
        .unwrap_or_default();
    let mut params_sig = Vec::new();
    if let Some(params) = def.child_by_field_name("parameters") {
        let mut cursor = params.walk();
        for p in params.named_children(&mut cursor) {
            if let Some(sig) = param_signature(ctx, p, &qname, in_class) {
                params_sig.push(sig);
            }
        }
    }
    let sig = format!(
        "{}|{}|{}|{}",
        if is_async { "async" } else { "def" },
        decorators_text(ctx, outer),
        params_sig.join(","),
        ret
    );
    let body = def.child_by_field_name("body");
    let body_hash = body.map(|b| serialize(ctx, b)).unwrap_or_default();
    ctx.facts.symbols.push(SymbolFact {
        qname: qname.clone(),
        kind: if in_class {
            SymbolKind::Method
        } else {
            SymbolKind::Function
        },
        line_start: outer.start_position().row + 1,
        line_end: outer.end_position().row + 1,
        sig_hash: hash_of(&sig),
        body_hash: hash_of(&body_hash),
    });
    if in_class && let Some(b) = body {
        self_attributes(ctx, b, prefix);
    }
}

/// Normaliza um parâmetro e, se for nomeado, registra o `RECEBE` (spec §5.3).
fn param_signature(ctx: &mut Ctx, p: Node, qname: &str, in_class: bool) -> Option<String> {
    let (name, annotation, has_default) = match p.kind() {
        "identifier" => (ctx.text(p).to_string(), None, false),
        "typed_parameter" => {
            let mut cursor = p.walk();
            let ident = p.named_children(&mut cursor).find(|n| {
                matches!(
                    n.kind(),
                    "identifier" | "list_splat_pattern" | "dictionary_splat_pattern"
                )
            })?;
            let name = ctx.text(ident).trim_start_matches('*').to_string();
            let ann = p
                .child_by_field_name("type")
                .map(|t| ctx.text(t).to_string());
            (name, ann, false)
        }
        "default_parameter" | "typed_default_parameter" => {
            let name = ctx.text(p.child_by_field_name("name")?).to_string();
            let ann = p
                .child_by_field_name("type")
                .map(|t| ctx.text(t).to_string());
            (name, ann, true)
        }
        "list_splat_pattern" | "dictionary_splat_pattern" => {
            (ctx.text(p).trim_start_matches('*').to_string(), None, false)
        }
        other => return Some(other.to_string()),
    };
    if !(in_class && matches!(name.as_str(), "self" | "cls")) {
        ctx.facts.receives.push(ReceiveFact {
            src_qname: qname.to_string(),
            attr: name.clone(),
            dst_text: annotation.as_deref().map(unwrap_type),
        });
    }
    Some(format!(
        "{name}:{}{}",
        annotation
            .map(|a| a.split_whitespace().collect::<String>())
            .unwrap_or_default(),
        if has_default { "=" } else { "" }
    ))
}

/// Desembrulha `list[X]`, `set[X]`, `Optional[X]`, `X | None`, `Awaitable[X]`, `Coroutine[..., X]`.
fn unwrap_type(raw: &str) -> String {
    let mut t = raw.split_whitespace().collect::<String>();
    loop {
        if let Some(rest) = t.strip_suffix("|None").or_else(|| t.strip_prefix("None|")) {
            t = rest.to_string();
            continue;
        }
        let inner = ["list", "set", "Optional", "Awaitable", "Coroutine"]
            .iter()
            .find_map(|w| {
                t.strip_prefix(&format!("{w}["))
                    .and_then(|r| r.strip_suffix(']'))
            });
        match inner {
            Some(i) => t = i.rsplit(',').next().unwrap_or(i).to_string(),
            None => return t,
        }
    }
}

fn push_attribute(ctx: &mut Ctx, class_qname: &str, name: &str, node: Node) {
    let qname = format!("{class_qname}.{name}");
    if ctx.facts.symbols.iter().any(|s| s.qname == qname) {
        return;
    }
    let rhs = node
        .child_by_field_name("type")
        .map(|t| ctx.text(t).split_whitespace().collect::<String>())
        .unwrap_or_default();
    ctx.facts.symbols.push(SymbolFact {
        qname,
        kind: SymbolKind::Attribute,
        line_start: node.start_position().row + 1,
        line_end: node.end_position().row + 1,
        sig_hash: hash_of(&rhs),
        body_hash: hash_of(""),
    });
}

/// Atributo anotado no corpo da classe (`x: int = 0`).
fn class_body_attribute(ctx: &mut Ctx, stmt: Node, class_qname: &str) {
    if stmt.kind() != "expression_statement" {
        return;
    }
    let Some(assign) = stmt.named_child(0).filter(|n| n.kind() == "assignment") else {
        return;
    };
    if assign.child_by_field_name("type").is_none() {
        return;
    }
    if let Some(left) = assign
        .child_by_field_name("left")
        .filter(|n| n.kind() == "identifier")
    {
        let name = ctx.text(left).to_string();
        push_attribute(ctx, class_qname, &name, assign);
    }
}

/// `self.x = ...` em qualquer ponto do método, sem entrar em funções locais.
fn self_attributes(ctx: &mut Ctx, node: Node, class_qname: &str) {
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        match child.kind() {
            "function_definition" | "class_definition" | "lambda" => continue,
            "assignment" => {
                if let Some(left) = child.child_by_field_name("left")
                    && left.kind() == "attribute"
                    && left
                        .child_by_field_name("object")
                        .is_some_and(|o| ctx.text(o) == "self")
                    && let Some(attr) = left.child_by_field_name("attribute")
                {
                    let name = ctx.text(attr).to_string();
                    push_attribute(ctx, class_qname, &name, child);
                }
            }
            _ => {}
        }
        self_attributes(ctx, child, class_qname);
    }
}

fn is_docstring(node: Node) -> bool {
    node.kind() == "expression_statement"
        && node.named_child_count() == 1
        && node.named_child(0).is_some_and(|c| c.kind() == "string")
}

/// Serializa o corpo por tipos de nó e folhas, sem comentários, docstring e posições.
fn serialize(ctx: &Ctx, node: Node) -> String {
    let mut out = String::new();
    write_node(ctx, node, &mut out, true);
    out
}

fn write_node(ctx: &Ctx, node: Node, out: &mut String, first_in_block: bool) {
    if node.kind() == "comment" || (first_in_block && is_docstring(node)) {
        return;
    }
    out.push('(');
    out.push_str(node.kind());
    if node.child_count() == 0 {
        out.push(' ');
        out.push_str(ctx.text(node));
    } else {
        // Inclui tokens anônimos (operadores, palavras-chave): `a * b` difere de `a + b`.
        let is_block = matches!(node.kind(), "block" | "module");
        let mut cursor = node.walk();
        let mut first_named = true;
        for child in node.children(&mut cursor) {
            let leading = is_block && child.is_named() && first_named;
            write_node(ctx, child, out, leading);
            if child.is_named() && child.kind() != "comment" {
                first_named = false;
            }
        }
    }
    out.push(')');
}
