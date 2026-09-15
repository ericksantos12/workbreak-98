# Tasks: Flowmodoro Windows

Status: APROVADAS pelo usuário em 15/09/2026 — execução iniciada pela T01.
Spec e PLAN aprovados em 15/09/2026: `specs/flowmodoro.md` e `tasks/plan.md`.
Executar a partir da T01 e respeitar os gates de checkpoint.

## Convenções de execução

- Executar sequencialmente. Dependência de checkpoint significa que sua evidência foi revisada e aprovada pelo usuário; não basta ter escrito código.
- XS: um arquivo/ajuste pontual; S: 1–2 arquivos; M: 3–5 arquivos. Os arquivos abaixo incluem testes, manifests e lockfile quando aplicável; atualizações de status/evidências nesta lista são registro do trabalho. Se o escopo real exceder cinco arquivos de implementação, subdividir antes de prosseguir.
- Testes de lógica seguem TDD; carregar `test-driven-development` e `incremental-implementation` ao iniciar implementação. Nenhuma dependência adicional além das aprovadas sem revisão do plano.
- Testes Rust ficam junto aos módulos, usando relógio controlado e diretórios temporários isolados. Vitest testa apresentação/cliente, sem duplicar regras do timer Rust.
- Comandos abaixo partem da raiz. Filtros de testes são alvos a criar, não testes que já existem; zero testes executados não é evidência de aprovação.
- Builds/testes nativos podem gerar Cargo.lock; criá-lo pela ferramenta na T01 e mantê-lo nas mudanças de dependências. Não editar `target/`, `dist/` ou schemas gerados.
- Nos testes manuais, registrar Windows/hardware/modo de energia e resultado observado. Solicitar participação do usuário para suspender/retomar a máquina; não provocar suspensão durante uma sessão sem combinar. Sem acesso ao desktop, registrar bloqueio, sem substituir a evidência por mocks.
- Atualizar cada checkbox somente após verificação. Ao terminar uma task, registrar: comando/cenário, resultado, arquivos reais, limitações e próximo passo. Checkpoint reprovado bloqueia dependentes.

## A — Viabilidade nativa (A1/A2 do plano)

### T01 — Preparar toolchain e dependência Win32 [S]
Dependências: aprovação de TASKS. Arquivos: `src-tauri/Cargo.toml`, `src-tauri/Cargo.lock`.
- [x] Verificar Node compatível com Vitest 5, Rust/Cargo e pré-requisitos Windows do Tauri; registrar versões e bloqueios reais.
- [x] Adicionar `windows-sys` somente para Windows, com features necessárias para relógio, energia e arquivos; resolver lockfile sem alterar dependências npm.
Verificar: `node --version`; `rustc --version`; `cargo --version`; `cargo check --manifest-path src-tauri/Cargo.toml`. Se precisar reinstalar frontend: `npm ci`.

### T02 — Provar relógio e notificações de suspensão [M]
Dependências: T01. Arquivos: `src-tauri/src/clock.rs`, `src-tauri/src/power.rs`, `src-tauri/src/lib.rs`, `src-tauri/examples/power_probe.rs`.
- [x] Adaptadores isolados retornam timestamps de duração e eventos de energia; registro tem cleanup seguro, callback curto e erro explícito, sem relógio civil como fallback.
- [x] Diagnóstico executável reutiliza os adaptadores para registrar amostras/eventos; segue contando minimizado e demonstra exclusão de suspensão/hibernação quando suportada.
- [ ] Registrar evidência real de suspensão/retomada e ordem dos eventos; testar erro de registro e vida útil do callback sem deixar callbacks acessarem memória liberada.
Verificar: `cargo test --manifest-path src-tauri/Cargo.toml clock::tests`; `cargo test --manifest-path src-tauri/Cargo.toml power::tests`; `cargo run --manifest-path src-tauri/Cargo.toml --example power_probe`. Combinar teste de suspensão com usuário; registrar Modern Standby disponível/testado ou limitação.

