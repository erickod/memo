//! Formatação de saída em texto compacto (spec §8.3).

use memo_core::query::{
    Coverage, EdgeView, RelItem, Relation, SymbolAddr, SymbolInfo, TypeInfo, WordInfo,
};

fn coverage_line(c: &Coverage) -> String {
    format!(
        "cobertura: {} resolvidas, {} aprox, {} não resolvidas, {} arquivos com erro",
        c.resolvidas, c.aproximadas, c.nao_resolvidas, c.arquivos_com_erro
    )
}

fn abbrev(addr: &SymbolAddr) -> &'static str {
    if addr.is_test {
        return "T";
    }
    match addr.kind.as_str() {
        "class" => "C",
        "function" => "F",
        "method" => "M",
        "attribute" => "A",
        _ => "mod",
    }
}

fn item(addr: &SymbolAddr) -> String {
    format!(
        "  {} {}   {}:L{}-L{}",
        abbrev(addr),
        addr.short,
        addr.path,
        addr.line_start,
        addr.line_end
    )
}

fn list_line(label: &str, values: &[String]) -> String {
    if values.is_empty() {
        format!("{label}: nenhum")
    } else {
        format!("{label}: {}", values.join(","))
    }
}

/// Monta `cabeçalho`, `itens` e `rodapé`; corta os itens do fim até caber em `max_chars`
/// e termina com `+N omitidos`. O limite `limit` (se houver) também corta itens.
pub fn fit(
    head: &[String],
    items: &[String],
    tail: &[String],
    max_chars: usize,
    limit: Option<usize>,
) -> String {
    let render = |kept: usize, omitted: usize| {
        let mut lines: Vec<&str> = head.iter().map(String::as_str).collect();
        lines.extend(items[..kept].iter().map(String::as_str));
        lines.extend(tail.iter().map(String::as_str));
        let extra = format!("+{omitted} omitidos");
        if omitted > 0 {
            lines.push(&extra);
        }
        let mut text = lines.join("\n");
        text.push('\n');
        text
    };
    let mut kept = limit.map_or(items.len(), |l| l.min(items.len()));
    loop {
        let text = render(kept, items.len() - kept);
        if text.chars().count() <= max_chars || kept == 0 {
            return text;
        }
        kept -= 1;
    }
}

fn edge_line(e: &EdgeView) -> String {
    let arrow = if e.dir == "out" { "->" } else { "<-" };
    let attr = e.attr.as_ref().map(|a| format!(" {a}")).unwrap_or_default();
    let approx = if e.approx { " ~" } else { "" };
    let line = e.line.map(|l| format!(" :L{l}")).unwrap_or_default();
    let other = if e.other.is_empty() { "?" } else { &e.other };
    format!("  {} {arrow} {other}{attr}{approx}{line}", e.kind)
}

pub fn symbol_text(
    info: &SymbolInfo,
    idx: &str,
    cov: &Coverage,
    max_chars: usize,
    limit: Option<usize>,
) -> String {
    let mut head = vec![
        format!("symbol {}  idx={idx}", info.addr.qname),
        item(&info.addr),
        format!(
            "assinatura: sig={}",
            &info.sig_hash[..info.sig_hash.len().min(6)]
        ),
        "arestas:".to_string(),
    ];
    if info.edges.is_empty() {
        head.push("  nenhuma".to_string());
    }
    let items: Vec<String> = info.edges.iter().map(edge_line).collect();
    let tail = ["notas: nenhuma".to_string(), coverage_line(cov)];
    fit(&head, &items, &tail, max_chars, limit)
}

fn rel_item(i: &RelItem) -> String {
    let attr = i.attr.as_ref().map(|a| format!(" {a}")).unwrap_or_default();
    let approx = if i.approx { " ~" } else { "" };
    let line = i.line.map(|l| format!(" :L{l}")).unwrap_or_default();
    format!("{}{attr}{approx}{line}", item(&i.addr))
}

pub fn relation_text(
    label: &str,
    rel: &Relation,
    idx: &str,
    cov: &Coverage,
    max_chars: usize,
    limit: Option<usize>,
) -> String {
    let mut head = vec![format!("{label} {}  idx={idx}", rel.target.short)];
    if rel.items.is_empty() {
        head.push("  nenhum".to_string());
    }
    let items: Vec<String> = rel.items.iter().map(rel_item).collect();
    fit(&head, &items, &[coverage_line(cov)], max_chars, limit)
}

pub fn type_text(
    info: &TypeInfo,
    idx: &str,
    cov: &Coverage,
    max_chars: usize,
    limit: Option<usize>,
) -> String {
    let head = vec![format!("type {}  idx={idx}", info.target.short)];
    let mut items = vec!["produz:".to_string()];
    if info.producers.is_empty() {
        items.push("  nenhum".to_string());
    }
    items.extend(info.producers.iter().map(rel_item));
    items.push("consome:".to_string());
    if info.consumers.is_empty() {
        items.push("  nenhum".to_string());
    }
    items.extend(info.consumers.iter().map(rel_item));
    fit(&head, &items, &[coverage_line(cov)], max_chars, limit)
}

pub fn word_text(
    info: &WordInfo,
    idx: &str,
    cov: &Coverage,
    max_chars: usize,
    limit: Option<usize>,
) -> String {
    let mut head = vec![
        format!("word {}  idx={idx}", info.word),
        list_line("verbos", &info.verbs),
        list_line("papéis", &info.roles),
        list_line("camadas", &info.layers),
        list_line("vizinhas", &info.neighbors),
        "símbolos:".to_string(),
    ];
    if info.symbols.is_empty() {
        head.push("  nenhum".to_string());
    }
    let items: Vec<String> = info.symbols.iter().map(item).collect();
    fit(&head, &items, &[coverage_line(cov)], max_chars, limit)
}

pub fn ambiguous_text(target: &str, candidates: &[String]) -> String {
    let mut out = format!("ambíguo: {target}\ncandidatas:\n");
    for c in candidates {
        out.push_str(&format!("  {c}\n"));
    }
    out
}
