# Limitações

O que o `memo` não faz, faz mal ou não foi verificado. Cada item diz de onde vem a evidência. As funcionalidades estão em [Funcionalidades](funcionalidades.md), e o que pode vir depois em [Possíveis caminhos](caminhos.md).

## Ranking e centralidade do `brief`

- A TASK-8 prioriza alvos estruturais explícitos e código que carrega a palavra antes de módulos e imports. Isso evita a dominância de módulos nas fixtures; ainda não é uma classificação de centralidade de domínio nem uma calibração em vários repositórios.
- **O `contorno` pode vir vazio** quando os símbolos selecionados não têm chamadas conhecidas.
- Os métodos de `OrderEntity` aparecem, mas como vizinhos (nível 2, 5 vagas). Os valores 10 e 5 são iniciais, sem calibração, e não são configuráveis.
- O `score` no JSON não é monotônico entre grupos de evidência. A prioridade do grupo precede a pontuação.

## Ruído e calibração

- **Palavras de infraestrutura fora da lista `skeleton`** (`is`, `id`, `usecase`) também viram âncora e podem trazer ruído. O `memo` não decide sozinho o que é genérico: a lista é sua.
- **A coerência não separa conceito de ruído.** No repositório real, `order` teve coerência 0,21 e `id` 0,48. Por isso ela é só informativa. As medições de representatividade e de concentração entre contextos são de **um repositório e de 14 termos escolhidos à mão**, sem validação em palavras novas.
- **A ficha lista as vizinhas por arquivos em comum**, o que põe palavras muito frequentes (`id`, `test`, `item`) no topo, embora os números de representatividade estejam lá.
- Pesos de ponte, piso de 3 arquivos em comum e as 10 vizinhas da ficha são valores iniciais, a calibrar.
- O relatório de aliases por vizinhança sugere pares sem sentido em repositórios pequenos.

## Resolução de chamadas

- **Rotas HTTP:** só decoradores `x.<método>("/caminho")` com caminho literal. Prefixos de `APIRouter(prefix=...)`, `include_router(prefix=...)` e blueprints não são aplicados, então um teste que requisita o caminho com prefixo não casa. Requisições só por `cliente.<método>("/...")` com o caminho literal como 1º argumento; `requests.request("GET", ...)` e URLs montadas a partir de uma base (`f"{BASE}/x"`) ficam fora. Um `.get("/x")` que não é HTTP (dicionário com chave começando em `/`) também vira requisição.
- **Entrypoints de worker/fila:** só por `entrypoint_decorators`; callbacks registrados por chamada (`subscribe(callback)`) não são reconhecidos.
- **Relevância:** os pesos de `[strategies]` e os limiares de nome genérico (P99 com pisos 4 e 10) vêm de 8 alvos de um repositório, sem calibração em outros. Um grupo de homônimos exige chamada exata direta entre as candidatas.
- Num repositório real, a cobertura foi de 4.450 chamadas resolvidas, 1.492 aproximadas e 1.740 **não resolvidas** (cerca de 23% do total). O resultado vazio nunca prova ausência.
- Escrita em variável de módulo ou `global` ainda não gera a aresta `ESCREVE`.
- Escritas SQLAlchemy reconhecem cadeias diretas `update/insert(Model)…values(attr=...)` com import verificável e atributo indexado. Builders guardados em variáveis, argumentos em dicionários/`**kwargs`, SQL textual e APIs de outros ORMs ficam fora. Inicialização e construção de comando SQL não comprovam persistência ou execução.
- Factories, metaclasses, monkey-patching e callbacks de dependências externas não têm execução inferida. Herança externa não comprova que a biblioteca chama um método sobrescrito. A proteção contra nomes importados ocultados cobre parâmetros e locais de tipo conhecido; não há análise completa de escopo e fluxo de nomes.
- `ler:` completa blocos curtos que cruzam a janela até 40 linhas; não segue o valor até decisões posteriores nem faz análise de fluxo de dados. Símbolos grandes continuam sujeitos ao orçamento.
- Quando outro arquivo ganha um homônimo de uma função já chamada, quem a chamava não é resolvido de novo até o próprio arquivo mudar: a chamada segue apontando para a definição antiga. (achado na TASK-6, não corrigido)

## `impact` por palavra

- **O "mesmo uso" ainda é a linha do símbolo.** Chamadas diretas `Classe(...).metodo()` agora preservam o receptor e podem ser exatas; o fallback lexical continua usando coincidências na mesma linha, portanto `x = Alpha(); y.run()` numa linha só também pode virar evidência `forte` aproximada. (TASK-8; decisão D14 da TASK-6)
- **Usos de teste nunca aparecem**, com ou sem `--no-tests`; chamadas no nível do módulo não são extraídas. (teste)
- **Nome de alvo genérico** (`run`, `execute`) lista centenas de usos `só nome` (no `bope`, 384 para um `execute`); o nível ordena, mas dentro dele a ordem é alfabética, sem ranking por pontes.
- **A evidência quase nunca passa de `só nome` em código com injeção de dependência**: o receptor (`self._repo.run()`) não cita a classe. No `bope`, um único `forte` em seis alvos medidos, e ele era verdadeiro.
- **Custo:** no `bope`, índice completo de 54 s antes e depois da TASK-6 e `impact` de 50 a 80 ms por alvo; não medido no gerador sintético nem para um arquivo alterado.

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
- As medições vêm de um gerador sintético e de dois repositórios; não há medição em repositórios de outro perfil. O gerador que produziu os números das TASKs se perdeu; o versionado em `scripts/measure/` dá outro perfil, e nele o `brief` leva 0,66 s, acima da meta de 500 ms (causa não investigada). Ver [Como reproduzir as medições](medicoes.md).

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
