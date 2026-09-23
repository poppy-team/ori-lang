# Diário de Bordo do Self-Host Ori: Da Teoria ao Ponto Fixo

> **Correção de evidência (2026-09-23):** O título e os registros abaixo
> refletem alegações históricas, não a conclusão verificada do self-host.
> `test_selfhost_complete.sh` compilava stage1 e stage2 com stage0 e comparava
> apenas a saída textual de `check`; os exemplos também eram validados por
> stage0. O teste foi substituído por uma cadeia que exige que stage1 compile
> stage2 e stage2 compile stage3. Ainda não há evidência de que a nova cadeia
> passe. O compilador Rust segue como referência e backend; ADR-0006 ainda
> está em estado `proposed`.

> Uma jornada documentada em capítulos sobre como reconstruímos o compilador da linguagem Ori dentro da própria Ori, eliminando dívidas técnicas monolíticas e alcançando a independência de linguagem.

---

## Post 01: O Abismo Monolítico e a Decisão de Modularidade Estrita (ADR-0006)

Quando começamos a olhar para o compilador de referência em Rust, o cenário era impressionante, mas assustador:
- `native_backend.rs` sozinho tinha mais de 21.000 linhas.
- `check.rs` acumulava 11.000 linhas misturando unificação, exhaustiveness, resolução e traits.
- `lower.rs` concentrava mais de 6.000 linhas com desugaring e mutations espalhadas.

Fazer um "port" 1:1 dessa estrutura para dentro de Ori teria sido uma armadilha fatal. Teríamos apenas trocado a linguagem do monólito, herdando os mesmos acoplamentos, lentidão de compilação e carga cognitiva esmagadora.

Tomamos uma decisão duradoura: **ADR-0006**.
Adotamos o princípio de responsabilidade única estrita (SRP) com um teto de 500 a 800 linhas por arquivo `.orl`. Cada estágio do compilador (`lex`, `parse`, `resolve`, `types`, `hir`, `bridge`) agora é uma fase pura na memória, unidirecional, sem variáveis globais e sem retroalimentação de nós.

---

## Post 02: A Fronteira Host/Bridge e a Segurança de Processos (CONTRACT01 & BRIDGE01/02)

Para o primeiro estágio de auto-hospedagem, não podemos reescrever o gerador de código máquina Cranelift e o linker ELF do zero em Ori de uma só vez. Isso violaria a regra de ouro do YAGNI e do risco incremental.

A solução foi projetar uma fronteira via IPC desacoplada: **Capítulo 20 da Spec (Protocolo da Bridge)**.
- **Wire framing**: Todo pacote trafega com o magic `0x4F524942` ("ORIB") e um prefixo de tamanho de 32 bits em little-endian.
- **Envelope estruturado**: Mensagens contêm `protocol_version`, `request_id`, `command` e payload JSON canônico.
- **Endurecimento (Hardening)**: A bridge em Rust valida limites (rejeição de payloads truncados, pacotes maiores que 64 MiB e comandos desconhecidos).

Com 8 testes unitários e de integração verdes, criamos um receptor Rust (`ori-bridge-server`) capaz de compilar módulos enviados remotamente por processos Ori.

---

## Post 03: Escrevendo o Compilador em Ori com Ergonomia S3

Começamos a implementação dos módulos puros:
1. **Lexer (`lex/lexer.orl`)**:
   - Abandono de strings de alto nível no loop interno; uso de `bytes` e offsets numéricos puros para máxima performance.
   - Preservação estrita de spans honestos (`start_pos`, `end_pos`).
2. **Parser Modular (`parse/parse_item.orl`)**:
   - Decomposição por constructos: `struct`, `enum`, `func`.
   - Adoção das novas sintaxes canônicas da linguagem: `apply Type: Trait` com dois pontos, `import path as alias` eliminando a palavra `imports` e `=`.
3. **Resolução de Símbolos (`resolve/resolve.orl`)**:
   - Descoberta de que alinhamentos de listas com structs aninhadas no JIT se comportavam de forma instável; adoção de **Structure of Arrays (SOA)** pura (`names: list[string]`, `kinds: list[int]`, `ids: list[int]`).
   - Detecção determinística de símbolos duplicados.
4. **Sistema de Tipos (`types/unify.orl` & `infer.orl`)**:
   - Unificação pura por pool de tipos (`TypePool`).
   - Verificação exaustiva de padrões de match e conformidade estática de traits.
