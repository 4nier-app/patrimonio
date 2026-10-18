# Arquitetura: Controle de Investimentos e Rentabilidade Auditável

Versão 0.2 · 14/10/2026

## 1. Resumo das decisões

| Tema | Decisão | Por quê |
|---|---|---|
| Plataforma | App desktop **Tauri 2** (Windows, macOS, Linux) | Instalador de ~10–15 MB, usa o navegador do próprio sistema, sem servidor, sem nuvem, gratuito e "plug and play". Electron gera 150 MB+; app web exigiria hospedagem. |
| Núcleo (cálculos, banco, cotações) | **Rust** com `rust_decimal` | Aritmética decimal exata (nunca `float` em dinheiro), binário único, testes rápidos. |
| Interface | **React + TypeScript + Vite**, gráficos **Apache ECharts**, tabelas **TanStack Table** | Maior base de contribuidores open source; tudo licença livre. |
| Banco | **SQLite + SQLCipher** (AES-256) via `rusqlite` | Arquivo único local, criptografado, fácil de exportar/importar. |
| Acesso | Senha local → **Argon2id** → chave do banco | Sem servidor de autenticação. Sem senha, o arquivo é ilegível. |
| Auditoria | **Log de eventos imutável com cadeia de hash** + **memória de cálculo** gravada em cada resultado | Nenhum número sem origem: operação, cotação, PTAX, taxa e fórmula ficam visíveis. |
| Rentabilidade | **Modified Dietz por dia de exposição** (mensal) e **encadeamento geométrico** (anual) | É exatamente a regra pedida: o capital conta da data da compra até a data da venda. |
| Corretoras e fontes de cotação | **Módulos plugáveis** com registro automático | Nova corretora = nova pasta; o núcleo não muda. |
| Distribuição | **GitHub** (código, Releases, Actions) | Gratuito para projetos públicos, incluindo build dos instaladores. |
| Custo | **R$ 0** para mantenedor e usuário | Nenhuma API paga, nenhum servidor, nenhuma chave obrigatória. |

## 2. Visão geral

```
┌──────────────────────────── App Tauri (processo único, roda local) ───────────────────────────┐
│  UI (React/TS)                                                                                │
│   Telas: Login · Carteira · Lançamentos · Rentabilidade · Gráficos · Proventos · DARF ·       │
│          Importar nota · Auditoria · Histórico · Configurações · Backup                      │
│   brokers/<corretora>/   ← módulos de importação de notas (TS, autodescobertos)               │
│        │ comandos Tauri (IPC tipado)                                                          │
│  Núcleo Rust                                                                                  │
│   core      domínio, motor de rentabilidade, preço médio, regras de taxa, memória de cálculo  │
│   ledger    log de eventos com cadeia de hash, projeções (estado atual)                       │
│   storage   SQLCipher, migrações, export/import                                               │
│   providers PTAX-BCB · B3 COTAHIST · EUA · Cripto · Tesouro · SGS-BCB (módulos plugáveis)     │
│   tax       apuração mensal, compensação de prejuízo, alertas de DARF                         │
└───────────────────────────────────────────────────────────────────────────────────────────────┘
          │ HTTPS apenas quando o usuário clica "Atualizar" (atualização sob demanda)
          ▼
   Fontes públicas gratuitas (BCB, B3, Nasdaq Trader, Yahoo/Stooq, Binance/CoinGecko, Tesouro)
```

Regras gerais:
- **Nada sai da máquina** além das consultas de cotação, que não carregam dados do usuário (só ticker e data).
- **Sem internet**: usa a última cotação salva e mostra o selo "cotação de dd/mm/aaaa (offline)".
- **Atualização sob demanda**: botão "Atualizar cotações"; cada execução é registrada no histórico de atualizações.

## 3. Estrutura do repositório

```
/
├─ src-tauri/
│  ├─ Cargo.toml                  (workspace)
│  └─ crates/
│     ├─ core/        domínio puro, sem IO; motor de cálculo e fórmulas versionadas
│     ├─ ledger/      eventos, hash chain, projeções
│     ├─ storage/     SQLCipher, migrações SQL numeradas, backup
│     ├─ providers/   um módulo por fonte (ptax_bcb, b3_cotahist, us_nasdaqtrader, yahoo, stooq,
│     │               binance, coingecko, tesouro, sgs_bcb), todos implementando os mesmos traits
│     ├─ tax/         regras de IR versionadas por data de vigência
│     └─ app/         comandos Tauri, orquestração
├─ src/               UI React
│  └─ brokers/        sinacor/ (XP, Clear, Rico, BTG, Inter...), nomad/, csv-generico/ ...
├─ docs/              esta arquitetura, catálogo de fórmulas, guia "como criar um módulo de corretora"
├─ fixtures/sinteticas/ notas fictícias geradas do zero + resultados esperados (testes de ouro)
├─ CHANGELOG.md       embutido no app (tela Histórico do sistema)
└─ .github/workflows/ CI (testes) e release (instaladores)
```

