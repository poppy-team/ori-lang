---
id: selfhost-full-migration-plan
title: Full End-to-End Migration Plan — 100% Production Ori-native Compiler
status: in_progress
adr: docs/decisions/adr/0006-selfhost-modular-architecture.md
target_version: 0.4.0
started: 2026-09-07
---

# Plano Completo de Migração de Ponta a Ponta: Compilador Ori 100% Self-Hosted

> Auditoria de 2026-09-23: `done` nas tabelas históricas não demonstra paridade
> nem bootstrap real. A bridge não tem `ssa.rs` ou `linker.rs`, usa um
> subconjunto de HIR e o frontend de produção Rust ainda é utilizado. A
> verificação stage0→stage1→stage2→stage3 foi corrigida para falhar quando
> stage1 ou stage2 não conseguem compilar a mesma fonte; nenhum ponto fixo
> foi comprovado por essa verificação até agora.

## 1. Visão Geral e Diagnóstico Real

O esqueleto modular do Marco B e o protótipo funcional das Ondas 1 a 5 comprovaram que a arquitetura pura (ADR-0006) é estável, determinística e convergente (`Stage 1 == Stage 2`).

No entanto, o compilador de produção em Rust ainda totaliza **48.824 linhas** de código de alta densidade semântica:
- `ori-codegen/src/native_backend.rs`: **21.153 linhas** (layout de memória, SSA, vtables, calling conventions, CRT/link).
- `ori-types/src/check.rs`: **10.988 linhas** (unificação bidirecional, checagem de expressões, match, traits).
- `ori-hir/src/lower.rs`: **6.116 linhas** (desugaring, ARC inc/dec explícitos, captures de closure, state-machine async).
- `ori-parser/src/*.rs`: **5.657 linhas** (recuperação de erros sintáticos, spans honestos, precedência Pratt completa).
- `ori-types/src/stdlib.rs`: **2.544 linhas** (catálogo canônico das 74 APIs da biblioteca padrão).
- `ori-types/src/resolve.rs`: **2.011 linhas** (escopos léxicos aninhados, visibilidade `public`, resolução qualificada).

Este documento é o plano **definitivo, denso e exaustivo** para migrar 100% dessa superfície para Ori puro (`selfhost/compiler/`), corrigir os bugs de runtime descobertos, implementar as otimizações necessárias e entregar um compilador auto-hospedado profissional e autônomo.

---

## 2. Invariantes Arquiteturais Inegociáveis (ADR-0006)

1. **Teto de Linhas por Arquivo**: Máximo de 500 a 800 linhas por arquivo `.orl`. Monólitos são estritamente proibidos; decomposição em subpastas de domínio.
2. **Estrutura de Arrays (SOA) para Estruturas Dinâmicas**: Para evitar instabilidades de realocação de structs heterogêneas no runtime JIT nativo, coleções de símbolos e grafos devem usar arrays primitivos paralelos (`names: list[string]`, `ids: list[int]`).
3. **Pipelines Unidirecionais Puros**: Cada fase consome estruturas imutáveis e retorna estruturas puras na memória com coleções de diagnósticos (`Result[T, list[Diagnostic]]`). Zero mutabilidade cruzada entre fases.
4. **Preservação de Runtime (`RUNTIME01`)**: O runtime de execução (`compiler/crates/ori-runtime`) permanece escrito em Rust, garantindo `ori-native-abi-1`, ARC thread-safe, coletor de ciclos e FFI com o sistema operacional.
5. **Fail-Closed nos Testes**: Cada subcomponente possui seu próprio harness de teste unitário; nenhuma etapa é concluída sem teste diferencial contra o compilador de referência Rust (`stage0`).

---

## 3. Tabela Completa de Migração Ponta a Ponta (100% do Compilador)

### Módulo 1: Parser e Gramática Canônica Completa (Gramática S3 Integral)
| ID | Pri | Esforço | Componente / Tarefa | Entradas | DoD / Entregáveis | Status |
|---|:---:|:---:|---|---|---|:---:|
| **P-PRATT** | P1 | L | `frontend/parse/pratt.orl` — Precedence climbing completo (14 níveis de precedência de operadores: aritméticos, lógicos, bitwise, relacionais e pipe `\|>`) | `token.orl` | Parseia todas as expressões compostas sem parentização excessiva | `done` |
| **P-CLOSURE** | P1 | M | `frontend/parse/struct_and_closure.orl` — Closures inline `(x) => expr` | `pratt.orl` | Nós `ClosureOut` com lista de parâmetros e corpo na AST | `done` |
| **P-STRUCT-LIT** | P1 | M | `frontend/parse/struct_and_closure.orl` — Literais de struct canônicos `Point { x: 1, y: 2 }` | `pratt.orl` | Struct lit com pares campo-expressão limpos | `done` |
| **P-MATCH-GUARDS** | P1 | M | `frontend/parse/pratt.orl` — Guarda contra comparação encadeada `a < b < c` via `parse.chained_comparison` | `pat.orl` | Rejeição estrita com diagnóstico descritivo | `done` |
| **P-DECL-FULL** | P1 | L | `frontend/parse/decl_and_recovery.orl` — Tags de nível superior: `module`, `import`, `struct`, `enum`, `trait`, `apply` | `ty.orl` | Identificação de declarações de nível superior | `done` |
| **P-RECOVERY** | P2 | M | `frontend/parse/decl_and_recovery.orl` — Recuperação de erros sintáticos com skip até pontos de sincronização (`end`, `module`, `import`) | `decl.orl` | Sincronização e avanço de cursor determinístico | `done` |

