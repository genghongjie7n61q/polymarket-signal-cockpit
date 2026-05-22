import { useState } from "react";
import { saveNotificationChannel, sendFeishuDryRun } from "../api/client";
import type { CockpitMarket, FeishuDryRunResponse, NotificationChannel } from "../api/types";

export interface NotificationsTabProps {
  market: CockpitMarket;
  apiBase: string;
  adminToken: string;
}

export function NotificationsTab({ market, apiBase, adminToken }: NotificationsTabProps) {
  const [channels, setChannels] = useState<NotificationChannel[]>(market.notification_channels);
  const [name, setName] = useState("primary");
  const [webhookUrl, setWebhookUrl] = useState("");
  const [enabled, setEnabled] = useState(true);
  const [message, setMessage] = useState<string | null>(null);
  const [dryRun, setDryRun] = useState<FeishuDryRunResponse | null>(null);

  async function onSave() {
    setMessage(null);
    try {
      const saved = await saveNotificationChannel(
        apiBase,
        {
          market_key: market.summary.market_key,
          channel_type: "feishu",
          name,
          webhook_url: webhookUrl,
          enabled,
        },
        adminToken || undefined,
      );
      setChannels((current) => [...current.filter((channel) => channel.name !== saved.name), saved]);
      setWebhookUrl("");
      setMessage("飞书配置已保存");
    } catch (error) {
      setMessage(error instanceof Error ? error.message : String(error));
    }
  }

  async function onDryRun() {
    setMessage(null);
    try {
      setDryRun(await sendFeishuDryRun(apiBase, { market_key: market.summary.market_key }, adminToken || undefined));
    } catch (error) {
      setMessage(error instanceof Error ? error.message : String(error));
    }
  }

  return (
    <div className="config-form">
      <label>
        名称
        <input value={name} onChange={(event) => setName(event.target.value)} />
      </label>
      <label>
        Webhook URL
        <input value={webhookUrl} onChange={(event) => setWebhookUrl(event.target.value)} />
      </label>
      <label className="checkbox-row">
        <input checked={enabled} type="checkbox" onChange={(event) => setEnabled(event.target.checked)} />
        启用
      </label>
      <div className="button-row">
        <button className="secondary-button" type="button" onClick={onSave}>
          保存飞书
        </button>
        <button className="secondary-button" type="button" onClick={onDryRun}>
          Dry Run
        </button>
      </div>
      {message ? <p className="form-message">{message}</p> : null}
      <div className="channel-list">
        {channels.map((channel) => (
          <div className="channel-row" key={channel.id}>
            <strong>{channel.name}</strong>
            <span>{channel.webhook_url_masked}</span>
            <span>{channel.enabled ? "enabled" : "disabled"}</span>
          </div>
        ))}
      </div>
      {dryRun ? (
        <div className="dry-run-result">
          {dryRun.sent.map((delivery) => (
            <p key={delivery.channel.id}>
              Dry-run {delivery.status}: {delivery.response_summary ?? "-"}
            </p>
          ))}
        </div>
      ) : null}
      <h3>最近真实投递</h3>
      <div className="channel-list">
        {market.notification_deliveries.map((delivery) => (
          <div className="channel-row" key={delivery.id}>
            <span>
              {delivery.status} / {delivery.channel_name}
            </span>
            <span>{delivery.response_summary ?? "-"}</span>
          </div>
        ))}
      </div>
    </div>
  );
}
