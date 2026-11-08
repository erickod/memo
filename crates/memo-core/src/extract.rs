//! Extração estática de Python (spec §5).

use std::collections::HashMap;
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
    /// Classe que herda de `Protocol`.
    pub is_protocol: bool,
    /// Tipo anotado (texto bruto) de um atributo, quando conhecido.
    pub type_text: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefKind {
    Retorna,
    Levanta,
    Especializa,
    Escreve,
    Le,
    Chama,
}

impl RefKind {
    pub fn as_str(self) -> &'static str {
        match self {
            RefKind::Retorna => "RETORNA",
            RefKind::Levanta => "LEVANTA",
            RefKind::Especializa => "ESPECIALIZA",
            RefKind::Escreve => "ESCREVE",
            RefKind::Le => "LE",
            RefKind::Chama => "CHAMA",
        }
    }
}

/// Referência bruta lida da AST, resolvida depois contra o índice (spec §3, `raw_ref`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefFact {
    pub src_qname: String,
    pub kind: RefKind,
    /// Tipo (RETORNA, LEVANTA, ESPECIALIZA) ou caminho após `self` (ESCREVE, LE: `a.b`).
    pub text: String,
    /// Tipo (texto bruto) do receptor local, para chamadas `x.m()` (spec §5.4, regra 6).
    pub receiver: Option<String>,
    pub line: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportFact {
    pub local_name: String,
    /// Módulo ou símbolo importado, já com imports relativos resolvidos.
    pub target: String,
    pub line: usize,
    pub type_checking: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReceiveFact {
    pub src_qname: String,
    /// Nome do parâmetro.
    pub attr: String,
    /// `None` quando o parâmetro não tem anotação (spec §5.3).
    pub dst_text: Option<String>,
    pub pos: usize,
    pub line: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FileFacts {
    pub symbols: Vec<SymbolFact>,
    pub receives: Vec<ReceiveFact>,
    pub refs: Vec<RefFact>,
    pub imports: Vec<ImportFact>,
    /// Docstring (texto) de cada função ou método que a tem.
    pub docs: Vec<DocFact>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocFact {
    pub src_qname: String,
    pub text: String,
}

#[derive(Debug)]
pub struct ExtractError(pub String);

const MUTATORS: &[&str] = &[
    "append",
    "extend",
    "update",
    "add",
    "pop",
    "remove",
    "clear",
    "insert",
    "setdefault",
];

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
    extract_python_file(module, false, source)
}

/// Como `extract_python`; `is_package` (arquivo `__init__.py`) afeta imports relativos.
pub fn extract_python_file(
    module: &str,
    is_package: bool,
    source: &str,
) -> Result<FileFacts, ExtractError> {
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
        is_protocol: false,
        type_text: None,
    });
    visit_block(&mut ctx, root, module, false);
    collect_imports(&mut ctx, root, module, is_package, false);
    Ok(ctx.facts)
}