### T03 — Provar armazenamento substituível sem truncar destino [S]
Dependências: T01. Ordem operacional: depois de T02. Arquivos: `src-tauri/src/storage.rs`, `src-tauri/src/lib.rs`.
- [x] Encapsular leitura e escrita de envelope JSON versionado em caminho injetado; temporário fica no mesmo diretório, com sincronização e substituição adequada ao Windows.
- [x] Round-trip funciona para primeira gravação e substituição; falhas antes da substituição preservam o destino anterior e são retornadas ao chamador.
- [x] Arquivo ausente, inválido e versão desconhecida são resultados distintos; os dois últimos nunca causam sobrescrita automática.
Verificar: `cargo test --manifest-path src-tauri/Cargo.toml storage::tests`; incluir destino existente/bloqueado e conteúdo inválido em diretório temporário. `cargo check --manifest-path src-tauri/Cargo.toml`.

### CP1 — Gate de viabilidade (T01–T03)
- [ ] Evidência Windows real de T02 revisada; duração não depende de ticks, perda de foco da janela ou relógio civil. Adiada pelo usuário; risco permanece aberto.
- [x] Substituição de destino e falhas de T03 verificadas; nenhuma API/dependência não aprovada exigida.
- [x] Exceção aprovada pelo usuário em 15/09/2026 para continuar a partir de T04 sem fechar a evidência real de suspensão. Revalidar antes do aceite final.

## B — Foco utilizável (B1/B2/B3 do plano)

### T04 — Modelar foco ativo e pausado [S]
Dependências: CP1. Arquivos: `src-tauri/src/timer.rs`, `src-tauri/src/lib.rs`.
- [x] Implementar estados iniciais e transições iniciar/pausar/retomar com timestamps injetados; pausas não acumulam foco.
- [x] Projeção atrasada retorna tempo correto sem somar ticks; comandos inválidos/repetidos não duplicam início nem segmentos.
- [x] Testes cobrem 10 min foco + 5 min pausa + 20 min foco = 30 min efetivos, amostras inválidas e proteção de limites numéricos.
Verificar: `cargo test --manifest-path src-tauri/Cargo.toml timer::tests`.

### T05 — Expor serviço serializado e contrato IPC [M]
Dependências: T04. Arquivos: `src-tauri/src/service.rs`, `src-tauri/src/commands.rs`, `src-tauri/src/lib.rs`, `src/flowmodoro.ts`.
- [x] Serviço único recebe ações e relógio, serializa mudanças e expõe snapshot com revisão de transição; comandos Tauri registrados validam revisão/ação.
- [x] Contrato TS acompanha payloads Rust; snapshot atrasado/ação com revisão antiga não sobrescreve estado nem executa comando obsoleto.
- [x] Serviço sinaliza falha do relógio e bloqueia iniciar/retomar; testes usam fontes de tempo controladas, sem depender da UI.
Verificar: `cargo test --manifest-path src-tauri/Cargo.toml service::tests`; `cargo test --manifest-path src-tauri/Cargo.toml commands::tests`; `npx tsc --noEmit`.

### T06 — Sincronizar snapshots e formatar duração [M]
Dependências: T05. Arquivos: `src/flowmodoro.ts`, `src/flowmodoro.test.ts`, `src/useFlowmodoro.ts`, `src/formatTime.ts`, `src/formatTime.test.ts`.
- [x] Cliente mantém no máximo uma consulta visual pendente, ignora respostas obsoletas e limpa assinaturas/timers na desmontagem; ações pendentes e erros ficam expostos ao hook.
- [x] Consultar aproximadamente a cada 250 ms quando visível e imediatamente ao voltar; lifecycle do StrictMode não deixa controladores antigos atualizarem o estado. Backend ausente retorna erro explícito.
- [x] Formatador cobre truncamento de foco/ganho, arredondamento para cima do restante, zero e mais de 24 horas; não calcula duração de negócio.
Verificar: `npx vitest run src/flowmodoro.test.ts src/formatTime.test.ts`; `npx tsc --noEmit`. Testar lifecycle pelo controlador puro/injeção de transporte, sem adicionar ambiente DOM.

