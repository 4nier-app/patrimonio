import { useEffect, useRef, useState } from "react";
import * as echarts from "echarts/core";
import { BarChart, LineChart, PieChart } from "echarts/charts";
import { GridComponent, LegendComponent, MarkLineComponent, TooltipComponent } from "echarts/components";
import { CanvasRenderer } from "echarts/renderers";
import type { EChartsCoreOption } from "echarts/core";

echarts.use([BarChart, LineChart, PieChart, GridComponent, LegendComponent, MarkLineComponent, TooltipComponent, CanvasRenderer]);

/**
 * Cores dos gráficos. Paleta categórica validada (daltonismo e contraste) nos
 * dois temas; a ordem é fixa e cada série tem sempre a mesma cor.
 */
const LIGHT = {
  series: ["#2a78d6", "#eb6834", "#1baf7a", "#eda100", "#e87ba4", "#008300"],
  pos: "#2a78d6",
  neg: "#e34948",
  ink: "#1c2430",
  muted: "#5d6877",
  grid: "#e6e9ee",
  axis: "#c3c8d0",
  surface: "#ffffff",
};
const DARK = {
  series: ["#3987e5", "#d95926", "#199e70", "#c98500", "#d55181", "#008300"],
  pos: "#3987e5",
  neg: "#e66767",
  ink: "#e6ebf1",
  muted: "#9aa5b3",
  grid: "#262e38",
  axis: "#3a4450",
  surface: "#1b2129",
};
export type Palette = typeof LIGHT;

const query = "(prefers-color-scheme: dark)";

/** Paleta do tema atual; muda junto com o tema do sistema. */
export function usePalette(): Palette {
  const [dark, setDark] = useState(() => window.matchMedia?.(query).matches ?? false);
  useEffect(() => {
    const mq = window.matchMedia?.(query);
    if (!mq) return;
    const on = (e: MediaQueryListEvent) => setDark(e.matches);
    mq.addEventListener("change", on);
    return () => mq.removeEventListener("change", on);
  }, []);
  return dark ? DARK : LIGHT;
}

/** Opções comuns: fonte, eixos discretos, grade fraca. */
export function base(p: Palette): EChartsCoreOption {
  return {
    textStyle: { fontFamily: "inherit", color: p.muted },
    grid: { left: 8, right: 16, top: 40, bottom: 8, containLabel: true },
    tooltip: {
      backgroundColor: p.surface,
      borderColor: p.axis,
      textStyle: { color: p.ink },
      confine: true,
    },
    legend: { top: 0, left: 0, textStyle: { color: p.ink }, icon: "roundRect", itemWidth: 12, itemHeight: 8 },
  };
}

export function axisStyle(p: Palette) {
  return {
    axisLine: { lineStyle: { color: p.axis } },
    axisTick: { show: false },
    axisLabel: { color: p.muted },
    splitLine: { lineStyle: { color: p.grid } },
  };
}

type Props = {
  option: EChartsCoreOption;
  height?: number;
  /** Clique num elemento: índice no eixo e nome da série. */
  onSelect?: (index: number, series: string) => void;
  ariaLabel: string;
};

export default function Chart({ option, height = 300, onSelect, ariaLabel }: Props) {
  const ref = useRef<HTMLDivElement>(null);
  const chart = useRef<echarts.ECharts | null>(null);
  const select = useRef(onSelect);
  select.current = onSelect;

  useEffect(() => {
    if (!ref.current) return;
    const c = echarts.init(ref.current, undefined, { renderer: "canvas" });
    chart.current = c;
    c.on("click", (e) => {
      const ev = e as { dataIndex?: number; seriesName?: string };
      if (ev.dataIndex != null) select.current?.(ev.dataIndex, ev.seriesName ?? "");
    });
    const ro = new ResizeObserver(() => c.resize());
    ro.observe(ref.current);
    return () => {
      ro.disconnect();
      c.dispose();
      chart.current = null;
    };
  }, []);

  useEffect(() => {
    chart.current?.setOption(option, true);
  }, [option]);

  return <div ref={ref} className="chart" style={{ height }} role="img" aria-label={ariaLabel} />;
}