/// Python permite redefinir um nome (`@property` e `@x.setter`, `@overload`, atributo e método
/// com o mesmo nome). O `qname` é identidade, então vale a primeira definição; os efeitos e
/// referências das redefinições seguem atribuídos a ela.
fn add_symbol(ctx: &mut Ctx, fact: SymbolFact) {
    if ctx.facts.symbols.iter().all(|s| s.qname != fact.qname) {
        ctx.facts.symbols.push(fact);
    }
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
    let line = def.start_position().row + 1;
    let mut is_protocol = false;
    if let Some(bases) = def.child_by_field_name("superclasses") {
        let mut cursor = bases.walk();
        for base in bases.named_children(&mut cursor) {
            if base.kind() == "keyword_argument" {
                continue;
            }
            let text = ctx.text(base).split_whitespace().collect::<String>();
            let text = text.split('[').next().unwrap_or("").to_string();
            is_protocol |= text == "Protocol" || text.ends_with(".Protocol");
            ctx.facts.refs.push(RefFact {
                src_qname: qname.clone(),
                kind: RefKind::Especializa,
                text,
                receiver: None,
                line,
            });
        }
    }
    let body = def.child_by_field_name("body");
    let body_hash = body.map(|b| serialize(ctx, b)).unwrap_or_default();
    add_symbol(
        ctx,
        SymbolFact {
            qname: qname.clone(),
            kind: SymbolKind::Class,
            line_start: outer.start_position().row + 1,
            line_end: outer.end_position().row + 1,
            sig_hash: hash_of(&sig),
            body_hash: hash_of(&body_hash),
            is_protocol,
            type_text: None,
        },
    );
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
    if let Some(text) = def
        .child_by_field_name("body")
        .and_then(|b| docstring_text(ctx, b))
    {
        ctx.facts.docs.push(DocFact {
            src_qname: qname.clone(),
            text,
        });
    }
    let def_line = def.start_position().row + 1;
    if let Some(r) = def.child_by_field_name("return_type") {
        let text = unwrap_type(ctx.text(r));
        if text != "None" {
            ctx.facts.refs.push(RefFact {
                src_qname: qname.clone(),
                kind: RefKind::Retorna,
                text,
                receiver: None,
                line: def_line,
            });
        }
    }
    let param_types: HashMap<String, String> = ctx
        .facts
        .receives
        .iter()
        .filter(|r| r.src_qname == qname)
        .filter_map(|r| r.dst_text.clone().map(|t| (r.attr.clone(), t)))
        .collect();
    let sig = format!(
        "{}|{}|{}|{}",
        if is_async { "async" } else { "def" },
        decorators_text(ctx, outer),
        params_sig.join(","),
        ret
    );
    let body = def.child_by_field_name("body");
    let body_hash = body.map(|b| serialize(ctx, b)).unwrap_or_default();
    add_symbol(
        ctx,
        SymbolFact {
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
            is_protocol: false,
            type_text: None,
        },
    );
    if let Some(b) = body {
        if in_class {
            self_attributes(ctx, b, prefix, &param_types);
        }
        let mut locals = param_types.clone();
        collect_locals(ctx, b, &mut locals);
        scan_effects(ctx, b, &qname, in_class, &locals);
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
        let pos = ctx
            .facts
            .receives
            .iter()
            .filter(|r| r.src_qname == qname)
            .count();
        ctx.facts.receives.push(ReceiveFact {
            src_qname: qname.to_string(),
            attr: name.clone(),
            dst_text: annotation.as_deref().map(unwrap_type),
            pos,
            line: p.start_position().row + 1,
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

fn push_attribute(
    ctx: &mut Ctx,
    class_qname: &str,
    name: &str,
    node: Node,
    type_text: Option<String>,
) {
    let qname = format!("{class_qname}.{name}");
    if let Some(existing) = ctx.facts.symbols.iter_mut().find(|s| s.qname == qname) {
        if existing.type_text.is_none() {
            existing.type_text = type_text;
        }
        return;
    }
    let rhs = node
        .child_by_field_name("type")
        .map(|t| ctx.text(t).split_whitespace().collect::<String>())
        .unwrap_or_default();
    add_symbol(
        ctx,
        SymbolFact {
            qname,
            kind: SymbolKind::Attribute,
            line_start: node.start_position().row + 1,
            line_end: node.end_position().row + 1,
            sig_hash: hash_of(&rhs),
            body_hash: hash_of(""),
            is_protocol: false,
            type_text,
        },
    );
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
        let type_text = assign
            .child_by_field_name("type")
            .map(|t| unwrap_type(ctx.text(t)));
        push_attribute(ctx, class_qname, &name, assign, type_text);
    }
}

/// `self.x = ...` em qualquer ponto do método, sem entrar em funções locais.
fn self_attributes(
    ctx: &mut Ctx,
    node: Node,
    class_qname: &str,
    param_types: &HashMap<String, String>,
) {
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
                    let type_text = child
                        .child_by_field_name("type")
                        .map(|t| unwrap_type(ctx.text(t)))
                        .or_else(|| {
                            let right = child.child_by_field_name("right")?;
                            param_types.get(ctx.text(right)).cloned()
                        });
                    push_attribute(ctx, class_qname, &name, child, type_text);
                }
            }
            _ => {}
        }
        self_attributes(ctx, child, class_qname, param_types);
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

/// Caminho após `self` de uma cadeia `self.a.b` (vazio para o próprio `self`).
fn self_chain(ctx: &Ctx, node: Node) -> Option<Vec<String>> {
    match node.kind() {
        "identifier" if ctx.text(node) == "self" => Some(Vec::new()),
        "attribute" => {
            let mut path = self_chain(ctx, node.child_by_field_name("object")?)?;
            path.push(ctx.text(node.child_by_field_name("attribute")?).to_string());
            Some(path)
        }
        _ => None,
    }
}

fn push_ref(ctx: &mut Ctx, src: &str, kind: RefKind, text: String, node: Node) {
    ctx.facts.refs.push(RefFact {
        src_qname: src.to_string(),
        kind,
        text,
        receiver: None,
        line: node.start_position().row + 1,
    });
}

fn write_target(
    ctx: &mut Ctx,
    target: Node,
    src: &str,
    track_self: bool,
    locals: &HashMap<String, String>,
) {
    match target.kind() {
        "attribute" | "subscript" => {
            let base = if target.kind() == "subscript" {
                target.child_by_field_name("value")
            } else {
                Some(target)
            };
            match base.and_then(|b| track_self.then(|| self_chain(ctx, b)).flatten()) {
                Some(path) if !path.is_empty() => {
                    push_ref(ctx, src, RefKind::Escreve, path.join("."), target);
                }
                _ => {
                    if let Some(b) = base {
                        scan_effects(ctx, b, src, track_self, locals);
                    }
                }
            }
            if target.kind() == "subscript" {
                let mut cursor = target.walk();
                for idx in target.children_by_field_name("subscript", &mut cursor) {
                    scan_effects(ctx, idx, src, track_self, locals);
                }
            }
        }
        "pattern_list" | "tuple_pattern" | "list_pattern" => {
            let mut cursor = target.walk();
            for child in target.named_children(&mut cursor) {
                write_target(ctx, child, src, track_self, locals);
            }
        }
        _ => {}
    }
}

/// Efeitos de um símbolo: `ESCREVE`, `LE` (só `self.x`) e `LEVANTA` (spec §5.5).
fn scan_effects(
    ctx: &mut Ctx,
    node: Node,
    src: &str,
    track_self: bool,
    locals: &HashMap<String, String>,
) {
    match node.kind() {
        "class_definition" => return,
        "assignment" | "augmented_assignment" => {
            if let Some(left) = node.child_by_field_name("left") {
                write_target(ctx, left, src, track_self, locals);
            }
            if let Some(right) = node.child_by_field_name("right") {
                scan_effects(ctx, right, src, track_self, locals);
            }
            return;
        }
        "call" => {
            let func = node.child_by_field_name("function");
            if let Some(callee) = func.and_then(|f| callee_text(ctx, f)) {
                let first = callee.split('.').next().unwrap_or("");
                let receiver = (callee.contains('.') && !matches!(first, "self" | "cls"))
                    .then(|| locals.get(first).cloned())
                    .flatten();
                ctx.facts.refs.push(RefFact {
                    src_qname: src.to_string(),
                    kind: RefKind::Chama,
                    text: callee,
                    receiver,
                    line: node.start_position().row + 1,
                });
            }
            match func {
                Some(f) if f.kind() == "attribute" => {
                    let obj = f.child_by_field_name("object");
                    let method = f
                        .child_by_field_name("attribute")
                        .map(|m| ctx.text(m).to_string())
                        .unwrap_or_default();
                    let path = obj
                        .and_then(|o| track_self.then(|| self_chain(ctx, o)).flatten())
                        .filter(|p| !p.is_empty());
                    match path {
                        Some(path) => {
                            let kind = if MUTATORS.contains(&method.as_str()) {
                                RefKind::Escreve
                            } else {
                                RefKind::Le
                            };
                            push_ref(ctx, src, kind, path.join("."), node);
                        }
                        None => {
                            if let Some(o) = obj {
                                scan_effects(ctx, o, src, track_self, locals);
                            }
                        }
                    }
                }
                Some(f) => scan_effects(ctx, f, src, track_self, locals),
                None => {}
            }
            if let Some(args) = node.child_by_field_name("arguments") {
                scan_effects(ctx, args, src, track_self, locals);
            }
            return;
        }
        "attribute" => {
            if let Some(path) = track_self.then(|| self_chain(ctx, node)).flatten()
                && !path.is_empty()
            {
                push_ref(ctx, src, RefKind::Le, path.join("."), node);
                return;
            }
        }
        "raise_statement" => {
            let raised = node.named_child(0).map(|n| match n.kind() {
                "call" => n.child_by_field_name("function").unwrap_or(n),
                _ => n,
            });
            if let Some(r) = raised {
                let text = ctx.text(r).to_string();
                push_ref(ctx, src, RefKind::Levanta, text, node);
            }
        }
        _ => {}
    }
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        scan_effects(ctx, child, src, track_self, locals);
    }
}

