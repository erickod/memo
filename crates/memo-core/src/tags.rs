//! Por que um símbolo aparece para um conjunto de palavras: as pontes que o ligam a elas.

use crate::error::CoreError;
use rusqlite::Connection;

fn id_list(word_ids: &[i64]) -> String {
    word_ids
        .iter()
        .map(i64::to_string)
        .collect::<Vec<_>>()
        .join(",")
}

/// Etiquetas, em ordem fixa: `nome` (palavra no nome), `docstring` (palavra na docstring do
/// teste), `teste` (testado por chamada exata de um teste com a palavra), `chamada` (chamador
/// ou chamado de produção com a palavra), `tipo` (liga por `RECEBE` ou `RETORNA` a símbolo com
/// a palavra) e `import` (módulo cujo arquivo importa a palavra).
pub(crate) fn bridge_tags(
    conn: &Connection,
    symbol_id: i64,
    word_ids: &[i64],
) -> Result<Vec<&'static str>, CoreError> {
    if word_ids.is_empty() {
        return Ok(Vec::new());
    }
    let ids = id_list(word_ids);
    let checks: [(&'static str, String); 6] = [
        (
            "nome",
            format!(
                "SELECT EXISTS(SELECT 1 FROM occurrence WHERE owner_kind = 'symbol' AND owner_id = ?1
                 AND source = 'name' AND word_id IN ({ids}))"
            ),
        ),
        (
            "docstring",
            format!(
                "SELECT EXISTS(SELECT 1 FROM occurrence WHERE owner_kind = 'symbol' AND owner_id = ?1
                 AND source = 'test_doc' AND word_id IN ({ids}))"
            ),
        ),
        (
            "teste",
            format!(
                "SELECT EXISTS(SELECT 1 FROM edge e JOIN symbol t ON t.id = e.src
                 WHERE e.dst = ?1 AND t.is_test = 1 AND e.kind IN ('CHAMA', 'CRIA') AND e.approx = 0
                 AND EXISTS(SELECT 1 FROM occurrence o WHERE o.owner_kind = 'symbol' AND o.owner_id = t.id
                            AND o.source IN ('name', 'test_doc') AND o.word_id IN ({ids})))"
            ),
        ),
        (
            "chamada",
            format!(
                "SELECT EXISTS(SELECT 1 FROM edge e JOIN symbol p
                   ON p.id = CASE WHEN e.src = ?1 THEN e.dst ELSE e.src END
                 WHERE (e.src = ?1 OR e.dst = ?1) AND e.kind IN ('CHAMA', 'CRIA') AND p.is_test = 0
                 AND EXISTS(SELECT 1 FROM occurrence o WHERE o.owner_kind = 'symbol' AND o.owner_id = p.id
                            AND o.source = 'name' AND o.word_id IN ({ids})))"
            ),
        ),
        (
            "tipo",
            format!(
                "SELECT EXISTS(SELECT 1 FROM edge e JOIN symbol p
                   ON p.id = CASE WHEN e.src = ?1 THEN e.dst ELSE e.src END
                 WHERE (e.src = ?1 OR e.dst = ?1) AND e.kind IN ('RECEBE', 'RETORNA')
                 AND EXISTS(SELECT 1 FROM occurrence o WHERE o.owner_kind = 'symbol' AND o.owner_id = p.id
                            AND o.source = 'name' AND o.word_id IN ({ids})))"
            ),
        ),
        (
            "import",
            format!(
                "SELECT EXISTS(SELECT 1 FROM symbol s JOIN import i ON i.file_id = s.file_id
                 JOIN occurrence o ON o.owner_kind = 'import' AND o.owner_id = i.rowid
                 WHERE s.id = ?1 AND s.kind = 'module' AND o.word_id IN ({ids}))"
            ),
        ),
    ];
    let mut tags = Vec::new();
    for (tag, sql) in checks {
        let hit: bool = conn.query_row(&sql, [symbol_id], |r| r.get(0))?;
        if hit {
            tags.push(tag);
        }
    }
    Ok(tags)
}
