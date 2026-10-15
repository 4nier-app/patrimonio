# Changelog

Todas as mudanças relevantes do projeto ficam registradas aqui. O app mostra
este histórico na tela "Histórico do sistema".

O formato segue [Keep a Changelog](https://keepachangelog.com/pt-BR/1.1.0/) e
as versões seguem [Versionamento Semântico](https://semver.org/lang/pt-BR/).

## [0.5.0] - Não lançada

### Adicionado
- Aba **Painel**, a primeira ao abrir, com filtro de período (12 meses, este
  ano, desde o início):
  - Cartões de rentabilidade (desde o início, 12 meses e último mês) com o CDI
    do mesmo período e a diferença em pontos percentuais.
  - Comparação com CDI, IPCA, Ibovespa, IVVB11 e dólar, num gráfico só com
    botão "%" / "R$". Em %: rentabilidade acumulada, medida nos mesmos dias em
    que havia dinheiro na carteira. Em R$: quanto você teria se cada compra e
    venda tivesse ido para o índice, nas mesmas datas.
  - Evolução do patrimônio: valor aplicado e ganho de capital por mês.
  - Lucro ou prejuízo em reais por mês ou por ano.
  - O que puxou o resultado: lucro de cada ativo no período.
  - Ativos na carteira por tipo (rosca e lista que abre os ativos).
  - Queda desde o pico.
  - Lucro realizado (vendas) e não realizado (no papel) por ativo.
  - Linha do tempo das posições: o tempo real de cada ativo na carteira.
  - Clique em qualquer cartão, barra ou ponto para ver a memória de cálculo.
- CDI e IPCA do Banco Central (SGS) e Ibovespa e IVVB11 (Yahoo) baixados
  automaticamente desde a primeira operação.

## [0.4.0] - Não lançada

### Adicionado
- Chave de recuperação: criada junto com o cofre (ou depois, na tela
  Segurança). O aplicativo abre o seu programa de e-mail com a chave numa
  mensagem para você mesmo; nada é enviado pela internet pelo aplicativo.
- "Esqueci minha senha" na tela de entrada: com a chave de recuperação, cria
  uma senha nova sem perder nenhum dado.
- Backup: "Fazer backup agora" grava um arquivo único (.cabackup) onde você
  escolher, ainda criptografado. Também há um backup automático por semana na
  pasta de dados (ficam os 5 mais recentes).
- Restaurar backup, na tela de entrada (útil num computador novo) e na tela
  Segurança. O backup é conferido (senha, versão e cadeia de hash) antes de
  substituir os dados atuais, que ficam guardados na pasta "anteriores".
- Aviso no topo enquanto a carteira não tiver chave de recuperação.

### Alterado
- O cofre passa a usar uma chave de banco aleatória protegida pela senha e pela
  chave de recuperação. Cofres da versão 0.3.0 são convertidos na primeira
  abertura, sem recriptografar os dados.
- Trocar a senha não invalida a chave de recuperação.
- Preço em dólar com a mesma máscara de caixa do real, só números, sempre com
  8 casas (53473610000 vira US$ 534,73610000). Ao colar um valor das notas do
  exterior, ponto ou vírgula valem como decimal (534.7361 vira
  US$ 534,73610000). Na conta em dólar, a quantidade usa o ponto como decimal
  (1.234 = 1,234).
- O histórico de versões do aplicativo saiu da barra de abas e virou a seção
  "Versões do aplicativo", no fim da aba Auditoria.
- Relatório de atualização: os catálogos aparecem como "lista de ativos", com
  a data em que foram baixados e a data da próxima atualização (a cada 7 dias).
  Os preços do dia continuam sendo buscados a cada abertura.

## [0.3.0] - 2026-10-15

### Adicionado
- Contas em dólar (exterior): a moeda de cada corretora é definida no primeiro
  lançamento, sem campo para preencher: corretoras do exterior (Nomad, Avenue
  e outras) ficam em dólar; nas demais vale o ativo (ação dos EUA → dólar; da
  B3 → reais).
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
  direita para a esquerda (digitar 1 mostra R$ 0,01). Em dólar, o preço
  aceita vírgula e até 8 casas decimais, como o preço médio das notas da Nomad.
- Operações em dólar não recebem as taxas da B3.
- Ao abrir o cofre, o app atualiza sozinho catálogos, cotações e PTAX em
  segundo plano (o botão continua disponível).
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
