# 4Nier · Patrimônio Auditável

Controle de investimentos B3 + exterior com rentabilidade auditável, 100% local e gratuito.

> **Status:** em desenvolvimento (versão 0.1, fundação). Ainda não controla
> operações; esta versão traz o cofre criptografado, o histórico de auditoria e
> o cálculo das taxas da B3.

## Por que existe

- **Rentabilidade pelo tempo real de exposição:** o capital conta a partir da
  data exata da compra e deixa de contar na data exata da venda (Modified Dietz
  diário por mês, encadeado no ano).
- **Nada é caixa-preta:** cada número mostra a operação, a cotação, o câmbio
  (PTAX), as taxas e a fórmula usada, com os valores substituídos.
- **Brasil + exterior** consolidados em reais, com a variação cambial separada.
- **Seus dados ficam com você:** tudo roda localmente, num banco criptografado
  pela sua senha. Nenhum servidor, nenhuma conta, nenhum custo.

## Instalação (Windows)

Enquanto não há versão publicada, o instalador de teste é gerado
automaticamente a cada alteração: na aba **Actions** do GitHub, abra a execução
mais recente do CI e baixe o artefato `instalador-windows`.

Na primeira execução, o Windows pode mostrar "O Windows protegeu o computador",
porque o instalador ainda não tem assinatura digital. Clique em **Mais
informações → Executar assim mesmo**.

## Como funciona

| Parte | Tecnologia |
|---|---|
| Aplicativo desktop | [Tauri 2](https://tauri.app) |
| Cálculos e banco | Rust, com aritmética decimal exata |
| Interface | React + TypeScript |
| Armazenamento | SQLite criptografado (SQLCipher, AES-256), chave Argon2id |
| Auditoria | Log de eventos somente-inclusão com cadeia de hash SHA-256 |

Arquitetura completa, fórmulas e roteiro: [docs/ARQUITETURA.md](docs/ARQUITETURA.md).

## Desenvolvimento

Pré-requisitos: [Rust](https://rustup.rs), Node.js 22 e os
[pré-requisitos do Tauri](https://tauri.app/start/prerequisites/) do seu sistema.

```bash
npm install
npm run tauri dev        # abre o app em modo de desenvolvimento
cargo test --workspace   # testes do núcleo
npm test                 # testes da interface
npm run tauri build      # gera o instalador
```

Estrutura:

```
crates/core      regras de cálculo (taxas B3; rentabilidade vem a seguir)
crates/ledger    log de eventos com cadeia de hash
crates/storage   cofre criptografado, migrações, histórico do sistema
src-tauri        ponte entre o núcleo e a interface
src              interface React
```

## Privacidade

Notas de corretagem e extratos reais **nunca** entram neste repositório, nem
anonimizados. Os testes usam apenas notas fictícias em `fixtures/sinteticas/`,
e o CI recusa qualquer PDF ou planilha fora dessa pasta.

## Aviso

Este software é uma ferramenta de apoio. Cálculos de imposto devem ser
conferidos com um contador. Não há garantia de nenhum tipo (veja a licença).

## Licença

[GPL-3.0-or-later](LICENSE). Você pode usar, estudar, modificar e redistribuir;
versões modificadas distribuídas também devem ser abertas sob a GPL.
