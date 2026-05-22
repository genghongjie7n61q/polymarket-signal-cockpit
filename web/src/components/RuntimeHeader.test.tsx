import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { bootstrapFixture } from "../test/fixtures";
import { RuntimeHeader } from "./RuntimeHeader";

describe("RuntimeHeader", () => {
  it("shows live status when runtime sources are fresh", () => {
    render(<RuntimeHeader connection="live" generatedAt={bootstrapFixture.generated_at} runtime={bootstrapFixture.runtime} />);

    expect(screen.getByText("Live")).toBeInTheDocument();
  });

  it("shows degraded status when runtime details are missing", () => {
    render(<RuntimeHeader connection="connecting" generatedAt={null} runtime={null} />);

    expect(screen.getByText("Degraded")).toBeInTheDocument();
  });
});
