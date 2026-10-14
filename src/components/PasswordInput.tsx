import { useState, type KeyboardEvent } from "react";

type Props = {
  label: string;
  value: string;
  onChange: (value: string) => void;
  autoFocus?: boolean;
};

/** Campo de senha com botão para mostrar o texto e aviso de Caps Lock ligado. */
export default function PasswordInput({ label, value, onChange, autoFocus }: Props) {
  const [visible, setVisible] = useState(false);
  const [capsLock, setCapsLock] = useState(false);

  function checkCaps(e: KeyboardEvent<HTMLInputElement>) {
    setCapsLock(e.getModifierState("CapsLock"));
  }

  return (
    <label>
      {label}
      <span className="password-field">
        <input
          type={visible ? "text" : "password"}
          autoFocus={autoFocus}
          autoComplete="off"
          spellCheck={false}
          value={value}
          onChange={(e) => onChange(e.target.value)}
          onKeyDown={checkCaps}
          onKeyUp={checkCaps}
          onBlur={() => setCapsLock(false)}
        />
        <button
          type="button"
          className="reveal"
          onClick={() => setVisible((v) => !v)}
          aria-label={visible ? "Ocultar senha" : "Mostrar senha"}
          title={visible ? "Ocultar senha" : "Mostrar senha"}
        >
          {visible ? "Ocultar" : "Mostrar"}
        </button>
      </span>
      {capsLock && <span className="caps">Caps Lock está ligado</span>}
    </label>
  );
}