### CP2a — Contrato e sincronização (T04–T06)
- [x] Testes de domínio/serviço/cliente passam com pausas, revisões e respostas atrasadas.
- [x] `npm run build` e `cargo check --manifest-path src-tauri/Cargo.toml` passam; checkpoint confirmado pelo usuário em 15/09/2026.

### T07 — Entregar painel de foco 98.css [M]
Dependências: CP2a. Arquivos: `src/App.tsx`, `src/App.css`, `src/components/TimerPanel.tsx`.
- [x] Substituir saudação por painel conectado ao hook; iniciar, pausar e retomar funcionam no Tauri, com duração e estado visíveis.
- [x] Importar 98.css antes de ajustes locais, eliminar estilos conflitantes do starter; controles semânticos em português, sem ações decorativas inertes.
- [x] Desabilitar ações conflitantes enquanto pendentes e mostrar erro nativo; teclado e layout 800 × 600 utilizáveis.
Verificar: `npm run build`; `npm run tauri dev` e testar foco, pausa, retomada, minimização e atualização ao voltar. Teste manual de StrictMode sem consultas duplicadas persistentes.

### T08 — Proporção dinâmica e descanso conquistado [M]
Dependências: T07. Arquivos: `src-tauri/src/timer.rs`, `src-tauri/src/service.rs`, `src-tauri/src/commands.rs`, `src/flowmodoro.ts`, `src/components/TimerPanel.tsx`.
- [ ] 3:1 é padrão; aceitar somente 3/4/5 e projetar ganho em milissegundos usando divisão inteira sem arredondar a cada tick.
- [ ] Seletor recalcula todo o foco atual, ativo ou pausado; 60 min produzem 20/15/12 min e 30 min de 3:1 para 5:1 mudam ganho 10 → 6 min.
- [ ] UI exibe foco e ganho simultaneamente; alteração chega ao domínio via comando validado e retorna preferência no snapshot.
Verificar: `cargo test --manifest-path src-tauri/Cargo.toml timer::tests`; `cargo test --manifest-path src-tauri/Cargo.toml service::tests`; `npm run build`; seletor ativo/pausado em `npm run tauri dev`.

### CP2 — Foco end-to-end (T07–T08)
- [ ] Foco real inicia/pausa/retoma, UI retro funcional e cálculo das três proporções comprovado.
- [ ] Verificações de T07/T08 revisadas e aprovação humana registrada antes de conectar persistência.

## C — Continuidade e energia (C2/D1 do plano)

### T09 — Persistir e recuperar foco/preferência [M]
Dependências: CP2. Arquivos: `src-tauri/src/storage.rs`, `src-tauri/src/timer.rs`, `src-tauri/src/service.rs`, `src-tauri/src/lib.rs`.
- [ ] Definir documento tipado/versionado para preferência, sessão e metadados; validar números/estado e usar app_local_data_dir em produção, caminho isolado em testes.
- [ ] Salvar transições/preferência; carregar arquivo válido restaura foco ativo como pausado somente até duração consolidada, descartando âncora de execução anterior.
- [ ] Falha mantém estado em memória e status de persistência no snapshot; conteúdo inválido/versão desconhecida bloqueia escrita automática e não é convertido silenciosamente em primeira execução.
Verificar: `cargo test --manifest-path src-tauri/Cargo.toml storage::tests`; `cargo test --manifest-path src-tauri/Cargo.toml service::tests`; `cargo check --manifest-path src-tauri/Cargo.toml`.

