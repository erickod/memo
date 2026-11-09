//! Formatação de saída em texto compacto (spec §8.3).

use memo_core::brief::Brief;
use memo_core::evidence::Evidence;
use memo_core::notes::Note;
use memo_core::query::{
    ByWordItem, Chain, ChainItem, Coverage, EdgeView, Impact, Origem, RelItem, Relation,
    SymbolAddr, SymbolInfo, TestsInfo, TypeInfo, WordInfo,
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

/// Representatividade em arquivos: repositório, cada contexto e as vizinhas mais fortes.
fn representativeness_lines(
    word: &str,
    r: &memo_core::representativeness::Representativeness,
) -> Vec<String> {
    let ratio = |files: usize, total: usize| {
        if total == 0 {
            0.0
        } else {
            files as f64 / total as f64
        }
    };
    let mut lines = vec![
        "representatividade (arquivos):".to_string(),
        format!(
            "  repositório: {}/{} ({:.2})",
            r.repository.files,
            r.repository.total,
            ratio(r.repository.files, r.repository.total)
        ),
    ];
    for c in &r.contexts {
        lines.push(format!(
            "  contexto {}: {}/{} ({:.2})",
            c.context,
            c.files,
            c.total,
            ratio(c.files, c.total)
        ));
    }
    for n in &r.neighbors {
        lines.push(format!(
            "  vizinha {}: {word}|{}={:.2} {}|{word}={:.2} comum={}",
            n.word, n.word, n.word_given_neighbor, n.word, n.neighbor_given_word, n.shared
        ));
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

/// Seções com mais itens que isto mostram só os primeiros e contam o resto por módulo (D3).
const GROUP_AFTER: usize = 10;
/// Módulos citados na linha de contagem; o resto vira `…`.
const GROUP_MODULES: usize = 8;

/// `<n> em: mod ×a · mod ×b …`: contagem por módulo, a maior primeiro, depois pelo nome.
fn module_counts(modules: &[&str]) -> String {
    let mut counts: Vec<(&str, usize)> = Vec::new();
    for m in modules {
        match counts.iter_mut().find(|(name, _)| name == m) {
            Some((_, n)) => *n += 1,
            None => counts.push((m, 1)),
        }
    }
    counts.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
    let mut parts: Vec<String> = counts
        .iter()
        .take(GROUP_MODULES)
        .map(|(m, n)| format!("{m} ×{n}"))
        .collect();
    if counts.len() > GROUP_MODULES {
        parts.push("…".to_string());
    }
    format!("{} em: {}", modules.len(), parts.join(" · "))
}

/// Itens de uma seção, cada um com o módulo; acima de 10, os 10 primeiros e uma linha
/// `+N em: …` com o resto contado por módulo. Com `--all`, todos (TASK-7, D3).
fn grouped(items: Vec<(String, String)>, all: bool) -> Vec<String> {
    if all || items.len() <= GROUP_AFTER {
        return items.into_iter().map(|(line, _)| line).collect();
    }
    let rest: Vec<&str> = items[GROUP_AFTER..]
        .iter()
        .map(|(_, m)| m.as_str())
        .collect();
    let mut lines: Vec<String> = items[..GROUP_AFTER]
        .iter()
        .map(|(line, _)| line.clone())
        .collect();
    lines.push(format!("  +{}", module_counts(&rest)));
    lines
}

fn module_of_addr(addr: &SymbolAddr) -> String {
    memo_core::index::module_of(&addr.path)
}

/// Pacote de um módulo (`p1.m00` → `p1`); módulo de topo é o próprio pacote.
fn package_of(module: &str) -> &str {
    module.rsplit_once('.').map_or(module, |(pkg, _)| pkg)
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
    all: bool,
) -> String {
    let mut head = vec![
        format!("symbol {}  idx={idx}", info.addr.qname),
        item(&info.addr),
        format!(
            "assinatura: sig={}",
            &info.sig_hash[..info.sig_hash.len().min(6)]
        ),
    ];
    if !info.membros.is_empty() {
        head.push(members_line(&info.membros, all));
    }
    head.push("arestas:".to_string());
    if info.edges.is_empty() {
        head.push("  nenhuma".to_string());
    }
    let mut items = Vec::new();
    for (dir, arrow) in [("in", "<-"), ("out", "->")] {
        let imports: Vec<&str> = info
            .edges
            .iter()
            .filter(|e| !all && e.kind == "IMPORTA" && e.dir == dir)
            .map(|e| package_of(&e.other_module))
            .collect();
        if !imports.is_empty() {
            items.push(format!("  IMPORTA {arrow} {}", module_counts(&imports)));
        }
    }
    let rest: Vec<(String, String)> = info
        .edges
        .iter()
        .filter(|e| all || e.kind != "IMPORTA")
        .map(|e| (edge_line(e), e.other_module.clone()))
        .collect();
    items.extend(grouped(rest, all));
    let mut tail = notes_lines(&info.notes);
    tail.extend(read_lines(&info.ler));
    tail.push(coverage_line(cov));
    fit(&head, &items, &tail, max_chars, limit)
}

fn rel_item(i: &RelItem) -> String {
    let attr = i.attr.as_ref().map(|a| format!(" {a}")).unwrap_or_default();
    let approx = if i.approx { " ~" } else { "" };
    let line = i.line.map(|l| format!(" :L{l}")).unwrap_or_default();
    format!("{}{attr}{approx}{line}", item(&i.addr))
}

fn rel_items(items: &[RelItem]) -> Vec<(String, String)> {
    items
        .iter()
        .map(|i| (rel_item(i), module_of_addr(&i.addr)))
        .collect()
}

fn chain_items(items: &[ChainItem]) -> Vec<(String, String)> {
    items
        .iter()
        .map(|i| {
            let module = i.addr.as_ref().map_or("ext".to_string(), module_of_addr);
            (chain_item(i), module)
        })
        .collect()
}

/// Item do nível por palavra: o item, com `~` e a linha, a origem `tipo` quando vem do tipo
/// anotado, e o nível no fim.
fn by_word_item(b: &ByWordItem) -> String {
    let nivel = match b.nivel {
        Evidence::Forte => "forte",
        Evidence::Media => "média",
        Evidence::SoNome => "só nome",
    };
    let origem = match b.origem {
        Origem::Tipo => " tipo",
        Origem::Uso => "",
    };
    format!("{}{origem} {nivel}", rel_item(&b.item))
}

pub fn relation_text(
    label: &str,
    rel: &Relation,
    idx: &str,
    cov: &Coverage,
    max_chars: usize,
    limit: Option<usize>,
    all: bool,
) -> String {
    let mut head = vec![format!("{label} {}  idx={idx}", rel.target.short)];
    if rel.items.is_empty() {
        head.push("  nenhum".to_string());
    }
    let items = grouped(rel_items(&rel.items), all);
    fit(&head, &items, &[coverage_line(cov)], max_chars, limit)
}

pub fn type_text(
    info: &TypeInfo,
    idx: &str,
    cov: &Coverage,
    max_chars: usize,
    limit: Option<usize>,
    all: bool,
) -> String {
    let head = vec![format!("type {}  idx={idx}", info.target.short)];
    let mut items = vec!["produz:".to_string()];
    if info.producers.is_empty() {
        items.push("  nenhum".to_string());
    }
    items.extend(grouped(rel_items(&info.producers), all));
    items.push("consome:".to_string());
    if info.consumers.is_empty() {
        items.push("  nenhum".to_string());
    }
    items.extend(grouped(rel_items(&info.consumers), all));
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
    all: bool,
) -> String {
    let mut head = vec![format!("{label} {}  idx={idx}", chain.target.short)];
    if chain.items.is_empty() {
        head.push("  nenhum".to_string());
    }
    let items = grouped(chain_items(&chain.items), all);
    fit(&head, &items, &[coverage_line(cov)], max_chars, limit)
}

fn tests_sections(tests: &TestsInfo, items: &mut Vec<String>, all: bool) {
    section(
        "testes exatos",
        grouped(rel_items(&tests.exact), all),
        items,
    );
    section(
        "testes via HTTP",
        grouped(rel_items(&tests.via_http), all),
        items,
    );
    section("testes ~", grouped(rel_items(&tests.approx), all), items);
}

fn strategy_label(name: &str) -> &str {
    match name {
        "vizinhanca" => "viz",
        "caminho" => "cam",
        "conexao" => "con",
        other => other,
    }
}

/// `  F nome   caminho:Lx-Ly d1 chamador entrada GET /x [viz 1.00 · cam 1.00] 2.00`.
fn relevant_item(r: &memo_core::relevance::Relevant) -> String {
    let depth = r.depth.map(|d| format!(" d{d}")).unwrap_or_default();
    let entry = match (&r.route, r.entrypoint) {
        (Some(route), _) => format!(" entrada {route}"),
        (None, true) => " entrada".to_string(),
        _ => String::new(),
    };
    let weights: Vec<String> = r
        .strategies
        .iter()
        .map(|w| format!("{} {:.2}", strategy_label(w.strategy), w.weight))
        .collect();
    format!(
        "{}{depth} {}{entry} [{}] {:.2}",
        item(&r.addr),
        r.direction,
        weights.join(" · "),
        r.score
    )
}

pub fn tests_text(
    target: &SymbolAddr,
    tests: &TestsInfo,
    idx: &str,
    cov: &Coverage,
    max_chars: usize,
    limit: Option<usize>,
    all: bool,
) -> String {
    let head = vec![format!("tests {}  idx={idx}", target.short)];
    let mut items = Vec::new();
    tests_sections(tests, &mut items, all);
    fit(&head, &items, &[coverage_line(cov)], max_chars, limit)
}

pub fn impact_text(
    info: &Impact,
    idx: &str,
    cov: &Coverage,
    max_chars: usize,
    limit: Option<usize>,
    all: bool,
) -> String {
    let mut head = vec![format!("impact {}  idx={idx}", info.target.short)];
    let n = &info.nome;
    let generic = if n.generic { " · genérico" } else { "" };
    head.push(format!(
        "nome: {} · {} definições · {}/{} usos resolvidos{generic}",
        n.name, n.definitions, n.resolved, n.uses
    ));
    let w = &info.pesos;
    head.push(format!(
        "pesos: vizinhança {:.2} · caminho {:.2} · conexão {:.2} · nome {:.2}",
        w.vizinhanca, w.caminho, w.conexao, w.nome
    ));
    let mut items = Vec::new();
    let relevant = info
        .relevantes
        .iter()
        .map(|r| (relevant_item(r), module_of_addr(&r.addr)))
        .collect();
    section("relevantes", grouped(relevant, all), &mut items);
    section(
        "chamadores",
        grouped(chain_items(&info.callers), all),
        &mut items,
    );
    section(
        "consumidores",
        grouped(rel_items(&info.consumers), all),
        &mut items,
    );
    section(
        "protocolo",
        grouped(rel_items(&info.protocol), all),
        &mut items,
    );
    // Nome genérico: os usos `só nome` não distinguem o alvo; ficam como contagem, salvo `--all`.
    let hide_name_only = n.generic && !all;
    let by_word: Vec<(String, String)> = info
        .by_word
        .iter()
        .filter(|b| !(hide_name_only && b.nivel == Evidence::SoNome))
        .map(|b| (by_word_item(b), module_of_addr(&b.item.addr)))
        .collect();
    let omitted = info.by_word.len() - by_word.len();
    let mut by_word = grouped(by_word, all);
    if omitted > 0 {
        by_word.push(format!(
            "  +{omitted} só nome omitidos: nome genérico ({}, {} definições); --all lista",
            n.name, n.definitions
        ));
    }
    section("por palavra", by_word, &mut items);
    let hidden = &info.testes_ocultos;
    if hidden.total() > 0 {
        items.push(format!(
            "testes ocultos: {} exatos · {} via HTTP · {} ~ (--tests mostra)",
            hidden.exatos, hidden.via_http, hidden.aproximados
        ));
    } else {
        tests_sections(&info.tests, &mut items, all);
    }
    let mut tail = notes_lines(&info.notes);
    tail.extend(unchanged_line(info.unchanged));
    tail.push(coverage_line(cov));
    fit(&head, &items, &tail, max_chars, limit)
}

/// `membros: A = "a" · B · …` (TASK-7, D6); acima de 10, os 10 primeiros e `+N`, salvo `--all`.
fn members_line(members: &[memo_core::query::EnumMember], all: bool) -> String {
    let shown = if all {
        members.len()
    } else {
        members.len().min(GROUP_AFTER)
    };
    let mut parts: Vec<String> = members[..shown]
        .iter()
        .map(|m| match &m.value {
            Some(v) => format!("{} = {v}", m.name),
            None => m.name.clone(),
        })
        .collect();
    if shown < members.len() {
        parts.push(format!("+{}", members.len() - shown));
    }
    format!("membros: {}", parts.join(" · "))
}

/// Seção `ler:` (TASK-7, D4): um trecho por linha, `caminho:Lini-Lfim — motivo`.
fn read_lines(spots: &[memo_core::query::ReadSpot]) -> Vec<String> {
    if spots.is_empty() {
        return Vec::new();
    }
    let mut lines = vec!["ler:".to_string()];
    lines.extend(
        spots
            .iter()
            .map(|s| format!("  {}:L{}-L{} — {}", s.path, s.start, s.end, s.motivo)),
    );
    lines
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
    if !s.generic_anchors.is_empty() {
        out.push_str(&format!(
            "âncoras genéricas: {}\n",
            s.generic_anchors.join(",")
        ));
    }
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

pub fn note_list_text(notes: &[Note], generic: &[String]) -> String {
    if notes.is_empty() {
        return "nenhuma nota\n".to_string();
    }
    notes
        .iter()
        .map(|n| {
            let mark = if generic.contains(&n.id) {
                " âncora genérica"
            } else {
                ""
            };
            format!(
                "{} {} {} {}  {}{mark}\n",
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
    tail.extend(read_lines(&b.ler));
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
    all: bool,
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
    if let Some(r) = &info.representativeness {
        head.extend(representativeness_lines(&info.word, r));
    }
    if !info.ignored.is_empty() {
        head.push(list_line("ignoradas", &info.ignored));
    }
    head.push("símbolos:".to_string());
    if info.symbols.is_empty() {
        head.push("  nenhum".to_string());
    }
    let symbols = info
        .symbols
        .iter()
        .map(|s| (tagged_item(&s.addr, &s.tags), module_of_addr(&s.addr)))
        .collect();
    let items = grouped(symbols, all);
    fit(&head, &items, &[coverage_line(cov)], max_chars, limit)
}

/// Palavra composta (TASK-7, D7): a interpretação, a ficha curta de cada parte e os símbolos
/// cujo nome tem todas as partes.
pub fn composite_word_text(
    info: &memo_core::query::CompositeWord,
    idx: &str,
    cov: &Coverage,
    max_chars: usize,
    limit: Option<usize>,
    all: bool,
) -> String {
    let mut head = vec![
        format!("word {}  idx={idx}", info.input),
        format!("interpretado como: {}", info.parts.join(" + ")),
    ];
    for card in &info.cards {
        let stats = card.stats.as_ref().map_or(String::new(), |st| {
            format!(
                " uso={} alcance={} espec={:.2}",
                st.uso, st.alcance, st.especificidade
            )
        });
        head.push(format!(
            "{}:{stats} papéis={} verbos={}",
            card.word,
            dash_join(&card.roles),
            dash_join(&card.verbs)
        ));
    }
    if !info.unknown.is_empty() {
        head.push(list_line("desconhecidas", &info.unknown));
    }
    head.push("símbolos com todas:".to_string());
    if info.symbols.is_empty() {
        head.push("  nenhum".to_string());
    }
    let symbols = info
        .symbols
        .iter()
        .map(|s| (item(s), module_of_addr(s)))
        .collect();
    let items = grouped(symbols, all);
    fit(&head, &items, &[coverage_line(cov)], max_chars, limit)
}

/// Relatório de aliases: cada par com o motivo e o `memo note add` pronto para copiar.
pub fn aliases_text(pairs: &[memo_core::aliases::AliasPair], idx: &str, cov: &Coverage) -> String {
    let mut out = format!("aliases  idx={idx}\n");
    if pairs.is_empty() {
        out.push_str("  nenhum par sugerido\n");
    }
    for p in pairs {
        out.push_str(&format!(
            "  {} ~ {}  {} {:.2}\n",
            p.first, p.second, p.reason, p.similarity
        ));
        out.push_str(&format!(
            "    memo note add --tipo glossario --ancora word:{} --origem humano --alias {} \"{} e {} são o mesmo conceito.\"\n",
            p.first, p.second, p.first, p.second
        ));
    }
    out.push_str(&coverage_line(cov));
    out.push('\n');
    out
}

pub fn ambiguous_text(target: &str, candidates: &[String]) -> String {
    let mut out = format!("ambíguo: {target}\ncandidatas:\n");
    for c in candidates {
        out.push_str(&format!("  {c}\n"));
    }
    out
}
