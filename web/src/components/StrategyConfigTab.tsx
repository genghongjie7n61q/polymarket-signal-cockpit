import { useEffect, useState } from "react";
import { saveModelAssignment } from "../api/client";
import type { CockpitMarket, ModelAssignment } from "../api/types";

export interface StrategyConfigTabProps {
  market: CockpitMarket;
  apiBase: string;
  adminToken: string;
}

function assignmentDefaults(market: CockpitMarket): Pick<ModelAssignment, "model_key" | "display_name" | "version" | "parameters"> {
  return {
    model_key: market.active_model?.model_key ?? "",
    display_name: market.active_model?.display_name ?? "",
    version: market.active_model?.version ?? "",
    parameters: market.active_model?.parameters ?? {},
  };
}

export function StrategyConfigTab({ market, apiBase, adminToken }: StrategyConfigTabProps) {
  const [draft, setDraft] = useState(() => assignmentDefaults(market));
  const [parametersText, setParametersText] = useState(() => JSON.stringify(draft.parameters, null, 2));
  const [message, setMessage] = useState<string | null>(null);

  useEffect(() => {
    const next = assignmentDefaults(market);
    setDraft(next);
    setParametersText(JSON.stringify(next.parameters, null, 2));
    setMessage(null);
  }, [market]);

  async function onSave() {
    setMessage(null);
    let parameters: ModelAssignment["parameters"];
    try {
      parameters = JSON.parse(parametersText) as ModelAssignment["parameters"];
    } catch {
      setMessage("参数不是有效 JSON");
      return;
    }

    try {
      await saveModelAssignment(
        apiBase,
        market.summary.market_key,
        {
          ...draft,
          parameters,
        },
        adminToken || undefined,
      );
      setMessage("模型配置已保存");
    } catch (error) {
      setMessage(error instanceof Error ? error.message : String(error));
    }
  }

  return (
    <div className="config-form">
      <label>
        模型 Key
        <input value={draft.model_key} onChange={(event) => setDraft({ ...draft, model_key: event.target.value })} />
      </label>
      <label>
        显示名称
        <input value={draft.display_name} onChange={(event) => setDraft({ ...draft, display_name: event.target.value })} />
      </label>
      <label>
        版本
        <input value={draft.version} onChange={(event) => setDraft({ ...draft, version: event.target.value })} />
      </label>
      <label>
        参数 JSON
        <textarea rows={7} value={parametersText} onChange={(event) => setParametersText(event.target.value)} />
      </label>
      <button className="secondary-button" type="button" onClick={onSave}>
        保存模型
      </button>
      {message ? <p className="form-message">{message}</p> : null}
    </div>
  );
}
