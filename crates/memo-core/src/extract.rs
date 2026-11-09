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
    /// Membro de Enum: o valor literal, ou vazio quando o valor não é literal (TASK-7, D6).
    pub value_text: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefKind {
    Retorna,
    Levanta,
    Especializa,
    Escreve,
    Le,
    /// Leitura `X.attr` por um nome que não é `self` (TASK-7, D5); resolvida como `LE`.
    LeAttr,
    EscreveAttr,
    /// Leitura de nome simples que pode ser variável de módulo, própria ou importada (TASK-10).
    LeVar,
    /// Atribuição a nome simples no módulo ou sob `global` numa função (TASK-10).
    EscreveVar,
    Inicializa,
    SqlUpdate,
    SqlInsert,
    ChamaConstruida,
    /// Requisição HTTP por caminho literal (`client.get("/x")`): texto = caminho normalizado,
    /// receptor = método em maiúsculas.
    Requisita,
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
            RefKind::LeAttr => "LE_ATTR",
            RefKind::EscreveAttr => "ESCREVE_ATTR",
            RefKind::LeVar => "LE_VAR",
            RefKind::EscreveVar => "ESCREVE_VAR",
            RefKind::Inicializa => "INICIALIZA",
            RefKind::SqlUpdate => "SQL_UPDATE",
            RefKind::SqlInsert => "SQL_INSERT",
            RefKind::ChamaConstruida => "CHAMA_CONSTRUIDA",
            RefKind::Requisita => "REQUISITA",
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
    pub regions: Vec<RegionFact>,
    /// Funções decoradas: rota HTTP literal e nomes dos decoradores (entrypoints).
    pub entries: Vec<EntryFact>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntryFact {
    pub src_qname: String,
    /// `GET /caminho/{}` quando um decorador `x.get("/caminho/{id}")` declara a rota.
    pub route: Option<String>,
    /// Nome de cada decorador, sem `@` nem argumentos (`router.get`, `shared_task`).
    pub decorators: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegionFact {
    pub src_qname: String,
    pub start: usize,
    pub end: usize,
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
    /// Nomes que podem ser variável de módulo: atribuídos no nível do módulo ou importados.
    var_names: std::collections::HashSet<String>,
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
        var_names: std::collections::HashSet::new(),
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
        value_text: None,
    });
    collect_imports(&mut ctx, root, module, is_package, false);
    collect_var_names(&mut ctx, root);
    visit_block(&mut ctx, root, module, false);
    module_var_effects(&mut ctx, root, module);
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
            _ => body_variable(ctx, child, prefix),
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

const HTTP_METHODS: &[&str] = &["get", "post", "put", "patch", "delete", "head", "options"];

/// Caminho HTTP com parâmetros trocados por `{}` e sem query string; `None` se não começa em `/`.
pub fn normalize_path(raw: &str) -> Option<String> {
    let path = raw.split(['?', '#']).next().unwrap_or("");
    if !path.starts_with('/') {
        return None;
    }
    let mut out = String::new();
    let mut depth = 0;
    for c in path.chars() {
        match c {
            '{' => {
                if depth == 0 {
                    out.push_str("{}");
                }
                depth += 1;
            }
            '}' if depth > 0 => depth -= 1,
            _ if depth == 0 => out.push(c),
            _ => {}
        }
    }
    let trimmed = out.trim_end_matches('/');
    Some(if trimmed.is_empty() {
        "/".to_string()
    } else {
        trimmed.to_string()
    })
}

/// Texto de uma string literal (ou f-string), com cada interpolação como `{}`.
fn literal_text(ctx: &Ctx, node: Node) -> Option<String> {
    if node.kind() != "string" {
        return None;
    }
    let mut out = String::new();
    let mut cursor = node.walk();
    for part in node.named_children(&mut cursor) {
        match part.kind() {
            "string_content" | "escape_sequence" => out.push_str(ctx.text(part)),
            "interpolation" => out.push_str("{}"),
            _ => {}
        }
    }
    Some(out)
}

/// `@x.get("/rota")` vira `GET /rota`; os nomes de todos os decoradores vão junto.
fn entry_fact(ctx: &Ctx, outer: Node, qname: &str) -> Option<EntryFact> {
    let mut cursor = outer.walk();
    let mut route = None;
    let mut decorators = Vec::new();
    for deco in outer
        .named_children(&mut cursor)
        .filter(|n| n.kind() == "decorator")
    {
        let Some(expr) = deco.named_child(0) else {
            continue;
        };
        let (func, args) = if expr.kind() == "call" {
            (
                expr.child_by_field_name("function"),
                expr.child_by_field_name("arguments"),
            )
        } else {
            (Some(expr), None)
        };
        let Some(name) = func.and_then(|f| dotted_name(ctx, f)) else {
            continue;
        };
        let method = name.rsplit('.').next().unwrap_or("");
        if route.is_none()
            && name.contains('.')
            && HTTP_METHODS.contains(&method)
            && let Some(path) = args
                .and_then(|a| a.named_child(0))
                .and_then(|a| literal_text(ctx, a))
                .and_then(|t| normalize_path(&t))
        {
            route = Some(format!("{} {path}", method.to_uppercase()));
        }
        decorators.push(name);
    }
    (!decorators.is_empty()).then(|| EntryFact {
        src_qname: qname.to_string(),
        route,
        decorators,
    })
}

/// `cliente.get("/rota", ...)` com caminho literal: requisição ao endpoint da rota.
fn http_request(ctx: &mut Ctx, call: Node, src: &str) {
    let Some(func) = call
        .child_by_field_name("function")
        .filter(|f| f.kind() == "attribute")
    else {
        return;
    };
    let method = func
        .child_by_field_name("attribute")
        .map(|m| ctx.text(m).to_string())
        .unwrap_or_default();
    if !HTTP_METHODS.contains(&method.as_str()) {
        return;
    }
    let Some(path) = call
        .child_by_field_name("arguments")
        .and_then(|a| a.named_child(0))
        .and_then(|a| literal_text(ctx, a))
        .and_then(|t| normalize_path(&t))
    else {
        return;
    };
    ctx.facts.refs.push(RefFact {
        src_qname: src.to_string(),
        kind: RefKind::Requisita,
        text: path,
        receiver: Some(method.to_uppercase()),
        line: call.start_position().row + 1,
    });
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
    let mut is_enum = false;
    if let Some(bases) = def.child_by_field_name("superclasses") {
        let mut cursor = bases.walk();
        for base in bases.named_children(&mut cursor) {
            if base.kind() == "keyword_argument" {
                continue;
            }
            let text = ctx.text(base).split_whitespace().collect::<String>();
            let text = text.split('[').next().unwrap_or("").to_string();
            is_protocol |= text == "Protocol" || text.ends_with(".Protocol");
            is_enum |= is_enum_base(&text);
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
            value_text: None,
        },
    );
    if let Some(b) = body {
        if is_enum {
            // Antes do corpo: a primeira definição do `qname` vence e só o membro leva o valor.
            enum_members(ctx, b, &qname);
        }
        visit_block(ctx, b, &qname, true);
    }
}

fn collect_regions(ctx: &mut Ctx, node: Node, qname: &str) {
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if matches!(
            child.kind(),
            "function_definition" | "class_definition" | "decorated_definition" | "lambda"
        ) {
            continue;
        }
        if child.kind().ends_with("_statement") {
            ctx.facts.regions.push(RegionFact {
                src_qname: qname.to_string(),
                start: child.start_position().row + 1,
                end: child.end_position().row + 1,
            });
        }
        collect_regions(ctx, child, qname);
    }
}