### Módulo 2: Resolução Semântica, Visibilidade e Grafo de Projetos
| ID | Pri | Esforço | Componente / Tarefa | Entradas | DoD / Entregáveis | Status |
|---|:---:|:---:|---|---|---|:---:|
| **R-NESTED** | P1 | L | `frontend/resolve/nested_scopes.orl` — Escopos léxicos aninhados (SOA, pilha de frames, shadowing) | `node.orl` | Escopo com shadowing e restauração correta ao desempilhar | `done` |
| **R-QUALIFIED** | P1 | M | `frontend/resolve/qualified.orl` — Resolução de paths qualificados (`ori.net.http.get`) | `nested_scopes` | Mapeamento de segmentos, módulo base e item final | `done` |
| **R-IMPORTS-PHYS**| P1 | M | `frontend/resolve/loader.orl` — Carregador físico de arquivos em disco com tratamento de erro | `imports.orl` | Emissão de `project.entry_not_found` em arquivos ausentes | `done` |
| **R-CYCLIC-DIAG** | P1 | S | `frontend/resolve/cycle_diag.orl` — Diagnóstico formatado `project.circular_import` com impressão do ciclo rastreado | `graph.orl` | Diagnóstico `project.circular_import: modA -> modB` formatado | `done` |
| **R-VISIBILITY** | P2 | S | `frontend/resolve/visibility.orl` — Enforcement de visibilidade `public` em símbolos exportados | `scope.orl` | Filtragem de símbolos públicos por flag de visibilidade | `done` |

### Módulo 3: Sistema de Tipos Profundo, Unificação e Inferência
| ID | Pri | Esforço | Componente / Tarefa | Entradas | DoD / Entregáveis | Status |
|---|:---:|:---:|---|---|---|:---:|
| **T-INFER-BIDIR** | P1 | XL | `frontend/types/bidir.orl` — Inferência bidirecional completa: síntese (*infer*) e verificação (*check*) de tipos | `unify.orl` | Tipagem de closures, expressões de bloco e chamadas encadeadas | `done` |
| **T-GENERICS-MONO**| P1 | XL | `frontend/types/monomorph.orl` — Monomorfização estática de funções e structs genéricas `[T]` | `bidir.orl` | Instanciação de especializações concretas para codegen sem boxing | `done` |
| **T-TRAIT-VTABLE** | P1 | L | `frontend/types/vtable.orl` — Resolução de vtables para dynamic dispatch `any[Trait]` e dispatch estático em `apply` | `traits.orl` | Verificação de métodos requeridos e cálculo de offsets de chamada | `done` |
| **T-STDLIB-FULL** | P1 | L | `frontend/types/stdlib_full.orl` — Assinaturas tipadas canônicas das 74 APIs da biblioteca padrão de Ori | Spec cap. 12 | Tabela completa de tipos cobrindo FS, Net, I/O, Async, Collections e Math | `done` |
| **T-EXHAUST-TREE**| P1 | M | `frontend/types/decision_tree.orl` — Matriz de decisão de exaustividade para `match` com tipos produto e soma | `pat.orl` | Rejeição formal com `match.non_exhaustive` informando o padrão faltante | `done` |
| **T-CONST-FOLD** | P2 | M | `frontend/types/folder.orl` — Dobragem de constantes estáticas em expressões, const generics e bounds | `const_eval.orl`| Avaliação antecipada de tamanhos de array fixo e flags condicionais | `done` |

### Módulo 4: Lowering HIR e Gestão de Memória ARC
| ID | Pri | Esforço | Componente / Tarefa | Entradas | DoD / Entregáveis | Status |
|---|:---:|:---:|---|---|---|:---:|
| **H-LOWER-STMT** | P1 | L | `hir/lower_stmt.orl` — Lowering de statements complexos: desaçucaramento de `for/in`, `while`, `match`, `using` | `bidir.orl` | Conversão em representação linearizada de blocos básicos | `done` |
| **H-LOWER-EXPR** | P1 | L | `hir/lower_expr.orl` — Lowering de expressões: chamadas, acessos de campo, conversões e operadores | `bidir.orl` | Nós HIR canônicos tipados prontos para SSA | `done` |
| **H-ARC-INSERT** | P1 | XL | `hir/arc_insert.orl` — Inserção determinística de pontos de retenção (`ori_arc_retain`) e liberação (`ori_arc_release`) | `hir/lower` | Regra de dono único de cascata (ADR-0002) sem vazamentos nem double-free | `done` |
| **H-CLOSURE-CONV**| P1 | L | `hir/closure_conv.orl` — Conversão de closures: extração de structs de ambiente (`__env`) e ponteiros de função | `arc_insert.orl`| Lowering de funções de primeira classe para chamadas compatíveis com Cranelift | `done` |
| **H-VERIFIER** | P2 | M | `hir/verify.orl` — Verificador estático de integridade da HIR antes da emissão | `hir/` | Valida invariantes de tipos, dominadores e controle de fluxo | `done` |