### T10 — Checkpoints ordenados e recuperação após crash [M]
Dependências: T09. Arquivos: `src-tauri/src/service.rs`, `src-tauri/src/storage.rs`, `src-tauri/src/lib.rs`, `src/App.tsx`.
- [ ] Scheduler nativo salva aproximadamente a cada segundo apenas quando ativo; um escritor impede snapshot antigo de sobrescrever transição nova.
- [ ] Testes controlados demonstram corrida checkpoint/pausa/proporção e crash após último salvamento; restauro não adiciona tempo ausente nem soma ticks.
- [ ] UI mostra status de salvamento indisponível desde esta etapa; dados inválidos ficam preservados, e encerramento forçado recupera último documento válido em teste desktop isolado.
Verificar: `cargo test --manifest-path src-tauri/Cargo.toml service::tests`; `cargo test --manifest-path src-tauri/Cargo.toml storage::tests`; `npm run build`; testar encerramento forçado/reabertura com `npm run tauri dev` e dados de teste.

### CP3 — Recuperação do foco (T09–T10)
- [ ] Preferência e foco pausado recuperam corretamente; checkpoints persistidos não regrediram estado.
- [ ] Falha de escrita visível e conteúdo inválido preservado; evidência de crash revisada e aprovação humana registrada.

### T11 — Integrar suspensão ao serviço [M]
Dependências: CP3. Arquivos: `src-tauri/src/power.rs`, `src-tauri/src/service.rs`, `src-tauri/src/timer.rs`, `src-tauri/src/lib.rs`.
- [ ] Evento captura limite temporal e pausa sessão ativa em ordem com ações/checkpoints; callback não faz I/O pesado. Registro inválido impede iniciar/retomar.
- [ ] Retomada automática/duplicada não reinicia foco; tempo acordado após retomar permanece excluído até ação manual, sem confundir minimizar com suspender.
- [ ] Testar evento atrasado em relação ao processamento de checkpoint/ação, cleanup e restauração pausada no desktop; conservar evidência do limite temporal aplicado.
Verificar: `cargo test --manifest-path src-tauri/Cargo.toml power::tests`; `cargo test --manifest-path src-tauri/Cargo.toml service::tests`; `npm run tauri dev` e suspensão real combinada com o usuário.

### T12 — Fechamento normal persistido [M]
Dependências: T11. Arquivos: `src-tauri/src/lib.rs`, `src-tauri/src/service.rs`, `src-tauri/src/commands.rs`, `src/flowmodoro.ts`, `src/App.tsx`.
- [ ] Fechar consolida/pausa e aguarda salvamento antes da saída; não depende de unload e não entra em loop ao autorizar a saída.
- [ ] Erro de gravação mantém janela aberta e oferece tentar novamente, cancelar fechamento ou sair explicitamente ciente da perda; comandos de decisão validados.
- [ ] Testes cobrem fechamento concorrente com checkpoint/energia, repetição de fechar e ausência de retomada automática ao cancelar fechamento.
Verificar: `cargo test --manifest-path src-tauri/Cargo.toml service::tests`; `cargo test --manifest-path src-tauri/Cargo.toml commands::tests`; `npm run build`; fechar/reabrir e simular erro de escrita com dados de teste no Tauri.

### T13 — Garantir instância única [M]
Dependências: T12. Arquivos: `src-tauri/Cargo.toml`, `src-tauri/Cargo.lock`, `src-tauri/src/lib.rs`.
- [ ] Adicionar plugin 2.x aprovado, registrado primeiro e antes de abrir armazenamento; segunda instância não inicializa outro escritor.
- [ ] Segunda execução restaura/foca janela existente; não inicia servidor Vite adicional para testar isso.
Verificar: `cargo check --manifest-path src-tauri/Cargo.toml`; `npm run tauri dev`, depois executar novamente o binário debug gerado para observar instância única e ausência de gravação concorrente. Registrar caminho efetivo do binário.

### CP4 — Ciclo de vida Windows (T11–T13)
- [ ] Minimizar conta; suspender pausa; retorno exige retomada manual; fechar salva; segunda instância não escreve.
- [ ] Falha de salvamento no fechamento tem decisão explícita; testes nativos pertinentes passam e evidência desktop aprovada pelo usuário.

