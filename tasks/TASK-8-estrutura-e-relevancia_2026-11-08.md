# TASK-8 — Preservar estrutura e apresentar evidências mais fortes primeiro

## Status
- Estado: CODE_REVIEW
- ROLE requerida: code-reviewer
- Branch: feat/structural-query-relevance; base do PR: master (confirmada pelo humano).
- Implementação registrada no commit e97106d; validações concluídas; preparando PR independente.
- Origem: pedido explícito do usuário para corrigir os problemas gerais do Memo e colocar os melhores resultados no topo.

## 1. Objetivo
Reduzir relações perdidas e priorizar resultados cuja evidência corresponde melhor à consulta.

## 2. Contexto
O receptor de `Classe(...).metodo()` se perde na extração; escritas em atributos fora de `self` e operações ORM não são representadas; o brief tokeniza símbolos e caminhos; relações são frequentemente ordenadas pelo nome antes da certeza.

## 3. Escopo
Chamadas encadeadas, escritas por atribuição/inicialização/SQLAlchemy, âncoras estruturais do brief, ordenação determinística por evidência antes dos cortes, trechos de leitura e contexto de mudanças da feature. Nenhum nome ou regra de negócio de repositórios externos. Dependências externas continuam explicitamente fora da análise de chamadas da biblioteca.

## 4. Critérios
- C1: construção direta de classe resolvida gera CRIA e CHAMA exatos, incluindo alias/import de módulo e herança; factory não vira classe exata.
- C2: writers encontra atribuições e argumentos nomeados de construção, update/insert SQLAlchemy com origem verificável; motivo diferencia inicialização, atribuição e comando SQL. Métodos homônimos de outras APIs não contam.
- C3: símbolos e caminhos do índice são resolvidos antes da tokenização; âncoras explícitas precedem matches lexicais; ambiguidades são expostas.
- C4: relações exatas precedem aproximadas; proximidade estrutural desempata; ordem estável e aplicada antes do agrupamento. O caminho exato prevalece sobre um caminho aproximado até o mesmo nó no mesmo nível.
- C5: preservar resultados aproximados identificados como candidatos; não afirmar ausência de testes quando só faltam relações exatas.
- C6: ler segue a prioridade do resultado, mantém os limites do símbolo e evita janelas que cortam uma instrução composta quando há orçamento para completá-la.
- C7: com feature, símbolos modificados são identificados por comparação de assinatura/corpo com a base, sem tratar o delta da sessão como diff de código.
- C8: fixtures sintéticas, fmt, clippy e testes do workspace passam; esquema derivado é invalidado quando a extração muda; notas são preservadas.

## 5. Restrições
Análise estática não comprova execução, commit SQL nem callbacks de bibliotecas externas. Não penalizar nomes de telemetria globalmente. Não ocultar candidatos do JSON. Sem novas dependências externas. Sem modificar o arquivo preexistente tasks/memo.md.

## 6. Dados
Grafo derivado reconstruído ao mudar versão; notas fora do banco preservadas.

## 7. Cenários de teste
C1: receptor construído/importado/herdado e factory; C2: SQLAlchemy com alias, homônimo, inicialização e atribuição; C3: símbolo/caminho/ambiguidade; C4: reader exato após homônimo aproximado, proximidade e caminhos concorrentes; C5: candidatos lexicais permanecem distintos; C6: bloco composto e delta de trechos; C7: diff base/feature; C8: suíte existente de schema e indexação incremental.

## 9. Decisões e plano
D1 (humano): trabalhar nas correções gerais do Memo e priorizar os melhores resultados, conforme conversa.
D2 (premissa de execução): interpretar melhor como força de evidência e proximidade estrutural; desempate por nome/linha, sem classificação de domínio.
D3 (premissa de execução): aprovação de implementação já dada no pedido atual; não repetir confirmação para trabalho reversível; sem commits/publicação implícitos.
D4: sem índice Rust no Memo (indexador Python); inspeção local de consumidores e testes substitui consultas do próprio Memo neste repositório.
D5 (ENGINEERING): ordenar por evidência antes do agrupamento; dentro do nível direto, nomes de código precedem módulos/imports, mantendo as cotas de vizinhança e separação produção/testes. Scores não são monotônicos entre grupos.
D6 (ENGINEERING): SQLAlchemy limitado a cadeias diretas com argumentos nomeados e import reconhecido; builders em variáveis/dicionários não interpretados. Inicialização e comando SQL não comprovam persistência.
D7 (ENGINEERING): contexto de mudanças compara hashes base × feature; delta de sessão inclui hashes para não esconder edições sem mudança de endereço. Base incompatível é aberta somente para leitura, rejeitada e sinalizada; nenhum rebuild implícito da base durante brief da feature.
D8 (ENGINEERING): regiões sintáticas derivadas, janela até 40 linhas, sem sobreposição no brief. Não implementar análise de fluxo de dados.