/// Bases de Enum reconhecidas pelo nome declarado (TASK-7, D6): `Enum`, `IntEnum`, `StrEnum`,
/// `Flag`, sozinhas ou como `enum.<nome>`. Herança indireta não é seguida.
fn is_enum_base(text: &str) -> bool {
    const BASES: [&str; 4] = ["Enum", "IntEnum", "StrEnum", "Flag"];
    match text.split_once('.') {
        None => BASES.contains(&text),
        Some(("enum", name)) => BASES.contains(&name),
        Some(_) => false,
    }
}

/// Membros de um Enum: `NOME = valor` no corpo da classe, sem anotação. O valor literal (texto,
/// número, `True`, `False`, `None`) é guardado; os demais ficam vazios (`auto()`, expressões).
fn enum_members(ctx: &mut Ctx, body: Node, class_qname: &str) {
    const LITERALS: [&str; 6] = ["string", "integer", "float", "true", "false", "none"];
    let mut cursor = body.walk();
    let statements: Vec<Node> = body.named_children(&mut cursor).collect();
    for stmt in statements {
        let Some(assign) = (stmt.kind() == "expression_statement")
            .then(|| stmt.named_child(0))
            .flatten()
            .filter(|n| n.kind() == "assignment" && n.child_by_field_name("type").is_none())
        else {
            continue;
        };
        let Some(name) = assign
            .child_by_field_name("left")
            .filter(|n| n.kind() == "identifier")
            .map(|n| ctx.text(n).to_string())
            .filter(|n| !n.starts_with('_'))
        else {
            continue;
        };
        let value = assign
            .child_by_field_name("right")
            .filter(|v| LITERALS.contains(&v.kind()))
            .map(|v| ctx.text(v).to_string())
            .unwrap_or_default();
        let qname = format!("{class_qname}.{name}");
        if ctx.facts.symbols.iter().any(|s| s.qname == qname) {
            continue;
        }
        add_symbol(
            ctx,
            SymbolFact {
                qname,
                kind: SymbolKind::Attribute,
                line_start: assign.start_position().row + 1,
                line_end: assign.end_position().row + 1,
                sig_hash: hash_of(&value),
                body_hash: hash_of(""),
                is_protocol: false,
                type_text: None,
                value_text: Some(value),
            },
        );
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
            value_text: None,
        },
    );
    if let Some(entry) = entry_fact(ctx, outer, &qname) {
        ctx.facts.entries.push(entry);
    }
    if let Some(b) = body {
        if in_class {
            self_attributes(ctx, b, prefix, &param_types);
        }
        collect_regions(ctx, b, &qname);
        let mut locals = param_types.clone();
        collect_locals(ctx, b, &mut locals);
        scan_effects(ctx, b, &qname, in_class, &locals);
        function_var_effects(ctx, def, b, &qname);
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
            value_text: None,
        },
    );
}