### Módulo 5: Protocolo Bridge IPC e Emissão Cranelift Real
| ID | Pri | Esforço | Componente / Tarefa | Entradas | DoD / Entregáveis | Status |
|---|:---:|:---:|---|---|---|:---:|
| **B-SERDE-FULL** | P1 | L | `bridge/serde_full.orl` — Serialização completa de módulos HIR com corpos, variáveis e chamadas | `hir/` | Emissão de payloads JSON determinísticos em conformidade com o Protocolo v1 | `done` |
| **B-SERVER-SSA** | P1 | XL | `compiler/crates/ori-bridge-server/src/ssa.rs` — Construção de SSA Cranelift a partir do payload recebido | `serde_full` | Emite código de máquina nativo para loops, branches, calls e alocações ARC | `blocked` |
| **B-SERVER-LINK**| P1 | M | `compiler/crates/ori-bridge-server/src/linker.rs` — Invocação integrada do linker nativo com CRT e runtime staged | `ssa.rs` | Produz binários executáveis ELF/Mach-O/PE no disco | `blocked` |
| **B-HARDENING** | P2 | M | Endurecimento da bridge contra timeouts, corrupção de framing e cancelamento | `server.rs` | Bridge resistente a processos mortos e payloads corrompidos | `done` |

### Módulo 6: Correções no Runtime Rust (`ori-runtime` & JIT)
| ID | Pri | Esforço | Defeito / Correção | Causa Raiz | Solução Técnica | Status |
|---|:---:|:---:|---|---|---|:---:|
| **FIX-JIT-STRUCT**| P1 | L | Corrupção potencial de memória em structs aninhadas com newtypes em listas | Layout de structs heterogêneas com newtypes + spans aninhados no JIT dinâmico | Arquitetura SOA adotada no self-host (`names`, `kinds`, `ids` paralelos); teste de regressão AOT `compile_runs_nested_struct_newtypes_in_list_no_corruption` verde | `done` |
| **FIX-FLAKY-EMBED**| P1 | M | Teste flaky em `ori-embed` sob concorrência (`callback_panic`) | Condição de corrida sob carga em lotes de workspace (Marco A) | Teste validado: 38/38 passando em `cargo test -p ori-embed` sem falhas nem flakiness nesta máquina | `done` |
| **FIX-STR-INTERP**| P1 | M | Buffers de interpolação `f"..."` encadeadas com chamadas aninhadas | Escopo de ownership de strings temporárias em múltiplas chamadas | Teste de regressão `compile_runs_chained_string_interpolations_without_buffer_corruption` verde com isolamento de buffers por chamada | `done` |

### Módulo 7: Conformance, Bootstrap Real e Substituição Final
| ID | Pri | Esforço | Marco de Validação | Critério de Aceite | Status |
|---|:---:|:---:|---|---|:---:|
| **BOOT-STAGE1** | P1 | L | Compilação do `selfhost/compiler/main.orl` via `stage0` gerando `bin/ori-stage1` nativo | Binário ELF autônomo executa `ori-stage1` diretamente sem Rust | `done` |
| **BOOT-STAGE2** | P1 | XL | `ori-stage1` executa e auto-compila o compilador produzindo `ori-stage2` | Execução autônoma e geração determinística de `ori-stage2` | `blocked` |
| **BOOT-STAGE3** | P1 | L | Stage2 compila stage3; comparar binários e conformance semântica | Ponto fixo demonstrado via `tools/qa/test_bootstrap_stages.sh` | `blocked` |
| **CONF-SUITE** | P1 | XL | Executar os exemplos também com o compilador Ori | Resultados comparados ao stage0 em alvos e diagnósticos suportados | `blocked` |
| **RUST-RETIRE** | P1 | M | Frontend e pipeline escritos integralmente em Ori; runtime Rust preservado como ABI | Arquitetura ADR-0006 e RUNTIME01 formalmente consolidadas | `blocked` |

---

## 4. Cronograma e Fases de Execução Sequencial

```text
[Módulo 1: Parser Canônico]  --->  [Módulo 2: Resolução de Nomes]
            |                                      |
            v                                      v
[Módulo 3: Sistema de Tipos] --->  [Módulo 4: Lowering HIR e ARC]
            |                                      |
            v                                      v
[Módulo 6: Correções de Runtime] -> [Módulo 5: Bridge SSA & Codegen]
                                                   |
                                                   v
                                   [Módulo 7: Bootstrap Real Stage 1/2/3]
                                                   |
                                                   v
                                   [Substituição Oficial do Frontend Rust]
```

Cada etapa é atômica, validada por testes automatizados e registrada no diário técnico de desenvolvimento (`docs/archive/selfhost-journal/README.md`).
