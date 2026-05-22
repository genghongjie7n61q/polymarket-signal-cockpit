import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { bootstrapFixture } from "../test/fixtures";
import { NotificationsTab } from "./NotificationsTab";

afterEach(() => {
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

describe("NotificationsTab", () => {
  it("saves webhook, clears raw input, and renders only masked webhook", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue({
        ok: true,
        json: async () => ({
          ...bootstrapFixture.markets[0].notification_channels[0],
          webhook_url: null,
          webhook_url_masked: "https://open.feishu.cn/.../masked",
        }),
      }),
    );

    render(<NotificationsTab adminToken="admin-token" apiBase="/api" market={bootstrapFixture.markets[0]} />);
    const webhookInput = screen.getByLabelText("Webhook URL");
    expect(webhookInput).toHaveAttribute("type", "password");
    fireEvent.change(webhookInput, {
      target: { value: "https://open.feishu.cn/open-apis/bot/v2/hook/raw-secret" },
    });
    fireEvent.click(screen.getByRole("button", { name: "保存飞书" }));

    await waitFor(() => expect(screen.getByText("https://open.feishu.cn/.../masked")).toBeInTheDocument());
    expect(screen.getByLabelText("Webhook URL")).toHaveValue("");
    expect(screen.queryByText(/raw-secret/)).not.toBeInTheDocument();
  });

  it("clears raw webhook input after save failures", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue({
        ok: false,
        status: 401,
        text: async () => "unauthorized",
      }),
    );

    render(<NotificationsTab adminToken="" apiBase="/api" market={bootstrapFixture.markets[0]} />);
    fireEvent.change(screen.getByLabelText("Webhook URL"), {
      target: { value: "https://open.feishu.cn/open-apis/bot/v2/hook/raw-secret" },
    });
    fireEvent.click(screen.getByRole("button", { name: "保存飞书" }));

    await waitFor(() => expect(screen.getByText(/401 unauthorized/)).toBeInTheDocument());
    expect(screen.getByLabelText("Webhook URL")).toHaveValue("");
  });

  it("syncs channel state when switching markets", () => {
    const { rerender } = render(<NotificationsTab adminToken="" apiBase="/api" market={bootstrapFixture.markets[0]} />);
    expect(screen.getByText("https://open.feishu.cn/.../abcd")).toBeInTheDocument();

    rerender(<NotificationsTab adminToken="" apiBase="/api" market={bootstrapFixture.markets[1]} />);

    expect(screen.queryByText("https://open.feishu.cn/.../abcd")).not.toBeInTheDocument();
  });

  it("does not clear in-progress edits when the same market refreshes", () => {
    const { rerender } = render(<NotificationsTab adminToken="" apiBase="/api" market={bootstrapFixture.markets[0]} />);
    fireEvent.change(screen.getByLabelText("Webhook URL"), {
      target: { value: "https://open.feishu.cn/open-apis/bot/v2/hook/in-progress" },
    });

    rerender(<NotificationsTab adminToken="" apiBase="/api" market={structuredClone(bootstrapFixture.markets[0])} />);

    expect(screen.getByLabelText("Webhook URL")).toHaveValue("https://open.feishu.cn/open-apis/bot/v2/hook/in-progress");
  });

  it("displays dry-run result separately from delivery history", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue({
        ok: true,
        json: async () => ({
          market_key: "btc5m",
          card_summary: { title: "Dry Run", reason: "probe" },
          sent: [
            {
              channel: bootstrapFixture.markets[0].notification_channels[0],
              status: "sent",
              response_summary: "ok",
            },
          ],
          notification: null,
        }),
      }),
    );

    render(<NotificationsTab adminToken="admin-token" apiBase="/api" market={bootstrapFixture.markets[0]} />);
    fireEvent.click(screen.getByRole("button", { name: "Dry Run" }));

    await waitFor(() => expect(screen.getByText("Dry-run sent: ok")).toBeInTheDocument());
    expect(screen.getByText("最近真实投递")).toBeInTheDocument();
    expect(screen.getByText("sent / primary")).toBeInTheDocument();
  });
});