## 4. Modelo de dados

Todos os valores monetários são `TEXT` decimal (ex.: `"3801.14"`) para não perder precisão; datas em ISO `AAAA-MM-DD`.

### 4.1 Cadastros
- **asset**: `id, ticker, nome, classe (ACAO|ETF|FII|BDR|STOCK_US|ETF_US|REIT|CRIPTO|TESOURO|RENDA_FIXA|CAIXA), mercado (B3|US|CRIPTO|RF), moeda (BRL|USD), isin, fonte_catalogo, ativo_desde, ativo_ate`
- **broker**: `id, modulo_id, nome, pais`
- **account**: `id, broker_id, moeda, apelido`

### 4.2 Operações (estado atual, projeção do log)
- **operation**: `id, account_id, asset_id, tipo (COMPRA|VENDA), data_pregao, data_liquidacao, quantidade, preco_unitario, valor_bruto, moeda, origem (MANUAL|NOTA|CSV), nota_id, versao, removida_em`
- **operation_fee**: `operation_id, tipo (EMOLUMENTOS|LIQUIDACAO|CORRETAGEM|IOF|OUTRAS), base, aliquota, valor, fee_rule_id, informado_ou_calculado`
- **fee_rule**: `id, mercado, tipo_taxa, modalidade (NORMAL|DAY_TRADE), aliquota, vigente_de, vigente_ate, url_fonte` (tabela versionada: quando a B3 mudar a taxa, entra uma nova linha, as operações antigas continuam usando a regra da sua data)

### 4.3 Proventos (armazenados, fora da rentabilidade)
- **incomes** (migração 0009): `id, account, ticker, kind (DIVIDENDO|JCP|RENDIMENTO|JUROS|OUTRO), pay_date, ex_date, gross, withheld, currency, note, removed_at`. O líquido é `gross − withheld`; no exterior, os valores em reais usam a PTAX venda do dia do pagamento (último boletim até a data), com o líquido em reais = bruto em reais − retido em reais. Cada mudança gera evento `PROVENTO` no log de auditoria. O valor por cota usa a quantidade somada em todas as contas no fim da data com (ou do pagamento).

### 4.3a Eventos corporativos (bonificação, desdobramento, grupamento)
Lançados uma vez por ativo, com a data com e a proporção do anúncio. Cada evento vira, em cada conta que tinha o ativo no fim da data com, um ajuste de quantidade na data ex (lado `AJUSTE`, sem dinheiro): na bonificação entram as ações novas com o custo por ação informado pela empresa (o valor é rendimento isento na declaração); no desdobramento e no grupamento muda só a quantidade, e o custo total fica igual. Na B3 a quantidade nova é arredondada para baixo (a fração é vendida em leilão e paga em dinheiro, lançada como provento); no exterior a fração fica. O ajuste vale desde a abertura do pregão: entra antes das compras e vendas do dia e nunca forma day trade. Os ajustes não são gravados como lançamentos; são recalculados dos eventos a cada consulta.

### 4.3b Saldos e reservas (cofrinhos, FGTS, conta corrente, cartão benefício)
Guiados pelo saldo que o banco mostra, porque a taxa dos cofrinhos muda sem aviso. Cada saldo tem movimentos: depósito, resgate (valor líquido que caiu na conta, com IR e IOF opcionais) e saldo informado. O cofrinho vira uma cota sintética (`RF:<id>`) de R$ 1. Desde a 0.12.0 o app não estima nada entre dois saldos (nem CDI, nem IR/IOF; escolha do dono do projeto, porque os saldos são informados mês a mês na Evolução). Depósito é compra de cotas, resgate é venda (IR e IOF só se informados) e o saldo informado ajusta a quantidade de cotas sem dinheiro (`AJUSTE`), que é onde o rendimento aparece; diferença acima de 2% (e de R$ 5) gera aviso de movimento esquecido. O primeiro saldo informado sem depósitos é o saldo inicial. Desde a 0.12.0, FGTS, conta e benefício também não têm estimativa: vale o último saldo informado. Só cofrinhos entram nas operações do motor (carteira, rentabilidade); os demais aparecem apenas no patrimônio ("Outros saldos") e no saldo de 31/12 para Bens e Direitos. Como os fundos, cofrinhos não têm taxas da B3 nem DARF (o banco retém). As operações e cotas são recalculadas a cada consulta, não gravadas. Cada saldo tem um período (`since` obrigatório, `until` opcional, sempre dia 1 do mês; migração 12 preenche `since` com o mês do primeiro movimento): entre os dois a linha aparece na Evolução mesmo vazia; movimentos depois do fim do mês de `until` são ignorados e, no dia seguinte, um `AJUSTE` sintético zera as cotas. Na Evolução, FGTS, conta e benefício mostram variação simples (último − primeiro saldo do período; último − primeiro saldo informado desde o início).


