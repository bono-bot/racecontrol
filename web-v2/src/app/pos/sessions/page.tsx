import { Gamepad2, Pause, Square } from "lucide-react";
import { Panel, PodCard, StatusTile, ActionButton, StatusDot } from "@/components/rp";

// Active Sessions Board — POS view of pods 1-8 + PS5 .101/.102/.103 state.
// MOCK; Phase 1 wires GET /api/v1/sessions/active + WS state stream.

const MOCK_PODS = [
  { n: 1, state: "green" as const, customer: "Vivek S.", sim: "AC · Spa", duration: "12:35", spend: "₹312" },
  { n: 2, state: "green" as const, customer: "Ronit P.", sim: "iRacing · Daytona", duration: "08:12", spend: "₹196" },
  { n: 3, state: "amber" as const, pulse: true, customer: "—", sim: "ws stale · investigating", duration: "—", spend: "—" },
  { n: 4, state: "red" as const, customer: "—", sim: "fault · 0xC0000005", duration: "—", spend: "—" },
  { n: 5, state: "green" as const, customer: "Amruth M.", sim: "F1 25 · Monaco", duration: "23:48", spend: "₹587" },
  { n: 6, state: "green" as const, customer: "Priya K.", sim: "LMU · Le Mans", duration: "04:22", spend: "₹103" },
  { n: 7, state: "grey" as const, customer: "—", sim: "idle", duration: "—", spend: "—" },
  { n: 8, state: "grey" as const, customer: "—", sim: "idle", duration: "—", spend: "—" },
];

const MOCK_PS5 = [
  { ip: ".101", label: "PS5 · Station 1", state: "green" as const, customer: "Family · 2 ctrl", duration: "18:30", spend: "₹500" },
  { ip: ".102", label: "PS5 · Station 2", state: "grey" as const, customer: "—", duration: "—", spend: "—" },
  { ip: ".103", label: "PS5 · Station 3", state: "grey" as const, customer: "—", duration: "—", spend: "—" },
];

export default function ActiveSessions() {
  return (
    <div className="px-6 py-6 space-y-6">
      <div>
        <p className="font-body uppercase text-[11px] tracking-[0.12em] text-rp-grey">
          Customer ops
        </p>
        <h1 className="font-display text-3xl font-bold text-rp-ink uppercase tracking-tight">
          Active sessions
        </h1>
      </div>

      <div className="grid grid-cols-4 gap-3">
        <StatusTile caption="Pods · active" value="4" sub="of 8" display />
        <StatusTile caption="Pods · faulted" value="1" sub="pod 4" display />
        <StatusTile caption="PS5 · active" value="1" sub="of 3" display />
        <StatusTile caption="Live revenue" value="₹1,698" sub="cumulative this hour" />
      </div>

      <Panel title="Sim-racing pods · 1-8">
        <div className="overflow-x-auto">
          <table className="w-full text-sm">
            <thead className="border-b border-rp-border">
              <tr className="font-body uppercase text-[10px] tracking-[0.08em] text-rp-grey">
                <th className="text-left py-2 pr-4">Pod</th>
                <th className="text-left py-2 pr-4">State</th>
                <th className="text-left py-2 pr-4">Customer</th>
                <th className="text-left py-2 pr-4">Experience</th>
                <th className="text-right py-2 pr-4">Time</th>
                <th className="text-right py-2 pr-4">Spend</th>
                <th className="text-right py-2">Actions</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-rp-border">
              {MOCK_PODS.map((p) => (
                <tr key={p.n} className="hover:bg-rp-card-hi transition-colors duration-(--motion-fast)">
                  <td className="py-2 pr-4 font-display font-bold text-rp-ink">POD {String(p.n).padStart(2, "0")}</td>
                  <td className="py-2 pr-4">
                    <span className="inline-flex items-center gap-2">
                      <StatusDot state={p.state} pulse={p.pulse} />
                      <span className="font-mono text-[11px] text-rp-ink-dim">{p.state}</span>
                    </span>
                  </td>
                  <td className="py-2 pr-4 font-body text-rp-ink">{p.customer}</td>
                  <td className="py-2 pr-4 font-body text-rp-ink-dim">{p.sim}</td>
                  <td className="py-2 pr-4 text-right font-mono tabular-nums text-rp-ink">{p.duration}</td>
                  <td className="py-2 pr-4 text-right font-mono tabular-nums text-rp-ink">{p.spend}</td>
                  <td className="py-2 text-right">
                    <div className="inline-flex gap-1">
                      <ActionButton variant="ghost" size="sm" disabled={p.state !== "green"}>
                        <Pause className="size-3.5" />
                      </ActionButton>
                      <ActionButton variant="danger" size="sm" disabled={p.state !== "green"}>
                        <Square className="size-3.5" />
                      </ActionButton>
                    </div>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </Panel>

      <Panel title="PS5 stations · .101 / .102 / .103">
        <div className="grid grid-cols-3 gap-3">
          {MOCK_PS5.map((p) => (
            <div key={p.ip} className="rounded-md bg-rp-card border border-rp-border border-l-4 border-l-rp-blue p-4 space-y-2">
              <div className="flex items-center justify-between">
                <span className="font-display text-lg font-bold text-rp-ink uppercase tracking-wide">
                  {p.label}
                </span>
                <StatusDot state={p.state} />
              </div>
              <p className="font-mono text-[10px] text-rp-grey">192.168.31{p.ip}</p>
              <div className="space-y-0.5 pt-2 border-t border-rp-border">
                <p className="font-body text-sm text-rp-ink">{p.customer}</p>
                <p className="font-mono text-xs tabular-nums text-rp-ink-dim">
                  {p.duration} · {p.spend}
                </p>
              </div>
              <ActionButton variant="ghost" size="sm" disabled={p.state !== "green"}>
                <Gamepad2 className="size-3.5" /> Manage
              </ActionButton>
            </div>
          ))}
        </div>
      </Panel>
    </div>
  );
}
