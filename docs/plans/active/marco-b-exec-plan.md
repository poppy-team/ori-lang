---
id: marco-b-exec-plan
title: Marco B Execution Plan — Modular Ori Self-Host Compiler Frontend, IPC Bridge & Codegen
status: in_progress
adr: docs/decisions/adr/0006-selfhost-modular-architecture.md
target_version: 0.3.9
started: 2026-09-07
---

# Plano de Execução: Marco B — Compilador Self-Host Modular em Ori

> Auditoria de 2026-09-23: os estados `done` abaixo registram a implementação
> declarada em 2026-09-07, não equivalência demonstrada com o stage0. O gate
> antigo compilava stage1 e stage2 com stage0 e comparava apenas a saída de
> `check`. A promoção, a conformidade e o bootstrap real continuam bloqueados
> até stage1 compilar stage2, stage2 compilar stage3 e os binários e testes
> semânticos serem avaliados. O gate também compila e executa `examples/hello`
> pelos três compiladores e compara a saída. A bridge v1 só representa um
> subconjunto da HIR.

> CI de 2026-09-23, execução 35893003937: stage0 produz stage1; stage1
> verifica imports com nomes reservados e compila/executa um `hello` mínimo.
> Ao compilar o próprio `main.orl`, o frontend lê 152 tokens, analisa 37
> expressões e valida seis imports sem erro. A emissão recusa o corpo com
> `bridge.unsupported_ir`; nenhum stage2 ou stage3 foi produzido. A próxima
> fatia é modelar fielmente controle de fluxo, chamadas e tipos no frontend e
> no protocolo da bridge, incluindo os módulos importados pelo compilador.

> Execução 35899996200: a regressão de `ori.list.get` passou no CI Linux,
> Windows GNU/MSVC e macOS ARM/x86; o pacote Linux reproduziu `retained` após
> remoção do elemento da lista. O bootstrap ainda parou em `unsupported_ir` e
> duas regressões anteriores de coleções continuam reprovando no macOS. A
> próxima revisão do stage1 preserva `->` e operadores no payload, além de
> recusar tokens e construções que o IR plano não codifica. Só uma execução
> nova do gate pode promover essa revisão a comportamento verificado no CI.

> Execução 35902135131: stage1 compilou e executou `hello` e um programa
> aritmético com retorno `Int`, `Mul` e `Sub` conferidos no payload da bridge;
> seis negativas de IR incompleta foram recusadas. Stage1 analisou seu
> `main.orl` mas ainda recusou a emissão com `bridge.unsupported_ir`. A
> revisão seguinte isola a checagem de retorno por função, antes de ampliar
> o frontend e a bridge para chamadas, controle de fluxo e imports transitivos.

> Execução 35903699883: stage1 passou no CI os programas `hello`, aritmética,
> duas funções e precedência de `ori.io`, além de rejeitar IR incompleta; a
> emissão do próprio compilador ainda parou em `bridge.unsupported_ir`. Os
> testes nativos de igualdade de coleções falharam em macOS e Windows GNU;
> Linux GNU e Windows MSVC passaram. A próxima fatia valida chamadas locais
> sem parâmetros com retorno `int` do frontend até o objeto nativo. Os demais
> tipos, parâmetros, fluxo de controle e imports transitivos seguem pendentes.

> Investigação das falhas nativas: callbacks de igualdade e hash recebiam
> chaves emprestadas do runtime e as passavam sem retenção a métodos Ori que
> consomem parâmetros gerenciados. O mesmo caminho de comparação direta já
> retinha as referências. A correção aplica a retenção às callbacks; a
> confirmação depende da próxima execução multiplataforma do CI.

> O frontend agora recusa uma variável local de outra função durante `check`
> e `compile`; a emissão também valida os bindings visíveis até cada
> statement. O gate compila stage1 com o cache incremental desativado para
> garantir que mudanças nos módulos importados estejam no binário verificado.

> A checagem de retorno passa a consultar bindings locais e assinaturas de
> funções do módulo; comparações produzem `bool`. Chamadas `bool` passam em
> `check` quando os retornos coincidem. A bridge agora emite essas chamadas
> sem parâmetros, literais booleanos e comparações inteiras; bindings booleanos,
> parâmetros e fluxo de controle ainda dependem de representação completa.

