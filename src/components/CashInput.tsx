import { cashTyping, cashZero, type Currency } from "../format";

/** Campo de dinheiro com máscara de caixa: só números, da direita para a esquerda. */
export default function CashInput({
  value,
  decimals,
  currency,
  onChange,
  required,
}: {
  value: string;
  decimals: number;
  currency: Currency;
  onChange: (v: string) => void;
  required?: boolean;
}) {
  return (
    <input
      inputMode="numeric"
      value={value}
      placeholder={cashZero(decimals, currency)}
      required={required}
      onChange={(e) => onChange(cashTyping(e.target.value, value, decimals, currency))}
    />
  );
}