## E — Finalização e histórico (E1/E2 do plano)

### T14 — Finalizar foco com histórico transacional [M]
Dependências: CP4. Arquivos: `src-tauri/src/timer.rs`, `src-tauri/src/storage.rs`, `src-tauri/src/service.rs`.
- [ ] Finalizar foco ativo/pausado cria registro com identificador, data/hora civil, foco efetivo, proporção final e descanso calculado; sessão vira breakReady no mesmo documento.
- [ ] Foco zero volta a idle sem registro; comandos repetidos não duplicam histórico; IDs não dependem só de data civil e são validados na leitura.
- [ ] Descanso disponível não diminui com espera; preferência posterior afeta apenas próximo foco. Restauro de breakReady preserva histórico/descanso congelados.
Verificar: `cargo test --manifest-path src-tauri/Cargo.toml timer::tests`; `cargo test --manifest-path src-tauri/Cargo.toml storage::tests`; `cargo test --manifest-path src-tauri/Cargo.toml service::tests`.

### T15 — Exibir finalização e histórico na interface [M]
Dependências: T14. Arquivos: `src-tauri/src/commands.rs`, `src/flowmodoro.ts`, `src/components/TimerPanel.tsx`, `src/components/HistoryPanel.tsx`, `src/App.tsx`.
- [ ] Finalizar disponível em foco ativo/pausado e protegido contra cliques repetidos; mostrar descanso disponível sem regressivo automático.
- [ ] Histórico mais recente primeiro, com campos aprovados e estado vazio; datas em português, durações pelo formatador existente.
- [ ] UI indica quando proporção vale para próximo foco; alterações não mudam registros antigos nem descanso concedido.
Verificar: `cargo test --manifest-path src-tauri/Cargo.toml commands::tests`; `npm run build`; `npm run tauri dev` e finalizar/reabrir/trocar proporção em breakReady. Início do descanso será habilitado em T17, sem botão que simule funcionalidade nesta etapa.

### CP5 — Finalização persistida (T14–T15)
- [ ] Caminho foco → histórico/descanso disponível funciona e sobrevive a reabertura sem duplicatas.
- [ ] Casos foco zero, finalizar pausado e proporção congelada comprovados; aprovação humana registrada.

## F — Descanso completo (F1/F2 do plano)

### T16 — Modelar regressivo e controles do descanso [M]
Dependências: CP5. Arquivos: `src-tauri/src/timer.rs`, `src-tauri/src/service.rs`, `src-tauri/src/storage.rs`.
- [ ] Implementar início manual, pausa/retomada, encerrar/pular e término em zero; sobra descartada, histórico mantido e próximo foco manual.
- [ ] Normalizar conclusão antes de consulta/ação/checkpoint, sem negativos; pausa exclui tempo e preferência não recalcula descanso concedido.
- [ ] Recuperar breakRunning como breakPaused; breakReady/breakPaused/breakCompleted preservados. Cobrir subsegundos, duração concedida zero por divisão e comandos no limite da conclusão.
Verificar: `cargo test --manifest-path src-tauri/Cargo.toml timer::tests`; `cargo test --manifest-path src-tauri/Cargo.toml service::tests`; `cargo test --manifest-path src-tauri/Cargo.toml storage::tests`.

### T17 — Conectar descanso à interface [M]
Dependências: T16. Arquivos: `src-tauri/src/commands.rs`, `src/flowmodoro.ts`, `src/components/TimerPanel.tsx`, `src/App.tsx`.
- [ ] Iniciar descanso aparece em breakReady; controles de pausa/retomada/encerramento correspondem ao estado e exibem restante arredondado para cima.
- [ ] Conclusão/encerramento oferece iniciar novo foco manualmente; não transfere saldo nem altera histórico.
- [ ] Pausa por suspensão/recuperação aparece claramente; não anunciar cada tick por leitor de tela nem simular conclusão no React.
Verificar: `cargo test --manifest-path src-tauri/Cargo.toml commands::tests`; `npm run build`; ciclo curto completo, pular, pausar/retomar e novo foco em `npm run tauri dev`.

