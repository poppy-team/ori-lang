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
> semânticos serem avaliados. A bridge v1 só representa um subconjunto da HIR.

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
