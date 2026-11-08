# Limitações

O que o `memo` não faz, faz mal ou não foi verificado. Cada item diz de onde vem a evidência. As funcionalidades estão em [Funcionalidades](funcionalidades.md), e o que pode vir depois em [Possíveis caminhos](caminhos.md).

## O `brief` ainda não mostra o que é central

- **No `brief order` de um repositório real, os 10 lugares dos símbolos que carregam a palavra foram todos para módulos** (`mod ... [nome, import]`). A classe `OrderEntity` nem aparece entre eles. Um módulo soma nome e import e passa a classe, que só tem o nome. (medido no repositório real)
- **Entre os símbolos que carregam a palavra não há critério que distinga o central.** `order` tem 421 símbolos diretos, quase todos com a mesma pontuação; o desempate é alfabético. (medido; a explicação é uma hipótese, ainda não testada)
- **O `contorno` pode vir vazio**, porque ele resume as chamadas dos 5 primeiros símbolos de produção, e módulos não têm arestas de chamada. (medido)
- Os métodos de `OrderEntity` aparecem, mas como vizinhos (nível 2, 5 vagas). Os valores 10 e 5 são iniciais, sem calibração, e não são configuráveis.
- O `score` no JSON do `brief` não é mais monotônico na lista: há uma subida na fronteira entre os dois níveis.

## Ruído e calibração

- **Palavras de infraestrutura fora da lista `skeleton`** (`is`, `id`, `usecase`) também viram âncora e podem trazer ruído. O `memo` não decide sozinho o que é genérico: a lista é sua.
- **A coerência não separa conceito de ruído.** No repositório real, `order` teve coerência 0,21 e `id` 0,48. Por isso ela é só informativa. As medições de representatividade e de concentração entre contextos são de **um repositório e de 14 termos escolhidos à mão**, sem validação em palavras novas.
- **A ficha lista as vizinhas por arquivos em comum**, o que põe palavras muito frequentes (`id`, `test`, `item`) no topo, embora os números de representatividade estejam lá.
- Pesos de ponte, piso de 3 arquivos em comum e as 10 vizinhas da ficha são valores iniciais, a calibrar.
- O relatório de aliases por vizinhança sugere pares sem sentido em repositórios pequenos.

## Resolução de chamadas

- Num repositório real, a cobertura foi de 4.450 chamadas resolvidas, 1.492 aproximadas e 1.740 **não resolvidas** (cerca de 23% do total). O resultado vazio nunca prova ausência.
- Escrita em variável de módulo ou `global` ainda não gera a aresta `ESCREVE`.

## Notas

- O CLI só registra a `origem` de uma nota; não verifica quem confirma.
- Âncoras secundárias de uma nota não geram `stale` nem `orfa`.
- Nome definido mais de uma vez (`@property` com `@x.setter`, `@overload`, atributo e método com o mesmo nome): vale a primeira definição; mudar só o corpo de um setter não marca notas como `stale`.

## Integração

- **O servidor MCP e os formatos de `.mcp.json` e `.codex/config.toml` não foram verificados contra Claude Code nem Codex.** Vieram de documentação lida por busca resumida.
- O harness `agy` não é suportado (dá erro, exit 1).
- O `memo init` ainda grava `min_specificity` e `min_coherence` no `memo.toml`, que não têm mais efeito.

## Desempenho

- Uma mudança de config que molda o índice refaz todos os arquivos: cerca de 9 s em 801 arquivos e cerca de 44 s em 5.000. O CLI não avisa que o índice foi refeito.
- `query word` custa de 1 a 2,3 s em repositório grande, com palavras presentes em quase todo arquivo, porque carrega todos os pares arquivo×palavra a cada consulta.
- As medições vêm de um gerador sintético e de dois repositórios; não há medição em repositórios de outro perfil.

## Escopo

- Só Python.
- Docstrings e comentários de produção ainda não geram palavras (só as de testes).
- O `memo` entrega fatos e não julga código nem classifica o que é domínio.

## Dívida técnica declarada

Itens aceitos pelo humano nas revisões, sem prazo:

- `brief_weighted` tem cerca de 190 linhas e mistura a pontuação com a montagem das seções.
- `word_core` usa `Option<&BridgeWeights>` como flag de modo para três efeitos; falta separar `word_pattern` e `word_card`.
- Três cálculos de "vizinha" (pontes por palavra, `core_neighborhoods` em lote, representatividade) sem teste que os cruze.
- Sem teste de `write_methods` no hash de config, da interrupção do rebuild, de arestas duplicadas no grau do `brief` nem de coincidência entre `neighbors()` e `link_degrees`.
- A chave `entrypoint_decorators` não tem consumidor no código.