### T18 — Provar recuperação e concorrência do ciclo completo [M]
Dependências: T17. Arquivos: `src-tauri/src/service.rs`, `src-tauri/src/storage.rs`, `src/flowmodoro.test.ts`.
- [ ] Testes cobrem todos os estados persistidos, crash e disputa finalização/checkpoint/fechamento/energia sem histórico duplicado ou snapshot antigo.
- [ ] Testar ajuste do relógio civil, respostas tardias na conclusão e ações com revisão anterior; durações e UI não regridem.
- [ ] Verificar descanso em suspensão/retomada real, encerramento forçado e fechamento normal, além de minimização por pelo menos cinco minutos em foco; registrar cenários observados.
Verificar: `cargo test --manifest-path src-tauri/Cargo.toml`; `npx vitest run src/flowmodoro.test.ts`; `npm run tauri dev` com dados de teste. Corrigir somente problemas dentro deste escopo; outros geram follow-up antes de fechar checkpoint.

### CP6 — Ciclo completo e recuperação (T16–T18)
- [ ] Foco → descanso manual → conclusão/encerramento → novo foco funciona; restauro de todos os estados comprovado.
- [ ] Testes automatizados e evidência real de energia/fechamento no descanso revisados e aprovados pelo usuário.

## G — Falhas e entrega (G1/G2/G3 do plano)

### T19 — Recuperação explícita de armazenamento inválido [M]
Dependências: CP6. Arquivos: `src-tauri/src/storage.rs`, `src-tauri/src/service.rs`, `src-tauri/src/commands.rs`.
- [ ] Ação explícita de reinicialização preserva cópia dos bytes originais antes de criar estado novo; se a preservação falhar, não sobrescrever o original.
- [ ] Rejeitar versão desconhecida, dados incoerentes/duplicados e excesso numérico; não recuperar parcialmente silenciosamente. Estado bloqueado não executa checkpoints sobre o arquivo inválido.
- [ ] Expor tentar salvar novamente e reinicialização via comandos específicos, com resultados claros e testes de falhas/ações repetidas.
Verificar: `cargo test --manifest-path src-tauri/Cargo.toml storage::tests`; `cargo test --manifest-path src-tauri/Cargo.toml service::tests`; `cargo test --manifest-path src-tauri/Cargo.toml commands::tests`.

### T20 — Completar experiência de erros locais [M]
Dependências: T19. Arquivos: `src/flowmodoro.ts`, `src/components/StorageStatus.tsx`, `src/App.tsx`, `src/App.css`.
- [ ] Mostrar erro de leitura/gravação e recuperação não garantida, com tentar novamente e reinicialização confirmada; mensagens acessíveis e em português.
- [ ] Integrar as decisões de fechamento de T12 sem perder a proteção existente; não afirmar salvamento enquanto pendente/com falha.
- [ ] Dados inválidos preservados e falha de cópia não escondida; verificar fluxos de cancelar/repetir em desktop com armazenamento de teste.
Verificar: `npm run build`; `npm run tauri dev` e cenários de erro em diretório/arquivo isolado, nunca corromper dados reais do usuário para testar.

### CP7a — Integridade e recuperação (T19–T20)
- [ ] Conteúdo original preservado, ações explícitas e erros visíveis; tests de falha e fluxo desktop revisados.
- [ ] Aprovação humana registrada antes de fechar revisão visual/documentação.