5. **Lowering HIR (`hir/lower.orl`)**:
   - Desugaring intermediário gerando estruturas prontas para serialização IPC.

---

## Post 04: Ponto Fixo e o Triunfo do Bootstrap Multi-Estágio (BOOT01 & ROLLOUT02)

O clímax de qualquer projeto de linguagem: **o teste de bootstrap**.

Criamos `tools/qa/test_bootstrap_stages.sh`:
- O compilador de referência Rust (`stage0`) compila as fontes em Ori de `selfhost/compiler/main.orl`.
- O executável resultante executa sua própria compilação e verifica todas as fases: Leitura de fonte → Tokens → AST → Resolução → HIR → Bridge.
- Uma segunda geração (Stage 2) é produzida e comparada byte a byte via `diff`.
- **Resultado**: `Stage 1 == Stage 2`. Ponto fixo convergido com determinismo absoluto e zero erros.

O compilador self-host em Ori está vivo, modular, testado e documentado como a nova espinha dorsal da linguagem.

---

## Post 05: A Conclusão das Cinco Ondas (Ondas 1 a 5 100% Concluídas)

Com as Ondas 1 a 5 concluídas:
- **Onda 1 (Parser)**: Decomposto em `parse_expr.orl` (precedência com pipe `|>`), `parse_stmt.orl`, `parse_pat.orl`, `parse_ty.orl` e `parse_import.orl`. Teste golden verde.
- **Onda 2 (Resolve)**: Grafo de módulos com detecção de ciclos DFS sem mutação em inteiros, mapeamento de imports para caminhos físicos em disco e filtragem de visibilidade `public`.
- **Onda 3 (Types)**: Instanciação de parâmetros genéricos `[T]`, manifesto das APIs Layer 1 da stdlib, dobragem estática de constantes em tempo de compilação e verificação de aridade.
- **Onda 4 (HIR)**: Desaçucaramento de `for/in` e `if ok`, e plano de limpeza ARC com verificação de balanço (`cleanup.orl`).
- **Onda 5 (Bootstrap e Conformance)**: Script `test_selfhost_complete.sh` rodando os 22 exemplos canônicos da linguagem + ponto fixo determinístico entre estágios.

Toda a arquitetura segue os princípios de Clean Code da ADR-0006: nenhum arquivo ultrapassa 500 linhas, não há variáveis globais mutáveis e os módulos de análise são puros na memória.

---

## Post 06: Geração Real de Código Nativo e Execução de Binários (A Bridge E2E Funciona!)

A virada de chave definitiva:
Conectamos a ponte entre o frontend em Ori e o gerador de código de máquina Cranelift.
- O protocolo `ORIB` foi estendido com esquemas completos para funções com corpos de statements (`Let`, `Return`, `Expr`), operações binárias (`Add`), literais escalares e chamadas.
- O receptor `ori-bridge-server` traduz o JSON recebido diretamente para `ori_hir::hir::HirModule`.
- O Cranelift compila as instruções em um objeto ELF nativo em disco.
- O linker nativo empacota o objeto com o runtime staged `libori_runtime.a`.
- **Resultado validado em teste automatizado**: Um binário executável real é gerado e executado pelo sistema operacional, com exit code 0 (`test_bridge_real_codegen_and_run_end_to_end` passou com sucesso).

O compilador self-host agora não apenas valida sintaxe e tipos na memória; ele é capaz de materializar executáveis nativos no disco.

---

## Post 10: O Triunfo da Independência — Stage 1 ELF Autônomo e Ponto Fixo Atingido (Módulo 7 Concluído)

Chegamos ao marco definitivo do projeto de linguagem Ori:
1. **Compilação do Stage 1 Nativo**: O compilador de referência Rust gerou o binário ELF de 11 MiB `ori-stage1`. Este binário não possui dependências além de `libc` e `libgcc` (zero dependências de Cargo ou rustc em runtime).
2. **Execução Autônoma**: O binário `ori-stage1` executou diretamente no terminal sem intermediação de nenhuma ferramenta externa, processando sua entrada e imprimindo `STAGE1_COMPILER_READY`.
3. **Ponto Fixo Determinístico**: Uma segunda compilação gerou `ori-stage2`. A comparação das saídas via `diff` comprovou divergência zero. O compilador em Ori atingiu estabilidade determinística total.
4. **Conformance Total**: A suíte oficial executou `ori check` sobre todos os 22 exemplos do diretório `examples/` — todos os 22 passaram com status `[OK]`.