### 4.3c Evolução do patrimônio

`src-tauri/src/evolution.rs` monta a aba Evolução: para cada coluna (fim de
mês, 31/12 ou hoje), a posição de cada ativo em `positions_with_fx` na data,
avaliada em reais pela PTAX, e o saldo de cada reserva em
`SavingsReport::balance_on` (cotas na data × valor da cota). É um retrato do
patrimônio, não rentabilidade: a variação simples entre colunas inclui
aportes e saques. Para explicar a variação, o "dinheiro novo" soma o fluxo
de caixa (`Trade::cash_flow`) dos investimentos entre duas colunas, e o
rendimento é a variação dos investimentos menos esse dinheiro novo. Desde a
0.12.1 só o rendimento aparece na tela: com a conta corrente no patrimônio,
o dinheiro novo só mostrava dinheiro mudando de lugar.
Investimento é toda linha com código no motor (ativos, fundos e
cofrinhos); FGTS, conta corrente e cartões benefício entram só no total
geral.

O Painel (`src/screens/Panel.tsx`, desde a 0.12.0) usa a mesma visão da
Evolução: total da última coluna, variação no mês contra o último fim de mês,
variação no ano pelo `total_gain` e rendimento pelo `investments_gain`. Os fundos de cada banco viram uma fatia
só. A rentabilidade detalhada fica em Carteira › Rentabilidade.

### 4.4 Cotações e câmbio (imutáveis)
- **quote**: `id, asset_id, data, fechamento, moeda, fonte, oficial (bool), obtida_em, hash_payload`
- **fx_rate**: `id, data, par (USD/BRL), compra, venda, tipo_boletim, fonte (PTAX-BCB), obtida_em, hash_payload`
- **index_value** (CDI, SELIC, IPCA via SGS-BCB; preços do Tesouro): `serie, data, valor, fonte, obtida_em`

Uma cotação nunca é sobrescrita. Se a fonte corrigir um valor, entra nova linha e a antiga fica marcada como substituída, para que cálculos antigos continuem reproduzíveis.

### 4.5 Auditoria
- **event_log** (append-only): `seq, ocorrido_em, entidade, entidade_id, acao (CRIADA|ALTERADA|REMOVIDA), antes_json, depois_json, motivo, versao_app, hash_anterior, hash`
  `hash = SHA-256(seq ‖ ocorrido_em ‖ entidade ‖ acao ‖ antes ‖ depois ‖ hash_anterior)`
  Qualquer edição direta no banco quebra a cadeia, e o app avisa em "Verificar integridade".
- **calc_run**: `id, tipo, escopo, periodo, executado_em, versao_app, versao_motor`
- **calc_result**: `id, run_id, asset_id|carteira, periodo, metrica, valor, formula_id, formula_versao, entradas_json, formula_expandida`
- **update_log**: execuções de "Atualizar cotações" (início, fim, fonte, ativos, sucesso/erro)
- **broker_note**: `id, hash_arquivo, modulo, versao_modulo, importada_em, pdf_blob (opcional), resultado_json`

### 4.6 Sistema e impostos
- **app_version_history**: `versao, instalada_em, changelog`
- **schema_migrations**: versão do esquema
- **tax_rule**: regras de IR por categoria e vigência
- **darf_alert**: `competencia, categoria, base_calculo, prejuizo_compensado, imposto, vencimento, status (PENDENTE|PAGO|IGNORADO)`

## 5. Motor de rentabilidade

### 5.1 Regra pedida
"O capital só começa a contar a partir da data exata da compra e deixa de contar na data exata da venda." Dividendos não entram. Compras são aportes, vendas são retiradas.

### 5.2 Fórmula mensal (Modified Dietz por dia)
Para um ativo (ou a carteira) num mês:

```
início = max(1º dia do mês, data da primeira compra)
fim    = min(último dia do mês, data em que a posição zerou)   (se zerou no mês)
D      = dias entre início e fim (inclusive)
V0     = valor de mercado em início (0 se a primeira compra é no mês)
V1     = valor de mercado em fim  (quantidade × fechamento × PTAX se USD)
Fᵢ     = fluxo da operação i: compra = +(bruto + taxas); venda = −(bruto − taxas)
wᵢ     = (dias restantes a partir da data da operação i, inclusive) / D

Lucro/Prejuízo (R$) = V1 − V0 − ΣFᵢ
Rentabilidade (%)   = Lucro / (V0 + Σ wᵢ·Fᵢ)
```

O denominador é o **capital médio efetivamente exposto**: cada real conta só nos dias em que esteve investido.

### 5.3 Exemplo (PETR4)
- 15/01/2026: compra 100 PETR4 a R$ 38,00 → bruto R$ 3.800,00
- Taxas B3 calculadas (regra da seção 7): liquidação 0,0224% = R$ 0,85; emolumentos 0,005% = R$ 0,19; transferência 0,0026% = R$ 0,09 → F = R$ 3.801,13
- 30/01/2026 (último pregão de janeiro): fechamento R$ 40,00 → V1 = R$ 4.000,00
- Início = 15/01 (primeira compra), V0 = 0, w = 1