### Verificação local da implementação
- C1/C2: fixtures de construtor com alias e herança, factory, homônimo SQLAlchemy, nomes importados ocultados por parâmetros, inicialização e atribuição, limpeza incremental de relações.
- C3/C4/C5: símbolo/caminho explícito e ambíguo; reader exato acima de 12 aproximados antes do agrupamento; desempate por pacote; rota exata preservada na propagação; import hub abaixo do código com nome; candidatos lexicais continuam distintos.
- C6/C7: bloco curto completado, trechos sem sobreposição, comparação base/feature, prioridade explícito→alterado, repetição/full via CLI e mudança de corpo nas mesmas linhas reapresentada.
- C8: suíte completa de schema, indexação, notas, CLI e MCP aprovada. 19 novos testes de regressão (16 no núcleo e 3 na CLI), total 395.
- Revisão local: diff de extração/resolução, consultas, seleção/delta, armazenamento e integração relido; snapshots alterados conferidos antes de aceitar. Testes antigos de fallback agora usam receptor desconhecido para continuar verificando casos genuinamente não resolvidos.
- Limites: sem benchmark de latência em repositório grande ou nova medição em código privado; sem revisão formal de PR, commits, merge ou instalação global do binário. Referência Rust específica das guidelines ausente; usados critérios gerais e verificadores do workspace.

### N_PARTITE (DEFINITION)
[N_PARTITE] modo: concept | alvo: snapshot | referência: conversa | perfil: core | budget: 10
[N_PARTITE:concept] par: referência extraída (L2) ↔ resolução (L3) — acoplamento: alto | evidência: não definido | abstração: preservar receptor e motivo no fato bruto | lifecycle: coerente → resolver sem inferir identidade por nome de método.
[N_PARTITE:concept] par: evidência (L1) ↔ ordenação (L3) — acoplamento: alto | evidência: não definido | abstração: certeza precede proximidade e nome | lifecycle: divergente → ranking não altera fatos do índice.
[N_PARTITE:concept] par: seleção (L3) ↔ apresentação (L3) — acoplamento: alto | evidência: não definido | abstração: ordenar antes do corte e manter candidatos identificados | lifecycle: coerente → trechos acompanham símbolos selecionados.
[N_PARTITE.STRUCT:concept] alvo: consulta estrutural — problema: tokenização destrói identidade | estrutura proposta: resolver símbolos/caminhos antes do fallback lexical | base: brief.rs tokenize/brief_weighted.
[N_PARTITE.ESCALATE] gatilho: reconstrução do grafo compartilhado → perfil recomendado: failure_only; testes existentes de schema, lock e indexação serão executados.

Plano: testes regressivos → extração/resolução → ranking/âncoras → contexto/trechos → documentação → testes/fmt/clippy → revisão do diff e log. Dependências: ranking depende dos fatos preservados; saída depende da seleção. Riscos: falsos exatos, reconstrução de índices, custo de ranking; mitigação por casos negativos, teste de schema e carregamento em lote.

## 10. Log
- 2026-11-08: DEFINITION concluída a partir do escopo discutido e do pedido de implementação; nenhuma nova confirmação exigida. TEST_ANALYSIS: cenários acima definidos antes das alterações de produção; baseline do workspace iniciado.
- 2026-11-08T19:35:00-03:00 — TEST_ANALYSIS → ENGINEERING: baseline aprovado; sete testes iniciais falharam por asserção reproduzindo as lacunas. Escopo e autorização mantidos conforme D1/D3; sem alterações no arquivo preexistente tasks/memo.md.
- 2026-11-08T19:55:00-03:00 — ENGINEERING: casos adicionais de proximidade, caminhos concorrentes, feature, trechos, imports ocultados e delta do corpo. Teste de corpo editado sem mudança de linha falhou antes da correção da chave de sessão. Dois snapshots atualizados após inspeção; expectativas obsoletas ajustadas sem eliminar cenários de fallback.
- 2026-11-08T20:02:00-03:00 — ENGINEERING → CODE_REVIEW: cargo test --workspace --no-fail-fast --quiet (395 aprovados, exit 0); cargo clippy --workspace --all-targets -- -D warnings (exit 0); cargo fmt --all --check (exit 0); git diff --check (exit 0). README, spec e docs atualizados. Implementação disponível na árvore de trabalho da branch main; sem commits/publicação. Não marcado DONE: revisão formal e merge não realizados.
- 2026-11-08T20:19:06-03:00 — Publicação autorizada pelo pedido de PR. Humano confirmou PR independente e base master após verificar ausência de main remota. Base origin/master 8835ee4; main local contém 100 commits pendentes das TASKs 6/7, necessários às correções; incorporados ao escopo do PR. Branch feat/structural-query-relevance criada e implementação/testes registrados em e97106d. Nova execução: 395 testes aprovados, fmt/clippy/diff-check sem erros. Tentativa inicial de commit falhou na assinatura GPG; commit criado com commit.gpgsign=false somente na chamada, sem mudar configuração global. tasks/memo.md preservado fora dos commits. Binário global já atualizado na etapa anterior, com checksum conferido.