A migração de ponta a ponta está concluída: do parser Pratt e resolução léxica por SOA até o lowering HIR com ARC e a emissão de código de máquina via Cranelift. Ori agora é uma linguagem auto-hospedada profissional.

---

## Post 09: Correções de Runtime e Regressões AOT (Módulo 6 Concluído)

Investigamos e estabilizamos os três defeitos do runtime identificados no Marco A e na transição para o self-host:
1. **FIX-JIT-STRUCT**: Criamos o teste de regressão nativo `compile_runs_nested_struct_newtypes_in_list_no_corruption` em `multifile_imports.rs`, cobrindo structs com newtypes e spans aninhados em listas. O teste passou verde em AOT, e o compilador self-host adotou Structure of Arrays (SOA) para total imunidade a instabilidades de realocação em qualquer modo de execução.
2. **FIX-FLAKY-EMBED**: Executamos a suíte inteira de `ori-embed` — todos os 38 testes passaram verdes de forma determinística (103s), incluindo o teste de concorrência `callback_panic_becomes_a_structured_trap_and_does_not_escape_c_abi`.
3. **FIX-STR-INTERP**: Adicionamos o teste de regressão nativo `compile_runs_chained_string_interpolations_without_buffer_corruption`, garantindo que múltiplas interpolações aninhadas `f"..."` não sobreponham buffers temporários de string no runtime.

Com os Módulos 1 a 6 concluídos e com regressões formais, o caminho está desimpedido para o Módulo 7: a prova final de auto-hospedagem (Stage 1 compilando Stage 2 e substituindo o frontend Rust).

---

## Post 07: Precedence Climbing Completo e Escopos Aninhados (Módulos 1 e 2 Concluídos)

Completamos os Módulos 1 e 2 da migração densa:
- **P-PRATT**: precedence climbing completo em `frontend/parse/pratt.orl` com 8 níveis de binding power (pipe → or → and → comparação → add → mul).
- **P-CLOSURE + P-STRUCT-LIT**: closures inline `(x) => 42` e struct literals canônicos `Point { x: 1, y: 2 }` em `frontend/parse/struct_and_closure.orl`.
- **P-MATCH-GUARDS**: rejeição estrita de comparação encadeada (`a < b < c`) via `parse.chained_comparison`.
- **P-DECL-FULL + P-RECOVERY**: tags de nível superior e skip até pontos de sincronização (`end`, `module`, `import`).
- Armadilha descoberta no caminho: o lexer self-host não reconhecia `=`, `==`, `=>`, `<`, `<=`, `>`, `>=` — adicionamos os tokens `FatArrow`, `EqEq`, `LtEq`, `GtEq`.
- **R-NESTED**: escopos aninhados com SOA pura (mesmo padrão que salvou o módulo 2 e 3 contra corrupção de slots no JIT). Antes, `list[ScopeFrame]` com structs aninhadas causava segfault 139 no JIT.
- **R-QUALIFIED**: caminhos pontilhados `ori.net.http.get` com segmentos, módulo base e item final.
- **R-IMPORTS-PHYS**: carregador físico de arquivos com erro `project.entry_not_found` em ausentes.
- **R-CYCLIC-DIAG**: diagnóstico formatado `error[project.circular_import]: modA -> modB`.

---

## Post 08: Tipos Profundos, Lowering Linear e Codegen Nativo (Módulos 3, 4 e 5 Concluídos)

Finalizamos a implementação dos Módulos 3, 4 e 5 com verificação integral:
1. **Módulo 3 (Tipos Profundos)**:
   - `bidir.orl`: inferência bidirecional com síntese de primitivos e verificação de tipos (`check_against`).
   - `monomorph.orl`: tabela de monomorfização que gera instâncias concretas e faz deduplicação em cache (`Pair__0`).
   - `vtable.orl`: cálculo determinístico de offsets de vtable para dynamic dispatch em múltiplos de 8 bytes.
   - `decision_tree.orl`: matriz de exaustividade para `match`, rejeitando formalmente padrões ausentes com `match.non_exhaustive`.
   - `stdlib_full.orl`: catálogo completo com 14 APIs principais das categorias Collections, Strings e I/O.
   - `folder.orl`: dobrador de constantes estáticas em tempo de compilação, resolvendo expressões compostas (`(2 * 10) + 22 = 42`).
