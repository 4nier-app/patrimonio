# Changelog

Todas as mudanças relevantes do projeto ficam registradas aqui. O app mostra
este histórico na tela "Histórico do sistema".

O formato segue [Keep a Changelog](https://keepachangelog.com/pt-BR/1.1.0/) e
as versões seguem [Versionamento Semântico](https://semver.org/lang/pt-BR/).

## [0.9.0] - Não lançada

### Adicionado
- Aba **Proventos**: dividendos, JCP, rendimentos de FIIs e juros, lançados
  pelo valor recebido (bruto e imposto retido), com data com opcional.
  - Resumo por ano: líquido, bruto, imposto retido, média por mês e últimos
    12 meses; gráfico por mês (ou por ano) separado por tipo.
  - Total por ativo e rendimento sobre o custo dos últimos 12 meses.
  - Valor por cota, pela quantidade que havia na data com.
  - Resumo para a declaração: isentos, tributação exclusiva e exterior
    (Lei 14.754/2023, com estimativa do imposto no ajuste anual).
  - Exterior convertido pela PTAX do dia do pagamento.
  - Cada inclusão, alteração e remoção fica na Auditoria.
  - Proventos não entram na rentabilidade, que mede só a variação de preço.
- A página do ativo mostra os proventos recebidos dele.

### Alterado
- Lançamentos e proventos: depois de incluir, o formulário mantém conta,
  data, operação e ativo (ou fundo, ou tipo de provento), para lançar a
  próxima linha da nota ou do extrato só com quantidade e valores.

### Corrigido
- IPCA: a atualização não mostra mais erro quando o Banco Central responde
  vazio ou fica fora do ar; o app tenta de novo e, com o último mês já
  gravado, informa "em dia" e busca na próxima atualização.

## [0.8.0] - Não lançada

### Adicionado
- **Fundos de investimento**: lançamento pelo extrato do banco, em cotas.
  - O fundo é achado pelo nome (como aparece no extrato) ou pelo CNPJ, na
    lista de fundos da CVM, baixada na primeira vez.
  - Aplicação, resgate (com o IR e o IOF descontados) e come-cotas.
  - Cota diária automática pelo informe que os fundos entregam à CVM.
  - Fundos aparecem na Carteira, no Painel e na Rentabilidade pelo nome;
    não pagam taxa da B3 e não entram no DARF (o banco já retém o imposto).
- **Página do ativo**: clique num ativo da Carteira para ver o gráfico.
  - Velas do dia de 1, 5 ou 15 minutos, atualizando sozinhas a cada minuto
    (B3 com cerca de 15 minutos de atraso; cripto em tempo real), e
    períodos de 5 dias, 1 mês, 6 meses, 1 ano, 5 anos e tudo.
  - Compras e vendas marcadas no gráfico e a linha do seu preço médio.
  - Resumo da posição e os lançamentos do ativo.
  - Clique e arraste no gráfico para ver a variação entre dois pontos.
  - Ao passar o mouse numa vela: abertura, máxima, mínima, fechamento,
    variação, volume e o seu preço médio, em português.
- Aba **Home broker**: busca qualquer ativo do catálogo (B3, EUA, cripto),
  mesmo sem ter na carteira, com atalhos para os seus ativos e os vistos
  recentemente.

### Corrigido
- O aplicativo abre uma vez só: abrir de novo traz para a frente a janela que
  já está aberta, em vez de duas cópias mexendo no mesmo cofre.
- Tela branca ao sair da página do ativo no Windows (o WebView2 mais novo
  devolve uma Promise em `window.scrollTo`, e o React a tratava como função
  de limpeza).
- Se uma tela falhar ao desenhar, o app mostra o erro e um botão para voltar,
  em vez de deixar a janela em branco. O botão "voltar" do mouse fecha a
  página do ativo.
- Qualquer falha que escape das telas aparece numa faixa no rodapé com a
  mensagem e um botão para recarregar; fechar o gráfico não derruba mais a
  janela.

## [0.7.0] - 2026-10-16

### Adicionado
- Aba **Impostos**, com o imposto de renda calculado a partir dos lançamentos:
  - **B3, apuração mensal**: operações comuns (15%, com a isenção de vendas
    de ações até R$ 20 mil no mês), **day trade** (20%, compra e venda do
    mesmo ativo na mesma corretora e no mesmo dia, na quantidade casada) e
    FIIs (20%). Custo pelo preço médio de todas as compras, com as taxas.
  - Prejuízo compensado nos meses seguintes, dentro do mesmo grupo.
  - IRRF estimado (0,005% das vendas comuns e 1% do lucro de day trade)
    abatido do imposto; o que sobra passa para os meses seguintes.
  - **DARF** (código 6015) com valor e vencimento no último dia útil do mês
    seguinte; abaixo de R$ 10,00 o valor soma ao mês seguinte. Botão para
    marcar como pago, registrado na Auditoria.
  - Memória de cálculo de cada mês, com cada venda.
  - **Exterior e criptomoedas, apuração anual** (Lei 14.754/2023): 15% sobre
    o ganho do ano em reais, cada operação pela PTAX da sua data.
  - **Bens e Direitos em 31/12** de cada ano: quantidade e custo de
    aquisição (em reais e, no exterior, também em dólar).
- Aviso no topo do aplicativo quando há DARF a pagar ou atrasado.

### Corrigido
- Criptomoedas voltaram a aparecer como "Criptomoedas" no Painel depois da
  mudança para dólar.

## [0.6.0] - 2026-10-16

### Adicionado
- Aba **Importar notas**: escolha um ou mais PDFs e o aplicativo lê as notas,
  mostra cada uma para conferir e grava os lançamentos.
  - Notas da B3 no padrão SINACOR (Itaú, Nubank e outras), nos dois formatos
    (com e sem "R$"); um PDF pode ter várias notas e uma nota pode ter várias
    folhas.
  - Confirmações da Nomad nos dois layouts (DriveWealth e Apex). A parte
    inteira e a fração da mesma ordem viram um lançamento; vendas de fração
    mínima com valor zero da troca de custodiante vêm desmarcadas.
  - Quando a nota traz o nome de pregão ("EMPRESA PN N2") em vez do código, o
    código é procurado no catálogo da B3; se houver mais de um, você escolhe.
  - Conferência antes de gravar: quantidade × preço de cada linha, valor das
    operações e líquido da nota (operações − taxas − IRRF). Nota que não
    confere não é importada.
  - As taxas da nota entram como taxa de cada lançamento, rateadas pelo valor
    de cada linha.
  - PDF com senha: o aplicativo pede a senha (muitas corretoras usam o CPF).
  - A mesma nota não entra duas vezes, mesmo vinda de outro arquivo; se os
    lançamentos dela forem removidos, ela pode ser importada de novo.
  - Cada importação fica registrada na Auditoria.
- O PDF é lido no próprio computador, com o pdf.js, e não é enviado para
  lugar nenhum.
- **Criptomoedas** em carteira própria (BTC, ETH, SOL, USDT e outras 20):
  lançamento à mão numa conta em dólar, quantidade e preço com até 8 casas,
  sem taxas da B3. A conversão para reais usa a PTAX do dia, como no
  exterior. A cotação diária em dólar vem do Yahoo (par BTC-USD). No Painel
  aparecem como "Criptomoedas".

### Alterado
- Aba **Rentabilidade** refeita, sem rolagem para os lados: a carteira em
  cima, com o ano e o "desde o início", e os 12 meses em blocos; os ativos
  embaixo, separados em "Na carteira" e "Já vendidos", e um clique abre os
  meses do ativo. Cada ano lista só os ativos com rentabilidade nele.
- Rentabilidade também tem o botão "Desde o início", com um bloco por ano.
- Sobra de fração menor que 0,0001 cota (arredondamento da corretora ou troca
  de custodiante) conta como posição zerada: o ativo não aparece mais com
  R$ 0,00 na Carteira e no Painel.
- Mês sem lançamento fica em branco, sem traço; falta de cotação aparece
  escrita.

### Corrigido
- A atualização mostrava falha no CDI quando o Banco Central ainda não tinha
  publicado o dia; agora aparece "em dia".
- Operações antigas lançadas depois das mais novas ficavam
  sem cotação e a Rentabilidade mostrava só "—". Agora o histórico de preços é
  buscado desde a operação mais antiga, também nos meses da B3 já baixados que
  não tinham o ativo novo.

## [0.5.0] - Não lançada

### Adicionado
- Aba **Painel**, a primeira ao abrir:
  - Período dos gráficos: 12 meses, este ano, desde o início, um ano
    escolhido (sempre com os 12 meses, de janeiro a dezembro) ou meses
    escolhidos (de/até). O mesmo mês nos dois mostra os dias úteis dele.
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
  - Clique em qualquer cartão para ver a memória de cálculo.
- CDI e IPCA do Banco Central (SGS) e Ibovespa e IVVB11 (Yahoo) baixados
  automaticamente desde a primeira operação.
### Alterado
- Código do mercado fracionário da B3 (ABCD3F, como aparece nas notas) é
  gravado como o do lote padrão (ABCD3): mesmo ativo, cotação e preço médio.
- Remover um lançamento não pede mais motivo: basta confirmar.
- Lista de lançamentos com filtros de ano (o mais recente ao abrir), mês,
  conta e ativo. Lançamentos da mesma conta no mesmo dia (uma nota, importada
  ou digitada) ficam agrupados numa linha que abre com um clique, com o botão
  "Remover todos" para tirar o grupo inteiro de uma vez.
- Depois de incluir, alterar, remover ou importar lançamentos, as cotações são
  atualizadas sozinhas.


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
