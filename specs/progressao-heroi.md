# Especificação: Progressão do Herói (MVP — fase 1)

Status: APROVADA e implementada em 19/11/2026 (pedido do usuário via autopilot, com as propostas das decisões em aberto e sprites gerados com a skill pxl).

## Objetivo

Aumentar o engajamento com um personagem de RPG em pixel art que ganha experiência (XP) e sobe de nível conforme o usuário completa blocos de foco e respeita o descanso.

O XP deve recompensar **foco saudável**, não cronômetro ligado. O descanso também conta, porque é o que diferencia o Flowmodoro.

## Escopo da fase 1

Incluído:

- XP por bloco de foco finalizado e bônus por descanso cumprido.
- Nível e progresso até o próximo nível, persistidos entre sessões e entre reinícios.
- Sprite do herói com estados visuais ligados à fase do timer.
- Aviso de recompensa ao finalizar foco, ao cumprir descanso e ao subir de nível.

Fora da fase 1 (fases futuras, cada uma com spec própria):

- Categorias de foco e atributos (fase 2).
- Diário de aventura (fase 2).
- Debuff de fadiga por pular descanso (fase 2, se a fase 1 mostrar necessidade).
- Loot, inventário, conquistas, sequência diária e ícone na bandeja (fase 3).
- Detecção de ociosidade (`GetLastInputInfo`) contra trapaça.
- Nome, classe ou customização do herói.

## Relação com a spec aprovada

`specs/flowmodoro.md` diz: "Não há histórico de foco no produto". Esta spec mantém a regra: a progressão guarda **apenas agregados** (XP total e estado pendente), não uma lista de sessões, datas ou durações.

Nenhuma regra temporal da spec aprovada muda. O cálculo de foco, descanso, pausa, suspensão e recuperação continua igual. A progressão só **lê** os resultados das transições.

## Regras de XP

Unidade interna: XP inteiro (`u64`). O tempo continua em 100 ns no Rust, como no `timer.rs`.

### XP de foco

- Bloco de foco: o foco efetivo acumulado entre `start_focus` e `finish_focus` de um mesmo ciclo. Pausas não contam, já que `effective_at` exclui pausas.
- **1 XP por minuto completo** de foco efetivo no bloco, arredondado para baixo.
- **Teto de 120 XP por bloco** (2 h). Foco além disso não rende XP extra. O teto evita incentivar blocos exaustivos.
- O XP de foco é concedido **na transição `finish_focus`**, na mesma escrita que inicia o descanso.
- Bloco com foco efetivo zero (volta a `Idle`) não gera XP.
- `reset_focus` antes de finalizar descarta o bloco atual sem gerar XP. O XP já concedido nunca é removido.

Observação: `focusElapsedMs` é cumulativo na sessão (preservado entre ciclos). Por isso, o bloco é medido como `foco_efetivo_na_finalização − foco_efetivo_no_início_do_bloco`. O início fica gravado em `focusBlockStart100ns` ao iniciar o foco.

### Bônus de descanso

- Ao finalizar o foco, o bônus pendente é calculado como **20% do XP de foco do bloco**, arredondado para baixo, e fica guardado como `pendingRestBonusXp`.
- O bônus é concedido quando o descanso chega a `BreakCompleted` **e** o usuário descansou pelo menos **90%** do saldo disponível naquele descanso:
  - `descansado = break_earned − max(remaining, 0)`
  - concede se `descansado ≥ 0,9 × break_earned`
- `break_earned` é o saldo disponível no descanso, incluindo saldo positivo trazido de descansos anteriores encerrados cedo. Por isso, encerrar cedo torna o próximo limiar de 90% maior.
- Isso cobre o esgotamento natural (via `normalize_break`) e o encerramento manual perto do fim (`end_break`).
- Se `end_break` ocorrer antes dos 90%, o bônus pendente é descartado. Não existe penalidade além disso na fase 1.
- Se o saldo disponível for ≤ 0 (`start_break` vai direto a `BreakCompleted`), o bônus é descartado: não houve descanso para respeitar.
- `reset_focus` durante o descanso descarta o bônus pendente.
- Pausas e suspensão durante o descanso não afetam o bônus; só o tempo ativo de descanso conta, como já acontece no timer.