### T21 — Revisar tema e acessibilidade [M]
Dependências: CP7a. Arquivos: `src/App.css`, `src/App.tsx`, `src/components/TimerPanel.tsx`, `src/components/HistoryPanel.tsx`, `src/components/StorageStatus.tsx`.
- [ ] Aplicação consistente com 98.css em todos os estados, sem dark mode herdado conflitante, ícones/botões falsos ou texto da saudação.
- [ ] Navegação por teclado, foco visível, labels/estados e mensagens acessíveis; timer não causa anúncios contínuos.
- [ ] Janela 800 × 600 e conteúdo longo permanecem utilizáveis por rolagem; durações acima de 24 horas e histórico extenso não sobrepõem controles.
Verificar: `npm run build`; `npm run tauri dev`, inspeção visual e teclado nos estados de foco, descanso, histórico vazio/preenchido e erro. Sem testes que apenas espelhem CSS.

### T22 — Atualizar documentação operacional [S]
Dependências: T21. Arquivos: `README.md`, `AGENTS.md`.
- [ ] README explica comandos reais, Windows, comportamento de pausa/recuperação, localização efetiva dos dados e limitação de checkpoints.
- [ ] AGENTS substitui fatos do starter por comandos/fronteiras/gotchas verificados da implementação; permanece compacto e referencia spec/plano quando necessário.
Verificar: conferir cada comando/caminho contra manifests, módulos e evidências dos checkpoints; nenhuma afirmação de teste executado sem registro.

### T23 — Validar entrega desktop [XS]
Dependências: T22. Arquivo de registro: `tasks/todo.md`; nenhum código planejado.
- [ ] Executar suites frontend/nativa e bundle desktop; registrar resultados, versões e caminho do artefato gerado.
- [ ] Testar artefato desktop: ciclo completo, minimização, suspensão, fechar/reabrir, instância única, persistência, proporções, teclado e 800 × 600; medir checkpoint com histórico representativo e registrar tamanho/latência.
- [ ] Relacionar todos os critérios da spec com testes/evidências; falhas geram tasks corretivas pequenas e nova verificação pertinente, sem marcar entrega pronta.
Verificar: `npx vitest run`; `cargo test --manifest-path src-tauri/Cargo.toml`; `npm run tauri build` (inclui build frontend); testar o executável produzido. Não publicar release, assinar, fazer commit ou instalar silenciosamente.

### CP7 — Aceite final (T21–T23)
- [ ] Critérios da spec cobertos, documentação confere com implementação e build desktop concluído.
- [ ] Cenários manuais registrados com ambiente/limitações; não há falha conhecida mascarada por mocks ou teste de zero casos.
- [ ] Usuário aprova entrega final ou indica correções. Não avançar para features fora do MVP.

## Rastreabilidade resumida

| Requisito | Tasks principais |
| --- | --- |
| Foco iniciar/pausar/retomar e tempo efetivo | T04–T07 |
| 3:1 padrão, 4:1/5:1 e ganho dinâmico | T08, T14–T15 |
| Timestamps reais, segundo plano e relógio civil | T02, T04–T06, T11, T18 |
| Preferências/checkpoints/recuperação | T03, T09–T12, T16, T18 |
| Instância única | T13, T23 |
| Histórico simples e finalização única | T14–T15, T18 |
| Descanso manual, regressivo, pausa e descarte | T16–T18 |
| Falhas de escrita, dados inválidos e saída | T03, T09–T12, T19–T20 |
| 98.css, português, teclado, 800 × 600 | T07, T15, T17, T20–T21 |
| Verificação e documentação final | T22–T23 |

## Registro de execução

### 15/09/2026 — início da T01

- Lista de tasks aprovada pelo usuário.
- `node --version`: `v24.16.0`, compatível com o Vitest bloqueado.
- `rustc`, `cargo`, `rustup` e `cl` não foram encontrados; também não existem `%USERPROFILE%\.cargo\bin\cargo.exe` ou `%USERPROFILE%\.rustup`.
- Diretório não é um repositório Git; checkpoints serão registrados nos documentos, sem commits.
- T01 permanece em andamento e T02/T03 bloqueadas até instalar o toolchain Rust e MSVC necessário ao Tauri.

### 15/09/2026 — T01 concluída; T02/T03 implementadas

