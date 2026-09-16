# Plano técnico: WorkBreak 98

Status: APROVADO pelo usuário em 15/09/2026 — fase PLAN concluída.
Base: `specs/flowmodoro.md`, aprovada em 15/09/2026.
Arquitetura, dependências Rust e sequência/checkpoints aprovados. Este documento não libera implementação: `tasks/todo.md` tem seu próprio gate de aprovação.

## Resultado esperado

Entregar o ciclo foco → descanso manual → novo foco, com proporção dinâmica, histórico e recuperação local, usando React/Tauri e 98.css. Minimização não pausa; suspensão e fechamento pausam. Windows é a plataforma do MVP, não uma promessa de compatibilidade com Windows 98.

## Decisões arquiteturais aprovadas

### 1. Uma autoridade de estado em Rust

- Domínio puro para estados e transições, recebendo amostras de tempo em vez de chamar o relógio diretamente. Sem dependência de React/Tauri nos cálculos.
- Serviço nativo serializa ações da UI, eventos de energia, checkpoints e fechamento. Escritas ordenadas; callbacks do SO apenas capturam amostra de tempo e encaminham evento, sem I/O demorado.
- React recebe snapshots autoritativos. Não persiste sessão em localStorage nem mantém um segundo cronômetro de negócio.
- Estados: idle, focusRunning, focusPaused, breakReady, breakRunning, breakPaused e breakCompleted. Encerrar/pular descanso leva a breakCompleted; foco zero volta a idle.
- Ações explícitas: iniciar/pausar/retomar/finalizar/resetar foco, mudar proporção, iniciar/pausar/retomar/encerrar descanso. Reset descarta o foco em andamento sem registrar histórico ou descanso. Repetições fora de estado válido não criam transições extras.
- Snapshot contém revisão crescente, estado, proporção preferida, durações calculadas, histórico e situação da persistência. Comandos validam a revisão esperada para rejeitar ações obsoletas; avanço temporal de projeção não precisa incrementar a revisão de transição.
- Uma requisição de atualização visual por vez, aproximadamente a cada 250 ms enquanto visível; respostas obsoletas são ignoradas. Ao voltar à janela, buscar snapshot imediato. Intervalos nunca somam segundos.
- Conclusão do descanso é normalizada pelo serviço ao consultar/agir/checkpointar, antes de aplicar novos comandos; término lógico é limitado à duração concedida.

### 2. Separar tempo decorrido de data civil

- Usar `QueryUnbiasedInterruptTime` para timestamps de duração durante a execução no Windows: não sofre ajuste de hora e exclui sleep/hibernação segundo a documentação Microsoft.
- Usar relógio civil somente para datas históricas. Persistir durações consolidadas; nunca comparar timestamps monotônicos de processos/boots diferentes para recuperar sessão.
- Observar suspensão/retomada com `PowerRegisterSuspendResumeNotification`; manter registro e callback vivos, cancelar registro antes de liberar recursos.
- Capturar o limite temporal no callback de suspensão e encaminhar pausa prioritária ao serviço. Retomada não reinicia timer. Eventos repetidos de retomada não produzem pausas/retomadas extras.
- O relógio que exclui sleep é proteção adicional, não substitui o evento que impede contar o tempo acordado após retomada automática. Validar ordenação em relação a checkpoints e ações antes de aceitar a integração.
- Falha de registro ou leitura do relógio impede iniciar/retomar timer e aparece na UI; não substituir silenciosamente por heurística de intervalo ou Date.now().
- Investigar primeiro em Windows real, incluindo Modern Standby quando disponível. A documentação das APIs não comprova o comportamento de todo hardware; falha nessa prova bloqueia o restante e exige revisão da solução.

### 3. Persistência local transacional por snapshot

