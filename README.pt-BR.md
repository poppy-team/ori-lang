<p align="center">
  <img src="branding/ori-logo-w_text.svg" alt="Ori" width="280">
</p>

# Ori

**Superfície S3/0.4:** sintaxe inspirada na Auk9 sobre o motor Ori. Propósito (estudo, IA, legibilidade ND — **não** competição de mercado): [manifesto](docs/spec/00-manifesto.md). Auk9 lab **aposentada como produto**.

Ori é uma linguagem de programação compilada para código nativo, com tipagem
explícita e foco em leitura. O compilador é escrito em Rust e foi criado com um
objetivo direto: tornar programas mais fáceis de ler, inspecionar, diagnosticar
e manter.

Ori ainda é pre-1.0. O projeto já é útil para trabalho em compilador, design de
linguagem, ferramentas e runtime, mas a linguagem ainda pode mudar antes de um
contrato estável 1.0.

**Idiomas:** [English](README.md) (primário no GitHub) | Português | [日本語](README.ja.md)

**Documentação:** [Índice de docs](docs/README.pt-BR.md) · [Instalação](docs/install.pt-BR.md) ·
[Tour da linguagem](docs/language/tour.pt-BR.md) · [Guias](docs/guides/README.md) ·
[Performance](docs/guides/performance.pt-BR.md) ·
[Especificação](docs/spec/README.md) (normativa em inglês) ·
[Planejamento](docs/planning/README.md)

**Também:** [Manifesto](docs/spec/00-manifesto.md) · [Stdlib](stdlib/README.md) ·
[Runtime](runtime/README.md) · [Exemplos](examples/) · [Changelog](CHANGELOG.md) ·
[Contribuição](CONTRIBUTING.md)

## Conteúdo

