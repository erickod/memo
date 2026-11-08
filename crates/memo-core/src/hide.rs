//! `--no-tests`: tira os testes da exibição. Só filtra a saída: as estatísticas das palavras e
//! as pontes são calculadas antes, com o repositório inteiro (D12). Cada função devolve quantos
//! itens de teste escondeu, para a cobertura informar.

use crate::bridges::BridgeKind;
use crate::brief::Brief;
use crate::query::{Chain, Impact, Relation, SymbolInfo, TypeInfo, WordInfo};

fn drop_where<T>(items: &mut Vec<T>, is_test: impl Fn(&T) -> bool) -> usize {
    let before = items.len();
    items.retain(|i| !is_test(i));
    before - items.len()
}

pub fn brief(b: &mut Brief) -> usize {
    let mut hidden = drop_where(&mut b.symbols, |s| s.in_tests);
    for outline in &mut b.outline {
        outline.links.retain(|link| {
            let summary = link
                .strip_prefix("<- +")
                .and_then(|rest| rest.strip_suffix(" testes"))
                .and_then(|n| n.parse::<usize>().ok());
            hidden += summary.unwrap_or(0);
            summary.is_none()
        });
    }
    b.outline.retain(|o| !o.links.is_empty());
    hidden
}

pub fn chain(c: &mut Chain) -> usize {
    drop_where(&mut c.items, |i| {
        i.addr.as_ref().is_some_and(|a| a.in_tests())
    })
}

pub fn relation(r: &mut Relation) -> usize {
    drop_where(&mut r.items, |i| i.addr.in_tests())
}

pub fn type_info(t: &mut TypeInfo) -> usize {
    drop_where(&mut t.producers, |i| i.addr.in_tests())
        + drop_where(&mut t.consumers, |i| i.addr.in_tests())
}

pub fn symbol(s: &mut SymbolInfo) -> usize {
    drop_where(&mut s.edges, |e| e.other_is_test)
}

/// Esconde os chamadores e consumidores que são teste e a seção de testes inteira.
pub fn impact(i: &mut Impact) -> usize {
    let tests = i.tests.exact.len() + i.tests.approx.len();
    i.tests.exact.clear();
    i.tests.approx.clear();
    tests
        + drop_where(&mut i.callers, |c| {
            c.addr.as_ref().is_some_and(|a| a.in_tests())
        })
        + drop_where(&mut i.consumers, |c| c.addr.in_tests())
        + drop_where(&mut i.protocol, |c| c.addr.in_tests())
}

/// Esconde os símbolos de teste e a linha de pontes `teste`; as estatísticas ficam como estão.
pub fn word(w: &mut WordInfo) -> usize {
    w.bridges.retain(|b| b.kind != BridgeKind::Test);
    drop_where(&mut w.symbols, |s| s.addr.in_tests())
}