- JSON versionado em `app.path().app_local_data_dir()`, acessado somente por Rust. Reutilizar serde/serde_json existentes.
- Persistir preferência, sessão, histórico e próximo identificador em um único documento. Identificadores não dependem exclusivamente de data civil.
- Na finalização do foco, inserir histórico e transformar sessão em descanso disponível na mesma transição/documento.
- Checkpoint consolidado aproximadamente a cada segundo enquanto ativo; também salvar cada transição e mudança de preferência. Apenas um escritor; nunca permitir que checkpoint antigo sobrescreva transição nova.
- Gravar temporário no mesmo diretório, sincronizar e substituir destino com operação adequada ao Windows. Não assumir que remover o arquivo antigo antes de renomear é atômico. Provar substituição de arquivo existente no checkpoint inicial; isolar detalhes Win32 no adaptador de armazenamento.
- Ao reabrir, estados ativos viram pausados com a duração do último snapshot válido; não adicionar o período entre gravação e reabertura. Não reutilizar âncora monotônica persistida.
- Dados ausentes significam primeira execução. Dados inválidos/versão desconhecida bloqueiam sobrescrita automática e oferecem reinicialização explícita preservando cópia do original. Validar enums, limites numéricos, relações entre campos e unicidade dos registros.
- Erro de escrita mantém sessão em memória, marca persistência como indisponível e permite tentar salvar novamente. Fechamento normal espera confirmação de gravação; se falhar, oferecer tentar novamente, cancelar fechamento ou sair ciente da perda.
- Não prometer durabilidade absoluta em queda de energia; registrar evidência dos testes e a tolerância aprovada de perda do trecho não salvo.

### 4. Ciclo de vida e instância única

- Registrar `tauri-plugin-single-instance` antes dos demais plugins e antes de abrir o armazenamento; segunda instância foca a janela existente e encerra.
- Interceptar fechamento normal no nativo, consolidar/pausar e salvar antes de sair. Proteger contra recursão do evento de saída e disputa com checkpoint.
- Suspensão usa a mesma transição de pausa; minimizar/desfocar não a invoca.
- Recuperação após crash é baseada somente no documento válido; não depende de eventos de unload da WebView.

### 5. Interface 98.css

- Importar 98.css antes dos ajustes locais. Substituir tela e CSS do starter, incluindo estilos globais/dark mode conflitantes.
- `App` coordena a apresentação; cliente IPC e hook cuidam de snapshots, erros e cleanup compatível com StrictMode.
- Painéis de timer/ações, preferência de proporção e histórico. HH:MM:SS, foco/ganho truncados em segundos, restante arredondado para cima; precisão de negócio permanece em milissegundos.
- Mostrar onde a proporção se aplica: foco atual durante foco; próximo foco após finalização. Histórico usa proporção congelada.
- Labels em português, controles semânticos, estado textual, foco visível, rolagem em 800 × 600. Timer não anuncia cada tick em live region; mensagens de estado/erro podem anunciar mudanças.
- Comandos pendentes desabilitam ações conflitantes; mostrar indisponibilidade do backend em execução apenas no navegador, sem simular sucesso.

## Dependências novas aprovadas neste plano

| Dependência | Motivo | Limite |
| --- | --- | --- |
| `windows-sys` (Rust, somente Windows) | APIs de tempo, eventos de energia e substituição de arquivo | Selecionar versão compatível e apenas features necessárias no início da implementação |
| `tauri-plugin-single-instance` 2.x (Rust) | Evitar dois escritores/processos independentes | Primeiro plugin registrado; sem API JS ou permissões de frontend próprias |

Sem novas dependências npm planejadas. Se o spike exigir outra biblioteca ou mudar a arquitetura, revisar o plano antes de prosseguir. Atualizações de Cargo.toml/Cargo.lock integram o tamanho das tasks; não alterar lockfiles manualmente.

## Fronteiras de módulos propostas

- `src-tauri/src/timer.rs`: tipos de domínio, cálculos e transições; testes unitários Rust junto ao módulo.
- `src-tauri/src/clock.rs` e `power.rs`: relógio Windows e eventos de suspensão, com wrappers pequenos e lifetimes explícitos.
- `src-tauri/src/storage.rs`: documento versionado, validação, recuperação e substituição segura; testes em diretórios temporários isolados.
- `src-tauri/src/service.rs`: autoridade serializada, checkpoints e composição dos adaptadores.
- `src-tauri/src/commands.rs`: contrato IPC e validação de ações; `lib.rs` apenas wiring e ciclo de vida.
- `src/flowmodoro.ts`: contrato/cliente IPC; `src/useFlowmodoro.ts`: sincronização da UI.
- `src/components/`: painéis que só serão extraídos quando a fatia correspondente precisar; `src/formatTime.ts` para apresentação de duração.
- `src/App.tsx` e `src/App.css`: composição e tema. Preservar `main.rs`, nome `workbreak_98_lib` e schemas gerados.

