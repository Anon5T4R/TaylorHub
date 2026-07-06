# TaylorHub

**Instalador e central da suíte Taylor** — instala, atualiza e abre os apps
([LocalOffice](https://github.com/Anon5T4R/LocalOffice),
[LocalSheets](https://github.com/Anon5T4R/LocalSheets),
[LocalSlides](https://github.com/Anon5T4R/LocalSlides),
[LocalCode](https://github.com/Anon5T4R/LocalCode),
[TaylorMind](https://github.com/Anon5T4R/TaylorMind),
[OpenObsidian](https://github.com/Anon5T4R/OpenObsidian)) e faz cada tipo de
arquivo abrir no app certo.

## Como funciona

- **Catálogo-driven**: os apps vêm de um catálogo (`src/catalog.ts`). App novo = uma entrada nova — sem mudança de código.
- **Instalar/atualizar**: baixa a última release do GitHub de cada app e roda o instalador em modo silencioso (`/S` no Windows; AppImage → `~/Applications` + entrada de menu no Linux).
- **Associações de arquivo**: o Hub registra-se como handler (`taylorhub --open arquivo`) e **despacha** pro app escolhido na aba *Arquivos* — trocar quem abre `.md` é um clique, sem mexer no registro à mão.
- **Privacidade**: as únicas chamadas de rede são ao GitHub (releases), disparadas por você. Zero telemetria.

## Desenvolvimento

```bash
npm install
npm run tauri dev    # app completo (Tauri)
npm run dev          # só a UI no navegador (instalação desabilitada)
npm run tauri build  # instalador NSIS / AppImage
```

Ícones: gerados de `src-tauri/icons/source-hub.svg` via `npm run tauri icon src-tauri/icons/source-hub.svg`.

## Plataformas

- **Windows** (NSIS) e **Linux** (AppImage). macOS fica pra quando os apps da suíte tiverem builds de Mac.

## Licença

MIT.
