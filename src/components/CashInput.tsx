import { cashMask, type Currency } from "../format";

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
      placeholder={`${currency === "USD" ? "US$" : "R$"} 0,${"0".repeat(decimals)}`}
      required={required}
      onChange={(e) => onChange(cashMask(e.target.value, decimals, currency))}
    />
  );
}