Esses são destinos propostos, não scaffolding para criar em massa. Cada incremento altera apenas os arquivos necessários; dividir qualquer unidade que ultrapasse cinco arquivos contando testes e manifests.

## Dependências e sequência de entrega

```text
Prova nativa de relógio/energia + prova de armazenamento
    → domínio do foco → serviço/IPC → foco utilizável em 98.css
    → preferências e recuperação do foco → integração de suspensão/saída/instância única
    → finalizar foco + histórico persistido + descanso disponível
    → regressivo + pausa/retomada/encerramento + recuperação do descanso
    → robustez de armazenamento + revisão visual + validação desktop completa
```

As unidades abaixo orientam o PLAN; critérios detalhados, arquivos e comandos por task serão materializados em `tasks/todo.md` após aprovação. Todas devem virar tasks XS/S/M; nenhuma fase inteira será uma task.

| Etapa | Incrementos candidatos (cada item é uma unidade separada) | Aceite da etapa | Checkpoint |
| --- | --- | --- | --- |
| A — Riscos primeiro | A1: adaptador de relógio e registro de energia com diagnóstico mínimo; A2: round-trip/substituição de armazenamento versionado | Evidência real de suspensão versus segundo plano e gravação substituindo destino sem truncá-lo | CP1 após A1–A2: revisão humana da viabilidade; sem evidência de suspensão, bloquear |
| B — Primeiro caminho utilizável | B1: domínio de iniciar/pausar/retomar foco; B2: serviço e contrato IPC com snapshot; B3: painel de foco 98.css ligado ao backend | Usuário inicia/pausa/retoma foco; atualização atrasada mostra tempo correto | CP2 após B1–B3: testes de domínio e foco end-to-end |
| C — Continuidade do foco | C1: proporção e ganho dinâmicos na UI/domínio; C2: carregar/salvar preferências e checkpoint de foco | Pausa excluída, 3/4/5 corretos e reabertura recupera pausado | CP3 após C1–C2: cálculo e recuperação |
| D — Ciclo de vida real | D1: integração energia/fechamento com serviço; D2: instância única antes do armazenamento | Minimizar conta, suspender pausa, fechar salva; segunda instância não escreve | CP4 após D1–D2: verificação desktop obrigatória |
| E — Finalização e histórico | E1: transição atômica para descanso disponível e registro persistido; E2: ação de finalizar, apresentação do descanso disponível e painel de histórico | Finalizar repetidamente não duplica; espera manual não consome descanso | CP5 após E1–E2: jornada foco → histórico/descanso disponível |
| F — Descanso completo | F1: início manual, regressivo e controles de pausa/retomada/encerramento; F2: recuperação e concorrência do descanso | Zero sem negativos, sobra descartada, retorno pausado e novo foco manual | CP6 após F1–F2: ciclo completo + recuperação em cada estado |
| G — Falhas e entrega | G1: recuperação explícita de dados inválidos e UI de falhas de escrita/saída; G2: revisão visual/acessibilidade e atualização de README/AGENTS; G3: validação integrada e build desktop | Falhas visíveis, original preservado, documentação atual e todos os critérios da spec cobertos | CP7 após G1–G3: aceite final |

Tratamento mínimo de erros é obrigatório desde A/C; G1 completa a experiência e testes de falha, não adia validação ou proteção dos dados.
Cada etapa entrega o caminho já implementado funcionando. Infraestrutura inicial se limita aos dois riscos que podem invalidar o projeto.
Não executar agentes paralelos neste plano. Em eventual trabalho paralelo autorizado, apresentação/formatadores podem avançar após congelar o contrato IPC; domínio, persistência, wiring e ciclo de vida exigem sequência ou coordenação explícita.

## Verificação

Comandos a partir da raiz, após criar os testes correspondentes:

