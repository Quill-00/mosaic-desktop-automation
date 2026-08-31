import { useI18n } from "../i18n";

type CardData = Record<string, unknown>;

function records(value: unknown): CardData[] {
  return Array.isArray(value) ? value.filter((item): item is CardData => !!item && typeof item === "object") : [];
}

function text(value: unknown): string {
  if (value == null) return "";
  if (typeof value === "string" || typeof value === "number" || typeof value === "boolean") return String(value);
  return "";
}

export default function CardView({ card }: { card: unknown }) {
  const { t } = useI18n();
  if (!card || typeof card !== "object") return null;
  const data = card as CardData;

  switch (data.type) {
    case "metric":
      return (
        <div className="metrics">
          {records(data.metrics).map((m, i) => (
            <div key={i} className="metric">
              <div className="metric-label">{text(m.label)}</div>
              <div className="metric-value">{text(m.value) || "—"}</div>
              {!!text(m.delta) && <div className="metric-delta">{text(m.delta)}</div>}
            </div>
          ))}
        </div>
      );
    case "news":
      return (
        <ul className="list">
          {records(data.items).map((it, i) => (
            <li key={i}>
              <div className="li-title">{text(it.title) || t("Untitled item", "未命名条目")}</div>
              {!!(text(it.source) || text(it.time)) && (
                <div className="li-sub">
                  {text(it.source)}
                  {text(it.source) && text(it.time) ? " · " : ""}
                  {text(it.time)}
                </div>
              )}
            </li>
          ))}
        </ul>
      );
    case "table":
      return (
        <table className="table">
          <thead>
            <tr>
              {(Array.isArray(data.columns) ? data.columns : []).map((c, i) => (
                <th key={i}>{text(c)}</th>
              ))}
            </tr>
          </thead>
          <tbody>
            {(Array.isArray(data.rows) ? data.rows : []).filter(Array.isArray).map((r, i) => (
              <tr key={i}>
                {r.map((c, j) => (
                  <td key={j}>{text(c)}</td>
                ))}
              </tr>
            ))}
          </tbody>
        </table>
      );
    case "markdown":
      return <div className="md">{text(data.text)}</div>;
    case "list":
    default:
      return (
        <ul className="list">
          {records(data.items).map((it, i) => (
            <li key={i}>
              <div className="li-title">{text(it.text) || text(it.title) || t("Untitled item", "未命名条目")}</div>
              {!!text(it.subtitle) && <div className="li-sub">{text(it.subtitle)}</div>}
            </li>
          ))}
        </ul>
      );
  }
}
