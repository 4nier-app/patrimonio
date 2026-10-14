# Changelog

Todas as mudanças relevantes do projeto ficam registradas aqui. O app mostra
este histórico na tela "Histórico do sistema".

O formato segue [Keep a Changelog](https://keepachangelog.com/pt-BR/1.1.0/) e
as versões seguem [Versionamento Semântico](https://semver.org/lang/pt-BR/).

## [0.2.0] - Não lançada

### Adicionado
- Lançamentos de compra e venda na B3, com alteração e remoção registradas na
  Auditoria (estado antes e depois, e o motivo).
- Taxas da B3 calculadas automaticamente por nota (mesma corretora, mesmo
  pregão) e mostradas com a fórmula; opção de informar a taxa à mão.
- Catálogo de ativos da B3 com busca pelo código ou nome.
- Atualização de cotações sob demanda: fechamentos oficiais da B3 (arquivo
  COTAHIST) para meses encerrados e Yahoo Finance para o mês corrente. Sem
  internet, o app usa as últimas cotações gravadas.
- Carteira: quantidade, preço médio, custo, valor de mercado, resultado em
  aberto e resultado realizado.
- Rentabilidade mensal e anual por ativo e da carteira, pelo tempo real de
  exposição do capital (Modified Dietz diário), com a memória de cálculo de
  cada valor.
- Marca 4Nier: logo, ícone do aplicativo e visual renovado.
- Botão "Atualizar cotações" no topo, disponível em qualquer aba.

## [0.1.0] - Não lançada

### Adicionado
- Fundação do aplicativo desktop (Tauri 2 + React).
- Cofre local: banco SQLite criptografado (SQLCipher, AES-256) com chave
  derivada da senha por Argon2id. A senha nunca é gravada.
- Histórico de eventos somente-inclusão com cadeia de hash SHA-256 e
  verificação de integridade.
- Histórico do sistema: versão instalada, data da instalação e changelog.
- Cálculo das taxas da B3 por nota (liquidação, emolumentos e transferência),
  com truncamento no centavo e rateio entre as operações.
