//! Formatação de saída em texto compacto (spec §8.3).

use memo_core::query::{SymbolAddr, SymbolInfo, WordInfo};

pub struct Coverage {
    pub files_with_error: usize,
}

impl Coverage {
    pub fn line(&self) -> String {
        format!(
            "cobertura: 0 resolvidas, 0 aprox, 0 não resolvidas, {} arquivos com erro",
            self.files_with_error
        )
    }
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

pub fn symbol_text(info: &SymbolInfo, idx: &str, cov: &Coverage, max_chars: usize) -> String {
    let head = vec![
        format!("symbol {}  idx={idx}", info.addr.qname),
        item(&info.addr),
        format!(
            "assinatura: sig={}",
            &info.sig_hash[..info.sig_hash.len().min(6)]
        ),
        "arestas: nenhuma".to_string(),
        "notas: nenhuma".to_string(),
    ];
    fit(&head, &[], &[cov.line()], max_chars, None)
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
    fit(&head, &items, &[cov.line()], max_chars, limit)
}

pub fn ambiguous_text(target: &str, candidates: &[String]) -> String {
    let mut out = format!("ambíguo: {target}\ncandidatas:\n");
    for c in candidates {
        out.push_str(&format!("  {c}\n"));
    }
    out
}