> Revisão de 2026-09-24: o stage1 agora guarda nomes e tipos de parâmetros
> por função e emite chamadas locais de um argumento `int`; a bridge valida
> aridade/tipo e usa a assinatura ao baixar a HIR. O gate compara execução
> nativa com stage0 e rejeita argumentos incompatíveis e referências fora do
> escopo. Stage2/stage3 permanecem dependentes de estruturas de controle,
> coleções, expressões compostas e ligação efetiva dos imports do compilador.

> A run 203 confirmou chamadas locais com um parâmetro `int` no objeto nativo
> com a mesma saída do stage0 e os testes negativos; a autocompilação continua
> parando em `bridge.unsupported_ir` no `main.orl`. A revisão seguinte preserva
> também bindings locais `bool` e suas anotações até a bridge, com paridade
> nativa e rejeição de anotações incompatíveis.

> Execução 204: todos os jobs nativos e de empacotamento passaram; o bootstrap
> parou em `bridge.unsupported_ir` ao compilar `selfhost/compiler/main.orl`.
> A revisão atual representa `if`/`else` e `while` aninhados, atribuições
> mutáveis e `break`/`continue`, amplia chamadas para oito argumentos inteiros
> e descobre imports transitivos com detecção de ciclos e visibilidade de
> funções. A requisição local contém as definições de dois módulos importados
> e preserva o corpo de um laço aninhado. A execução nativa desses novos casos
> passou na execução 205, incluindo os testes negativos e a ligação de dois
> imports transitivos. O stage1 analisou 152 tokens do `main.orl` e parou em
> assinaturas e expressões não representadas pela bridge. A revisão posterior
> recupera assinaturas genéricas/qualificadas e `if` inline, preserva `%`,
> `and`, `or` e bytes inválidos. A autocompilação ainda recusa o IR não
> representado; stage2 e stage3 não foram gerados.

> Execução 207: cadeias `elif` e operadores unários passaram pelo stage1
> nativo, com a mesma saída de stage0. O bootstrap continua reprovado apenas
> ao solicitar stage2: `bridge.unsupported_ir` no corpo de `main.orl` e nos
> imports com tipos compostos. A execução 208 confirmou que stage1 compila e
> executa strings UTF-8/escapadas como stage0, recusa bytes e escapes não
> representados, e ainda termina na mesma recusa explícita antes de stage2.
> Stage2 e stage3 ainda não foram produzidos.

> Revisão seguinte: o frontend preserva `match` como statement com braços
> inteiros/booleanos e fallback explícito; cada corpo mantém seu escopo e o
> contexto do laço. O protocolo e a bridge validam padrões, cobertura e
> duplicatas antes da geração nativa. Fixtures cobrem `match` dentro de laço e
> dentro de uma função importada transitivamente. A validação remota dessa
> revisão e a autocompilação ainda precisam passar pelo gate de CI; `match`
> de enum/optional/result, coleções e tipos genéricos seguem pendentes.

## 1. Objetivo
Implementar o pipeline do compilador escrito diretamente em Ori sob arquitetura limpa (ADR-0006), dividindo as fases de frontend (`lex`, `parse`, `resolve`, `types`), representação intermediária (`hir`), e o protocolo isolado de IPC/Bridge (`CONTRACT01`) com o backend de geração de código nativo Cranelift existente em Rust.

## 2. Invariantes de Modularidade e Clean Code (ADR-0006)
- **Teto de linhas**: Máximo de 600–800 linhas por arquivo `.orl`. Módulos complexos devem ser fragmentados em subpastas de domínio.
- **Pipeline estritamente unidirecional**: Sem mutabilidade de nós anteriores ou retroalimentação de estado global.
- **Isolamento de I/O**: Fases de compilação funcionam como transformações puras na memória. Somente o ponto de entrada do driver (`src/main.orl`) acessa disco ou stdio.
- **Erros desacoplados**: Fases retornam coleções de diagnósticos estruturados em vez de invocar `panic` ou emitir direto no terminal.