/// Variável atribuída no corpo da classe ou no nível do módulo (`x = 0`, `x: int = 0`); locais
/// de função não entram (TASK-10, D2).
fn body_variable(ctx: &mut Ctx, stmt: Node, owner_qname: &str) {
    if stmt.kind() != "expression_statement" {
        return;
    }
    let Some(assign) = stmt.named_child(0).filter(|n| n.kind() == "assignment") else {
        return;
    };
    if let Some(left) = assign
        .child_by_field_name("left")
        .filter(|n| n.kind() == "identifier")
    {
        let name = ctx.text(left).to_string();
        let type_text = assign
            .child_by_field_name("type")
            .map(|t| unwrap_type(ctx.text(t)));
        push_attribute(ctx, owner_qname, &name, assign, type_text);
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
                    if target.kind() == "attribute"
                        && let Some(text) = dotted_name(ctx, target)
                    {
                        let owner = text.rsplit_once('.').map(|(o, _)| o).unwrap_or("");
                        ctx.facts.refs.push(RefFact {
                            src_qname: src.to_string(),
                            kind: RefKind::EscreveAttr,
                            receiver: locals.get(owner).cloned(),
                            text,
                            line: target.start_position().row + 1,
                        });
                    }
                    // `X.attr = v` escreve: só o objeto `X` é lido, não o atributo.
                    let read = match base {
                        Some(b) if b.kind() == "attribute" && target.kind() == "attribute" => {
                            b.child_by_field_name("object")
                        }
                        other => other,
                    };
                    if let Some(b) = read {
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
            let constructed = func.and_then(|f| constructed_receiver(ctx, f));
            if let Some(callee) = func.and_then(|f| callee_text(ctx, f)) {
                let first = callee.split('.').next().unwrap_or("");
                let dynamic_name = !callee.contains('.') && shadowed(ctx, src, &callee, locals);
                let receiver = if dynamic_name {
                    Some(String::new())
                } else {
                    constructed
                        .as_ref()
                        .map(|name| {
                            if shadowed(ctx, src, name, locals) {
                                String::new()
                            } else {
                                name.clone()
                            }
                        })
                        .or_else(|| {
                            (callee.contains('.') && !matches!(first, "self" | "cls"))
                                .then(|| locals.get(first).cloned())
                                .flatten()
                        })
                };
                ctx.facts.refs.push(RefFact {
                    src_qname: src.to_string(),
                    kind: if constructed.is_some() || dynamic_name {
                        RefKind::ChamaConstruida
                    } else {
                        RefKind::Chama
                    },
                    text: callee,
                    receiver,
                    line: node.start_position().row + 1,
                });
            }
            keyword_writes(ctx, node, src, locals);
            http_request(ctx, node, src);
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
            if let Some(owner) = node
                .child_by_field_name("object")
                .filter(|o| o.kind() == "identifier")
                .map(|o| ctx.text(o).to_string())
                .filter(|o| !matches!(o.as_str(), "self" | "cls"))
            {
                let attr = node
                    .child_by_field_name("attribute")
                    .map(|a| ctx.text(a).to_string())
                    .unwrap_or_default();
                ctx.facts.refs.push(RefFact {
                    src_qname: src.to_string(),
                    kind: RefKind::LeAttr,
                    text: format!("{owner}.{attr}"),
                    receiver: locals.get(&owner).cloned(),
                    line: node.start_position().row + 1,
                });
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
    if let Some(receiver) = constructed_receiver(ctx, func) {
        return Some(format!(
            "{receiver}.{}",
            ctx.text(func.child_by_field_name("attribute")?)
        ));
    }
    dotted_name(ctx, func).or_else(|| {
        (func.kind() == "attribute")
            .then(|| func.child_by_field_name("attribute"))
            .flatten()
            .map(|m| format!("?.{}", ctx.text(m)))
    })
}

/// Preserve the constructor expression without claiming its result type: resolution checks it.
fn constructed_receiver(ctx: &Ctx, func: Node) -> Option<String> {
    if func.kind() != "attribute" {
        return None;
    }
    let object = func.child_by_field_name("object")?;
    (object.kind() == "call")
        .then(|| dotted_name(ctx, object.child_by_field_name("function")?))
        .flatten()
}

/// A parameter or a known local binding prevents treating a spelling as an imported identity.
fn shadowed(ctx: &Ctx, src: &str, name: &str, locals: &HashMap<String, String>) -> bool {
    let root = name.split('.').next().unwrap_or(name);
    locals.contains_key(root)
        || ctx
            .facts
            .receives
            .iter()
            .any(|r| r.src_qname == src && r.attr == root)
}

/// Only SQLAlchemy imports establish query-builder semantics; a method named values does not.
fn sql_model(
    ctx: &Ctx,
    mut call: Node,
    src: &str,
    locals: &HashMap<String, String>,
) -> Option<(String, RefKind)> {
    loop {
        if call.kind() != "call" {
            return None;
        }
        let function = call.child_by_field_name("function")?;
        if let Some(name) = dotted_name(ctx, function) {
            if shadowed(ctx, src, &name, locals) {
                return None;
            }
            for import in &ctx.facts.imports {
                let expanded = if name == import.local_name {
                    import.target.clone()
                } else if let Some(rest) = name.strip_prefix(&format!("{}.", import.local_name)) {
                    format!("{}.{rest}", import.target)
                } else {
                    continue;
                };
                let kind = match expanded.as_str() {
                    "sqlalchemy.update"
                    | "sqlalchemy.sql.update"
                    | "sqlalchemy.sql.expression.update" => RefKind::SqlUpdate,
                    "sqlalchemy.insert"
                    | "sqlalchemy.sql.insert"
                    | "sqlalchemy.sql.expression.insert" => RefKind::SqlInsert,
                    _ => continue,
                };
                let model = call.child_by_field_name("arguments")?.named_child(0)?;
                return Some((dotted_name(ctx, model)?, kind));
            }
        }
        call = function.child_by_field_name("object")?;
    }
}

fn keyword_writes(ctx: &mut Ctx, call: Node, src: &str, locals: &HashMap<String, String>) {
    let Some(function) = call.child_by_field_name("function") else {
        return;
    };
    let is_values = function
        .child_by_field_name("attribute")
        .is_some_and(|a| ctx.text(a) == "values");
    let target = if is_values {
        function
            .child_by_field_name("object")
            .and_then(|o| sql_model(ctx, o, src, locals))
    } else {
        dotted_name(ctx, function).map(|name| (name, RefKind::Inicializa))
    };
    let Some((model, kind)) = target else { return };
    if shadowed(ctx, src, &model, locals) {
        return;
    }
    let Some(args) = call.child_by_field_name("arguments") else {
        return;
    };
    let mut cursor = args.walk();
    for arg in args.named_children(&mut cursor) {
        if arg.kind() == "keyword_argument"
            && let Some(name) = arg.child_by_field_name("name")
        {
            push_ref(ctx, src, kind, format!("{model}.{}", ctx.text(name)), name);
        }
    }
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

fn assigned_name(ctx: &Ctx, stmt: Node) -> Option<String> {
    if !matches!(stmt.kind(), "assignment" | "augmented_assignment") {
        return None;
    }
    let left = stmt.child_by_field_name("left")?;
    (left.kind() == "identifier").then(|| ctx.text(left).to_string())
}

/// Nomes candidatos a variável de módulo: atribuídos em instruções do módulo (fora de
/// funções e classes) ou importados (TASK-10, D2).
fn collect_var_names(ctx: &mut Ctx, root: Node) {
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

/// Nomes que a função ou o método vê como próprios: parâmetros (também de funções aninhadas)
/// e nomes atribuídos sem `global`. Esses ocultam a variável de módulo (C6).
fn local_scope(
    ctx: &Ctx,
    def: Node,
    body: Node,
) -> (
    std::collections::HashSet<String>,
    std::collections::HashSet<String>,
) {
    use std::collections::HashSet;
    fn walk(ctx: &Ctx, node: Node, locals: &mut HashSet<String>, globals: &mut HashSet<String>) {
        let mut cursor = node.walk();
        for child in node.named_children(&mut cursor) {
            match child.kind() {
                "global_statement" | "nonlocal_statement" => {
                    let mut c = child.walk();
                    for id in child.named_children(&mut c) {
                        globals.insert(ctx.text(id).to_string());
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
                            collect_identifiers(ctx, t, locals);
                        }
                    }
                }
                "parameters" | "lambda_parameters" => collect_identifiers(ctx, child, locals),
                _ => {}
            }
            walk(ctx, child, locals, globals);
        }
    }
    fn collect_identifiers(ctx: &Ctx, node: Node, out: &mut HashSet<String>) {
        if node.kind() == "identifier" {
            out.insert(ctx.text(node).to_string());
            return;
        }
        let mut cursor = node.walk();
        for child in node.named_children(&mut cursor) {
            if child.kind() != "attribute" && child.kind() != "subscript" {
                collect_identifiers(ctx, child, out);
            }
        }
    }
    let mut locals = HashSet::new();
    let mut globals = HashSet::new();
    if let Some(params) = def.child_by_field_name("parameters") {
        collect_identifiers(ctx, params, &mut locals);
    }
    walk(ctx, body, &mut locals, &mut globals);
    for g in &globals {
        locals.remove(g);
    }
    (locals, globals)
}

fn function_var_effects(ctx: &mut Ctx, def: Node, body: Node, src: &str) {
    let (locals, globals) = local_scope(ctx, def, body);
    var_effects(ctx, body, src, &locals, &globals, true);
}

/// Instruções do módulo (fora de funções e classes): escritas e leituras de nomes simples.
fn module_var_effects(ctx: &mut Ctx, root: Node, module: &str) {
    let none = std::collections::HashSet::new();
    var_effects(ctx, root, module, &none, &none, false);
}

/// `ESCREVE_VAR`/`LE_VAR` de nomes candidatos a variável de módulo. `in_function` decide se a
/// escrita exige `global` (função) ou vale sempre (módulo).
fn var_effects(
    ctx: &mut Ctx,
    node: Node,
    src: &str,
    locals: &std::collections::HashSet<String>,
    globals: &std::collections::HashSet<String>,
    in_function: bool,
) {
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        match child.kind() {
            "class_definition" => continue,
            "function_definition" | "decorated_definition" | "lambda" if !in_function => continue,
            "global_statement" | "nonlocal_statement" => continue,
            "identifier" => {
                let parent = node.kind();
                let name = ctx.text(child).to_string();
                let is_member =
                    parent == "attribute" && node.child_by_field_name("attribute") == Some(child);
                let is_label =
                    parent == "keyword_argument" && node.child_by_field_name("name") == Some(child);
                if !is_member
                    && !is_label
                    && ctx.var_names.contains(&name)
                    && !locals.contains(&name)
                {
                    push_ref(ctx, src, RefKind::LeVar, name, child);
                }
                continue;
            }
            "assignment" | "augmented_assignment" => {
                let left = child.child_by_field_name("left");
                if let Some(l) = left.filter(|l| l.kind() == "identifier") {
                    let name = ctx.text(l).to_string();
                    let writes = ctx.var_names.contains(&name)
                        && if in_function {
                            globals.contains(&name)
                        } else {
                            true
                        };
                    if writes {
                        push_ref(ctx, src, RefKind::EscreveVar, name, l);
                    }
                } else if let Some(l) = left {
                    var_effects(ctx, l, src, locals, globals, in_function);
                }
                if let Some(right) = child.child_by_field_name("right") {
                    if right.kind() == "identifier" {
                        let name = ctx.text(right).to_string();
                        if ctx.var_names.contains(&name) && !locals.contains(&name) {
                            push_ref(ctx, src, RefKind::LeVar, name, right);
                        }
                    } else {
                        var_effects(ctx, right, src, locals, globals, in_function);
                    }
                }
                continue;
            }
            _ => {}
        }
        var_effects(ctx, child, src, locals, globals, in_function);
    }
}
