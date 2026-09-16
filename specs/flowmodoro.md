# Especificação: WorkBreak 98

Status: APROVADA pelo usuário em 15/09/2026 — fase SPECIFY concluída.
A aprovação inclui as propostas de comportamento abaixo e o escopo inicial Windows. Escolhas técnicas e dependências foram aprovadas em `tasks/plan.md`.
Não autoriza implementação. PLAN e TASKS dependem dos gates posteriores.

## Objetivo e escopo confirmado

Aplicativo desktop Tauri para Windows para trabalhar por tempo livre e obter descanso proporcional, com interface em português e visual Windows 98 usando 98.css.

- Iniciar, pausar, retomar, finalizar e resetar foco; pausas não geram descanso. Reset descarta o foco atual, volta o timer a zero e não cria histórico nem descanso.
- Proporções disponíveis: 3:1 (padrão), 4:1 e 5:1.
- Durante o foco, mudar a proporção recalcula o descanso de todo o foco efetivo atual.
- Exibir foco transcorrido e descanso conquistado em tempo real.
- Finalizar foco registra a sessão e disponibiliza descanso; a contagem regressiva começa manualmente.
- Descanso pertence à sessão atual, sem banco transferido entre sessões.
- Histórico local simples: data/hora, foco efetivo, proporção final e descanso calculado. Registrar ao finalizar foco, não ao concluir descanso.
- Persistir preferências, histórico e sessão atual localmente.
- Segundo plano conta normalmente. Fechamento e suspensão do computador não contam; recuperar pausado.
- Sem versão web de produto, contas, sincronização, edição/exportação do histórico, sons ou notificações no MVP.

## Contexto verificado no repositório

- React 19, TypeScript 6, Vite 8 e Tauri 2; `98.css` está instalado, mas não importado.
- `src/main.tsx` monta `src/App.tsx` em StrictMode; App ainda é a tela de saudação do starter.
- `src/App.css` contém estilos globais do starter, inclusive dark mode; precisarão ser reconciliados com o tema 98.css, sem herdar aparência conflitante.
- `src-tauri/src/lib.rs` registra o comando greet e o plugin opener; `main.rs` chama a biblioteca.
- Ainda não há lógica de timer, persistência, integração com eventos de energia ou testes.
- Não há especificação anterior nem convenção existente de organização de testes.

## Regras temporais

- Calcular foco efetivo como soma das durações dos segmentos ativos, usando timestamps de início/fim; excluir segmentos pausados.
- Descanso calculado = foco efetivo / divisor selecionado. Exemplo: 60 min geram 20, 15 ou 12 min nas proporções 3:1, 4:1 ou 5:1.
- Não usar número de ticks como fonte de tempo. Intervals servem apenas para atualização visual/checkpoints.
- Atrasos de renderização e throttling da WebView não podem ser interpretados como suspensão ou pausa.
- Ao iniciar descanso, descontar apenas tempo ativo de descanso; nunca exibir valor negativo.
- Manter precisão em milissegundos, arredondar o descanso calculado uma única vez para baixo em milissegundos; exibir foco/ganho em segundos completos e restante com arredondamento para cima. Usar HH:MM:SS sem reiniciar após 24 horas.
- Alteração de proporção após finalizar foco vale apenas para o próximo foco; histórico e descanso já concedido permanecem imutáveis.
- Risco: relógio civil pode mudar por ajuste manual/NTP. Timestamps persistidos e medição de duração precisam de política explícita; não basta usar Date.now() indiscriminadamente.

## Fluxo aprovado

1. Sem sessão: iniciar foco.
2. Foco ativo: pausar ou finalizar; proporção editável e ganho recalculado.
3. Foco pausado: retomar ou finalizar; proporção também recalcula o ganho.
4. Foco finalizado: histórico gravado uma única vez; descanso disponível aguardando início manual.
5. Descanso ativo: contagem regressiva até zero.
6. Recuperação de descanso interrompido: descanso pausado com ação de retomar.
7. Descanso concluído: novo foco depende de ação manual, sem ciclo automático.

Permitir pausar/retomar e encerrar/pular descanso, descartando a sobra, sem apagar o histórico de foco.
Foco com duração efetiva zero não gera registro nem descanso; retorna ao estado inicial.
Comandos repetidos não podem duplicar sessões no histórico ou iniciar timers concorrentes.

## Recuperação e persistência

- Uma única autoridade de estado deve coordenar timer, transições, histórico e gravações; evitar timers independentes em React e Rust.
- Arquitetura aprovada no PLAN: autoridade no processo Rust, com React como interface. A integração nativa Windows será validada no primeiro checkpoint.
- Persistir em armazenamento local do aplicativo, com versão de formato, validação e gravação atômica. O PLAN aprovado define JSON local acessado por Rust.
- Salvar cada transição e mudança de preferência; checkpoint periódico de 1 segundo enquanto há timer ativo.
- Fechamento normal: consolidar segmento ativo, salvar estado pausado e só então encerrar; não transformar minimizar/perder foco da janela em pausa.
- Suspensão: observar eventos nativos e excluir o intervalo suspenso. Ao retornar, aguardar retomada manual. A viabilidade/API deve ser comprovada na plataforma-alvo antes da implementação do timer completo.
- Encerramento inesperado: restaurar pausado até o último checkpoint válido, sem cobrar o intervalo até reabrir. Pode haver perda do trecho ainda não salvo; periodicidade de 1 segundo é objetivo, não garantia rígida em falha de disco/processo/SO.
- Histórico e estado da sessão precisam ser persistidos de forma consistente para impedir duplicação após reabertura.
- Erro de gravação: manter estado em memória e mostrar aviso de que a recuperação não está garantida; não alegar sucesso de salvamento.
- Dados inválidos: não sobrescrever silenciosamente o conteúdo original; informar falha e oferecer recuperação/reinicialização explícita.
- Instância única por `tauri-plugin-single-instance`, aprovada no PLAN, impede escritores concorrentes de processos distintos.