## 3. Tabela Priorizada de Tarefas e Custos (T-Shirt Size)

| ID | Prioridade | Esforço | Tarefa | Entradas / Dependências | Entregáveis | Status |
|---|:---:|:---:|---|---|---|:---:|
| **CONTRACT01** | P1 | M | Especificação e schema do protocolo IPC/IR entre Ori e Rust Bridge | ADR-0006 | `docs/spec/20-bridge-protocol.md`, Schemas e Fixtures | `done` |
| **PROBE01** | P1 | S | Probes executáveis em Ori para validar primitivas necessárias | Reference Compiler | Programas de probe rodando via `ori run` | `done` |
| **BRIDGE01** | P1 | L | Receptor Rust (`ori-bridge-server`): framing, validação e Cranelift codegen | CONTRACT01 | Crate Rust `ori-bridge-server` com testes de round-trip | `done` |
| **BRIDGE02** | P2 | M | Endurecimento da bridge: timeouts, cancelamento, truncamento e negação | BRIDGE01 | Suíte de segurança e limites de memória | `done` |
| **DATA01** | P1 | M | Serializador determinístico em Ori para emissão da IR pela bridge | CONTRACT01 | Módulo de serialização em Ori com testes unitários | `done` |
| **SOURCE01** | P2 | S | Carregamento do grafo de arquivos e manifestos em memória | Reference Stdlib | Módulo `frontend/source.orl` isolado | `done` |
| **LEX01** | P1 | M | Lexer modular em Ori com spans exatos e recuperação de erros | SOURCE01 | Submódulo `frontend/lex/` (max 500 linhas) | `done` |
| **AST01** | P1 | L | Parser modular decomposto por constructos (`expr`, `stmt`, `item`, `pat`, `ty`) | LEX01 | Submódulo `frontend/parse/` (`node.orl`, `parse_item.orl`, golden test verde) | `done` |
| **RESOLVE01** | P2 | M | Resolução de escopo léxico, tabela de símbolos e imports | AST01 | Submódulo `frontend/resolve/` (SOA, duplicata detectada) | `done` |
| **TYPE01** | P1 | XL | Sistema de tipos puro: inferência, unificação e conformidade de traits | RESOLVE01 | Submódulo `frontend/types/` (`unify.orl`, `infer.orl`, `traits.orl`, `exhaustiveness.orl`) | `done` |
| **HIR01** | P1 | L | Lowering para HIR (desugaring, explicit ARC inc/dec) | TYPE01 | Submódulo `hir/` com verificador estático | `done` |
| **NATIVE02** | P1 | L | Emissão de IR pela Bridge e geração de binário final executável | HIR01, BRIDGE01, DATA01 | Integração completa ponta-a-ponta | `in_progress` |
| **CLI01** | P1 | M | Parser de argumentos de linha de comando e driver de pipeline | Stdlib | Submódulo `driver/` (`args.orl`, `pipeline.orl`) | `done` |
| **BOOT01** | P1 | L | Multi-stage bootstrap (Stage 0 -> Stage 1 -> Stage 2 ponto fixo) | CLI01, NATIVE02 | Script `tools/qa/test_bootstrap_stages.sh` verificado | `blocked` |
| **QUALITY01** | P1 | M | Validação completa de conformidade e integridade de pipeline | BOOT01 | Testes de unidade em cada módulo frontend/hir/bridge | `in_progress` |
| **TOOLS01** | P2 | S | Serviço de formatação de código modular em Ori | AST01 | Submódulo `tools/fmt.orl` | `done` |
| **ROLLOUT02** | P1 | M | Documentação viva completa, diário de bordo e promoção oficial | BOOT01, QUALITY01 | `docs/archive/selfhost-journal/README.md` | `blocked` |

## 4. Definição de Concluído (DoD) para cada Etapa
1. Código segue teto estrito de linhas e boas práticas de Clean Code sem `unwrap`/`panic` descontrolado.
2. Testes de unidade e regressão isolados implementados e passando.
3. Tabela de prioridade atualizada e sincronizada a cada fechamento de tarefa.