```
Lucro = 4.000,00 − 0 − 3.801,13 = R$ 198,87
Rent. = 198,87 / 3.801,13 = 5,23%
```

Se em 22/01 houvesse nova compra de 50 ações a R$ 39,00 (F = 1.950,57), D = 17 (15 a 31/01) e w = 10/17, então o denominador seria 3.801,13 + (10/17)·1.950,57, e esse segundo aporte pesaria só pelos 10 dias em que ficou investido.

### 5.4 Anual e acumulado
- **Rentabilidade anual** = (1+R_jan)·(1+R_fev)·…·(1+R_dez) − 1 (encadeamento geométrico, imune ao tamanho dos aportes).
- **Lucro/prejuízo anual em R$** = soma dos lucros mensais.
- **Mês corrente**: V1 usa a cotação mais recente obtida (ou a última salva, se offline).
- Opcional na tela: TIR (XIRR) da posição, para quem quiser a visão "ponderada pelo dinheiro".

### 5.5 Ativos em dólar (valorização + variação cambial)
- Tudo é calculado em USD e em BRL. Valor em BRL = quantidade × preço USD × PTAX da data.
- Fluxos convertidos pela PTAX da data da operação; posições pela PTAX da data de avaliação.
- A tela mostra a decomposição: `(1 + R_BRL) = (1 + R_ativo_USD) × (1 + R_câmbio)`.
- Sem PTAX na data (fim de semana/feriado), usa a do último dia útil anterior, e isso aparece na memória de cálculo.
- PTAX **venda de fechamento** do Banco Central na data de cada fato (compra, venda, avaliação), a mesma que a Lei 14.754/2023, art. 15, manda usar para ativos no exterior. Assim rentabilidade e imposto usam o mesmo câmbio.
- A Nomad converte reais pelo dólar comercial dela (com spread), e esse câmbio não aparece nas confirmações. Por isso ele não entra no cálculo; o usuário pode registrar o câmbio real da remessa como informação, para comparar com a PTAX.

### 5.6 Rentabilidade × preço médio
São dois motores separados, propositalmente:
- **Rentabilidade** (Modified Dietz) responde "quanto meu dinheiro rendeu no tempo em que ficou exposto".
- **Preço médio + lucro realizado** responde "quanto devo de imposto", seguindo a regra da Receita. Alimenta o módulo de DARF.

### 5.7 Memória de cálculo (nenhuma caixa-preta)
Cada número exibido tem um botão "Ver cálculo" que mostra:
- operações usadas (com link para o histórico de cada uma);
- cotação usada (valor, data, fonte, quando foi obtida);
- PTAX usada (compra/venda, data, boletim);
- taxas consideradas (regra, alíquota, vigência, fonte);
- fórmula (id e versão) **com os números substituídos**, como na seção 5.3.

As memórias aparecem no Painel e na Rentabilidade quando o usuário liga "Mostrar memórias de cálculo" em Segurança › Auditoria (desligada por padrão, escolha do dono do projeto em 16/10/2026); os números são os mesmos com ou sem a opção.

Fórmulas ficam num catálogo versionado (`docs/formulas.md` + código). Se uma fórmula mudar, a versão sobe e resultados antigos continuam reproduzíveis.

## 6. Fontes de dados gratuitas

| Dado | Fonte primária | Alternativa | Observação |
|---|---|---|---|
| Câmbio USD/BRL | **PTAX do Banco Central** (API Olinda, pública, sem chave) | — | Oficial. |
| Catálogo B3 (ações, ETFs, FIIs, BDRs) | Arquivo **COTAHIST** da B3 (público) | Lista de instrumentos da B3 | Atualizado sob demanda. |
| Cotações B3 (fechamento oficial) | **COTAHIST** diário/anual | Yahoo Finance (`.SA`) | Fechamento oficial para fim de mês; Yahoo para o preço "de hoje". |
| Catálogo EUA (NYSE, Nasdaq, ARCA) | Arquivos públicos **Nasdaq Trader** (`nasdaqlisted.txt`, `otherlisted.txt`) | — | Gratuito. |
| Cotações EUA | Yahoo Finance (endpoint público) | Stooq (CSV) | Endpoints não oficiais podem mudar; por isso há fallback e lançamento manual. |
| Cripto | Yahoo Finance (`BTC-USD`), fechamento diário | Coinbase (API pública) para o gráfico do dia | Cotação em USD convertida por PTAX. |
| Fundos de investimento | **Informe diário da CVM** (dados abertos: mensal desde 2021, anual até 2020) | — | Cota diária por CNPJ; cadastro da CVM (`cad_fi.csv` e registro de classes) para achar o fundo pelo nome. |
| Gráfico do ativo (intradiário) | Yahoo Finance (velas de 1, 5 ou 15 min) | Fechamentos guardados no cofre | Só para ver; não é gravado. B3 com cerca de 15 min de atraso (cotação ao vivo é paga); cripto em tempo real pela Coinbase. |
| Tesouro Direto | Tesouro Transparente (CSV público) | — | Preço unitário diário. |
| CDI, SELIC, IPCA | **SGS do Banco Central** | — | Para CDB/LCI/LCA indexados. |

