//! Texto de cada escopo: o que as palavras do corpo ligam ao símbolo que o contém (TASK-10).

use super::{Ctx, ScopeText, docstring_text};
use tree_sitter::Node;

/// Partes do escopo de uma classe ou função: tudo o que a definição contém, menos o nome (que já
/// gera as palavras do próprio símbolo), mais os decoradores da definição externa.
pub(super) fn scope_parts<'t>(def: Node<'t>, outer: Node<'t>) -> Vec<Node<'t>> {
    let name = def.child_by_field_name("name");
    let mut cursor = def.walk();
    let mut parts: Vec<Node> = def
        .named_children(&mut cursor)
        .filter(|c| Some(*c) != name)
        .collect();
    let mut cursor = outer.walk();
    parts.extend(
        outer
            .named_children(&mut cursor)
            .filter(|c| c.kind() == "decorator"),
    );
    parts
}

/// Nomes e comentários das partes dadas, sem entrar em imports (já têm fonte própria), e a
/// docstring do corpo. Literais de string ficam fora. `nested_are_symbols`: no módulo e no corpo
/// de classe as definições aninhadas são símbolos com escopo próprio e ficam de fora; numa função
/// elas não são símbolos, então as palavras delas pertencem à função.
pub(super) fn record_scope(
    ctx: &mut Ctx,
    qname: &str,
    parts: &[Node],
    body: Node,
    nested_are_symbols: bool,
) {
    fn walk(ctx: &Ctx, node: Node, out: &mut Vec<String>, nested_are_symbols: bool) {
        match node.kind() {
            "function_definition" | "class_definition" | "decorated_definition"
                if nested_are_symbols =>
            {
                return;
            }
            "import_statement" | "import_from_statement" | "future_import_statement" => return,
            "identifier" => out.push(ctx.text(node).to_string()),
            "comment" => out.push(ctx.text(node).trim_start_matches('#').to_string()),
            _ => {}
        }
        let mut cursor = node.walk();
        for child in node.named_children(&mut cursor) {
            walk(ctx, child, out, nested_are_symbols);
        }
    }
    let mut out = Vec::new();
    for part in parts {
        walk(ctx, *part, &mut out, nested_are_symbols);
    }
    if let Some(doc) = docstring_text(ctx, body) {
        out.push(doc);
    }
    if out.is_empty() {
        return;
    }
    let text = out.join(" ");
    // Uma redefinição (`@x.setter`) soma ao mesmo `qname`, que tem um só símbolo.
    match ctx.scope_index.get(qname) {
        Some(&i) => {
            let existing = &mut ctx.facts.scopes[i].text;
            existing.push(' ');
            existing.push_str(&text);
        }
        None => {
            ctx.scope_index
                .insert(qname.to_string(), ctx.facts.scopes.len());
            ctx.facts.scopes.push(ScopeText {
                src_qname: qname.to_string(),
                text,
            });
        }
    }
}
