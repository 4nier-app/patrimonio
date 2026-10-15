# Changelog

Todas as mudanças relevantes do projeto ficam registradas aqui. O app mostra
este histórico na tela "Histórico do sistema".

O formato segue [Keep a Changelog](https://keepachangelog.com/pt-BR/1.1.0/) e
as versões seguem [Versionamento Semântico](https://semver.org/lang/pt-BR/).

## [0.3.0] - Não lançada

### Adicionado
- Contas em dólar (exterior): cada corretora tem a sua moeda, definida no
  primeiro lançamento pelo ativo (ação dos EUA → dólar; da B3 → reais), com
  opção de corrigir.
- Catálogo de ações e ETFs dos EUA (Nasdaq Trader) e cotações pelo Yahoo
  Finance desde a primeira compra.
- PTAX de venda de fechamento do Banco Central, baixada automaticamente desde a
  primeira operação em dólar (Lei 14.754/2023, art. 15).
- Carteira com os ativos do exterior em dólar e em reais: custo pela PTAX de
  cada compra, valor atual pela PTAX mais recente. Totais da carteira em reais.
- Rentabilidade em reais de toda a carteira, e para cada ativo do exterior
  também em dólar e o efeito do câmbio:
  (1 + em reais) = (1 + em dólar) × (1 + câmbio).
- Transferência de custódia entre corretoras (ex.: troca de custodiante no
  exterior): a posição muda de conta mantendo preço médio e datas; um resíduo
  que ficou para trás sai da carteira sem gerar venda.

### Alterado
- Preço unitário e taxas com máscara de caixa: só números, preenchendo da
  direita para a esquerda (digitar 1 mostra R$ 0,01). Em dólar, o preço tem 4
  casas decimais.
- Operações em dólar não recebem as taxas da B3.
- Relatório de atualização mostra a data e a hora em que cada catálogo foi
  atualizado.

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
- Máscara nos campos de quantidade e preço ("1.500", "R$ 1.234,56").

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