### Exemplos (proporção irrelevante para XP)

| Bloco | XP de foco | Bônus pendente | Descansou ≥ 90%? | Total |
| --- | --- | --- | --- | --- |
| 25 min 40 s | 25 | 5 | sim | 30 |
| 50 min | 50 | 10 | não (encerrou com 50%) | 50 |
| 3 h | 120 (teto) | 24 | sim | 144 |
| 0 min 50 s | 0 | 0 | — | 0 |

## Níveis

- Nível inicial: 1, com 0 XP.
- XP para ir do nível `n` ao `n + 1`: **`60 + 30 × (n − 1)`**.
  - 1 → 2: 60 XP (~1 h de foco)
  - 2 → 3: 90 XP
  - nível 10 acumulado: 1.620 XP (~27 h de foco)
- Nível máximo: 99. Ao atingir 99, o XP total continua somando e a barra mostra cheia.
- Nível e XP no nível são **derivados** do XP total. Persistir apenas o total evita estado inconsistente.
- Um único ganho pode subir mais de um nível. O aviso informa o nível final.

## Estados visuais do herói

O herói é um pequeno mago (chapéu pontudo azul com faixa dourada, capa clara e cajado com gema), arte original inspirada em sprites clássicos de mago. Ele aparece num cenário side scrolling em pixel art.

| Fase do timer | Cena | Pose | Legenda |
| --- | --- | --- | --- |
| `idle`, `breakCompleted` | trilha parada | em pé, piscando | "Pronto para a aventura" |
| `focusRunning` | trilha rolando (parallax) | andando | "Em aventura" |
| `focusPaused` | trilha parada | sentado, cochilando | "Cochilando" |
| `breakReady`, `breakRunning` | acampamento ao entardecer | sentado junto à fogueira | "Descansando no acampamento" |
| `breakPaused` | acampamento ao entardecer | sentado, cochilando | "Cochilando" |

- **Trilha:** camadas de céu, nuvens, montanhas nevadas, colinas com árvores e chão de terra. Cada camada repete na horizontal e rola a uma velocidade (nuvens mais lentas, chão mais rápido). Ao pausar o foco, o cenário congela onde estava.
- **Inimigos:** durante `focusRunning`, um inimigo entra pela direita a cada 4 s, o mago dispara uma magia do cajado e o inimigo vira fumaça. O tamanho cresce com o foco efetivo do bloco atual (`focusBlockMs`, calculado no Rust): slime até 10 min, goblin até 25, ogro até 50 e golem de pedra depois disso. A sequência é só CSS; com `prefers-reduced-motion` os inimigos não aparecem.
- **Acampamento:** o mesmo mundo escurecido por filtro CSS, com tenda de lona e fogueira de toras cruzadas, com chamas e faíscas animadas.
- O estado e a cena derivam só de `snapshot.phase`. Nenhum timer novo no React: as animações são CSS (`steps()` nas tiras de quadros e `background-position` nas camadas).
- Respeitar `prefers-reduced-motion`: sem animação, mostrar o primeiro quadro.
- A legenda textual (rótulo da cena) garante que o estado não dependa só da imagem.

## Recompensas na interface

- Cada concessão de XP gera uma recompensa com `id` crescente, que o snapshot expõe como `lastReward`:
  - `kind`: `"focus"` ou `"rest"`
  - `xp`
  - `levelUpTo`: o novo nível, ou `null`
- O React mostra um aviso no estilo de caixa de diálogo do 98.css quando recebe um `lastReward.id` maior que o último exibido nesta execução. Ao abrir o app, o `id` atual conta como já visto, para não repetir o aviso após reinício.
- Textos:
  - foco: "Bloco concluído! +25 XP"
  - descanso: "Descanso respeitado! +5 XP"
  - nível: "Subiu para o nível 3!"
