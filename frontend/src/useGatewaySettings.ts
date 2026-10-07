import { useState } from "react";
export function useGatewaySettings(key: string, refresh: () => Promise<void>) {
  const [draftKeys, setDraftKeys] = useState<Record<string, string[]>>({});
  const [removedKeys, setRemovedKeys] = useState<Record<string, string[]>>({});
  const [clearKeys, setClearKeys] = useState<Record<string, boolean>>({});
  const [saving, setSaving] = useState(false);
  const [settingsMessage, setSettingsMessage] = useState("");
  const [exaKey, setExaKey] = useState("");
  const [clearExa, setClearExa] = useState(false);
  const [gatewayKey, setGatewayKey] = useState("");
  const [keyBusy, setKeyBusy] = useState(false);
  const [keyMessage, setKeyMessage] = useState("");
  const [keyVisible, setKeyVisible] = useState(false);
  async function loadGatewayKey(generate = false) {
    setKeyBusy(true);
    setKeyMessage("");
    try {
      const response = await fetch("/api/gateway-key", {
        method: generate ? "POST" : "GET",
        headers: {
          "X-Gateway-Settings": "1",
          Authorization: `Bearer ${key || "local"}`,
        },
        cache: "no-store",
      });
      const data = await response.json();
      if (!response.ok) throw Error(data.error?.message || "操作失败");
      setGatewayKey(data.key);
      setKeyMessage(
        generate
          ? "新密钥已生效，旧密钥已失效。请更新其他应用的配置。"
          : data.key
            ? "密钥已加载。"
            : "尚未生成网关密钥，请点击生成。",
      );
      await refresh();
    } catch (e) {
      setKeyMessage(`操作失败：${e instanceof Error ? e.message : String(e)}`);
    } finally {
      setKeyBusy(false);
    }
  }
  async function copyConnection(value: string) {
    try {
      await navigator.clipboard.writeText(value);
      setKeyMessage("已复制。");
    } catch {
      setKeyVisible(true);
      setKeyMessage("复制失败，请选中文本手动复制。");
    }
  }
  async function saveSettings() {
    setSaving(true);
    setSettingsMessage("");
    const updates: Record<string, unknown> = {};
    for (const id of ["openrouter"]) {
      if (clearKeys[id]) updates[id] = null;
      else {
        const add = (draftKeys[id] || []).map((k) => k.trim()).filter(Boolean);
        const remove = removedKeys[id] || [];
        if (add.length || remove.length) updates[id] = { add, remove };
      }
    }
    if (clearExa) updates.exa = null;
    else if (exaKey.trim()) updates.exa = exaKey.trim();
    try {
      const response = await fetch("/api/settings", {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
          "X-Gateway-Settings": "1",
          Authorization: `Bearer ${key || "local"}`,
        },
        body: JSON.stringify(updates),
      });
      const data = await response.json();
      if (!response.ok) throw Error(data.error?.message || "保存失败");
      setDraftKeys({});
      setRemovedKeys({});
      setClearKeys({});
      setExaKey(""); setClearExa(false);
      setSettingsMessage("设置已保存，立即生效。重启后仍然保留。");
      await refresh();
    } catch (e) {
      setSettingsMessage(
        `保存失败：${e instanceof Error ? e.message : String(e)}`,
      );
    } finally {
      setSaving(false);
    }
  }
  return { draftKeys, setDraftKeys, removedKeys, setRemovedKeys, clearKeys, setClearKeys, saving, settingsMessage, exaKey, setExaKey, clearExa, setClearExa, gatewayKey, keyBusy, keyMessage, keyVisible, setKeyVisible, loadGatewayKey, copyConnection, saveSettings };
}
export type GatewaySettingsState = ReturnType<typeof useGatewaySettings>;