- [O que é Ori](#o-que-é-ori)
- [Por que Ori existe](#por-que-ori-existe)
- [Status atual](#status-atual)
- [Snapshot de performance](#snapshot-de-performance)
- [Primeiros passos](#primeiros-passos)
- [Primeiro programa](#primeiro-programa)
- [Visão geral da CLI](#visão-geral-da-cli)
- [Visão geral da linguagem](#visão-geral-da-linguagem)
- [Arquitetura do compilador](#arquitetura-do-compilador)
- [Biblioteca padrão](#biblioteca-padrão)
- [Ferramentas de editor](#ferramentas-de-editor)
- [Layout do repositório](#layout-do-repositório)
- [Fluxo de desenvolvimento](#fluxo-de-desenvolvimento)
- [Layout de release](#layout-de-release)
- [Limitações conhecidas](#limitações-conhecidas)
- [Roadmap](#roadmap)
- [Licença](#licença)

## O que é Ori

Ori é uma linguagem estaticamente tipada com módulos explícitos (`module`),
tipos explícitos (`optional[T]`, `result[T, E]`), erros estruturados (`try`),
traits via `apply`/`use`, limpeza determinística (`using`) e geração de código
nativo.

Pipeline atual do compilador:

```text
fonte .orl
  -> lexer
  -> parser
  -> resolvedor de nomes
  -> type checker
  -> HIR
  -> backend nativo Cranelift
  -> binário com runtime ou execução JIT
```

O repositório contém compilador, runtime, fontes da biblioteca padrão,
especificação da linguagem, extensão VS Code, exemplos e ferramentas de release.

## Por que Ori existe

Ori otimiza leitura antes de escrita.

O código deve deixar informações importantes visíveis no ponto em que o leitor
precisa delas:

| Pergunta | Ori deixa visível por meio de |
|---|---|
| Onde este arquivo pertence? | `module` no topo de cada arquivo |
| Qual é o tipo deste valor? | anotações de tipo explícitas |
| Este valor pode estar ausente? | `optional[T]` |
| Esta operação pode falhar? | `result[T, E]` |
| Quando um recurso é liberado? | `using` |
| De onde vem este comportamento? | `trait` e `apply` / `use` |
| O que deu errado? | códigos de diagnóstico estruturados |

Esse design reduz carga cognitiva: menos regras escondidas, cadeias de
inferência menores e mensagens de erro mais claras.

## Status atual

| Área | Status |
|---|---|
| Versão | **S3/0.4** · inferência B **`0.3.1`** · workspace **`0.3.8-dev`** (release mais recente `v0.3.8`) |
| Estabilidade | pre-1.0; S3 quebra sintaxe pré-0.3 |
| Compilador | workspace Rust em `compiler/` |
| Backend nativo | Cranelift AOT + `ori-runtime`; ABI `ori-native-abi-1` |
| `ori run` | JIT por padrão com cdylib |
| `ori compile` / `ori test` | AOT; `rust-lld` empacotado primeiro, depois linker do SO |
| Biblioteca padrão | Layer 1 + `.orl`; API canônica `ori.X` |
| Docs | inglês primário + português paralelo · [examples/](examples/) |
| Foco agora | Linguagem, docs/exemplos, performance — não marketing multi-OS |
| Editores | VS Code + Zed **locais** (sem loja) |

> **Status da release de desenvolvimento:** a auditoria de 2026-08-24
> encontrou defeitos P0/P1 de semântica, memória, empacotamento e ciclo de vida
> no baseline `0.3.8-dev`. A onda atual fechou canais limitados, o pool
> compartilhado de I/O bloqueante, attributes fail-closed e o construtor de
> handle nulo tipado. Ainda faltam na linguagem: isolamento completo de tasks,
> chaves recursivas de collections, lifetime/afinidade de handles, prova total
> de ownership async no HIR e a matriz FFI hostil em várias plataformas.
> Ordem e evidências: [auditoria de implementação](docs/planning/roadmap-code-audit-performance-architecture.md).

S3: [CHANGELOG.md](CHANGELOG.md) `[0.3.0]`. Inferência: `[0.3.1]`. Package sem
Rust: `[0.3.2]`. Migrar: `ori migrate-syntax`.

## Snapshot de performance

Microbench polyglot local de **Ori AOT** contra Python, Rust, C, Go, JavaScript e Ruby
(setembro de 2026, Linux x86_64, mediana de **3** runs — suíte expandida com 8 workloads). Texto completo e ressalvas:
**[docs/guides/performance.pt-BR.md](docs/guides/performance.pt-BR.md)**
([EN](docs/guides/performance.md)).

| Workload | Ori | Python | Rust | C | Go | JS (Node) | Ruby |
|---|---|---|---|---|---|---|---|
| `sum_loop` 10⁷ passos | **0.003 s**\* | 2.558 s | 0.003 s\* | 0.003 s\* | 0.021 s | 0.066 s | 0.351 s |
| `fib_iter` 2·10⁷ passos | **0.037 s** | 8.486 s | 0.021 s | 0.028 s | 0.041 s | 2.009 s | 9.670 s |
| `list_sum` 10⁶ push+soma | **0.009 s** | 0.467 s | 0.008 s | 0.009 s | 0.010 s | 0.097 s | 0.178 s |
| `nested` 2000² passos | **0.002 s**\* | 1.588 s | 0.002 s | 0.002 s | 0.004 s | 0.058 s | 0.239 s |
| `vec4_simd` 5·10⁶ somas 4D | **0.008 s** | 1.878 s | 0.007 s | 0.007 s | 0.010 s | 0.087 s | 1.496 s |
| `arena_bulk_alloc` 10⁵ resets | **0.015 s** | 0.032 s | 0.002 s | 0.001 s | 0.003 s | 0.047 s | 0.089 s |
| `channel_throughput` 10⁵ msgs | **0.114 s** | 0.049 s | 0.006 s | 0.001 s | 0.010 s | 0.060 s | 0.103 s |
| `spatial_grid_bvh` 10⁶ AABB | **0.543 s** | 0.745 s | 0.003 s | 0.001 s | 0.004 s | 0.060 s | 0.512 s |

\* `sum_loop` e `nested` podem ir para forma fechada (mid-end Default da Ori; Rust/C também). Prefira **`fib_iter`**,
**`list_sum`** e **`vec4_simd`** para comparações de velocidade de execução.

**Leitura (pre-1.0):**
- **Vetorização SIMD:** Ori baixa `simd[float32, 4]` diretamente para registradores vetoriais Cranelift `F32x4`, completando 5M somas vetoriais em **8.1 ms** (≈1.1× GCC -O2, ≈1.15× Rust, superando Go e **~230× mais rápido que Python**).
- **Loops escalares e iterativos:** Ori completa 20M passos de fib em **37 ms**, próximo a C e Rust, e mais rápido que Go e interpretadores dinâmicos.
- **Listas e coleções:** 1M de inteiros empilhados e somados em **9.3 ms**, empatando com C e Rust.

Reproduzir:

```bash
SAMPLES=3 ./tools/bench/polyglot/run_polyglot_bench.sh
```

## Primeiros passos

### Usuários finais (sem Rust)

Guia completo: [docs/install.pt-BR.md](docs/install.pt-BR.md) · [EN](docs/install.md).

**Windows** (estilo Scoop — instala em `%LOCALAPPDATA%\Programs\Ori` + PATH do usuário):

```powershell
Set-ExecutionPolicy -ExecutionPolicy RemoteSigned -Scope CurrentUser   # opcional, uma vez
irm https://raw.githubusercontent.com/raillen/ori-lang/main/tools/windows/get.ps1 | iex
ori --version
ori doctor
```

**Linux** — baixe em [Releases](https://github.com/raillen/ori-lang/releases), ou:

```bash
tar -xzf ori-v0.3.5-x86_64-unknown-linux-gnu.tar.gz
export PATH="$PWD/ori-v0.3.5-x86_64-unknown-linux-gnu:$PATH"
# ou: sudo dpkg -i ori_0.3.5_amd64.deb
ori doctor
```

AOT (`ori compile`) ainda precisa do linker do SO. `ori run` (JIT) só do runtime.

### Desenvolvimento do compilador

Pré-requisitos:

- Rust `1.95.0`, definido em `rust-toolchain.toml`
- Um linker da plataforma ou uma das estratégias explícitas de link do Ori
- PowerShell no Windows para scripts de smoke de release
- Toolchain C no Linux/macOS quando usar rotas de descoberta do sistema

O workspace Cargo fica em `compiler/` (ver
`docs/planning/repo-and-project-layout.md`):

```bash
cd compiler
cargo check --workspace
cargo test --workspace
cargo run -p ori-driver -- check ../examples/hello
cargo run -p ori-driver -- run ../examples/hello
```

No Windows, valide um pacote em formato de release com:

```powershell
.\tools\smoke_native_release.ps1
```

No Linux ou macOS:

```sh
sh tools/smoke_native_release.sh
```

## Primeiro programa

```ori
module app.hello

import ori.io = io

main()
    io.print("Hello, Ori!")

    const answer: int = 21 * 2
    io.print(f"The answer is {answer}")
end
```

Execute pelo repositório:

```bash
cargo run -p ori-driver -- run ../examples/hello
```

Ori usa blocos delimitados por `end`, declarações separadas por linha, imports
explícitos e tipos explícitos em bindings e contratos públicos.

## Visão geral da CLI

A CLI `ori` é implementada em `compiler/crates/ori-driver`.

| Comando | Função |
|---|---|
| `ori check <file.orl>` | faz parse, resolução e checagem de tipos |
| `ori run <file.orl>` | compila e executa por JIT ou AOT, conforme runtime e variáveis de ambiente |
| `ori compile <file.orl>` | emite executável nativo pelo backend Cranelift |
| `ori test <file.orl>` | executa funções marcadas com `@test` |
| `ori fmt <file.orl>` | formata fonte e imprime o resultado |
| `ori doc file <file.orl>` | extrai comentários de documentação como Markdown ou HTML |
| `ori doc export` | exporta símbolos stdlib, diagnósticos e keywords como JSON |
| `ori doctor` | reporta saúde da stdlib, runtime, linker, target e JIT |
| `ori explain <code>` | explica um código de diagnóstico |
| `ori summary [path]` | imprime entry file, módulos, imports e contagem de diagnósticos |
| `ori debug <file.orl>` | executa o debugger cooperativo; `--dap` serve DAP via stdio |
| `ori build <file.orl>` | compila pelo backend nativo |
| `ori lex <file.orl>` | imprime tokens para debug do compilador |
| `ori parse <file.orl>` | imprime AST para debug do compilador |
| `ori install <name> --path <dir>` | instala pacote local no cache |
| `ori install name[@ver]` | instala de `ORI_REGISTRY` |
| `ori install github.com/org/repo` | clona e instala um pacote Git no cache |
| `ori get [path]` | busca deps git/path do manifesto |
| `ori lock [path]` | resolve dependências e grava um snapshot em `ori.lock` (`--locked` detecta divergência da resolução atual; digests de conteúdo e restauração dirigida pelo lock continuam abertos em `AUD-PKG-2`) |
| `ori publish <path>` | publica em `ORI_REGISTRY` (árvore de arquivos ou HTTP) |
| `ori migrate-syntax <paths…>` | reescreve sintaxe pré-S3 → S3 |

O consumo remoto de packages é experimental. `AUD-PKG-1/2` agora impõem digest,
contenção da extração e identidade de fontes no lock; use HTTPS e confira a
origem do registry. Testes herméticos do registry HTTP e estresse de release
multiplataforma continuam como QA P2.

Variáveis úteis:

| Variável | Função |
|---|---|
| `ORI_STDLIB_ROOT` | sobrescreve a raiz `stdlib/` |
| `ORI_RUNTIME_LIB` | sobrescreve a biblioteca estática do runtime nativo |
| `ORI_RUNTIME_CDYLIB` | sobrescreve a cdylib do runtime usada pelo JIT |
| `ORI_USE_JIT=1` | força JIT em `ori run` |
| `ORI_USE_AOT=1` | força AOT em `ori run` |
| `ORI_USE_BUNDLED_RUST_LLD=1` | linka por `rust-lld` empacotado sem o driver `rustc` |
| `ORI_USE_SYSTEM_LINKER=1` | linka diretamente pelo linker da plataforma |
| `ORI_DISABLE_INCREMENTAL=1` | desativa a reutilização de saídas nativas em `.ori/incremental.json` |
| `ORI_OBJCOPY` | escolhe o executável `objcopy`/`llvm-objcopy` usado para as seções DWARF no Linux |
| `ORI_REQUIRE_PACKAGED_RUNTIME=1` | rejeita fallback para runtime do workspace durante validação de pacote |

A matriz completa de ambiente está em [AGENTS.md](AGENTS.md).

Os pacotes mantêm namespaces isolados. Imports locais ficam dentro do pacote
que os declara; módulos de dependências usam o prefixo qualificado do pacote,
como `math.format`. Assim, duas dependências podem ter um módulo `format` sem
colisão. `ori lock` registra um snapshot do caminho, da versão do registry ou da
revisão Git resolvida. O resolver atual ainda resolve as fontes novamente antes
de validar o lock; restauração dirigida pelo lock e digests de conteúdo estão
abertos em `AUD-PKG-2`.

## Visão geral da linguagem

O modelo central de Ori é pequeno:

- todo arquivo começa com `module`;
- imports: `import path (A)`, `import path = alias`, ou `import path` nu;
- declarações top-level são privadas, exceto quando marcadas como `public`;
- `struct` e `enum` definem dados;
- `trait` e `apply` / `use` definem comportamento;
- `optional[T]` modela ausência;
- `result[T, E]` modela falha recuperável;
- só `try expr` propaga (postfix `?` removido no S3);
- `using` deixa limpeza determinística explícita (ex: `using r: mem.Region = mem.region()`);
- recursos de sistemas de alta performance: checagem estática `@noalloc`, alinhamento explícito `@align(N)`, arenas de bump allocation (`mem.region`) e vetores SIMD portáteis de largura fixa (`simd[float32, 4]`);
- dependências nativas e integração declarativa com `pkg-config` via `[native.dependencies]` no `ori.pkg.toml`;
- diagnósticos usam códigos estáveis como `name.undefined` e `perf.allocation_in_noalloc`.

Exemplo com `result`:

```ori
module app.errors

import ori.io = io

divide(a: int, b: int) -> result[int, string]
    if b == 0
        return err("division by zero")
    end

    return ok(a / b)
end

main() -> result[void, string]
    const value: int = try divide(84, 2)
    io.print(f"value: {value}")
    return ok()
end
```

Para o contrato normativo da linguagem, comece por
[docs/spec/01-overview.md](docs/spec/01-overview.md).

## Arquitetura do compilador

O compilador é dividido em crates focadas:

| Crate | Papel |
|---|---|
| `ori-lexer` | tokenização |
| `ori-ast` | definições dos nós da AST |
| `ori-parser` | parser recursive descent |
| `ori-hir` | resolução de nomes e HIR |
| `ori-types` | sistema de tipos, manifesto stdlib e contratos do checker |
| `ori-codegen` | backend nativo Cranelift, JIT e headers FFI nativos |
| `ori-runtime` | biblioteca nativa de runtime e ABI |
| `ori-diagnostics` | códigos de diagnóstico e apoio de renderização |
| `ori-lsp` | implementação Language Server Protocol |
| `ori-driver` | CLI, orquestração do pipeline e testes de integração |

O runtime nativo é a referência semântica para `ori compile`, `ori run` e
`ori test`. A emissão de código C foi removida; FFI C nativa e headers de
exportação continuam suportados.

## Biblioteca padrão

A stdlib vive no module `ori.*`.

Forma atual:

| Camada | Local | Função |
|---|---|---|
| Layer 1 | `compiler/crates/ori-types/src/stdlib.rs` e `compiler/crates/ori-runtime/src/lib.rs` | manifesto, ABI e primitivas de runtime |
| Layer 2 | `stdlib/**/*.orl` | wrappers seguros sobre primitivas de runtime |
| Layer 3 | `stdlib/**/*.orl` | algoritmos puros escritos em Ori |

Áreas disponíveis:

- `ori.io`, `ori.fs`, `ori.path`
- `ori.string`, `ori.bytes`, `ori.convert`
- `ori.list`, `ori.map`, `ori.set`
- `ori.math`, `ori.random`, `ori.time`
- `ori.json`, `ori.net`, `ori.process`
- `ori.task`, `ori.channel`, `ori.concurrent`
- `ori.test` e helpers de teste

Veja [stdlib/README.md](stdlib/README.md) para o inventário atual e
[docs/spec/12-stdlib.md](docs/spec/12-stdlib.md) para os contratos normativos.

## Ferramentas de editor

`ori-lsp` + extensões **locais** (sem Marketplace / loja por agora):

| Editor | Caminho | Instalação |
|--------|---------|------------|
| VS Code / Cursor | [extensions/vscode-orl](extensions/vscode-orl/) | `.vsix` local |
| Zed | [extensions/zed-ori](extensions/zed-ori/) | dev extension |

```bash
cd compiler && cargo build -p ori-lsp -p ori-driver
cd ../extensions/vscode-orl && npm install && npm run compile
# coloque compiler/target/debug no PATH para ori-lsp
```

## Layout do repositório

```text
ori-lang/
  compiler/crates/        workspace Rust do compilador, LSP, runtime e driver
  docs/spec/              contratos normativos da linguagem e implementação
  docs/planning/          roadmap, backlog e planos de implementação
  stdlib/                 módulos fonte da biblioteca padrão
  runtime/                artefatos de runtime por target triple
  examples/               programas Ori de exemplo (S3)
  tests/                  fixtures E2E e documentação de testes
  extensions/             DX local (vscode-orl, zed-ori)
  tools/                  scripts de staging, smoke, export e validação
  tools/bench/polyglot/   microbench runtime Ori / Python / Rust
  branding/               assets de marca
  docs/archive/           auditorias históricas de engenharia reversa
```

## Fluxo de desenvolvimento

Gates comuns:

```bash
cargo check --workspace
cargo test --workspace
cargo test -p ori-driver --test diagnostic_catalog
cargo test -p ori-lsp
```

Para mudanças na stdlib:

```bash
cargo test -p ori-types --lib stdlib
cargo test -p ori-driver --test multifile_imports
```

Para mudanças no runtime ou backend nativo, re-stage o runtime antes dos testes
de compile/run:

```powershell
.\tools\stage_native_runtime.ps1
```

Unix:

```sh
./tools/stage_native_runtime.sh
```

Regras do projeto:

- bugfix precisa de teste de regressão em `compiler/crates/ori-driver/tests/`;
- comportamento novo deve atualizar docs e `CHANGELOG.md`;
- códigos de diagnóstico novos devem entrar em
  [docs/spec/13-error-catalog.md](docs/spec/13-error-catalog.md);
- mudanças no runtime da stdlib devem manter manifesto, lowering, ABI, testes e
  docs em sincronia.

## Layout de release

Um pacote em formato de release deve seguir esta forma:

```text
ori.exe                         # ou `ori` no Unix
runtime/
  bin/
    rust-lld[.exe]              # linker empacotado opcional
  {target-triple}/
    ori_runtime.lib             # runtime estático Windows MSVC
    libori_runtime.a            # runtime estático Unix
    ori_runtime.dll             # cdylib Windows para JIT
    libori_runtime.so           # cdylib Linux para JIT
    libori_runtime.dylib        # cdylib macOS para JIT
    runtime-link.json
examples/
README.md
```

O workflow `native-route` cobre Windows MSVC, Windows GNU, Linux GNU,
macOS x86_64 e macOS aarch64. Detalhes de staging ficam em
[runtime/README.md](runtime/README.md).

## Limitações conhecidas

- Ori ainda não é self-hosting. O compilador escrito em Ori é um protótipo
  experimental fora da `main`; Rust continua como compilador de produto até a 1.0.
- `ori compile` (AOT) precisa do linker do SO; `ori run` usa JIT por padrão.
- Compilar Ori a partir do fonte exige Rust; package **Linux** de release não.
- A emissão de código C foi removida; a execução usa o backend nativo.
- Protocolo de pacotes/registry existe; marketplace público **não** é meta agora.
- Lojas de extensão (VS Code / Zed) **shelved** — use install local / dev.
- `ori repl` é deliberadamente pequeno.
- Contratos públicos ainda podem mudar antes de 1.0.

**Lista única do que falta:** [docs/planning/BACKLOG.md](docs/planning/BACKLOG.md).

## Roadmap

**Agora:** linguagem + docs/exemplos honestos + performance.

**Já entregue nos critérios de 1.0:** stdlib pais (M2), ABI `ori-native-abi-1`
(M3), caminho sem Rust no instalador Linux (M1).

**Depois:** packages multi-OS, publicação em lojas e demos externos continuam
adiados. O self-hosting é experimental e será reavaliado após a versão 1.0
baseada em Rust, sem bloquear esse lançamento
([ADR-0006](docs/decisions/adr/0006-rust-reference-through-1-0.md)).

## Licença

Ori é licenciado sob uma das duas opções:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

à sua escolha.