/// `from ..a import b` em `module` vira o módulo absoluto (spec §4, `IMPORTA`).
fn absolute_module(spec: &str, module: &str, is_package: bool) -> String {
    let dots = spec.chars().take_while(|c| *c == '.').count();
    let rest = &spec[dots..];
    if dots == 0 {
        return rest.to_string();
    }
    let mut base = if is_package {
        module.to_string()
    } else {
        module
            .rsplit_once('.')
            .map(|(p, _)| p.to_string())
            .unwrap_or_default()
    };
    for _ in 1..dots {
        base = base
            .rsplit_once('.')
            .map(|(p, _)| p.to_string())
            .unwrap_or_default();
    }
    [base.as_str(), rest]
        .iter()
        .filter(|p| !p.is_empty())
        .copied()
        .collect::<Vec<_>>()
        .join(".")
}

fn push_import(ctx: &mut Ctx, local: String, target: String, node: Node, type_checking: bool) {
    ctx.facts.imports.push(ImportFact {
        local_name: local,
        target,
        line: node.start_position().row + 1,
        type_checking,
    });
}

fn collect_imports(ctx: &mut Ctx, node: Node, module: &str, is_package: bool, tc: bool) {
    match node.kind() {
        "import_statement" => {
            let mut cursor = node.walk();
            for name in node.children_by_field_name("name", &mut cursor) {
                if name.kind() == "aliased_import" {
                    let target = name.child_by_field_name("name").map(|n| ctx.text(n));
                    let alias = name.child_by_field_name("alias").map(|n| ctx.text(n));
                    if let (Some(t), Some(a)) = (target, alias) {
                        push_import(ctx, a.to_string(), t.to_string(), node, tc);
                    }
                } else {
                    let text = ctx.text(name).to_string();
                    push_import(ctx, text.clone(), text, node, tc);
                }
            }
            return;
        }
        "import_from_statement" => {
            let spec = node
                .child_by_field_name("module_name")
                .map(|n| ctx.text(n).to_string())
                .unwrap_or_default();
            let base = absolute_module(&spec, module, is_package);
            let join = |n: &str| {
                if base.is_empty() {
                    n.to_string()
                } else {
                    format!("{base}.{n}")
                }
            };
            let mut cursor = node.walk();
            for name in node.children_by_field_name("name", &mut cursor) {
                if name.kind() == "aliased_import" {
                    let target = name.child_by_field_name("name").map(|n| ctx.text(n));
                    let alias = name.child_by_field_name("alias").map(|n| ctx.text(n));
                    if let (Some(t), Some(a)) = (target, alias) {
                        push_import(ctx, a.to_string(), join(t), node, tc);
                    }
                } else {
                    let text = ctx.text(name).to_string();
                    push_import(ctx, text.clone(), join(&text), node, tc);
                }
            }
            return;
        }
        "if_statement" => {
            let cond = node
                .child_by_field_name("condition")
                .map(|c| ctx.text(c).to_string())
                .unwrap_or_default();
            let guard = cond == "TYPE_CHECKING" || cond.ends_with(".TYPE_CHECKING");
            let consequence = node.child_by_field_name("consequence").map(|c| c.id());
            let mut cursor = node.walk();
            for child in node.named_children(&mut cursor) {
                let inner = tc || (guard && Some(child.id()) == consequence);
                collect_imports(ctx, child, module, is_package, inner);
            }
            return;
        }
        _ => {}
    }
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_imports(ctx, child, module, is_package, tc);
    }
}