- O aviso fecha com um botão "OK" e não bloqueia os controles do timer.
- O aviso sonoro de fim de descanso já existente não muda. Nenhum som novo na fase 1.

## Painel do herói

- Novo `fieldset` "Herói" em faixa abaixo do cronômetro, com:
  - a cena de 160 × 48 px em escala 2× (320 × 96) com `image-rendering: pixelated`
  - "Nível N"
  - barra de progresso do 98.css (`.progress-indicator`, presente no 98.css 0.1.21 instalado)
  - "X / Y XP" para o próximo nível
- Tudo deve caber na janela de 800 × 600 sem sobreposição. Usar rolagem se necessário, como na spec aprovada.
- A barra de progresso precisa de rótulo acessível ("Progresso para o nível N+1").

## Arquitetura

- **Rust é a autoridade da progressão**, como já é do tempo. O React só exibe.
- Novo módulo `src-tauri/src/progression.rs` com lógica pura e testável, sem I/O nem relógio:
  - `on_focus_started(focus_elapsed_100ns)`
  - `on_focus_finished(focus_elapsed_100ns) -> Option<Reward>`
  - `on_break_completed(break_earned_100ns, remaining_100ns) -> Option<Reward>`
  - `on_reset()`
  - funções derivadas: `level()`, `xp_into_level()` e `xp_for_next_level()`
- `service.rs` chama esses ganchos dentro das transições existentes, sob o mesmo lock e antes do mesmo `persist_at`. Assim, a progressão é gravada atomicamente com o timer.
  - `start_focus` → `on_focus_started`
  - `finish_focus` → `on_focus_finished`
  - `normalize_at` (esgotamento), `end_break` e `start_break` com saldo ≤ 0 → `on_break_completed`
  - `reset_focus` → `on_reset`
- Recuperação após crash reaproveita o checkpoint: como a progressão é gravada na mesma escrita da transição, não pode haver XP concedido duas vezes nem XP sem a transição correspondente.

### Persistência

`AppDocument` ganha o campo `progression` com `#[serde(default)]`:

```json
{
  "revision": 12,
  "timer": { "...": "inalterado" },
  "progression": {
    "totalXp": 345,
    "focusBlockStart100ns": 0,
    "pendingRestBonusXp": 0,
    "lastReward": { "id": 7, "kind": "focus", "xp": 25, "levelUpTo": null }
  }
}
```

- Documentos atuais, sem `progression`, carregam com XP zerado e com o bloco atual começando no foco já acumulado, para não conceder ciclos anteriores da sessão. `STORAGE_VERSION` permanece 1.
- Validação ao carregar:
  - `pendingRestBonusXp` maior que 24 (20% do teto) é inválido
  - `focusBlockStart100ns` maior que o foco efetivo é inválido
  - documento inválido segue a política aprovada: não sobrescrever e informar a falha

### Snapshot

`TimerSnapshot` (Rust e `src/flowmodoro.ts`) ganha `focusBlockMs` (foco efetivo do bloco atual, 0 fora do foco) e:

```ts
progression: {
  level: number;
  xpIntoLevel: number;
  xpForNextLevel: number; // 0 no nível máximo
  totalXp: number;
  lastReward: { id: number; kind: "focus" | "rest"; xp: number; levelUpTo: number | null } | null;
};
```

Nenhum comando Tauri novo: o snapshot já é consultado por `get_timer_snapshot` e devolvido por `apply_timer_action`.

### Assets