## Interface e critérios visuais

- Usar componentes/classes reais de 98.css, com ajustes locais apenas onde necessário.
- Janela retro, controles nativos semânticos, seletor de proporção, timer principal, descanso conquistado e histórico do mais recente para o mais antigo.
- Controles identificados em português, foco de teclado visível e estados ativo/pausado/aguardando descanso claros sem depender somente de cor.
- Validar na janela inicial existente de 800 × 600; conteúdo acessível por rolagem se necessário, sem sobreposição de controles.
- Sem controles decorativos de janela que aparentem ter funções não implementadas.

## Comandos e estrutura

Executar a partir da raiz:

- Instalar: `npm ci`. Node aceito pelo Vitest 5 bloqueado: `^22.12.0 || ^24.0.0 || >=26.0.0`.
- App desktop: `npm run tauri dev` (inicia Vite automaticamente na porta 1420).
- Frontend isolado: `npm run dev` (não verifica integração nativa).
- Typecheck: `npx tsc --noEmit`.
- Build frontend: `npm run build` (não compila Rust).
- Check nativo: `cargo check --manifest-path src-tauri/Cargo.toml`.
- Build desktop: `npm run tauri build`.
- Testes após sua criação: `npx vitest run` e `cargo test --manifest-path src-tauri/Cargo.toml`. Atualmente não há testes nem script npm test/lint.

Estrutura atual: `src/` para frontend; `src-tauri/src/` para Rust; `src-tauri/capabilities/` para permissões; `specs/` para especificação.
Testes de domínio junto aos módulos Rust; Vitest para apresentação/sincronização do cliente, conforme PLAN aprovado.
Plano aprovado: `tasks/plan.md`; tarefas para revisão: `tasks/todo.md`.

## Estilo observado

TSX utiliza componentes funcionais, aspas duplas, ponto e vírgula e indentação de dois espaços. Exemplo existente em `src/App.tsx`:

```tsx
const [greetMsg, setGreetMsg] = useState("");
```

TypeScript tem strict e checagem de variáveis/parâmetros não usados. Não existe formatter/linter configurado; não introduzir convenções como se já fossem obrigatórias.

## Estratégia de testes e critérios de aceite

- Testes determinísticos com relógio injetável/controlável, sem esperar minutos reais; testar o domínio onde a autoridade de estado for definida, sem duplicar regras no frontend.
- 60 min de foco geram exatamente 20/15/12 min de descanso para 3/4/5.
- 10 min ativos + 5 min pausados + 20 min ativos resultam em 30 min de foco e 10 min de descanso em 3:1.
- Mudar de 3:1 para 5:1 após 30 min efetivos altera o ganho de 10 para 6 min, inclusive se pausado.
- Finalizar registra uma única sessão, congela sua proporção e não inicia regressivo; esperar antes de iniciar descanso não consome tempo.
- Sem renderizar por 5 min em segundo plano, atualizar a interface revela duração correta, sem depender de 300 ticks.
- Fechar normalmente/reabrir e suspender/retomar preservam tempo já trabalhado e excluem tempo de ausência; retomada é manual.
- Forçar encerramento recupera o último checkpoint válido pausado; tolerância conforme política aprovada.
- Descanso conclui em zero e não inicia foco automaticamente.
- Preferência, histórico e estado sobrevivem a reinício; testar estados de foco ativo/pausado, descanso disponível/ativo/pausado e conclusão.
- Testar gravação indisponível, conteúdo inválido e transições repetidas; não duplicar histórico.
- Testes manuais desktop obrigatórios para suspensão real, minimização, fechamento e persistência. Mocks de relógio não comprovam eventos do SO.
- Verificar integração visual de 98.css, navegação por teclado e janela 800 × 600 no Tauri.
- Limites de arredondamento, ajustes de relógio e múltiplas instâncias precisam de casos derivados das políticas aprovadas.

## Limites do trabalho

- Sempre: distinguir decisões confirmadas de propostas, preservar requisitos de tempo, validar estado persistido e respeitar AGENTS.md.
- Pedir aprovação: plataforma-alvo, política de recuperação, dependências/plugins novos, políticas de relógio e decisões ainda abertas.
- Nunca: implementar antes dos gates, editar artefatos gerados, inventar garantias de recuperação em crashes ou introduzir features fora do MVP.
- Nesta etapa, editar somente documentação de especificação/planejamento; nenhum código de produto.

## Registro de aprovação e próximos gates

O usuário aprovou a especificação, incluindo Windows como escopo inicial, recuperação pelo último checkpoint válido, controles de descanso, arredondamento, foco zero, proporção após finalização e tratamento de falhas de persistência.
Durações devem ser protegidas contra alterações do relógio civil; datas históricas permanecem em horário civil. O PLAN aprovado usa timestamps de `QueryUnbiasedInterruptTime` e eventos nativos de energia.

O usuário também aprovou `tasks/plan.md` em 15/09/2026, incluindo as dependências Rust propostas. Próximo gate: revisar `tasks/todo.md`. Nenhuma implementação está autorizada nesta etapa.