Toda fonte é um módulo `QuoteProvider` com a mesma interface (`suporta(ativo)`, `buscar(ativo, datas)`), em ordem de prioridade configurável. Se todas falharem, o usuário pode lançar a cotação manualmente, e ela fica marcada como "manual".

Nota: a conectividade com essas fontes não pôde ser testada a partir do ambiente onde este documento foi escrito; ela será validada no primeiro protótipo.

## 7. Taxas B3 automáticas
- Calculadas na hora do lançamento a partir da tabela `fee_rule` vigente na data do pregão.
- **Regra conferida centavo a centavo contra notas reais de corretora** (que não fazem parte do projeto), pessoa física, mercado à vista, operação normal, sobre o **valor total das operações do pregão** na nota:

| Taxa | Alíquota | Arredondamento | Exemplo fictício: pregão de R$ 100.000,00 |
|---|---|---|---|
| Taxa de liquidação/CCP | 0,0224% | trunca no centavo | R$ 22,40 |
| Emolumentos | 0,0050% | trunca no centavo | R$ 5,00 |
| Taxa de transferência de ativos | 0,0026% | trunca no centavo | R$ 2,60 |
| **Total B3** | **0,0300%** | | líquido R$ 100.030,00 |

  A regra vale também com mercado fracionário. Arredondar em vez de truncar gera diferença de 1 centavo em alguns casos, por isso o truncamento.
- **Rateio**: a B3 cobra por nota (pregão), não por operação. O custo de cada operação recebe a parte proporcional ao seu valor; o resíduo de centavos vai para a maior operação, e isso aparece na memória de cálculo.
- Day trade tem alíquotas próprias, também na tabela.
- **Day trade é detectado automaticamente** (somente B3): compra e venda do mesmo ativo, na mesma conta e no mesmo pregão. A parte casada recebe as taxas de day trade e vai para a apuração de day trade; o restante segue como operação normal. O usuário pode ver e corrigir a classificação.
- IRRF "dedo-duro" de 0,005% sobre vendas normais (retido só se passar de R$ 1,00 no mês) e 1% sobre o ganho de day trade são registrados e abatidos no DARF.
- Taxas vindas de nota de corretagem prevalecem sobre as calculadas, e a diferença (se houver) aparece como aviso.
- Corretagem: pode ser zero (Home Broker) ou não; o campo existe para todas as corretoras.

## 8. Módulos de corretora (plugáveis)

Contrato em TypeScript:

```ts
interface BrokerModule {
  id: string; nome: string; versao: string;
  detectar(doc: DocumentoExtraido): number;          // 0..1 de confiança
  importar(doc: DocumentoExtraido): LoteImportacao;  // operações, taxas, proventos normalizados
}
```

- Cada corretora é uma pasta em `src/brokers/`; o registro é automático (`import.meta.glob`), então **adicionar corretora não altera o núcleo**.
- Extração de texto do PDF com **pdf.js** (Mozilla, livre), incluindo PDFs protegidos por senha (muitas notas usam o CPF). As notas analisadas têm texto selecionável, sem necessidade de OCR.
- Fluxo: PDF → módulo detectado → **pré-visualização** → usuário confirma → núcleo valida (quantidades, totais da nota batem com a soma das operações) → eventos gravados com `origem = NOTA`.
- Primeiros módulos: **Itaú** e **Nomad** e **CSV genérico**. Em seguida, **SINACOR** (layout comum a XP, Clear, Rico, BTG, Inter e outras). O app nasce preparado para qualquer outra corretora via novo módulo.

### 8.1 O que as notas reais mostraram

**Itaú Corretora** (layout SINACOR)
- A nota traz o **nome de pregão** (ex. fictício: "AMBEV S/A ON"), não o ticker. O módulo converte nome + classe em ticker (ABEV3) usando o cadastro de instrumentos da B3, e pede confirmação quando houver ambiguidade.
- Mercado **VISTA** e **FRACIONARIO** do mesmo ativo aparecem em linhas separadas; viram o mesmo ativo na carteira.
- Marcadores de observação: `@` taxa de transferência, `#` negócio direto, `D` day trade, `H` home broker.
- Datas: pregão e "Líquido para" (liquidação em D+2 úteis, respeitando feriados).
- **Validação automática**: valor das operações + taxas = líquido da nota. Se não bater, a importação para e mostra a diferença.

