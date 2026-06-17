import type { ReactNode } from "react";

type Props = {
  icon: ReactNode;
  title: string;
  children: ReactNode;
};

/** 对齐 DeepSeek Monitor 设置分区 */
export function SettingSection({ icon, title, children }: Props) {
  return (
    <section className="panel-settings-section">
      <h2 className="panel-settings-section-title">
        {icon}
        {title}
      </h2>
      {children}
    </section>
  );
}