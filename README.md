# WorkBreak 98

Um cronômetro Flowmodoro para Windows, com a aparência clássica do Windows 98. Trabalhe no seu ritmo e acumule descanso proporcional ao tempo de foco.

**Tauri 2 · Rust · React 19 · TypeScript · 98.css**

## Como funciona

1. Clique em **Iniciar foco** e trabalhe pelo tempo que fizer sentido.
2. Pause e retome quando precisar. Apenas o tempo ativo conta.
3. Clique em **Finalizar foco** para começar o descanso automaticamente.
4. Quando o saldo de descanso acabar, o aplicativo emite um aviso sonoro do Windows e volta à tela de foco.
5. Clique em **Iniciar novo foco** quando estiver pronto. O foco não recomeça automaticamente.

Escolha a proporção entre foco e descanso:

| Proporção | A cada… | Você acumula… |
| --- | --- | --- |
| **3:1** (padrão) | 3 minutos de foco | 1 minuto de descanso |
| **4:1** | 4 minutos de foco | 1 minuto de descanso |
| **5:1** | 5 minutos de foco | 1 minuto de descanso |

Por exemplo, 30 minutos de foco na proporção 3:1 rendem 10 minutos de descanso. Alterar a proporção durante o foco recalcula o saldo; durante o descanso, preserva o tempo já concedido.

Os tempos da sessão são preservados ao voltar ao foco. Para zerá-los, use **Reiniciar sessão** e confirme a ação.

## Sessão local e recuperação

- O estado é salvo localmente, sem conta ou sincronização.
- Ao fechar o aplicativo ou suspender o computador, o cronômetro é pausado. A retomada é manual.
- Minimizar a janela mantém o cronômetro funcionando.
- Em caso de encerramento inesperado, a recuperação usa o último checkpoint salvo e mantém a sessão pausada. O trecho ainda não salvo pode ser perdido.
- O aviso sonoro depende das configurações de áudio do Windows.

## Executar a partir do código

A implementação nativa atual usa APIs do Windows.

Instale os seguintes requisitos:

- Node.js **22.12+ na linha 22**, **24.x** ou **26+**, acompanhado do npm.
- Rust e Cargo pelo Rustup, com a toolchain MSVC.
- Visual Studio Build Tools com a carga **Desenvolvimento para desktop com C++** e o Windows SDK.
- Microsoft Edge WebView2 Runtime.

Na raiz do repositório:

```powershell
npm ci
npm run app
```

O script prepara o `PATH` do Cargo no Windows e inicia o Tauri junto com o Vite. A porta **1420** precisa estar livre; não é necessário executar `npm run dev` antes.

## Gerar o instalador Windows

```powershell
npm run build:exe
```

O comando compila o frontend e o aplicativo Rust e empacota um instalador `.exe` com NSIS.

| Saída | Local |
| --- | --- |
| Instalador `.exe` | `src-tauri/target/release/bundle/nsis/` |
| Executável da aplicação | `src-tauri/target/release/workbreak-98.exe` |

## Desenvolvimento

| Comando | O que faz |
| --- | --- |
| `npm run app` | Executa o aplicativo desktop com recarga durante o desenvolvimento |
| `npm run dev` | Executa apenas o frontend; não disponibiliza os comandos nativos do timer |
| `npm run build` | Verifica TypeScript e gera o frontend em `dist/`; não compila Rust |
| `npm run build:exe` | Compila o aplicativo e gera o instalador Windows |
| `npx vitest run` | Executa os testes frontend |
| `cargo test --manifest-path src-tauri/Cargo.toml` | Executa os testes Rust |
| `cargo check --manifest-path src-tauri/Cargo.toml` | Verifica o código Rust sem gerar o instalador |

### Organização do código

O Rust controla o tempo, as transições e a persistência. O React consulta o estado pelo Tauri e apresenta os controles usando 98.css.

| Caminho | Responsabilidade |
| --- | --- |
| `src/components/TimerPanel.tsx` | Relógios, configuração e ações da sessão |
| `src/flowmodoro.ts` e `src/useFlowmodoro.ts` | Comunicação com o serviço nativo e atualização da interface |
| `src-tauri/src/timer.rs` | Regras de foco, descanso e proporções |
| `src-tauri/src/service.rs` | Coordenação do estado e dos salvamentos |
| `src-tauri/src/storage.rs` | Persistência local em JSON |
| `src-tauri/src/power.rs` | Integração com suspensão e retomada do Windows |

---

Este projeto está sendo **vibecodado com Codex**.
