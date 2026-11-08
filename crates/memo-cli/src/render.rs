//! Formatação de saída em texto compacto (spec §8.3).

use memo_core::brief::Brief;
use memo_core::notes::Note;
use memo_core::query::{
    Chain, ChainItem, Coverage, EdgeView, Impact, RelItem, Relation, SymbolAddr, SymbolInfo,
    TestsInfo, TypeInfo, WordInfo,
};

fn coverage_line(c: &Coverage) -> String {
    let notes = if c.notas_com_erro > 0 {
        format!(", {} notas com erro", c.notas_com_erro)
    } else {
        String::new()
    };
    let hidden = if c.testes_ocultos > 0 {
        format!(", {} testes ocultos", c.testes_ocultos)
    } else {
        String::new()
    };
    format!(
        "cobertura: {} resolvidas, {} aprox, {} não resolvidas, {} arquivos com erro{notes}{hidden}",
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

/// Vizinhas por tipo de ponte, as 8 mais pesadas de cada tipo.
fn bridge_lines(bridges: &[memo_core::bridges::Bridge]) -> Vec<String> {
    use memo_core::bridges::BridgeKind::{Call, Context, File, Import, Symbol, Test};
    let mut lines = vec!["vizinhas por ponte:".to_string()];
    for kind in [Symbol, Test, Import, Call, File, Context] {
        let top: Vec<String> = bridges
            .iter()
            .filter(|b| b.kind == kind)
            .take(8)
            .map(|b| format!("{}({:.1})", b.other, b.weight))
            .collect();
        if !top.is_empty() {
            lines.push(format!("  {:<8}: {}", kind.as_str(), top.join(" ")));
        }
    }
    if lines.len() == 1 {
        lines.push("  nenhuma".to_string());
    }
    lines
}

fn tagged_item(addr: &SymbolAddr, tags: &[String]) -> String {
    let tags = if tags.is_empty() {
        "vizinho".to_string()
    } else {
        tags.join(", ")
    };
    format!("{} [{tags}]", item(addr))
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

/// Monta o `brief` com a seção de testes ao final. Sob `--max-chars` os testes são cortados
/// primeiro; o `+N omitidos` final soma o que foi cortado em cada seção.
fn fit_brief(
    head: &[String],
    main: &[String],
    tests: &[String],
    tail: &[String],
    max_chars: usize,
    limit: Option<usize>,
) -> String {
    let render = |kept_main: usize, kept_tests: usize| {
        let mut lines: Vec<String> = head.to_vec();
        lines.extend(main[..kept_main].iter().cloned());
        if !tests.is_empty() {
            lines.push("testes:".to_string());
            lines.extend(tests[..kept_tests].iter().cloned());
            if kept_tests < tests.len() {
                lines.push(format!("  +{} testes omitidos", tests.len() - kept_tests));
            }
        }
        lines.extend(tail.iter().cloned());
        let omitted = (main.len() - kept_main) + (tests.len() - kept_tests);
        if omitted > 0 {
            lines.push(format!("+{omitted} omitidos"));
        }
        let mut text = lines.join("\n");
        text.push('\n');
        text
    };
    let mut kept_main = limit.map_or(main.len(), |l| l.min(main.len()));
    let mut kept_tests = tests.len();
    loop {
        let text = render(kept_main, kept_tests);
        if text.chars().count() <= max_chars || (kept_main == 0 && kept_tests == 0) {
            return text;
        }
        if kept_tests > 0 {
            kept_tests -= 1;
        } else {
            kept_main -= 1;
        }
    }
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
    let mut tail = notes_lines(&info.notes);
    tail.push(coverage_line(cov));
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

fn chain_item(i: &ChainItem) -> String {
    let approx = if i.approx { " ~" } else { "" };
    let line = i.line.map(|l| format!(" :L{l}")).unwrap_or_default();
    match (&i.addr, &i.ext) {
        (Some(addr), _) => format!("{}  d{}{approx}{line}", item(addr), i.depth),
        (None, ext) => format!(
            "  ext {}  d{}{line}",
            ext.as_deref().unwrap_or("?"),
            i.depth
        ),
    }
}

fn section(label: &str, lines: Vec<String>, items: &mut Vec<String>) {
    items.push(format!("{label}:"));
    if lines.is_empty() {
        items.push("  nenhum".to_string());
    }
    items.extend(lines);
}

pub fn chain_text(
    label: &str,
    chain: &Chain,
    idx: &str,
    cov: &Coverage,
    max_chars: usize,
    limit: Option<usize>,
) -> String {
    let mut head = vec![format!("{label} {}  idx={idx}", chain.target.short)];
    if chain.items.is_empty() {
        head.push("  nenhum".to_string());
    }
    let items: Vec<String> = chain.items.iter().map(chain_item).collect();
    fit(&head, &items, &[coverage_line(cov)], max_chars, limit)
}

fn tests_sections(tests: &TestsInfo, items: &mut Vec<String>) {
    section(
        "testes exatos",
        tests.exact.iter().map(rel_item).collect(),
        items,
    );
    section(
        "testes ~",
        tests.approx.iter().map(rel_item).collect(),
        items,
    );
}

pub fn tests_text(
    target: &SymbolAddr,
    tests: &TestsInfo,
    idx: &str,
    cov: &Coverage,
    max_chars: usize,
    limit: Option<usize>,
) -> String {
    let head = vec![format!("tests {}  idx={idx}", target.short)];
    let mut items = Vec::new();
    tests_sections(tests, &mut items);
    fit(&head, &items, &[coverage_line(cov)], max_chars, limit)
}

pub fn impact_text(
    info: &Impact,
    idx: &str,
    cov: &Coverage,
    max_chars: usize,
    limit: Option<usize>,
) -> String {
    let head = vec![format!("impact {}  idx={idx}", info.target.short)];
    let mut items = Vec::new();
    section(
        "chamadores",
        info.callers.iter().map(chain_item).collect(),
        &mut items,
    );
    section(
        "consumidores",
        info.consumers.iter().map(rel_item).collect(),
        &mut items,
    );
    section(
        "protocolo",
        info.protocol.iter().map(rel_item).collect(),
        &mut items,
    );
    tests_sections(&info.tests, &mut items);
    let mut tail = notes_lines(&info.notes);
    tail.extend(unchanged_line(info.unchanged));
    tail.push(coverage_line(cov));
    fit(&head, &items, &tail, max_chars, limit)
}

/// Seção `notas`: `nenhuma`, ou uma linha por nota com o peso implícito na ordem.
fn notes_lines(notes: &[Note]) -> Vec<String> {
    if notes.is_empty() {
        return vec!["notas: nenhuma".to_string()];
    }
    let mut lines = vec!["notas:".to_string()];
    lines.extend(notes.iter().map(|n| format!("  {}", n.summary())));
    lines
}

pub fn index_text(report: Option<&memo_core::index::IndexReport>, cov: &Coverage) -> String {
    let Some(r) = report else {
        return format!("index idx=skipped\n{}\n", coverage_line(cov));
    };
    format!(
        "index idx=ok arquivos={} alterados={} removidos={}\n{}\n{}\n",
        r.files_total,
        r.files_changed,
        r.files_removed,
        r.events.line(),
        coverage_line(cov)
    )
}

pub fn promote_text(task: &str, main: &str, report: &memo_core::notes::PromoteReport) -> String {
    let mut out = format!(
        "promote {task}  base={main}  promovidas={} órfãs={}\n",
        report.promoted.len(),
        report.orphaned.len()
    );
    if !report.orphaned.is_empty() {
        out.push_str("órfãs:\n");
        for note in &report.orphaned {
            out.push_str(&format!("  {}\n", note.summary()));
        }
    }
    out
}

pub fn status_text(s: &memo_core::status::Status, cov: Option<&Coverage>) -> String {
    let graph = |g: &memo_core::status::GraphInfo| {
        let commit = g
            .indexed_commit
            .as_deref()
            .map_or("-", |c| &c[..c.len().min(7)]);
        format!(
            "arquivos={} símbolos={} ref={} commit={commit}",
            g.files,
            g.symbols,
            g.indexed_ref.as_deref().unwrap_or("-")
        )
    };
    let mut out = String::from("status\n");
    match &s.base {
        Some(base) => out.push_str(&format!("base: {}\n", graph(base))),
        None => out.push_str("base: ausente\n"),
    }
    out.push_str("features:\n");
    if s.features.is_empty() {
        out.push_str("  nenhuma\n");
    }
    for (task, g) in &s.features {
        out.push_str(&format!("  {task} {}\n", graph(g)));
    }
    let notes: Vec<String> = s.notes.iter().map(|(k, v)| format!("{k}={v}")).collect();
    out.push_str(&format!("notas: {}\n", notes.join(" ")));
    let promotable = if s.promotable.is_empty() {
        "-".to_string()
    } else {
        s.promotable.join(",")
    };
    out.push_str(&format!("promovíveis: {promotable}\n"));
    if let Some(c) = cov {
        out.push_str(&format!("{}\n", coverage_line(c)));
    }
    out
}

pub fn note_added_text(note: &Note, conflicts: &[Note]) -> String {
    let mut out = format!("note {} {}  {}\n", note.id, note.estado, note.ancora);
    if !conflicts.is_empty() {
        out.push_str("conflitos:\n");
        for c in conflicts {
            out.push_str(&format!("  {}\n", c.summary()));
        }
    }
    out
}

pub fn note_list_text(notes: &[Note]) -> String {
    if notes.is_empty() {
        return "nenhuma nota\n".to_string();
    }
    notes
        .iter()
        .map(|n| {
            format!(
                "{} {} {} {}  {}\n",
                n.id, n.tipo, n.estado, n.escopo, n.ancora
            )
        })
        .collect()
}

fn dash_join(values: &[String]) -> String {
    if values.is_empty() {
        "-".to_string()
    } else {
        values.join(",")
    }
}

fn unchanged_line(unchanged: Option<usize>) -> Option<String> {
    unchanged
        .filter(|n| *n > 0)
        .map(|n| format!("inalterados: {n}"))
}

fn anchors_line(b: &Brief) -> String {
    let anchors: Vec<String> = b
        .anchors
        .iter()
        .map(|a| {
            format!(
                "{}({:.2}){}",
                a.word,
                a.score,
                if a.approx { "~" } else { "" }
            )
        })
        .collect();
    let mut line = if anchors.is_empty() {
        "âncoras: nenhuma".to_string()
    } else {
        format!("âncoras: {}", anchors.join(" "))
    };
    if !b.ignored.is_empty() {
        line.push_str(&format!("   ignoradas: {}", b.ignored.join(" ")));
    }
    line
}

pub fn brief_text(
    b: &Brief,
    idx: &str,
    task: Option<&str>,
    cov: &Coverage,
    max_chars: usize,
    limit: Option<usize>,
) -> String {
    let task = task.map(|t| format!(" task={t}")).unwrap_or_default();
    let head = vec![
        format!("brief \"{}\"{task}  idx={idx}", b.request),
        anchors_line(b),
    ];
    let mut items = Vec::new();
    let symbols = b
        .symbols
        .iter()
        .filter(|s| !s.in_tests)
        .map(|s| tagged_item(&s.addr, &s.tags))
        .collect();
    section("símbolos", symbols, &mut items);
    let outline = b
        .outline
        .iter()
        .map(|o| format!("  {} {}", o.symbol, o.links.join(" ; ")))
        .collect();
    section("contorno", outline, &mut items);
    if let Some(p) = &b.pattern {
        items.push(format!(
            "padrão({}): verbos {} · papéis {} · camadas {}",
            p.word,
            dash_join(&p.verbs),
            dash_join(&p.roles),
            dash_join(&p.layers)
        ));
    }
    let writers = b
        .writers
        .iter()
        .map(|w| format!("  {} <- {}", w.attr, w.writers.join(", ")))
        .collect();
    section("escritores", writers, &mut items);
    let mut tail = if b.notes.is_empty() {
        vec!["notas: nenhuma".to_string()]
    } else {
        let mut lines = vec!["notas:".to_string()];
        lines.extend(b.notes.iter().map(|n| format!("  {n}")));
        lines
    };
    tail.extend(unchanged_line(b.unchanged));
    tail.push(coverage_line(cov));
    let tests: Vec<String> = b
        .symbols
        .iter()
        .filter(|s| s.in_tests)
        .map(|s| tagged_item(&s.addr, &s.tags))
        .collect();
    fit_brief(&head, &items, &tests, &tail, max_chars, limit)
}

pub fn word_text(
    info: &WordInfo,
    idx: &str,
    cov: &Coverage,
    max_chars: usize,
    limit: Option<usize>,
) -> String {
    let mut head = vec![format!("word {}  idx={idx}", info.word)];
    if let Some(st) = &info.stats {
        head.push(format!(
            "stats: uso={} alcance={} grau={} espec={:.2} central={:.3} coer={:.2}",
            st.uso, st.alcance, st.grau, st.especificidade, st.centralidade, st.coerencia
        ));
        if st.esqueleto {
            head.push("esqueleto: sim (nunca âncora)".to_string());
        }
    }
    head.extend([
        list_line("verbos", &info.verbs),
        list_line("papéis", &info.roles),
        list_line("camadas", &info.layers),
    ]);
    head.extend(bridge_lines(&info.bridges));
    if !info.ignored.is_empty() {
        head.push(list_line("ignoradas", &info.ignored));
    }
    head.push("símbolos:".to_string());
    if info.symbols.is_empty() {
        head.push("  nenhum".to_string());
    }
    let items: Vec<String> = info
        .symbols
        .iter()
        .map(|s| tagged_item(&s.addr, &s.tags))
        .collect();
    fit(&head, &items, &[coverage_line(cov)], max_chars, limit)
}

pub fn ambiguous_text(target: &str, candidates: &[String]) -> String {
    let mut out = format!("ambíguo: {target}\ncandidatas:\n");
    for c in candidates {
        out.push_str(&format!("  {c}\n"));
    }
    out
}