- Rust focado: `cargo test --manifest-path src-tauri/Cargo.toml timer::tests` e `cargo test --manifest-path src-tauri/Cargo.toml storage::tests`.
- Suite nativa: `cargo test --manifest-path src-tauri/Cargo.toml`.
- Frontend focado: `npx vitest run src/formatTime.test.ts`; suite frontend: `npx vitest run`.
- Typecheck focado: `npx tsc --noEmit`; build frontend: `npm run build`.
- Check nativo: `cargo check --manifest-path src-tauri/Cargo.toml`.
- Verificação desktop: `npm run tauri dev`; build/bundle final: `npm run tauri build`.

Usar testes determinísticos Rust para proporções, pausas, arredondamento, foco zero, descanso, comandos repetidos e relógio civil alterado. Testar serviço com relógio/armazenamento falsos para ordenação de checkpoint, revisão obsoleta e falhas de escrita. Vitest cobre formatadores e sincronização pura do cliente; não repetir as regras de domínio em TypeScript. Sem dependência DOM planejada; interações visuais serão verificadas no desktop.

Testes de persistência devem cobrir substituição de arquivo existente, JSON inválido, versão desconhecida, falha de escrita, preservação do original e recuperação de todos os estados. Usar armazenamento temporário, nunca dados reais do usuário.

Nos checkpoints rodar testes pertinentes aos arquivos alterados; quando houver integração frontend/nativo, verificar ambas as camadas e a jornada daquela etapa. Registrar comandos, resultados e limitações em `tasks/todo.md` durante a execução. Não marcar teste manual como aprovado com base apenas em mocks.

Validação final: foco 10 min + pausa 5 min + foco 20 min = foco efetivo 30 min; troca 3:1 → 5:1 reduz ganho 10 → 6 min; descanso inicia manualmente; histórico persiste sem duplicatas. Automatizar durações longas com relógio controlado. Testar realmente minimização, suspensão, retomada, fechamento, encerramento forçado e segunda instância no Windows. Verificar 800 × 600, teclado e tema; build frontend não substitui bundle nativo.

## Riscos e condições de bloqueio

| Risco | Mitigação / condição de avanço |
| --- | --- |
| Eventos de energia/Modern Standby diferentes no hardware | Spike A1 e validação desktop em CP1; registrar ambiente e estados testados. Não substituir por detecção de timer atrasado |
| Callback de energia competindo com UI/checkpoint | Capturar tempo no callback, serialização/prioridade e testes de ordenação; não fazer I/O pesado dentro do callback |
| Substituição de arquivo no Windows/antivírus/arquivo bloqueado | Spike A2, tratamento de falha e preservação do documento anterior; nada de delete-then-rename |
| Snapshot antigo ou StrictMode duplicando ações | Revisões, comandos validados, cleanup de polling e serviço como única autoridade |
| Histórico crescente tornando checkpoint caro | Medir custo com histórico representativo; não limitar/remover sessões sem aprovação. Se necessário, revisar persistência antes de alegar cadência de 1 s |
| Ambiente sem Rust/MSVC/WebView2 ou acesso ao desktop | Diagnosticar no início da execução; registrar bloqueio. Não afirmar validação nativa só por build web |

## Fontes técnicas consultadas

- Microsoft, `QueryUnbiasedInterruptTime`: https://learn.microsoft.com/en-us/windows/win32/api/realtimeapiset/nf-realtimeapiset-queryunbiasedinterrupttime — exclui sleep/hibernação e ajustes do relógio civil.
- Microsoft, `PowerRegisterSuspendResumeNotification`: https://learn.microsoft.com/en-us/windows/win32/api/powerbase/nf-powerbase-powerregistersuspendresumenotification — registro por callback para suspensão/retomada.
- Tauri, Single Instance: https://v2.tauri.app/plugin/single-instance/ — primeiro plugin registrado; não requer API JS/capability própria.
- Fontes locais: `package.json`, `package-lock.json`, `src-tauri/Cargo.toml`, `src-tauri/src/lib.rs`, `src-tauri/tauri.conf.json` e `AGENTS.md`.

## Próximo gate

Revisar e aprovar `tasks/todo.md`, que detalha as unidades acima em tasks XS/S/M e subdivide incrementos maiores, preservando os checkpoints. A aprovação deste plano inclui `windows-sys` e `tauri-plugin-single-instance`, mas não dispensa a prova nativa de viabilidade. Após aprovação das tasks, a implementação pode começar pela preparação do spike A1.