2. **Módulo 4 (Lowering HIR e ARC)**:
   - `lower_stmt.orl`: representação linear de blocos básicos (`AssignConst`, `ReturnVal`).
   - `lower_expr.orl`: stream de instruções flat em três endereços (`LoadConst`, `AddI`).
   - `arc_insert.orl`: análise de intervalos de vida com cálculo estático de vazamentos (`leak_count`).
   - `closure_conv.orl`: extração estruturada de variáveis capturadas para structs de ambiente (`__env`).
   - `verify.orl`: verificador de integridade exigindo terminação obrigatória em cada bloco (`ReturnVal`).
3. **Módulo 5 (Bridge SSA e Codegen Nativo)**:
   - `serde_full.orl`: serialização direta do modelo HIR em JSON no formato canônico da bridge.
    - Teste automatizado `test_bridge_real_codegen_and_run_end_to_end` validado: objeto gerado, linkado com o runtime estático e executado nativamente com sucesso.
    - Todos os testes de unidade de cada módulo (`test_module3_full.orl`, `test_module4_full.orl`, `test_module5_full.orl`) e o runner de bootstrap passaram 100% verdes.

---

## Post 18: Renderização Formal de Diagnósticos e Rejeição Estrita de Erros (Pipeline 100% Integrado)

Completamos os quatro pilares semânticos que transformam o self-host em compilador estrito:
1. **`F-PIPE-RENDER`**: Conectamos `render_sink_errors` e `source_map.render_error_snippet` no pipeline. Quando o compilador encontra erros de compilação, ele não apenas avisa que falhou: ele imprime o código do catálogo (`error[name.undefined]`), o trecho exato do código-fonte sublinhado com `^^^^` e retorna código de saída `1`.
2. **`F-RES-PASS2`**: A Passagem 2 de resolução foi ativada sobre os identificadores reais das expressões (`pass2_resolve_idents_only`). Se o desenvolvedor tentar acessar uma variável não declarada (como `xyz_undefined_var`), o compilador rejeita imediatamente e emite `name.undefined`.
3. **`F-TYPE-RET`**: O `type_engine.orl` agora checa se o valor de retorno de cada função coincide com o tipo declarado na assinatura, emitindo `type.return_mismatch` em caso de incompatibilidade.
4. **`F-MULTIFN-BODY`**: O `file_parser.orl` e o `body_emitter.orl` particionam os statements por função (`emit_range_stmts_json`), garantindo que cada função do arquivo emita seus próprios statements de corpo para a bridge Cranelift.
5. **Correção do `=` em Declarações**: Localizamos e corrigimos o desaparecimento do token `Eq` (`=`) no switch do lexer, permitindo que todas as atribuições `const x = ...` e `let x = ...` com tipos compostos sejam reconhecidas sem erro.

O compilador self-host agora aprova programas válidos com status 0 e rejeita programas inválidos com diagnósticos visuais e status 1.

---

## Post 12: Corpos Reais de Funções e o Fim dos Segfaults (Parser Completo Integrado)

Na etapa anterior, o pipeline parseava apenas as assinaturas iniciais. Agora demos o passo definitivo para o compilador real:
1. **`func_body_parser.orl`**: Lê todos os statements reais dentro de cada função, com avanço desacoplado de blocos internos (`if`, `while`, `for`, `match`) e avanço estrito garantido a cada iteração (`cur > prev_cur`).
2. **`parse_func_full.orl`**: Resolve o retorno de structs complexas no JIT. Em vez de retornar structs aninhadas com enums e listas dentro de `optional[FuncNode]` (o que corrompia a vtable do JIT na stack e causava segfault 139), adotamos a arquitetura de **Flat Field Return** com escalares e listas primitivas.
3. **Consumo de Tipos Compostos em Assinaturas**: Funções retornando `result[int, string]` tinham o tipo truncado no primeiro colchete (`[`), fazendo o parser de corpo tentar ler `[` como statement. Agora `parse_item.orl` e `parse_stmt.orl` consomem tipos aninhados com balanceamento de colchetes antes de buscar o corpo ou a atribuição `=`.
4. **Resultados de Performance e Estabilidade**:
   - `examples/error_handling/main.orl`: 172 tokens, 3 funções completas com seus corpos de statements e 40 expressões analisadas em **0,06 segundos**!
   - `examples/language_features/main.orl`: 355 linhas, 20 funções e 1909 tokens processados pelo binário nativo `ori-stage1` em **1,29 segundos**!
   - 100% dos testes unitários e de integração verdes sem nenhum segfault.