**Nomad**, **dois layouts diferentes**:
- *Layout antigo*: "Transaction Confirmation" da DriveWealth. Cada ordem vira duas linhas: parte inteira (Agency) e fração (Principal/Riskless Principal), mesmo horário e preço. O módulo agrupa as duas numa operação, preservando as linhas originais na auditoria. Datas em formato americano (M/D/AAAA).
- *Layout atual*: "Trade Confirmation" da Apex Clearing (Nomad Investment Services). Uma linha por ordem com quantidade fracionária, seções "You Bought"/"You Sold", Trade#, datas ISO.
- Na troca de custodiante (DriveWealth → Apex) aparecem vendas de frações minúsculas com valor US$ 0,00, que são resíduos da migração; as posições foram transferidas sem venda. O sistema precisa de uma operação de **transferência de custódia** que move a posição mantendo custo e datas originais, para não gerar venda/compra fictícia nem quebrar a contagem de tempo de exposição.
- Comissões e taxas zeradas em todas as confirmações; preços e quantidades com até 8 casas decimais (o banco guarda decimais sem limite de casas).
- As confirmações não trazem a taxa de câmbio da remessa, o que confirma a decisão de converter pela PTAX.
- A detecção de day trade não se aplica ao exterior (venda e recompra no mesmo dia lá fora é tributada no regime anual da Lei 14.754).

### 8.2 Regras gerais de importação
- Implementação (0.6.0): leitores em `src/import/readers/` (`sinacor.ts`, `nomad.ts`), registrados em `src/import/index.ts`; o texto vem do pdf.js, montado em linhas com colunas separadas por dois espaços (`src/import/lines.ts`). Testes com notas sintéticas em `src/import/import.test.ts`.
- A mesma nota não entra duas vezes: a impressão digital é o SHA-256 de corretora + número + data do pregão (ou do texto das linhas, quando a nota não tem número), guardada em `imported_notes`. Assim a mesma nota em outro arquivo ou em outro layout também é reconhecida. Se todos os lançamentos da nota forem removidos, ela pode ser importada de novo.
- As taxas da nota (liquidação, registro, emolumentos, transferência, corretagem, ISS e outras) são rateadas entre as linhas pelo valor e gravadas como taxa informada de cada lançamento. O IRRF retido aparece como aviso.
- Ativos sem nota (de outros usuários) entram por uma tela de **saldo inicial**: quantidade, custo e data de compra.
- Cripto em carteira própria entra por **lançamento manual**, com cotação automática.
- Futuro: módulos em WebAssembly para permitir instalar corretoras sem atualizar o app.

## 9. Histórico e auditoria

**Histórico funcional** (tela "Auditoria"), direto do `event_log`: compra criada / alterada / removida, venda criada / alterada / removida, com antes e depois, data/hora e motivo opcional. Remoção é lógica: a operação some da carteira, mas continua no histórico.

**Histórico do sistema**: versão instalada, data da atualização e changelog (`app_version_history`, preenchido na primeira execução de cada versão).

**Histórico de atualizações de cotação**: `update_log`.

**Verificar integridade**: recalcula a cadeia de hash e informa se algum registro foi adulterado.

## 10. Impostos e alerta de DARF
- Apuração mensal por categoria a partir do preço médio: ações (isenção para vendas até R$ 20 mil/mês em operações comuns), ETFs, FIIs, cripto no Brasil e **day trade** (alíquota própria, IRRF de 1% na fonte deduzido do DARF, prejuízo compensado só com day trade, preço médio do dia separado do preço médio da carteira).
- Compensação de prejuízos acumulados por categoria.
- Alerta na abertura do app e na tela DARF quando houver imposto devido, com vencimento (último dia útil do mês seguinte) e memória de cálculo.
- Ativos no exterior seguem a Lei 14.754/2023 (apuração anual na declaração, câmbio PTAX de venda), então geram relatório anual, não DARF mensal.
- Regras ficam em tabela versionada por vigência. O app é ferramenta de apoio; o documento recomendará conferência com contador.
- Implementado na 0.7.0 (`crates/core/src/tax.rs`): três grupos na B3 (comum 15% com isenção de ações até R$ 20 mil/mês; day trade 20%, casado por ativo, corretora e dia; FII 20%), prejuízo por grupo sem prazo, IRRF estimado (0,005% e 1%) abatido e levado adiante, DARF mínimo de R$ 10,00 acumulado, vencimento no último dia útil do mês seguinte (feriados bancários nacionais), exterior e cripto anuais pela PTAX de cada data e posição de Bens e Direitos em 31/12. DARFs pagos ficam em `darf_payments`, com evento no log.

