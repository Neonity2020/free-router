export type Item = {
  id: string; type: "user" | "assistant" | "tool" | "notice";
  text: string; thinking?: string; name?: string; args?: string; status?: string; error?: boolean;
};
export type ToolRow = { item: Item; count: number };
export type TimelineEntry = { kind: "message"; item: Item } | { kind: "tools"; id: string; items: Item[] };

export function buildTimeline(items: Item[]): TimelineEntry[] {
  const entries: TimelineEntry[] = [];
  for (const item of items) {
    if (item.type === "assistant" && !item.text.trim() && !item.thinking?.trim()) continue;
    const last = entries.at(-1);
    if (item.type === "tool" || (item.type === "assistant" && !item.text.trim() && item.thinking?.trim())) {
      if (last?.kind === "tools") last.items.push(item);
      else entries.push({ kind: "tools", id: item.id, items: [item] });
    } else entries.push({ kind: "message", item });
  }
  return entries;
}

// Only adjacent, completed, identical calls share a display row. Execution history stays intact.
export function compactTools(items: Item[]): ToolRow[] {
  const rows: ToolRow[] = [];
  for (const item of items) {
    const last = rows.at(-1);
    if (item.status === "done" && last?.item.status === "done" &&
      item.name === last.item.name && item.args === last.item.args && item.text === last.item.text) last.count++;
    else rows.push({ item, count: 1 });
  }
  return rows;
}