/// `a.b.c` quando o nó é só nomes e pontos; `None` caso contrário.
fn dotted_name(ctx: &Ctx, node: Node) -> Option<String> {
    match node.kind() {
        "identifier" => Some(ctx.text(node).to_string()),
        "attribute" => {
            let object = dotted_name(ctx, node.child_by_field_name("object")?)?;
            Some(format!(
                "{object}.{}",
                ctx.text(node.child_by_field_name("attribute")?)
            ))
        }
        _ => None,
    }
}

/// Texto do chamado; `?.m` quando o receptor não é um nome (ex.: `f().m()`), só o método importa.
fn callee_text(ctx: &Ctx, func: Node) -> Option<String> {
    dotted_name(ctx, func).or_else(|| {
        (func.kind() == "attribute")
            .then(|| func.child_by_field_name("attribute"))
            .flatten()
            .map(|m| format!("?.{}", ctx.text(m)))
    })
}

/// Tipos de variáveis locais: `x: T = ...` e `x = Tipo(...)` (primeira ocorrência vale).
fn collect_locals(ctx: &Ctx, node: Node, locals: &mut HashMap<String, String>) {
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        match child.kind() {
            "function_definition" | "class_definition" | "lambda" => continue,
            "assignment" => {
                let left = child.child_by_field_name("left");
                if let Some(l) = left.filter(|l| l.kind() == "identifier") {
                    let name = ctx.text(l).to_string();
                    let ty = child
                        .child_by_field_name("type")
                        .map(|t| unwrap_type(ctx.text(t)))
                        .or_else(|| {
                            let right = child.child_by_field_name("right")?;
                            if right.kind() != "call" {
                                return None;
                            }
                            dotted_name(ctx, right.child_by_field_name("function")?)
                        });
                    if let Some(ty) = ty {
                        locals.entry(name).or_insert(ty);
                    }
                }
            }
            _ => {}
        }
        collect_locals(ctx, child, locals);
    }
}

/// Texto da docstring (primeira instrução do corpo, se for uma string), sem as aspas.
fn docstring_text(ctx: &Ctx, body: Node) -> Option<String> {
    let first = body.named_child(0)?;
    if !is_docstring(first) {
        return None;
    }
    let string = first.named_child(0)?;
    let mut cursor = string.walk();
    let content: Vec<&str> = string
        .named_children(&mut cursor)
        .filter(|n| n.kind() == "string_content")
        .map(|n| ctx.text(n))
        .collect();
    Some(content.join(" "))
}
