/**
 * ManagerPill render-state component tests.
 *
 * Covers the render-condition contract documented in ManagerPill.tsx:
 *   pinFresh=true    → passive badge, no click action, ✓ indicator
 *   pinFresh=false   → interactive button, click fires onPinRequired, "PIN" indicator
 *   disabled=true    → muted, click is no-op, no callback fires
 */

import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import ManagerPill from "./ManagerPill";

describe("ManagerPill — Q5-A render-condition contract", () => {
  it("renders action label + category label for any privileged action", () => {
    render(<ManagerPill action="issue_refund" />);
    expect(screen.getByText("Issue refund")).toBeInTheDocument();
    expect(screen.getByText("Wallet & billing")).toBeInTheDocument();
  });

  it("when pinFresh=true: shows ✓ indicator + button is disabled (passive badge)", () => {
    render(<ManagerPill action="issue_refund" pinFresh />);
    expect(screen.getByText("✓")).toBeInTheDocument();
    expect(screen.getByRole("button")).toBeDisabled();
    expect(screen.getByRole("button")).toHaveAttribute(
      "aria-label",
      "Issue refund (manager PIN verified)",
    );
  });

  it("when pinFresh=false + onPinRequired set: shows PIN indicator + button is interactive", () => {
    const onPinRequired = vi.fn();
    render(<ManagerPill action="issue_refund" onPinRequired={onPinRequired} />);
    expect(screen.getByText("PIN")).toBeInTheDocument();
    expect(screen.getByRole("button")).toBeEnabled();
    expect(screen.getByRole("button")).toHaveAttribute(
      "aria-label",
      "Issue refund — manager PIN required",
    );
  });

  it("clicking the pill in needsPin state calls onPinRequired with the action", async () => {
    const user = userEvent.setup();
    const onPinRequired = vi.fn();
    render(<ManagerPill action="issue_refund" onPinRequired={onPinRequired} />);
    await user.click(screen.getByRole("button"));
    expect(onPinRequired).toHaveBeenCalledTimes(1);
    expect(onPinRequired).toHaveBeenCalledWith("issue_refund");
  });

  it("clicking the pill in pinFresh state does NOT call onPinRequired", async () => {
    const user = userEvent.setup();
    const onPinRequired = vi.fn();
    render(<ManagerPill action="issue_refund" pinFresh onPinRequired={onPinRequired} />);
    await user.click(screen.getByRole("button"));
    expect(onPinRequired).not.toHaveBeenCalled();
  });

  it("when disabled=true: button is non-interactive even if onPinRequired set", async () => {
    const user = userEvent.setup();
    const onPinRequired = vi.fn();
    render(
      <ManagerPill action="issue_refund" disabled onPinRequired={onPinRequired} />,
    );
    expect(screen.getByRole("button")).toBeDisabled();
    await user.click(screen.getByRole("button"));
    expect(onPinRequired).not.toHaveBeenCalled();
  });

  it("without onPinRequired callback: button is non-interactive (no-op click)", async () => {
    const user = userEvent.setup();
    render(<ManagerPill action="issue_refund" />);
    // No callback to assert against; just verify click doesn't throw
    await expect(user.click(screen.getByRole("button"))).resolves.not.toThrow();
    expect(screen.getByRole("button")).toBeDisabled();
  });

  it("data-action and data-category attributes mirror props (drift detector)", () => {
    render(<ManagerPill action="approve_refund_over_threshold" />);
    const button = screen.getByRole("button");
    expect(button).toHaveAttribute("data-action", "approve_refund_over_threshold");
    expect(button).toHaveAttribute("data-category", "manager_escalation");
  });

  it("renders correctly for a session_terminal action (cross-category render check)", () => {
    render(<ManagerPill action="end_session_early" pinFresh />);
    expect(screen.getByText("End session early")).toBeInTheDocument();
    expect(screen.getByText("Session end")).toBeInTheDocument();
  });

  it("renders correctly for an identity_record action (cross-category render check)", () => {
    render(<ManagerPill action="merge_customer_profiles" />);
    expect(screen.getByText("Merge customer profiles")).toBeInTheDocument();
    expect(screen.getByText("Customer record")).toBeInTheDocument();
  });
});
