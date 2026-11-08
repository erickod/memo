# Possíveis caminhos

Ideias para depois, com o que as sustenta e o que ainda precisa ser decidido. Nada aqui é compromisso: são hipóteses, algumas com medição e outras só com argumento. As limitações que motivam cada uma estão em [Limitações](limitacoes.md), e o que existe hoje em [Funcionalidades](funcionalidades.md).

## 1. Fazer o `brief` mostrar o que é central

É o ponto mais próximo: o `brief order` deixou de trazer hubs, mas o nível dos símbolos que carregam a palavra virou uma lista de módulos.

- **Medir antes de decidir**, no repositório real, três candidatos: peso menor para módulos, limite de módulos no primeiro nível, e desempate pelo número de símbolos da âncora que apontam para cada símbolo (a ideia: `OrderEntity` é muito referenciado entre os símbolos de `order`, e um módulo de handler não).
- **O que sustenta:** a medição do `brief order` e o empate entre 421 símbolos diretos.
- **Falta decidir:** o peso dos módulos e se vira chave do `memo.toml`.

## 2. Usar a representatividade onde ela ajuda

- Ordenar as vizinhas da ficha pela representatividade em vez de só pelos arquivos em comum.
- Dizer "quem vive dentro do mundo de quem" (`P(order|cancel)` 0,77 contra `P(cancel|order)` 0,19) também no `brief`.
- **O que sustenta:** as medições em arquivos, que separaram `cancel` de `id` melhor que qualquer nota por palavra.
- **Cuidado:** o `memo` não classifica domínio; a representatividade só informa.

## 3. Mais fontes de palavras

- Docstrings e comentários de produção (a spec já prevê o valor `doc`).
- **Falta decidir:** como evitar que prosa livre vire ruído, já que hoje só entram palavras que existem em algum identificador.

## 4. Menos chamadas sem resolução

- 23% das chamadas ficaram sem resolução no repositório real. Cada regra nova de resolução (como a evidência de import) reduziu esse número em teste.
- **Falta:** olhar os casos não resolvidos mais frequentes do repositório real e ver qual regra os pegaria.

## 5. Validar a integração com agentes

- Testar `memo serve`, `.mcp.json` e `.codex/config.toml` contra Claude Code e Codex de verdade, e suportar o `agy`.
- Avisar no CLI quando o índice foi refeito por mudança de config.
- Parar de gravar `min_specificity` e `min_coherence` no `memo init`.

## 6. Desempenho em repositório grande

- Guardar em cache, no processo do `serve`, a carga arquivo×palavra que a ficha refaz a cada consulta.
- Só materializar pontes em tabela se a medição exigir (hoje elas são derivadas por consulta).

## 7. Calibrar com mais repositórios

- Os limiares e pesos (piso de 3 arquivos, 10 vizinhas, divisão 10/5, fator de salto 0,5, pesos de ponte) vieram de um repositório real e de um sintético.
- Repetir as medições em outros repositórios antes de fixar valores; tornar configuráveis os que se mostrarem sensíveis.

## 8. Outras linguagens

- O núcleo prevê a interface, mas o MVP só implementa Python. Seria uma TASK própria, com extrator, vocabulário e testes por linguagem.

## 9. Pagar a dívida técnica

- Separar `word_pattern` e `word_card`, quebrar `brief_weighted`, e acrescentar os testes de borda listados em [Limitações](limitacoes.md#dívida-técnica-declarada).
