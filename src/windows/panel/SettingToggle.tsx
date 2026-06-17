type Props = {
  label: string;
  checked: boolean;
  onChange: (checked: boolean) => void;
};

/** 对齐 DeepSeek Monitor 的 pill 开关 */
export function SettingToggle({ label, checked, onChange }: Props) {
  return (
    <label className="panel-toggle-row">
      <span>{label}</span>
      <input
        type="checkbox"
        checked={checked}
        onChange={(event) => onChange(event.target.checked)}
      />
      <i aria-hidden />
    </label>
  );
}