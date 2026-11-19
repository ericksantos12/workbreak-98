# Changelog

As versões mais recentes aparecem primeiro. Cada release usa a versão e as notas da primeira seção numerada.

## [0.2.0] - 2026-11-19

### Adicionado

- Herói em pixel art que ganha experiência e sobe de nível com foco saudável: 1 XP por minuto completo de foco (até 120 por bloco) e bônus de 20% ao cumprir pelo menos 90% do descanso.
- Painel "Herói" com nível, barra de progresso e aviso de recompensa ao concluir um bloco, respeitar o descanso ou subir de nível.
- Cenário side scrolling com parallax durante o foco e acampamento com tenda e fogueira durante o descanso.
- Inimigos que o mago derrota com magia durante o foco, maiores conforme o bloco se estende: slime, goblin, ogro e golem.
- Arte gerada a partir de código com a skill pxl (`art/*.px.js`).
- Automação no GitHub Actions: checks em cada push e PR, instalador de teste na `dev`, PR de release em rascunho e publicação da release na `main` a partir deste changelog.

### Alterado

- O progresso salvo passa a incluir a experiência do herói. Sessões salvas por versões anteriores continuam abrindo normalmente.

## [0.1.0] - 2026-11-03

### Adicionado

- Cronômetro Flowmodoro para Windows com visual Windows 98.
- Proporções de foco e descanso 3:1, 4:1 e 5:1.
- Pausa, retomada e recuperação local da sessão.
- Aviso sonoro ao terminar o descanso e retomada manual do foco.
- Ícone de monitor clássico e instalador Windows.