## 10a. Fundos de investimento e página do ativo
- Implementado na 0.8.0. O fundo é identificado pelo **CNPJ**, que vira o código do ativo (`12.345.678/0001-95`, `crates/core/src/fund.rs`). Lançamentos pelo extrato do banco: aplicação (compra de cotas), resgate (venda de cotas, com **IR** e **IOF** informados) e come-cotas (venda de cotas em que todo o valor é IR). Valor bruto truncado no centavo, como fazem os administradores.
- IR e IOF ficam nas colunas `ir` e `iof` de `operations` (migração 0008) e entram como desconto do valor recebido: a rentabilidade do fundo é líquida do imposto retido. Fundos não têm taxa da B3 e ficam fora do DARF e do relatório do exterior.
- Cota diária: `crates/providers/src/cvm.rs` lê o informe diário da CVM linha a linha, só para os CNPJs da carteira, colunas achadas pelo cabeçalho (antes e depois da Resolução CVM 175). Cada arquivo é baixado uma vez; o mês corrente e o anterior, uma vez por dia.
- Página do ativo (`src-tauri/src/asset.rs`, `src/screens/AssetPage.tsx`): clique na Carteira. Gráfico de velas do dia (1, 5 ou 15 min, atualiza a cada minuto) a "Tudo", com as compras e vendas marcadas, a linha do preço médio e o resumo da posição.

## 11. Segurança
- Primeiro uso: usuário cria a senha. O banco é criptografado com uma chave aleatória de 32 bytes, guardada em `cofre.json` duas vezes, cada uma cifrada (ChaCha20-Poly1305) com uma chave derivada por Argon2id: uma da senha e outra da **chave de recuperação**. Nem a senha, nem a chave de recuperação, nem a chave do banco em aberto ficam gravadas. (Até a 0.3.0 a chave do banco era o próprio Argon2id da senha; esses cofres são convertidos na primeira abertura, sem recriptografar.)
- Bloqueio automático após inatividade (configurável).
- **Chave de recuperação** (aprovada em 14/10/2026), sem servidor: 32 caracteres aleatórios gerados ao criar o cofre (ou depois, na tela Segurança). O app abre o programa de e-mail do próprio usuário com a chave numa mensagem para ele mesmo (`mailto:`); nada é enviado pelo app. "Esqueci minha senha", na tela de entrada, aceita a chave e define uma senha nova. Gerar uma chave nova invalida a anterior.
- Troca de senha protege a mesma chave do banco com a nova senha (novo sal); a chave de recuperação continua valendo.

## 12. Exportar e importar banco
- **Exportar backup**: arquivo único `.cabackup` com manifesto (versão do app e do esquema, data, número de eventos, último hash da cadeia, SHA-256 do banco), o `cofre.json` e a cópia do banco ainda criptografada. Além do backup manual, o app faz um backup automático por semana na pasta de dados e guarda os 5 mais recentes.
- **Exportar dados abertos**: CSV/JSON de operações, proventos, cotações e histórico, para planilha ou outro sistema.
- **Importar backup**: valida senha, versão do esquema (migra se for antiga) e cadeia de hash antes de substituir o banco atual; o banco anterior é preservado como cópia.

## 13. Distribuição e atualização
- Código e releases no **GitHub**. Desenvolvimento começa em repositório **privado** (gratuito) e vira público após os primeiros testes.
- Build automático de instaladores em **GitHub Actions** (gratuito): `.msi`/`.exe` (Windows, prioridade nos testes), `.dmg` (macOS), `.AppImage`/`.deb` (Linux).
- Atualização do app: o Tauri Updater verifica o GitHub Releases quando o usuário pede e instala com assinatura própria (chave gratuita). Cada versão registra changelog no histórico do sistema.
- Assinatura de código: no Windows, dá para usar o programa gratuito da SignPath Foundation para projetos open source; até lá, o Windows mostra o aviso "editor desconhecido". No macOS, a notarização da Apple é paga, então o usuário precisará abrir com "clique direito → Abrir" na primeira vez. Isso é documentado no README.
- Licença: **GPL-3.0** (garante que derivados continuem abertos).
- Divulgação no GitHub: descrição "Controle de investimentos B3 + exterior com rentabilidade auditável, 100% local e gratuito"; tópicos investimentos, b3, nomad, rentabilidade, imposto-de-renda, darf, ptax, tauri; README em português com capturas de tela.

## 14. Qualidade

**Privacidade das notas reais**: notas de corretagem de usuários **nunca** entram no repositório, em issues, PRs, logs de CI ou documentação, nem anonimizadas. Os testes usam notas **sintéticas** geradas do zero (nomes, contas, CPFs e valores fictícios) que imitam apenas o layout público de cada corretora. Um teste de CI bloqueia PDFs fora de `fixtures/sinteticas/`.

- Testes unitários do motor com exemplos de ouro calculados à mão (como a seção 5.3) e conferíveis em planilha.
- Testes de cada módulo de corretora com notas sintéticas em `fixtures/sinteticas/`.
- Teste de propriedade: recalcular do zero a partir do log de eventos produz exatamente os mesmos resultados.
- CI roda testes em Windows, macOS e Linux a cada PR.

## 15. Roteiro de entregas