- Usuário autorizou instalação por winget. Instalados Rustup `1.29.1`, Rust/Cargo `1.98.1` e Visual Studio Build Tools `17.14.41` com workload C++.
- T01: `windows-sys 0.61.2` foi limitado ao alvo Windows e às features de Foundation, FileSystem, Power, WindowsProgramming e WindowsAndMessaging. `cargo check --manifest-path src-tauri/Cargo.toml`: passou.
- T02: teste RED confirmou API ausente; implementação usa `QueryUnbiasedInterruptTime` e `PowerRegisterSuspendResumeNotification`. Testes de relógio/energia passaram; probe curto mostrou `wall_ms=2017` e `unbiased_ms=2018` acordado. Evidência real de suspensão ainda pendente.
- T03: teste RED confirmou armazenamento ausente. Quatro testes cobrem ausente/inválido/versão desconhecida, primeira gravação, substituição, falha de serialização e destino bloqueado; documento anterior preservado.
- Suite: `cargo test --manifest-path src-tauri/Cargo.toml`: 10 testes passaram. `cargo check --manifest-path src-tauri/Cargo.toml`: passou.
- CP1 continua bloqueado até executar o probe durante suspensão/retomada real e revisar a diferença entre relógio civil e tempo ativo.
- O usuário optou por adiar o teste de suspensão real. T02 permanece em andamento; nenhuma task dependente de CP1 foi iniciada.

### 15/09/2026 — exceção do CP1

- O usuário pediu para continuar a implementação sem executar agora o teste real de suspensão.
- CP1 foi liberado por exceção somente para desenvolvimento; a evidência real segue pendente e será exigida novamente antes do aceite final.

### 15/09/2026 — T04–T06 concluídas; CP2a aguardando revisão

- T04 começou com cinco testes RED e implementou foco idle/running/paused, timestamps injetados, rejeição de transições inválidas, regressão temporal e overflow.
- T05 começou com testes RED e adicionou autoridade serializada Rust, revisão otimista, snapshots e comandos Tauri; o contrato TypeScript corresponde aos payloads camelCase.
- T06 começou com seis testes RED e adicionou controlador com uma consulta visual pendente, descarte de respostas obsoletas, cleanup de StrictMode, visibilidade e formatadores HH:MM:SS.
- `npx vitest run src/flowmodoro.test.ts src/formatTime.test.ts`: 6 testes passaram. `npx tsc --noEmit`: passou. `npm run build`: passou.
- Após `cargo fmt`, `cargo test --manifest-path src-tauri/Cargo.toml`: 19 testes passaram. `cargo check --manifest-path src-tauri/Cargo.toml`: passou antes da formatação; a suite posterior recompilou o código formatado.
- CP2a foi posteriormente aprovado pelo usuário; T07 estava intacta no momento da aprovação.

### 15/09/2026 — CP2a aprovado

- Usuário confirmou o checkpoint e autorizou continuar com T07.
- Estratégia de commits definida: um commit por task concluída e um commit documental ao fechar cada fase/checkpoint.

### 15/09/2026 — T07 concluída

- Tela starter substituída por janela 98.css em português, com leitura HH:MM:SS, estado e ações iniciar/pausar/retomar ligadas ao serviço Rust.
- Controles pendentes ficam desabilitados; erro IPC é apresentado por `role=alert`; barra de status anuncia apenas mudanças de estado, não cada tick.
- `98.css 0.1.21` contém `@media (not(hover))`, rejeitado pelo minificador CSS do Vite 8. `build.cssMinify=false` preserva o design system sem editar `node_modules`; o build passou depois do ajuste.
- `npx vitest run`: 6 testes passaram. `cargo test --manifest-path src-tauri/Cargo.toml`: 19 testes passaram. `npm run build`: passou.
- `npm run tauri dev -- --no-watch` exigiu adicionar `%USERPROFILE%\.cargo\bin` ao PATH do processo atual; Vite iniciou na porta 1420, Rust compilou e `target\debug\flowmodoro.exe` foi lançado. O processo foi encerrado pelo limite da execução assistida.