- Fonte da arte (DSL da skill pxl, instalada em `.claude/skills/pxl`):
  - `art/hero.px.js`: poses do mago (`hero_idle`, `hero_walk`, `hero_sit`, `hero_sleep`), quadros de 24 × 28 px com os pés na mesma linha.
  - `art/enemies.px.js`: inimigos `foe_slime`, `foe_goblin`, `foe_ogre`, `foe_golem` (2 quadros, virados para a esquerda), `spell_bolt` e `foe_poof`.
  - `art/world.px.js`: camadas `world_sky`, `world_clouds`, `world_mountains`, `world_hills` e `world_ground` (160 × 48, repetem na horizontal), adereços `world_camp` e a animação `camp_fire`. As cenas `preview_*` servem só para conferência.
- Build: `node .claude/skills/pxl/bin/pxl.js build art/<arquivo>.px.js --out art/out` e copiar os PNGs listados no topo de cada arquivo para `src/assets/hero/`. `art/out` é ignorado pelo git.

## Critérios de aceite

Testes Rust (`progression.rs`, determinísticos, sem relógio):

- 25 min 40 s de bloco geram 25 XP de foco e 5 de bônus pendente.
- Bloco de 3 h gera 120 XP (teto) e 24 de bônus pendente.
- Bloco de 50 s gera 0 XP e nenhuma recompensa.
- Descanso com ≥ 90% cumprido concede o bônus. Com 89%, descarta.
- Saldo de descanso ≤ 0 descarta o bônus.
- Curva de nível: 59 XP = nível 1; 60 XP = nível 2 com 0/90; 1.620 XP = nível 10.
- Um ganho que atravessa dois níveis informa `levelUpTo` com o nível final.
- Nível nunca passa de 99, e `xpForNextLevel` vale 0 no 99.

Testes Rust (`service.rs`, com `FakeClock`):

- 10 min ativos + 5 min pausado + 20 min ativos, depois finalizar: +30 XP.
- Segundo ciclo da mesma sessão mede só o novo bloco, não o foco cumulativo.
- `reset_focus` em foco não gera XP. `reset_focus` em descanso descarta o bônus e mantém o XP já ganho.
- Finalizar, simular crash e recuperar: XP presente uma única vez. O bônus pendente sobrevive e é concedido ao completar o descanso recuperado.
- Documento sem `progression` carrega com XP 0 e regrava com o campo.
- Documento com `pendingRestBonusXp` inválido bloqueia a sobrescrita.
- Comandos repetidos ou com revisão antiga não concedem XP extra.

Testes Vitest:

- Painel mostra nível, barra e "X / Y XP" a partir do snapshot.
- Mapeamento fase → estado do sprite e legenda, conforme a tabela.
- Aviso aparece uma vez por `lastReward.id` novo. Não aparece para o `id` já existente ao carregar.

Manual (desktop):

- Janela 800 × 600 sem sobreposição, navegação por teclado e foco visível no botão "OK".
- Animações param com `prefers-reduced-motion`.
- Suspender durante o foco, retomar e finalizar: XP só do tempo ativo.

## Decisões tomadas

Aprovadas com a implementação:

1. **Arte do herói.** Opções:
   - (a) spritesheet desenhada pelo usuário ou encomendada
   - (b) pixel art simples gerada no projeto como placeholder, trocável depois
   - (c) pacote livre com licença compatível (CC0)

   Decidido: pixel art gerada no projeto com a skill pxl (`art/hero.px.js`).
2. **Números de balanceamento:** 1 XP/min, teto de 120, bônus de 20%, limiar de 90% e curva `60 + 30(n−1)`. Decidido: aceitos; reavaliar após uma semana de uso.
3. **Posição do painel:** faixa abaixo do cronômetro (a coluna lateral não comportava o sprite em 800 × 600).
4. **Opção de esconder o herói** nas configurações. Decidido: fora da fase 1.

## Limites do trabalho

- Sempre: Rust como autoridade, sem timers novos no React, progressão gravada na mesma escrita da transição e respeito ao `AGENTS.md`.
- Pedir aprovação: dependências novas, mudança de `STORAGE_VERSION` e qualquer alteração nas regras temporais.
- Nunca: remover XP já concedido, criar histórico de sessões, penalizar o usuário na fase 1 ou implementar antes da aprovação desta spec.