| Marco | Entrega |
|---|---|
| M0 Fundação | Repositório, Tauri + React, senha, banco SQLCipher, log de eventos, histórico do sistema, CI |
| M1 B3 manual | Catálogo B3, lançamento de compra/venda com taxas automáticas, cotações sob demanda, rentabilidade mensal/anual por ativo, memória de cálculo |
| M2 Exterior | Catálogo EUA, PTAX, decomposição ativo + câmbio, conta Nomad, transferência de custódia |
| M3 Visualização e backup | Gráficos mensais/anuais de lucro/prejuízo (B3 e exterior), carteira consolidada, exportar/importar |
| M4 Notas de corretagem | Importação PDF (Itaú, Nomad nos dois layouts, CSV genérico; depois SINACOR) |
| M5 "Tudo" | Cripto (manual), Tesouro Direto, renda fixa indexada, FIIs, proventos |
| M6 Impostos | Preço médio, apuração mensal, day trade, alerta de DARF, relatório anual do exterior |

## 16. Decisões
1. Licença: **GPL-3.0-or-later**.
2. Repositório: **marcelo-fournier/carteira-auditavel**, privado durante os primeiros testes.
3. PTAX: **venda de fechamento** do Banco Central (padrão da Receita para ativos no exterior, Lei 14.754/2023, art. 15).
4. Plataforma prioritária para testes: **Windows**.
5. Corretoras iniciais: Itaú e Nomad, mais CSV genérico. Outras via módulos.
6. Day trade, cripto (lançamento manual), renda fixa, Tesouro e FIIs: suportados conforme o roteiro.

## Perfis (0.13.0)

Cada perfil é um cofre independente (`src-tauri/src/profiles.rs`). O perfil principal usa a pasta de dados do app, como antes; os outros ficam em `perfis/<id>`. A lista de nomes fica em `perfis.json`, fora dos cofres e sem nenhum dado da carteira, para a tela de entrada mostrar os perfis antes da senha. Só se troca de perfil com a carteira bloqueada; perfil com cofre não pode ser removido pelo app. O app guarda os dados de uma pessoa por perfil: um FGTS, uma conta corrente (ou cartão benefício) por banco, e cofrinhos à vontade (só um sem nome por banco).

## Renda fixa, bens e corretagem (0.14.0)

Migração 13 (`0013_renda_fixa_corretagem_bens.sql`).

- **Renda fixa** é um tipo de saldo (`SavingsKind::RendaFixa`) com `FixedIncome { product, rate, maturity }`. Usa o mesmo motor do cofrinho (ativo `RF:<id>`, cota sempre 1, saldos informados, depósitos e resgates), sem estimativa de rendimento nem preço automático do Tesouro. Não tem limite por banco.
- **Bens** (`carteira_core::owned`): `OwnedAsset` (imóvel ou veículo, compra, venda opcional) e `Improvement` (obra, só imóvel). O valor em qualquer data é `cost_on` = compra + obras até a data; nada antes da compra nem a partir da venda. Entram na Evolução no grupo `bens` (fora dos investimentos) e em Impostos pelo custo em 31/12. Auditoria: entidades `BEM` e `OBRA`.
- **Corretagem** (`account_brokerage`, entidade `CORRETAGEM`): valor por ordem + ISS por conta, a partir de `since`. `price_operations_with` soma a corretagem às operações com taxa automática (B3, sem taxa informada) daquela data em diante; notas importadas trazem a taxa informada e não mudam.
- **Fundos em Bens e Direitos**: `tax::annual` junta B3, exterior e fundos (`fund_year_end`, custo médio) e repete a posição de cada origem nos anos sem operação (`fill_to`).

## Tesouro Direto e planilha CSV (0.15.0)

- **Tesouro Direto** (`carteira_core::tesouro`): o título é um ativo com código `TD:<sigla>:<vencimento AAAAMMDD>` (LFT, LTN, NTN-F, NTN-B, NTN-B-P, NTN-C, RENDA, EDUCA), tipo `TESOURO`. É lançado como operação (compra e venda em quantidade de títulos, com o preço unitário), como os fundos: sem taxas da B3, sem DARF, IR e IOF informados no resgate (`fund::bank_withholds` e `fund::by_statement`), Bens e Direitos pelo custo médio. Preço: `carteira_providers::tesouro` lê o CSV `PrecoTaxaTesouroDireto.csv` do Tesouro Transparente (colunas pelo nome; "PU Venda Manhã", ou "PU Base Manhã" quando zerado). `market::update_tesouro` baixa no máximo uma vez por dia, grava a lista de títulos (catálogo) e os preços dos títulos da carteira.
- **Planilha CSV** (`src/import/csv.ts`): colunas pelo nome do cabeçalho (sem acento e maiúsculas), separador detectado (";" com vírgula decimal, "," com ponto decimal, ou tabulação), Windows-1252 ou UTF-8. Cada conta+dia vira uma importação (`import_note`) com impressão digital SHA-256 das linhas, então a mesma planilha não entra duas vezes.
